use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;

use super::builder::EngineBuilder;
use aberred_core::components::persistent::Persistent;
use aberred_core::error::EngineError;
use crate::events::render::switchfullscreen::switch_fullscreen_observer;
use aberred_core::protocol::endpoints::{LogicBridge, LogicTx, shutdown_logic_bridge};
use aberred_core::protocol::render_assets::RenderAssetCmd;
use aberred_core::protocol::snapshot::SnapshotConsumer;
use aberred_core::resources::debugoverlayconfig::DebugOverlayConfig;
use aberred_core::resources::gameconfig::GameConfig;
use aberred_core::resources::guitheme::GuiThemeWarnCache;
use crate::resources::render::fontstore::FontStore;
use crate::resources::render::imgui_bridge::ImguiBridge;
use crate::resources::render::mirrors::{
    RenderActiveScene, RenderAppState, RenderCamera, RenderCameraFollow, RenderDebugSnapshot,
    RenderGameConfig, RenderGuiThemes, RenderPostProcess, RenderSignalSnapshot, RenderWorldTime,
};
use crate::resources::render::pending_imgui_capture::PendingImguiCapture;
use crate::resources::render::quit_requested::QuitRequested;
use crate::resources::render::rendertarget::RenderTarget;
use crate::resources::render::scene_table::RenderSceneTable;
use crate::resources::render::shaderstore::ShaderStore;
use crate::resources::render::sim_id_map::SimIdMap;
use crate::resources::render::texturestore::TextureStore;
use crate::resources::render::thread_stats::RenderStats;
use aberred_core::resources::screensize::ScreenSize;
use aberred_core::resources::signal_intents::SignalIntents;
use aberred_core::resources::windowsize::WindowSize;
use crate::systems::render::apply_gameconfig_changes;
use crate::systems::render::input::sample_and_send_input;
use crate::systems::render::messages::pump_render_msgs;
use crate::systems::render::output::send_render_mirrors;
use crate::systems::render::process_render_asset_cmds;
use crate::systems::render::render_system;
use crate::systems::render::snapshot::receive_snapshot;
use crate::systems::render::window::refresh_window_size;
use aberred_core::systems::render_assets::update_bevy_render_asset_cmds;

impl EngineBuilder {
    /// Build the render (main-thread) `World`: raylib window +
    /// GL/NonSend stores, plus the render-owned `InputState` mirror (written
    /// by the render loop from its `sample_raw_device_snapshot` result —
    /// it carries no resolved bindings/edges, see
    /// `DebugResources::input_state`'s doc comment) and `DebugOverlayConfig`
    /// (edited by the imgui panel). `InputBindings` is logic-thread-only —
    /// there is no render-side mirror. Holds no gameplay resources.
    /// This world DOES hold entities: retained mirror entities (`SimMirror` +
    /// one marker per category, `src/systems/render/mirror.rs`), reconciled
    /// write-only from each tick's `DrawableSnapshot` clone by
    /// `receive_snapshot` -- never stored as a resource, never gameplay
    /// entities. (The fullscreen-toggle `Observer` entity spawned below is
    /// infrastructure, not a drawable.)
    pub(super) fn setup_render_world(
        config: GameConfig,
        rl: raylib::RaylibHandle,
        thread: raylib::RaylibThread,
        render_target: RenderTarget,
        render_scene_table: Option<RenderSceneTable>,
        snapshot_consumer: SnapshotConsumer,
        bridge: LogicBridge,
    ) -> Result<World, EngineError> {
        let mut world = World::new();
        world.insert_resource(ScreenSize {
            w: config.render_width as i32,
            h: config.render_height as i32,
        });
        world.insert_resource(WindowSize {
            w: rl.get_screen_width(),
            h: rl.get_screen_height(),
        });
        // Seed RenderGameConfig with the REAL loaded config, not ::default()'s
        // baked-in defaults: apply_gameconfig_changes reads config exclusively
        // from this resource, and the first logic-built snapshot arrives a
        // frame or two later — a default-seeded copy would briefly apply the
        // wrong render/window size at startup.
        world.insert_resource(RenderGameConfig(config));
        world.insert_resource(snapshot_consumer);
        world.insert_resource(TextureStore::new());
        world.insert_resource(GuiThemeWarnCache::default());
        world.insert_resource(DebugOverlayConfig::default());
        world.insert_resource(SignalIntents::default());
        world.insert_resource(Messages::<RenderAssetCmd>::default());
        world.insert_resource(QuitRequested::default());
        world.insert_resource(PendingImguiCapture::default());
        world.insert_resource(RenderCamera::default());
        world.insert_resource(RenderSignalSnapshot::default());
        world.insert_resource(RenderAppState::default());
        world.insert_resource(RenderDebugSnapshot::default());
        world.insert_resource(RenderActiveScene::default());
        world.insert_resource(RenderWorldTime::default());
        world.insert_resource(RenderPostProcess::default());
        world.insert_resource(RenderGuiThemes::default());
        world.insert_resource(RenderCameraFollow::default());
        // Backs all 8 categories' mirror-entity
        // reconciliation done by receive_snapshot/reconcile_*. Must be
        // inserted before the first snapshot arrives -- each reconcile_*'s
        // resource_scope panics if this resource is absent.
        world.insert_resource(SimIdMap::default());
        world.insert_resource(RenderStats::default());
        if let Some(table) = render_scene_table {
            world.insert_resource(table);
        }
        world.insert_non_send(render_target);
        world.insert_non_send(FontStore::new());
        // Created before `bridge` is handed to the world: on failure here the
        // logic thread (already spawned by the caller) is still reachable
        // through the locally-owned `bridge` and can be shut down explicitly,
        // rather than being leaked by a dropped, never-populated World.
        let imgui_bridge = match ImguiBridge::new_dark() {
            Ok(imgui_bridge) => imgui_bridge,
            Err(message) => {
                shutdown_logic_bridge(bridge);
                return Err(EngineError::Imgui { message });
            }
        };
        world.insert_non_send(imgui_bridge);
        world.insert_non_send(ShaderStore::new());
        world.insert_non_send(rl);
        world.insert_non_send(thread);

        world.insert_resource(LogicTx(bridge.tx_logic.clone()));
        world.insert_resource(bridge);

        world.spawn((Observer::new(switch_fullscreen_observer), Persistent));
        world.flush();

        Ok(world)
    }

    /// Build the render thread's single per-frame schedule: every
    /// per-frame step is a system in this chain. `apply_gameconfig_changes`
    /// has no `run_if(state_is_playing)` gate — the render world has no
    /// `GameState`; the snapshot's config is seeded with the real loaded
    /// config at startup, so early application is a no-op, not a downgrade.
    pub(super) fn build_render_schedule(world: &mut World) -> Result<Schedule, EngineError> {
        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                refresh_window_size,
                sample_and_send_input,
                pump_render_msgs,
                update_bevy_render_asset_cmds,
                process_render_asset_cmds,
                receive_snapshot,
                apply_gameconfig_changes,
                render_system,
                send_render_mirrors,
            )
                .chain(),
        );
        schedule
            .initialize(world)
            .map_err(|source| EngineError::ScheduleInit {
                which: "render",
                source,
            })?;
        Ok(schedule)
    }
}
