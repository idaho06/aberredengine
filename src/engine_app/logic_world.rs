use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;

use super::builder::EngineBuilder;
use super::logic_thread::LogicInit;
use super::registrar::ObserverRegistrar;
#[cfg(feature = "lua")]
use crate::components::mapposition::MapPosition;
use crate::components::persistent::Persistent;
#[cfg(feature = "lua")]
use crate::components::rotation::Rotation;
#[cfg(feature = "lua")]
use crate::components::scale::Scale;
#[cfg(feature = "lua")]
use crate::components::screenposition::ScreenPosition;
use crate::error::EngineError;
use crate::events::gamestate::GameStateChangedEvent;
use crate::events::gamestate::observe_gamestate_change_event;
use crate::events::render_assets::RenderAssetCmd;
use crate::events::switchdebug::switch_debug_observer;
use crate::protocol::endpoints::{setup_audio, RenderTx};
#[cfg(any(test, feature = "test-support"))]
use crate::protocol::endpoints::setup_audio_stub;
use crate::resources::animationstore::AnimationStore;
use crate::resources::appstate::AppState;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::drawable_snapshot::DrawableSnapshot;
use crate::resources::fontmetrics::{FontMetricsStore, FontMetricsWarnCache};
use crate::resources::gameconfig::GameConfigDefaults;
use crate::resources::gamestate::{GameState, NextGameState, GameStates};
use crate::resources::group::TrackedGroups;
use crate::resources::guiinputstate::GuiInputState;
use crate::resources::guitheme::{GuiThemeStore, GuiThemeWarnCache};
use crate::resources::input::InputState;
use crate::resources::input_bindings::InputBindings;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::rawinput::{ImguiCaptureMirror, PrevRawSnapshot};
use crate::resources::scenemanager::SceneManager;
use crate::resources::screensize::ScreenSize;
use crate::resources::signal_intents::SignalIntents;
use crate::resources::systemsstore as hook_keys;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturedims::TextureDimsStore;
use crate::resources::thread_stats::{AudioStats, SimStats};
use crate::resources::windowsize::WindowSize;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::gamestate::{clean_all_entities, quit_game};
use crate::systems::gui_interactable_click::gui_interactable_click_observer;
use crate::systems::mapspawn::spawn_map_observer;
use crate::systems::menu::{menu_controller_observer, menu_despawn, menu_selection_observer};
use crate::systems::rust_collision::rust_collision_observer;
use crate::systems::scene_dispatch::{scene_enter_play, scene_switch_system};
use crate::systems::timer::timer_observer;
use raylib::prelude::{Camera2D, Vector2};

#[cfg(feature = "lua")]
use crate::resources::lua_runtime::LuaRuntime;
#[cfg(feature = "lua")]
use crate::systems::lua_animation_finished::lua_animation_finished_observer;
#[cfg(feature = "lua")]
use crate::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use crate::systems::lua_tween_finished::lua_tween_finished_observer;
#[cfg(feature = "lua")]
use crate::systems::luatimer::lua_timer_observer;

/// Helper: register a system into the world, mark it [`Persistent`], and insert
/// its ID into [`SystemsStore`].
pub(crate) fn register_persistent_system<M>(
    world: &mut World,
    store: &mut SystemsStore,
    name: &str,
    system: impl IntoSystem<(), (), M> + 'static,
) {
    let system_id = world.register_system(system);
    world.entity_mut(system_id.entity()).insert(Persistent);
    store.insert(name, system_id);
}

impl EngineBuilder {
    /// Build the logic-thread gameplay `World`: gameplay resources/entities,
    /// excluding GL/window state, plus the
    /// message-fed mirrors (`ScreenSize`/`WindowSize`/`DebugOverlayConfig`)
    /// and the logic-owned stores fed by render notifications
    /// (`FontMetricsStore`/`TextureDimsStore`). Runs INSIDE the thread
    /// closure so NonSend `LuaRuntime` is created on (and pinned to) the
    /// logic thread.
    pub(crate) fn setup_logic_world(init: &mut LogicInit) -> Result<World, EngineError> {
        let config = init.config.clone();
        let render_width = config.render_width;
        let render_height = config.render_height;
        let audio_hz = config.audio_hz;

        let mut world = World::new();
        world.insert_resource(WorldTime::default().with_time_scale(1.0));
        world.insert_resource(WorldSignals::default());
        world.insert_resource(AppState::default());
        world.insert_resource(SignalIntents::default());
        world.insert_resource(TrackedGroups::default());
        world.insert_resource(ScreenSize {
            w: render_width as i32,
            h: render_height as i32,
        });
        world.insert_resource(WindowSize {
            w: init.window_w,
            h: init.window_h,
        });
        world.insert_resource(GameConfigDefaults(config.clone()));
        world.insert_resource(config);
        world.insert_resource(InputState::default());
        world.insert_resource(InputBindings::default());
        world.insert_resource(PrevRawSnapshot::default());
        world.insert_resource(ImguiCaptureMirror::default());
        world.insert_resource(SimStats::default());
        world.insert_resource(AudioStats::default());

        #[cfg(any(test, feature = "test-support"))]
        let use_audio_stub = init.stub_audio;
        #[cfg(not(any(test, feature = "test-support")))]
        let use_audio_stub = false;

        if use_audio_stub {
            #[cfg(any(test, feature = "test-support"))]
            {
                init.audio_stub_ends = Some(setup_audio_stub(&mut world));
            }
        } else {
            setup_audio(&mut world, audio_hz);
        }

        world.insert_resource(GameState::new());
        world.insert_resource(NextGameState::new());
        world.insert_resource(FontMetricsStore::default());
        world.insert_resource(FontMetricsWarnCache::default());
        world.insert_resource(TextureDimsStore::default());
        world.insert_resource(Messages::<RenderAssetCmd>::default());
        world.insert_resource(Camera2DRes(Camera2D {
            target: Vector2 { x: 0.0, y: 0.0 },
            offset: Vector2 {
                x: render_width as f32 * 0.5,
                y: render_height as f32 * 0.5,
            },
            rotation: 0.0,
            zoom: 1.0,
        }));
        world.insert_resource(AnimationStore::default());
        world.insert_resource(PostProcessShader::new());
        world.insert_resource(CameraFollowConfig::default());
        world.insert_resource(DebugOverlayConfig::default());
        world.insert_resource(GuiInputState::default());
        world.insert_resource(GuiThemeStore::default());
        world.insert_resource(GuiThemeWarnCache::default());
        world.insert_resource(DrawableSnapshot::default());
        world.insert_resource(RenderTx(init.tx_render.clone()));
        world.insert_resource(
            init.snapshot_publisher
                .take()
                .expect("snapshot_publisher is set once in try_run and taken exactly once here"),
        );

        #[cfg(feature = "lua")]
        if let Some(ref script_path) = init.lua_script {
            let lua_runtime = LuaRuntime::new()?;
            let path_display = script_path.to_string_lossy();
            if let Err(e) = lua_runtime.run_script(&path_display) {
                log::error!("Failed to load Lua script '{path_display}': {e}");
                eprintln!(
                    "Failed to load Lua script '{path_display}': {e}\n\
                     The engine will continue running with no scenes loaded; fix the script and restart."
                );
            }
            world.insert_non_send(lua_runtime);
        }

        world.spawn((Observer::new(observe_gamestate_change_event), Persistent));

        Ok(world)
    }

    pub(super) fn validate_required_systems(
        systems_store: &SystemsStore,
        requires_switch_scene: bool,
    ) -> Result<(), EngineError> {
        let mut missing = Vec::new();

        for name in [hook_keys::SETUP, hook_keys::ENTER_PLAY, hook_keys::QUIT_GAME] {
            if systems_store.get(name).is_none() {
                missing.push(name);
            }
        }

        if requires_switch_scene && systems_store.get(hook_keys::SWITCH_SCENE).is_none() {
            missing.push(hook_keys::SWITCH_SCENE);
        }

        if missing.is_empty() {
            Ok(())
        } else {
            Err(EngineError::MissingSystems(missing.join(", ")))
        }
    }

    /// Register the hook/scene one-shot systems into the LOGIC world (runs on
    /// the logic thread; consumes the hooks out of `init`). The render-side
    /// scene table was already cloned off before `init` crossed the thread
    /// boundary.
    pub(crate) fn register_logic_systems(
        init: &mut LogicInit,
        world: &mut World,
        use_scene_manager: bool,
    ) -> Result<(), EngineError> {
        let mut systems_store = SystemsStore::new();
        #[cfg(feature = "lua")]
        let requires_switch_scene = use_scene_manager
            || init.switch_scene_hook.is_some()
            || init.lua_script.is_some();
        #[cfg(not(feature = "lua"))]
        let requires_switch_scene = use_scene_manager || init.switch_scene_hook.is_some();

        if let Some(hook) = init.setup_hook.take() {
            hook(world, &mut systems_store);
        }
        if let Some(hook) = init.enter_play_hook.take() {
            hook(world, &mut systems_store);
        }
        if let Some(hook) = init.switch_scene_hook.take() {
            hook(world, &mut systems_store);
        }

        if use_scene_manager {
            let mut scene_manager = SceneManager::new();
            scene_manager.initial_scene = init.initial_scene.take();
            for (name, descriptor) in init.scenes.drain(..) {
                scene_manager.insert(name, descriptor);
            }
            world.insert_resource(scene_manager);

            register_persistent_system(
                world,
                &mut systems_store,
                hook_keys::SWITCH_SCENE,
                scene_switch_system,
            );
            register_persistent_system(
                world,
                &mut systems_store,
                hook_keys::ENTER_PLAY,
                scene_enter_play,
            );
        }

        register_persistent_system(world, &mut systems_store, hook_keys::QUIT_GAME, quit_game);
        register_persistent_system(
            world,
            &mut systems_store,
            hook_keys::CLEAN_ALL_ENTITIES,
            clean_all_entities,
        );

        let menu_despawn_system_id = world.register_system(menu_despawn);
        world
            .entity_mut(menu_despawn_system_id.entity())
            .insert(Persistent);
        systems_store.insert_entity_system(hook_keys::MENU_DESPAWN, menu_despawn_system_id);

        Self::validate_required_systems(&systems_store, requires_switch_scene)?;

        world.insert_resource(systems_store);
        world.flush();

        {
            let mut next_state = world.resource_mut::<NextGameState>();
            next_state.set(GameStates::Setup);
        }
        world.trigger(GameStateChangedEvent {});

        Ok(())
    }

    pub(crate) fn spawn_observers(
        world: &mut World,
        has_lua: bool,
        extra_observers: Vec<ObserverRegistrar>,
    ) {
        #[cfg(feature = "lua")]
        if has_lua {
            world.spawn((Observer::new(lua_collision_observer), Persistent));
        }
        world.spawn((Observer::new(rust_collision_observer), Persistent));
        world.spawn((Observer::new(switch_debug_observer), Persistent));
        // switch_fullscreen_observer is NOT here: it lives in the RENDER
        // world (Phase 5e) — F10 toggles the window, which only exists there.
        world.spawn((Observer::new(menu_controller_observer), Persistent));
        world.spawn((Observer::new(menu_selection_observer), Persistent));
        world.spawn((Observer::new(gui_interactable_click_observer), Persistent));
        #[cfg(feature = "lua")]
        if has_lua {
            world.spawn((Observer::new(lua_timer_observer), Persistent));
            world.spawn((Observer::new(lua_animation_finished_observer), Persistent));

            fn spawn_tween_finished_observer<T: crate::components::tween::TweenValue>(
                world: &mut World,
            ) {
                world.spawn((Observer::new(lua_tween_finished_observer::<T>), Persistent));
            }
            spawn_tween_finished_observer::<MapPosition>(world);
            spawn_tween_finished_observer::<Rotation>(world);
            spawn_tween_finished_observer::<Scale>(world);
            spawn_tween_finished_observer::<ScreenPosition>(world);
        }
        #[cfg(not(feature = "lua"))]
        let _ = has_lua;
        world.spawn((Observer::new(timer_observer), Persistent));
        world.spawn((Observer::new(spawn_map_observer), Persistent));

        // Spawn user-registered persistent observers
        for registrar in extra_observers {
            registrar(world);
        }

        world.flush();
    }
}
