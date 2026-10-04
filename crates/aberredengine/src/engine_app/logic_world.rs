use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;

use super::builder::EngineBuilder;
use super::logic_thread::LogicInit;
use super::registrar::ObserverRegistrar;
use aberred_audio::systems::setup_audio;
#[cfg(feature = "lua")]
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::persistent::Persistent;
#[cfg(feature = "lua")]
use aberred_core::components::rotation::Rotation;
#[cfg(feature = "lua")]
use aberred_core::components::scale::Scale;
#[cfg(feature = "lua")]
use aberred_core::components::screenposition::ScreenPosition;
use aberred_core::error::EngineError;
use aberred_core::events::gamestate::GameStateChangedEvent;
use aberred_core::events::gamestate::observe_gamestate_change_event;
use aberred_core::events::switchdebug::switch_debug_observer;
use aberred_core::protocol::endpoints::RenderTx;
#[cfg(any(test, feature = "test-support"))]
use aberred_core::protocol::endpoints::setup_audio_stub;
use aberred_core::protocol::render_assets::RenderAssetCmd;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::appstate::AppState;
use aberred_core::resources::camera2d::{Camera2D, Camera2DRes};
use aberred_core::resources::camerafollowconfig::CameraFollowConfig;
use aberred_core::resources::collision_rule_index::CollisionRuleIndex;
use aberred_core::resources::debugoverlayconfig::DebugOverlayConfig;
use aberred_core::resources::deterministic_mode::DeterministicMode;
use aberred_core::resources::drawable_snapshot::DrawableSnapshot;
use aberred_core::resources::fontmetrics::{FontMetricsStore, FontMetricsWarnCache};
use aberred_core::resources::gameconfig::GameConfigDefaults;
use aberred_core::resources::gamestate::{GameState, GameStates, NextGameState};
use aberred_core::resources::group::TrackedGroups;
use aberred_core::resources::guiinputstate::GuiInputState;
use aberred_core::resources::guitheme::{GuiThemeStore, GuiThemeWarnCache};
use aberred_core::resources::input::InputState;
use aberred_core::resources::input_bindings::InputBindings;
use aberred_core::resources::loaded_assets::LoadedAssets;
use aberred_core::resources::pending_assets::PendingAssets;
use aberred_core::resources::postprocessshader::PostProcessShader;
use aberred_core::resources::rawinput::{ImguiCaptureMirror, PrevRawSnapshot};
use aberred_core::resources::screensize::ScreenSize;
use aberred_core::resources::signal_intents::SignalIntents;
use aberred_core::resources::sim_rng::SimRng;
use aberred_core::resources::systemsstore as hook_keys;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::texturedims::TextureDimsStore;
use aberred_core::resources::thread_stats::{AudioStats, SimStats};
use aberred_core::resources::windowsize::WindowSize;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_core::resources::worldtime::WorldTime;
use aberred_core::systems::collision_rule::collision_rule_observer;
use aberred_core::systems::gamestate::{clean_all_entities, quit_game};
use aberred_core::systems::mapspawn::spawn_map_observer;
use aberred_core::systems::menu::{
    menu_controller_observer, menu_despawn, menu_selection_observer,
};
use aberred_core::systems::scene_dispatch::{
    insert_scene_manager, scene_enter_loading, scene_enter_play, scene_switch_system,
};

#[cfg(feature = "lua")]
use aberred_lua::components::luacollision::LuaCollisionRuleIndex;
#[cfg(feature = "lua")]
use aberred_lua::resources::lua_runtime::LuaRuntime;
#[cfg(feature = "lua")]
use aberred_lua::systems::lua_animation_finished::lua_animation_finished_observer;
#[cfg(feature = "lua")]
use aberred_lua::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use aberred_lua::systems::lua_gui_interactable_click::lua_gui_interactable_click_observer;
#[cfg(feature = "lua")]
use aberred_lua::systems::lua_mapspawn::lua_map_spawned_observer;
#[cfg(feature = "lua")]
use aberred_lua::systems::lua_menu::lua_menu_selection_observer;
#[cfg(feature = "lua")]
use aberred_lua::systems::lua_tween_finished::lua_tween_finished_observer;
#[cfg(feature = "lua")]
use aberred_lua::systems::luatimer::lua_timer_observer;

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
        let mut tracked_groups = TrackedGroups::default();
        for name in init.tracked_groups.drain(..) {
            tracked_groups.add_persistent(name);
        }
        world.insert_resource(tracked_groups);
        world.insert_resource(CollisionRuleIndex::default());
        let screen = ScreenSize {
            w: render_width as i32,
            h: render_height as i32,
        };
        world.insert_resource(screen);
        world.insert_resource(WindowSize {
            w: init.window_w,
            h: init.window_h,
        });
        // SimRng::from_seed is the one construction path for both modes --
        // see that resource's doc comment. Deterministic mode seeds from
        // `deterministic_seed`; otherwise a throwaway entropy-seeded Rng is
        // read back via `get_seed()` so there's still a concrete, loggable
        // seed.
        let sim_seed = match init.deterministic_seed {
            Some(seed) => {
                log::info!("Deterministic mode: SimRng seeded with {seed}");
                world.insert_resource(DeterministicMode);
                seed
            }
            None => {
                let seed = fastrand::Rng::new().get_seed();
                log::info!("Non-deterministic mode: SimRng entropy-seeded with {seed}");
                seed
            }
        };
        world.insert_resource(SimRng::from_seed(sim_seed));
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
        world.insert_resource(PendingAssets::default());
        world.insert_resource(LoadedAssets::default());
        world.insert_resource(Messages::<RenderAssetCmd>::default());
        world.insert_resource(Camera2DRes(Camera2D::screen_centered(&screen)));
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
            let lua_runtime = LuaRuntime::new().map_err(|e| EngineError::Lua(e.to_string()))?;
            let path_display = script_path.to_string_lossy();
            if let Err(e) = lua_runtime.run_script(&path_display) {
                log::error!("Failed to load Lua script '{path_display}': {e}");
                eprintln!(
                    "Failed to load Lua script '{path_display}': {e}\n\
                     The engine will continue running with no scenes loaded; fix the script and restart."
                );
            }
            world.insert_non_send(lua_runtime);
            world.insert_resource(LuaCollisionRuleIndex::default());
        }

        world.spawn((Observer::new(observe_gamestate_change_event), Persistent));

        Ok(world)
    }

    /// Register the hook/scene one-shot systems into the LOGIC world (runs on
    /// the logic thread; consumes the setup hook and scenes out of `init`). A
    /// Lua game gets `lua_plugin`'s setup/enter_play/switch_scene and no
    /// scenes; every other game has at least `"main"`.
    pub(crate) fn register_logic_systems(
        init: &mut LogicInit,
        world: &mut World,
    ) -> Result<(), EngineError> {
        let mut systems_store = SystemsStore::new();

        #[cfg(feature = "lua")]
        if init.lua_script.is_some() {
            use aberred_lua::lua_plugin;
            register_persistent_system(
                world,
                &mut systems_store,
                hook_keys::SETUP,
                lua_plugin::setup,
            );
            register_persistent_system(
                world,
                &mut systems_store,
                hook_keys::ENTER_PLAY,
                lua_plugin::enter_play,
            );
            register_persistent_system(
                world,
                &mut systems_store,
                hook_keys::SWITCH_SCENE,
                lua_plugin::switch_scene,
            );
        }
        if let Some(hook) = init.setup_hook.take() {
            hook(world, &mut systems_store);
        }

        if !init.scenes.is_empty() {
            insert_scene_manager(
                world,
                std::mem::take(&mut init.scenes),
                init.initial_scene.take(),
                init.loading_scene.map(str::to_owned),
            );
            if init.loading_scene.is_some() {
                register_persistent_system(
                    world,
                    &mut systems_store,
                    hook_keys::ENTER_SETUP,
                    scene_enter_loading,
                );
            }

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

        world.insert_resource(systems_store);
        world.flush();

        Ok(())
    }

    /// The rest of the logic world's construction after
    /// [`Self::setup_logic_world`], in its one valid order: hook and scene
    /// systems, observers, entering `Setup` (so every observer exists when the
    /// setup hook runs and the loading scene is entered), then the `sim` and
    /// `present` schedules. Shared by the logic thread and `TestWorld`.
    pub(crate) fn init_logic_world(
        init: &mut LogicInit,
        world: &mut World,
    ) -> Result<(Schedule, Schedule), EngineError> {
        #[cfg(feature = "lua")]
        let has_lua = init.lua_script.is_some();
        #[cfg(not(feature = "lua"))]
        let has_lua = false;
        Self::register_logic_systems(init, world)?;
        Self::spawn_observers(world, has_lua, std::mem::take(&mut init.extra_observers));
        world.resource_mut::<NextGameState>().set(GameStates::Setup);
        world.trigger(GameStateChangedEvent {});
        Self::build_logic_schedules(std::mem::take(&mut init.extra_systems), world, has_lua)
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
        world.spawn((Observer::new(collision_rule_observer), Persistent));
        world.spawn((Observer::new(switch_debug_observer), Persistent));
        // switch_fullscreen_observer is NOT here: it lives in the RENDER
        // world — F10 toggles the window, which only exists there.
        world.spawn((Observer::new(menu_controller_observer), Persistent));
        world.spawn((Observer::new(menu_selection_observer), Persistent));
        #[cfg(feature = "lua")]
        if has_lua {
            world.spawn((Observer::new(lua_timer_observer), Persistent));
            world.spawn((Observer::new(lua_menu_selection_observer), Persistent));
            world.spawn((
                Observer::new(lua_gui_interactable_click_observer),
                Persistent,
            ));
            world.spawn((Observer::new(lua_animation_finished_observer), Persistent));
            world.spawn((Observer::new(lua_map_spawned_observer), Persistent));

            fn spawn_tween_finished_observer<T: aberred_core::components::tween::TweenValue>(
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
        world.spawn((Observer::new(spawn_map_observer), Persistent));

        // Spawn user-registered persistent observers
        for registrar in extra_observers {
            registrar(world);
        }

        world.flush();
    }
}
