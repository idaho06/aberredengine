//! Engine bootstrapping via the builder pattern.
//!
//! [`EngineBuilder`] captures all the boilerplate in `main.rs` — world setup,
//! window init, resources, system schedule, and main loop — into a single
//! configurable struct. The developer supplies only game-specific hooks.
//!
//! # Examples
//!
//! **Lua game:**
//! ```rust,no_run
//! # #[cfg(feature = "lua")]
//! # fn main() {
//! use aberredengine::engine_app::EngineBuilder;
//!
//! EngineBuilder::new()
//!     .with_lua("assets/scripts/main.lua")
//!     .run();
//! # }
//! # #[cfg(not(feature = "lua"))]
//! # fn main() {}
//! ```
//!
//! **Pure Rust game:**
//! ```rust,no_run,ignore
//! use aberredengine::engine_app::EngineBuilder;
//!
//! fn main() {
//!     EngineBuilder::new()
//!         .config("config.ini")
//!         .title("My Game")
//!         .on_setup(my_game::setup)
//!         .on_enter_play(my_game::enter_play)
//!         .on_update(my_game::update)
//!         .on_switch_scene(my_game::switch_scene)
//!         .run();
//! }
//! ```
//!
//! **Multiple per-frame systems and custom observers:**
//! ```rust,no_run,ignore
//! use aberredengine::engine_app::EngineBuilder;
//! use aberredengine::systems::scene_dispatch::SceneDescriptor;
//!
//! fn main() {
//!     EngineBuilder::new()
//!         .config("config.ini")
//!         .on_setup(load_assets)
//!         .add_system(tilemap_load_system)   // runs every frame while Playing
//!         .add_system(tilemap_save_system)   // multiple systems allowed
//!         .add_observer(on_tilemap_loaded)   // persistent observer for a custom event
//!         .add_scene("intro", SceneDescriptor { /* … */ })
//!         .add_scene("editor", SceneDescriptor { /* … */ })
//!         .initial_scene("intro")
//!         .run();
//! }
//! ```
//!
//! For scene-scoped (transient) observers — active only within one scene —
//! spawn them from the scene's `on_enter` callback without [`Persistent`]:
//! ```rust,no_run,ignore
//! fn my_scene_enter(ctx: &mut GameCtx) {
//!     // Cleaned up automatically by clean_all_entities on scene switch
//!     ctx.commands.spawn(Observer::new(on_my_scene_event));
//! }
//! ```

use std::path::PathBuf;

use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoObserverSystem;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::system::SystemParam;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use raylib::ffi::TraceLogLevel;

use crate::pacing::Pacer;
use crate::protocol::raw_input::{InputSample, RawDeviceSnapshot};
use crate::protocol::render_logic::{LogicMsg, RenderMsg};
use crate::events::switchfullscreen::SwitchFullScreenEvent;
use crate::protocol::endpoints::{
    LogicBridge, LogicTx, RenderTx, shutdown_logic, shutdown_logic_bridge,
};
use crate::protocol::snapshot::{SnapshotConsumer, SnapshotPublisher};
use crate::systems::logic_bridge::{forward_render_asset_cmds, send_drawable_snapshot};

use crate::components::mapposition::MapPosition;
use crate::components::screenposition::ScreenPosition;
use crate::components::persistent::Persistent;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::events::gamestate::GameStateChangedEvent;
use crate::events::gamestate::observe_gamestate_change_event;
use crate::events::render_assets::RenderAssetCmd;
use crate::events::switchdebug::switch_debug_observer;
use crate::events::switchfullscreen::switch_fullscreen_observer;
use crate::resources::animationstore::AnimationStore;
use crate::resources::appstate::AppState;
use crate::protocol::endpoints::{setup_audio, shutdown_audio};
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::fontmetrics::{FontMetricsStore, FontMetricsWarnCache};
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use crate::resources::group::TrackedGroups;
use crate::resources::guiinputstate::GuiInputState;
use crate::resources::guitheme::{GuiThemeStore, GuiThemeWarnCache};
use crate::systems::gui_interactable_click::gui_interactable_click_observer;
use crate::resources::imgui_bridge::{ImguiBridge, ImguiCaptureState};
use crate::resources::input::InputState;
use crate::resources::input_bindings::InputBindings;
use crate::resources::pending_imgui_capture::PendingImguiCapture;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::quit_requested::QuitRequested;
use crate::resources::render_mirrors::{
    RenderActiveScene, RenderAppState, RenderCamera, RenderCameraFollow, RenderDebugSnapshot,
    RenderGameConfig, RenderGuiThemes, RenderPostProcess, RenderSignalSnapshot, RenderWorldTime,
};
use crate::resources::rendertarget::RenderTarget;
use crate::resources::scenemanager::{RenderSceneTable, SceneManager};
use crate::resources::screensize::ScreenSize;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturedims::TextureDimsStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::windowsize::WindowSize;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::drawable_snapshot::{build_drawable_snapshot, DrawableSnapshot};
use crate::resources::worldtime::WorldTime;
use crate::resources::signal_intents::SignalIntents;
use crate::systems::animation::animation;
use crate::systems::animation::animation_controller;
use crate::systems::audio::{
    forward_audio_cmds, poll_audio_messages, update_bevy_audio_cmds, update_bevy_audio_messages,
};
use crate::systems::camera_follow::camera_follow_system;
use crate::systems::collision_detector::collision_detector;
use crate::systems::dynamictext_size::dynamictext_size_system;
use crate::systems::gameconfig::apply_gameconfig_changes;
use crate::systems::gamestate::{
    check_pending_state, clean_all_entities, quit_game, state_is_playing,
};
use crate::systems::gridlayout::gridlayout_spawn_system;
use crate::systems::group::update_group_counts_system;
use crate::systems::gui_hit_test::gui_hit_test_system;
use crate::systems::gui_image_state_sync::gui_image_state_sync_system;
use crate::systems::gui_layout::gui_layout_system;
use crate::systems::gui_progressbar_signal_update::gui_progressbar_signal_update_system;
use crate::systems::gui_spawn::{
    gui_button_spawn_system, gui_image_spawn_system, gui_label_spawn_system,
};
use crate::resources::rawinput::{ImguiCaptureMirror, PrevRawSnapshot};
use crate::systems::input::{resolve_input_backlog, sample_raw_device_snapshot};
use crate::systems::inputaccelerationcontroller::input_acceleration_controller;
use crate::systems::inputsimplecontroller::input_simple_controller;
use crate::systems::mapspawn::spawn_map_observer;
use crate::systems::menu::menu_selection_observer;
use crate::systems::menu::{menu_controller_observer, menu_despawn, menu_spawn_system};
use crate::systems::render_assets::{process_render_asset_cmds, update_bevy_render_asset_cmds};
use crate::systems::mousecontroller::mouse_controller;
use crate::systems::movement::movement;
use crate::systems::particleemitter::particle_emitter_system;
use crate::systems::phase::phase_system;
use crate::systems::propagate_transforms::{
    cleanup_orphaned_global_transforms, propagate_transforms,
};
use crate::systems::render::render_system;
use crate::systems::rust_collision::rust_collision_observer;
use crate::systems::signal_intents::apply_signal_intents;
use crate::systems::scene_dispatch::{
    SceneDescriptor, scene_enter_play, scene_switch_poll, scene_switch_system, scene_update_system,
};
use crate::systems::signalbinding::update_world_signals_binding_system;
use crate::systems::stuckto::stuck_to_entity_system;
use crate::systems::tilemap::tilemap_spawn_system;
use crate::systems::time::update_world_time;
use crate::systems::timer::{timer_observer, update_timers};
use crate::systems::ttl::ttl_system;
use crate::systems::tween::tween_system;
use raylib::prelude::{Camera2D, Vector2};

#[cfg(feature = "lua")]
use crate::resources::lua_runtime::LuaRuntime;
#[cfg(feature = "lua")]
use crate::systems::lua_animation_finished::lua_animation_finished_observer;
#[cfg(feature = "lua")]
use crate::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use crate::systems::lua_setup_entity::lua_setup_entity_system;
#[cfg(feature = "lua")]
use crate::systems::lua_tween_finished::lua_tween_finished_observer;
#[cfg(feature = "lua")]
use crate::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use crate::systems::luatimer::{lua_timer_observer, update_lua_timers};
#[cfg(feature = "lua")]
use crate::systems::mapspawn::process_lua_map_commands;

/// Closure that registers a system into the world and inserts its ID into
/// [`SystemsStore`]. Deferred until `run()` when the [`World`] exists.
/// `Send` because these are carried into the logic thread via `LogicInit`
/// (Phase 5e).
type HookRegistrar = Box<dyn FnOnce(&mut World, &mut SystemsStore) + Send>;

/// Closure that adds a game-update system to the [`Schedule`].
/// Deferred until `run()` when the schedule is being built.
/// `Send`: see [`HookRegistrar`].
type UpdateRegistrar = Box<dyn FnOnce(&mut Schedule) + Send>;

/// System sets partitioning the logic thread's `fixed` schedule pipeline
/// (Phase 7b Step 0). Replaces the old hand-enumerated `.after()`/`.before()`
/// edges between individual systems: [`EngineBuilder`]'s
/// `build_logic_schedules` declares one `configure_sets((...).chain())` over
/// this list as the single source of ordering truth between groups; edges
/// *within* a group that are still load-bearing remain explicit `.after()`
/// calls (see that function's doc comment for the convention).
///
/// Exported so [`configure_schedule`](EngineBuilder::configure_schedule) /
/// [`configure_fixed_schedule`](EngineBuilder::configure_fixed_schedule)
/// closures can position custom systems relative to engine groups, e.g.
/// `.in_set(SimSet::Movement)` or `.before(SimSet::Collision)`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimSet {
    /// Drain `SignalIntents` queued by the render thread's `GuiCallback` into
    /// `WorldSignals`, before anything this substep reads them.
    ApplyIntents,
    /// One-shot spawns reacting to `Added<T>` (menu/gridlayout/tilemap) and
    /// game-state bookkeeping (`check_pending_state`).
    Spawn,
    /// Audio command/message pump (`update_bevy_audio_cmds` ->
    /// `forward_audio_cmds` -> `poll_audio_messages` ->
    /// `update_bevy_audio_messages`, kept as an explicit `.chain()`).
    AudioPump,
    /// User `on_update`/`add_system`/`add_fixed_system` hooks. Lua's
    /// `on_update_<scene>` no longer lives here -- it's dispatched from
    /// `lua_plugin::update` in `SimSet::Bookkeeping` instead, restoring the
    /// pre-thread-split engine's ordering (after camera-follow/collision, not
    /// before).
    ScriptUpdate,
    /// Input-driven force/velocity controllers.
    Controllers,
    /// Particle emission, movement integration, TTL, and position/rotation/
    /// scale tweens.
    Movement,
    /// World-space transform propagation and camera follow.
    Transforms,
    /// Collision detection and its direct reactions (`stuck_to`, `phase`).
    Collision,
    /// GUI layout, hit-test, and per-state visual sync.
    Gui,
    /// Group counts, Lua phase callbacks, and animation controller
    /// resolution -- all downstream of this substep's collision results.
    PostCollision,
    /// Lua command-queue draining (map/asset commands, entity setup) and
    /// scene lifecycle polling.
    Drain,
    /// Tail-of-substep housekeeping: signal bindings, text sizing, input
    /// binding change notification, and (for Lua games) `lua_plugin::update`.
    Bookkeeping,
}

/// Closure that spawns an observer entity into the [`World`].
/// Deferred until `run()` when the world exists.
/// `Send`: see [`HookRegistrar`].
type ObserverRegistrar = Box<dyn FnOnce(&mut World) + Send>;

/// Builder for bootstrapping the engine.
///
/// Handles world setup, window init, resources, system schedule, and main loop.
/// The developer supplies only game-specific hooks: `setup`, `enter_play`,
/// `update`, and `switch_scene`.
///
/// In addition to the single-system hooks, the builder supports registering
/// multiple per-frame systems ([`add_system`](Self::add_system),
/// [`configure_schedule`](Self::configure_schedule)) and persistent observers
/// ([`add_observer`](Self::add_observer)) for custom event handling.
#[must_use = "EngineBuilder does nothing until .run() is called"]
pub struct EngineBuilder {
    config_path: PathBuf,
    config_str: Option<&'static str>,
    title_override: Option<String>,
    setup_hook: Option<HookRegistrar>,
    enter_play_hook: Option<HookRegistrar>,
    update_hook: Option<UpdateRegistrar>,
    fixed_update_hook: Option<UpdateRegistrar>,
    switch_scene_hook: Option<HookRegistrar>,
    scenes: Vec<(String, SceneDescriptor)>,
    initial_scene: Option<String>,
    extra_systems: Vec<UpdateRegistrar>,
    extra_fixed_systems: Vec<UpdateRegistrar>,
    extra_observers: Vec<ObserverRegistrar>,
    #[cfg(feature = "lua")]
    lua_script: Option<PathBuf>,
}

impl EngineBuilder {
    /// Create a new builder with default settings.
    ///
    /// Defaults: config path `"config.ini"`, no title override, no hooks.
    pub fn new() -> Self {
        Self {
            config_path: PathBuf::from("config.ini"),
            config_str: None,
            title_override: None,
            setup_hook: None,
            enter_play_hook: None,
            update_hook: None,
            fixed_update_hook: None,
            switch_scene_hook: None,
            scenes: Vec::new(),
            initial_scene: None,
            extra_systems: Vec::new(),
            extra_fixed_systems: Vec::new(),
            extra_observers: Vec::new(),
            #[cfg(feature = "lua")]
            lua_script: None,
        }
    }

    /// Set a custom path for the config file (default: `"config.ini"`).
    pub fn config(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = path.into();
        self
    }

    /// Supply config as an inline string instead of reading from a file.
    ///
    /// Takes priority over [`.config()`](Self::config) if both are called.
    /// Intended for use with `include_str!` to embed `config.ini` at compile time.
    pub fn config_str(mut self, content: &'static str) -> Self {
        self.config_str = Some(content);
        self
    }

    /// Override the window title. Takes precedence over `config.ini [window] title`.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title_override = Some(title.into());
        self
    }

    /// Register the `setup` hook (called during the `Setup` game state).
    ///
    /// The system is registered into [`SystemsStore`] under the key `"setup"`.
    pub fn on_setup<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.setup_hook = Some(Box::new(|world, store| {
            register_persistent_system(world, store, "setup", system);
        }));
        self
    }

    /// Register the `enter_play` hook (called when transitioning to `Playing`).
    ///
    /// The system is registered into [`SystemsStore`] under the key `"enter_play"`.
    pub fn on_enter_play<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.enter_play_hook = Some(Box::new(|world, store| {
            register_persistent_system(world, store, "enter_play", system);
        }));
        self
    }

    /// Register the `update` hook.
    ///
    /// Runs once per sim tick (`[simulation] hz` in `config.ini`, Phase 7b),
    /// in [`SimSet::ScriptUpdate`]. Treat it as idempotent/edge-triggered the
    /// same way Lua's `on_update_<scene>` must be
    /// (`.claude/context/system-order.md`): gate one-shot effects on an edge,
    /// not on "runs once per visible frame" -- the sim ticks faster than the
    /// render thread. The system is added with `.run_if(state_is_playing)`.
    pub fn on_update<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(system.run_if(state_is_playing).in_set(SimSet::ScriptUpdate));
        }));
        self
    }

    /// Register the `switch_scene` hook (called when a scene transition is requested).
    ///
    /// The system is registered into [`SystemsStore`] under the key `"switch_scene"`.
    pub fn on_switch_scene<M>(
        mut self,
        system: impl IntoSystem<(), (), M> + Send + 'static,
    ) -> Self {
        self.switch_scene_hook = Some(Box::new(|world, store| {
            register_persistent_system(world, store, "switch_scene", system);
        }));
        self
    }

    /// Add a system to the sim schedule alongside `on_update`/Lua's
    /// `on_update_<scene>`.
    ///
    /// Runs once per sim tick, in [`SimSet::ScriptUpdate`] -- see
    /// [`.on_update()`](Self::on_update)'s doc for the cadence implications.
    /// The system is added with `.run_if(state_is_playing)`, matching the
    /// behaviour of [`.on_update()`](Self::on_update). Can be called multiple
    /// times to register several systems.
    ///
    /// For custom ordering relative to other engine systems (e.g. `.after(movement)`)
    /// or for systems with different run conditions, use
    /// [`configure_schedule`](Self::configure_schedule) instead.
    ///
    /// # Scene-scoped (transient) observers
    ///
    /// If you need an observer that is only active within a specific scene, spawn
    /// it from the scene's `on_enter` callback **without** the [`Persistent`] component:
    ///
    /// ```rust,ignore
    /// fn my_scene_enter(ctx: &mut GameCtx) {
    ///     // No Persistent → cleaned up on scene switch by clean_all_entities
    ///     ctx.commands.spawn(Observer::new(on_my_event));
    /// }
    /// ```
    pub fn add_system<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.extra_systems
            .push(Box::new(move |schedule: &mut Schedule| {
                schedule.add_systems(system.run_if(state_is_playing).in_set(SimSet::ScriptUpdate));
            }));
        self
    }

    /// Add systems to the sim schedule with full control over ordering and
    /// run conditions.
    ///
    /// Targets the same sim schedule as [`add_system`](Self::add_system). The
    /// closure receives a `&mut Schedule` and can call
    /// `schedule.add_systems(…)` with any configuration, including
    /// `.in_set(SimSet::X)` (the sets used by the engine's own pipeline are
    /// exported as [`SimSet`] specifically for this). No automatic
    /// constraints are applied — the developer is responsible for
    /// `.run_if()`, `.after()`, `.before()`, `.in_set()` etc.
    ///
    /// ```rust,ignore
    /// .configure_schedule(|schedule| {
    ///     schedule.add_systems(
    ///         my_system
    ///             .run_if(state_is_playing)
    ///             .in_set(SimSet::Movement)
    ///             .before(SimSet::Collision),
    ///     );
    /// })
    /// ```
    pub fn configure_schedule(mut self, f: impl FnOnce(&mut Schedule) + Send + 'static) -> Self {
        self.extra_systems.push(Box::new(f));
        self
    }


    /// Add a system to the sim schedule.
    ///
    /// Use this for custom Rust-side physics/gameplay logic that needs a
    /// deterministic, render-rate-independent tick -- e.g. a game-specific
    /// movement modifier that must run alongside [`movement`](crate::systems::movement::movement)
    /// and before [`collision_detector`](crate::systems::collision_detector::collision_detector).
    ///
    /// The system is added with `.run_if(state_is_playing)`, matching
    /// [`.add_system()`](Self::add_system), but with no `SimSet` membership
    /// of its own -- it runs unordered relative to the engine's pipeline
    /// unless you also constrain it. For ordering relative to `SimSet`
    /// groups, use [`configure_fixed_schedule`](Self::configure_fixed_schedule)
    /// instead.
    pub fn add_fixed_system<M>(
        mut self,
        system: impl IntoSystem<(), (), M> + Send + 'static,
    ) -> Self {
        self.extra_fixed_systems
            .push(Box::new(move |schedule: &mut Schedule| {
                schedule.add_systems(system.run_if(state_is_playing));
            }));
        self
    }

    /// Add systems to the sim schedule with full control over ordering and
    /// run conditions.
    ///
    /// Mirrors [`configure_schedule`](Self::configure_schedule), including
    /// `SimSet` availability; kept as a separate builder method for games
    /// that want to distinguish "custom fixed-tick systems" from "custom
    /// scripted-update systems" in their own code organization, even though
    /// both target the same schedule.
    pub fn configure_fixed_schedule(mut self, f: impl FnOnce(&mut Schedule) + Send + 'static) -> Self {
        self.extra_fixed_systems.push(Box::new(f));
        self
    }

    /// Add a persistent observer for a custom (or engine) event.
    ///
    /// The observer is spawned with the [`Persistent`] component and therefore
    /// survives scene transitions. The observer function's first parameter must
    /// be `On<E>` where `E` is the event type.
    ///
    /// ```rust,ignore
    /// #[derive(Event)]
    /// struct TilemapLoaded { path: String }
    ///
    /// fn on_tilemap_loaded(trigger: On<TilemapLoaded>, mut ctx: GameCtx) {
    ///     // react to the event …
    /// }
    ///
    /// EngineBuilder::new()
    ///     .add_observer(on_tilemap_loaded)
    ///     // …
    /// ```
    ///
    /// To trigger the event from a system or scene callback:
    /// ```rust,ignore
    /// commands.trigger(TilemapLoaded { path: "…".into() });
    /// ```
    pub fn add_observer<E: Event, B: Bundle, M>(
        mut self,
        observer: impl IntoObserverSystem<E, B, M>,
    ) -> Self {
        self.extra_observers
            .push(Box::new(move |world: &mut World| {
                world.spawn((Observer::new(observer), Persistent));
            }));
        self
    }

    /// Register a named scene for [`SceneManager`]-based games.
    ///
    /// Scenes are stored and later inserted into a [`SceneManager`] resource
    /// at `.run()` time. Use with [`.initial_scene()`](Self::initial_scene) to
    /// specify which scene starts first.
    ///
    /// # Panics (at `.run()`)
    ///
    /// - If `.add_scene()` is combined with `.on_switch_scene()` or `.on_enter_play()`
    /// - If `.add_scene()` is used without `.initial_scene()`
    pub fn add_scene(mut self, name: impl Into<String>, descriptor: SceneDescriptor) -> Self {
        self.scenes.push((name.into(), descriptor));
        self
    }

    /// Set the initial scene for [`SceneManager`]-based games.
    ///
    /// This scene's `on_enter` callback will be the first called when the
    /// game transitions to the `Playing` state.
    pub fn initial_scene(mut self, name: impl Into<String>) -> Self {
        self.initial_scene = Some(name.into());
        self
    }

    /// Configure the builder for a Lua game.
    ///
    /// Sets up all four hooks to use `lua_plugin` functions and initialises the
    /// Lua runtime with the given script path.
    #[cfg(feature = "lua")]
    pub fn with_lua(mut self, script_path: impl Into<PathBuf>) -> Self {
        use crate::lua_plugin;

        self.lua_script = Some(script_path.into());

        self.setup_hook = Some(Box::new(|world, store| {
            register_persistent_system(world, store, "setup", lua_plugin::setup);
        }));
        self.enter_play_hook = Some(Box::new(|world, store| {
            register_persistent_system(world, store, "enter_play", lua_plugin::enter_play);
        }));
        // lua_plugin::update runs once per sim tick, in SimSet::Bookkeeping
        // (last among the engine's own groups) -- this is also where
        // on_update_<scene> itself is dispatched now, restoring the
        // pre-thread-split engine's explicit `.after(camera_follow_system)`
        // guarantee (see lua_plugin::update's own doc comment for the full
        // rationale and the mid-tick scene-switch consequence).
        self.update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(
                lua_plugin::update
                    .run_if(state_is_playing)
                    .in_set(SimSet::Bookkeeping),
            );
        }));
        self.switch_scene_hook = Some(Box::new(|world, store| {
            register_persistent_system(world, store, "switch_scene", lua_plugin::switch_scene);
        }));
        self
    }

    /// Build the engine and run the main loop.
    ///
    /// This consumes the builder and does not return until the game exits.
    /// Startup failures are logged and abort engine initialization without
    /// entering the main loop.
    pub fn run(self) {
        if let Err(err) = self.try_run() {
            log::error!("Failed to start engine: {err}");
        }
    }

    /// Build the engine and run the main loop.
    ///
    /// This variant returns startup errors to the caller instead of logging
    /// them internally.
    ///
    /// Phase 5e: two `World`s on two threads. The main thread owns the raylib
    /// window plus a small render `World` (GL stores, latest received
    /// [`DrawableSnapshot`]); the spawned logic thread builds the gameplay
    /// `World` inside its own closure (so NonSend `LuaRuntime` is created on,
    /// and pinned to, that thread) and runs the FIXED 240Hz accumulator on
    /// its own wall clock. Communication is crossbeam channels only
    /// ([`LogicMsg`]/[`RenderMsg`] — fully `Send` enums). Logic-thread
    /// startup errors are logged from that thread and surface as an
    /// immediate `RenderMsg::Quit`, not as an `Err` here.
    pub fn try_run(mut self) -> Result<(), String> {
        crate::protocol::shutdown::install_panic_hook();
        log::info!("Hello, world! This is the Aberred Engine!");

        let use_scene_manager = !self.scenes.is_empty();

        self.validate_builder(use_scene_manager)?;
        let config = self.load_config()?;
        let (rl, thread, render_target) = Self::setup_window(&config)?;

        let (tx_logic, rx_logic) = unbounded::<LogicMsg>();
        let (tx_render, rx_render) = unbounded::<RenderMsg>();
        // Phase 7d: input gets its own dedicated bounded channel, separate
        // from the unbounded LogicMsg channel above — see LogicBridge::tx_input.
        let (tx_input, rx_input) = bounded::<InputSample>(8);
        // Phase 7c: the DrawableSnapshot itself travels via a triple buffer,
        // not the RenderMsg channel above -- sim writes `snap_in`, render
        // reads `snap_out`, latest-wins, no queue growth. Seeded with
        // `DrawableSnapshot::default()`; the render world's actual
        // `DrawableSnapshot` resource is seeded separately with the real
        // loaded config below, so this initial buffer value is never read
        // before the sim's first real publish (`Output::update()` gates it).
        let (snap_in, snap_out) =
            triple_buffer::TripleBuffer::new(&DrawableSnapshot::default()).split();

        // Render-side clone of the scene-descriptor table (fn pointers, cheap)
        // for gui/world-draw callback resolution against RenderActiveScene
        // (Phase 7f-2; was snapshot.active_scene before the split).
        let render_scene_table = use_scene_manager.then(|| {
            RenderSceneTable(
                self.scenes
                    .iter()
                    .map(|(name, desc)| (name.clone(), desc.clone()))
                    .collect(),
            )
        });

        let init = LogicInit {
            config: config.clone(),
            setup_hook: self.setup_hook.take(),
            enter_play_hook: self.enter_play_hook.take(),
            switch_scene_hook: self.switch_scene_hook.take(),
            update_hook: self.update_hook.take(),
            fixed_update_hook: self.fixed_update_hook.take(),
            extra_systems: std::mem::take(&mut self.extra_systems),
            extra_fixed_systems: std::mem::take(&mut self.extra_fixed_systems),
            extra_observers: std::mem::take(&mut self.extra_observers),
            scenes: std::mem::take(&mut self.scenes),
            initial_scene: self.initial_scene.take(),
            #[cfg(feature = "lua")]
            lua_script: self.lua_script.take(),
            window_w: rl.get_screen_width(),
            window_h: rl.get_screen_height(),
            tx_render,
            rx_logic,
            rx_input,
            snapshot_publisher: Some(SnapshotPublisher(snap_in)),
        };
        let handle = std::thread::Builder::new()
            .name("aberred-logic".into())
            .spawn(move || logic_thread(init))
            .map_err(|err| format!("Failed to spawn logic thread: {err}"))?;

        let mut render_world = Self::setup_render_world(
            config,
            rl,
            thread,
            render_target,
            render_scene_table,
            SnapshotConsumer(snap_out),
            LogicBridge {
                tx_logic,
                tx_input,
                rx_render,
                handle,
            },
        )?;
        let mut render_schedule = Self::build_render_schedule(&mut render_world)?;
        Self::render_main_loop(&mut render_world, &mut render_schedule);

        Ok(())
    }

    fn validate_builder(&self, use_scene_manager: bool) -> Result<(), String> {
        if use_scene_manager {
            if self.switch_scene_hook.is_some() {
                return Err(
                    "EngineBuilder conflict: .add_scene() and .on_switch_scene() cannot be used \
                     together. Use .add_scene() for SceneManager-based games, or \
                     .on_switch_scene() for full manual control -- not both."
                        .to_string(),
                );
            }
            if self.enter_play_hook.is_some() {
                return Err(
                    "EngineBuilder conflict: .add_scene() and .on_enter_play() cannot be used \
                     together. SceneManager owns the enter_play hook. Use .on_setup() for \
                     asset loading instead."
                        .to_string(),
                );
            }
            if self.initial_scene.is_none() {
                return Err(
                    "EngineBuilder: .add_scene() requires .initial_scene(\"name\") to specify \
                     which scene to enter first."
                        .to_string(),
                );
            }
        }

        Ok(())
    }

    fn load_config(&self) -> Result<GameConfig, String> {
        let mut config = GameConfig::with_path(&self.config_path);
        if let Some(content) = &self.config_str {
            config
                .load_from_str(content)
                .map_err(|err| format!("Failed to parse embedded config: {err}"))?;
        } else {
            config.load_from_file().map_err(|err| {
                format!(
                    "Failed to load config '{}': {err}",
                    self.config_path.display()
                )
            })?;
        }
        if let Some(title) = &self.title_override {
            config.window_title = title.clone();
        }
        Ok(config)
    }

    fn raylib_log_level_from_env() -> TraceLogLevel {
        std::env::var("RUST_LOG")
            .ok()
            .as_deref()
            .map(Self::raylib_log_level_from_rust_log)
            .unwrap_or(TraceLogLevel::LOG_INFO)
    }

    fn raylib_log_level_from_rust_log(rust_log: &str) -> TraceLogLevel {
        let default_directive = rust_log
            .split(',')
            .map(str::trim)
            .find(|directive| !directive.is_empty() && !directive.contains('='));

        let level = default_directive
            .and_then(|directive| directive.split('/').next())
            .map(|directive| directive.trim().to_ascii_lowercase());

        match level.as_deref() {
            Some("trace") => TraceLogLevel::LOG_TRACE,
            Some("debug") => TraceLogLevel::LOG_DEBUG,
            Some("info") => TraceLogLevel::LOG_INFO,
            Some("warn") | Some("warning") => TraceLogLevel::LOG_WARNING,
            Some("error") => TraceLogLevel::LOG_ERROR,
            Some("off") => TraceLogLevel::LOG_NONE,
            _ => TraceLogLevel::LOG_INFO,
        }
    }

    fn setup_window(
        config: &GameConfig,
    ) -> Result<(raylib::RaylibHandle, raylib::RaylibThread, RenderTarget), String> {
        let raylib_log_level = Self::raylib_log_level_from_env();
        let (mut rl, thread) = raylib::init()
            .size(config.window_width as i32, config.window_height as i32)
            .resizable()
            .title(&config.window_title)
            .log_level(raylib_log_level)
            .highdpi()
            .msaa_4x()
            .build();
        rl.set_target_fps(config.target_fps);
        rl.set_exit_key(None);

        let render_target =
            RenderTarget::new(&mut rl, &thread, config.render_width, config.render_height)
                .map_err(|err| format!("Failed to create render target: {err}"))?;

        Ok((rl, thread, render_target))
    }

    /// Build the render (main-thread) `World` (Phase 5e): raylib window +
    /// GL/NonSend stores + the latest received [`DrawableSnapshot`], plus
    /// the render-owned `InputState` mirror (written by the render loop from
    /// its `sample_raw_device_snapshot` result — Phase 7d: this no longer
    /// carries resolved bindings/edges, see `DebugResources::input_state`'s
    /// doc comment) and `DebugOverlayConfig` (edited by the imgui panel).
    /// `InputBindings` became logic-thread-only in Phase 7d — no render-side
    /// mirror. Holds NO entities and no gameplay resources.
    fn setup_render_world(
        config: GameConfig,
        rl: raylib::RaylibHandle,
        thread: raylib::RaylibThread,
        render_target: RenderTarget,
        render_scene_table: Option<RenderSceneTable>,
        snapshot_consumer: SnapshotConsumer,
        bridge: LogicBridge,
    ) -> Result<World, String> {
        let mut world = World::new();
        world.insert_resource(ScreenSize {
            w: config.render_width as i32,
            h: config.render_height as i32,
        });
        world.insert_resource(WindowSize {
            w: rl.get_screen_width(),
            h: rl.get_screen_height(),
        });
        // Seed the snapshot's config with the REAL loaded config, not
        // DrawableSnapshot::default()'s baked-in defaults: apply_gameconfig_changes
        // reads config exclusively from the snapshot, and the first logic-built
        // snapshot arrives a frame or two later — a default-seeded copy would
        // briefly apply the wrong render/window size at startup.
        // RenderGameConfig gets the same real-config seed as DrawableSnapshot
        // just below, for the same reason: apply_gameconfig_changes reads it
        // from frame 1, before the first logic-built snapshot arrives.
        world.insert_resource(RenderGameConfig(config.clone()));
        world.insert_resource(DrawableSnapshot {
            game_config: config,
            ..Default::default()
        });
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
            Err(err) => {
                shutdown_logic_bridge(bridge);
                return Err(format!("Failed to initialize imgui bridge: {err}"));
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

    /// Build the logic-thread gameplay `World` (Phase 5e): everything the old
    /// single `setup_world` inserted except GL/window state, plus the
    /// message-fed mirrors (`ScreenSize`/`WindowSize`/`DebugOverlayConfig`)
    /// and the logic-owned stores fed by render notifications
    /// (`FontMetricsStore`/`TextureDimsStore`). Runs INSIDE the thread
    /// closure so NonSend `LuaRuntime` is created on (and pinned to) the
    /// logic thread.
    fn setup_logic_world(init: &mut LogicInit) -> Result<World, String> {
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
        world.insert_resource(config);
        world.insert_resource(InputState::default());
        world.insert_resource(InputBindings::default());
        world.insert_resource(PrevRawSnapshot::default());
        world.insert_resource(ImguiCaptureMirror::default());

        setup_audio(&mut world, audio_hz);

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
            let lua_runtime =
                LuaRuntime::new().map_err(|err| format!("Failed to create Lua runtime: {err}"))?;
            if let Err(e) = lua_runtime.run_script(script_path.to_str().unwrap_or("")) {
                log::error!("Failed to load Lua script: {}", e);
            }
            world.insert_non_send(lua_runtime);
        }

        world.spawn((Observer::new(observe_gamestate_change_event), Persistent));

        Ok(world)
    }

    fn validate_required_systems(
        systems_store: &SystemsStore,
        requires_switch_scene: bool,
    ) -> Result<(), String> {
        let mut missing = Vec::new();

        for name in ["setup", "enter_play", "quit_game"] {
            if systems_store.get(name).is_none() {
                missing.push(name);
            }
        }

        if requires_switch_scene && systems_store.get("switch_scene").is_none() {
            missing.push("switch_scene");
        }

        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "EngineBuilder missing required system registrations: {}",
                missing.join(", ")
            ))
        }
    }

    /// Register the hook/scene one-shot systems into the LOGIC world (runs on
    /// the logic thread; consumes the hooks out of `init`). The render-side
    /// scene table was already cloned off before `init` crossed the thread
    /// boundary.
    fn register_logic_systems(
        init: &mut LogicInit,
        world: &mut World,
        use_scene_manager: bool,
    ) -> Result<(), String> {
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
                "switch_scene",
                scene_switch_system,
            );
            register_persistent_system(world, &mut systems_store, "enter_play", scene_enter_play);
        }

        register_persistent_system(world, &mut systems_store, "quit_game", quit_game);
        register_persistent_system(
            world,
            &mut systems_store,
            "clean_all_entities",
            clean_all_entities,
        );

        let menu_despawn_system_id = world.register_system(menu_despawn);
        world
            .entity_mut(menu_despawn_system_id.entity())
            .insert(Persistent);
        systems_store.insert_entity_system("menu_despawn", menu_despawn_system_id);

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

    fn spawn_observers(world: &mut World, has_lua: bool, extra_observers: Vec<ObserverRegistrar>) {
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

    /// Build the two schedules the logic thread runs: `fixed` (Phase 7b:
    /// runs once per `Pacer`-paced sim tick at `[simulation] hz`, real dt --
    /// this is where essentially all gameplay logic lives: movement,
    /// collision, phases, animation, Lua scripting, GUI layout/hit-test,
    /// scene lifecycle, and per-tick housekeeping; the binding still called
    /// `fixed` returned from this function is bound to `sim` by its caller,
    /// `logic_thread_main`, since ticks are no longer fixed-duration) and
    /// `present` (Phase 7c: decimated to `[simulation] snapshot_hz`, not run
    /// once per received input sample anymore -- package the tick's
    /// fully-settled state into a `DrawableSnapshot` and publish it into the
    /// `SnapshotPublisher` triple buffer; nothing else). See
    /// `docs/render-simulation-separation-brainstorm.md`,
    /// `docs/render-logic-simplification-brainstorm.md`'s "Collapsing
    /// VARIABLE" section, `docs/plans/phase7b-pacing-configurable-frequencies.md`,
    /// `docs/plans/phase7c-triple-buffer-snapshot.md`, and
    /// `.claude/context/system-order.md` for the rationale behind the split
    /// and the full list of which system lives where. `present` was called
    /// `variable` before Phase 6d; the old name stopped describing anything
    /// running there once nearly everything moved to `fixed`.
    ///
    /// `fixed`'s internal ordering is expressed via [`SimSet`] (Phase 7b Step
    /// 0) rather than per-system `.after()`/`.before()` edges: one
    /// `configure_sets((...).chain())` call declares the pipeline, and each
    /// system joins its group with `.in_set(SimSet::X)`. Only *intra*-set
    /// edges that are still load-bearing (e.g.
    /// `cleanup_orphaned_global_transforms.after(propagate_transforms)`
    /// within `Transforms`) are kept explicit -- every edge that used to
    /// cross groups is now implied by set order and was deleted.
    ///
    /// `bevy_ecs` cannot express `.after()`/`.before()` across two separate
    /// `Schedule`s, so `present`'s `.after()` markers referencing `fixed`
    /// systems (e.g. `build_drawable_snapshot`'s list below) remain vacuous
    /// doc-value markers, not real constraints -- `logic_thread_main`'s loop
    /// structure guarantees the tick's `fixed`/`sim` run completes before
    /// `present` runs, which is the ordering those edges express.
    fn build_logic_schedules(
        update_hook: Option<UpdateRegistrar>,
        fixed_update_hook: Option<UpdateRegistrar>,
        extra_systems: Vec<UpdateRegistrar>,
        extra_fixed_systems: Vec<UpdateRegistrar>,
        world: &mut World,
        has_lua: bool,
        use_scene_manager: bool,
    ) -> Result<(Schedule, Schedule), String> {
        let mut fixed = Schedule::default();
        let mut present = Schedule::default();

        // Single source of truth for cross-group ordering within `fixed`
        // (Phase 7b Step 0) -- see `SimSet`'s doc comment for what each
        // group holds. Systems join a group via `.in_set(SimSet::X)`; only
        // load-bearing *intra*-group edges remain as explicit `.after()`
        // calls below.
        fixed.configure_sets(
            (
                SimSet::ApplyIntents,
                SimSet::Spawn,
                SimSet::AudioPump,
                SimSet::ScriptUpdate,
                SimSet::Controllers,
                SimSet::Movement,
                SimSet::Transforms,
                SimSet::Collision,
                SimSet::Gui,
                SimSet::PostCollision,
                SimSet::Drain,
                SimSet::Bookkeeping,
            )
                .chain(),
        );

        // --- FIXED: signal intents + state bookkeeping, one-shot spawns ---
        // apply_signal_intents runs first, before everything else this
        // tick (in particular before on_update_<scene>, dispatched from
        // lua_plugin::update in SimSet::Bookkeeping): intents queued by the
        // render thread's GuiCallback (inside render_system) each frame must
        // be visible to this tick's scene logic (Phase 5d). The ordering is
        // now implied by SimSet::ApplyIntents preceding every later group in
        // the chain.
        fixed.add_systems(apply_signal_intents.in_set(SimSet::ApplyIntents));
        fixed.add_systems(menu_spawn_system.in_set(SimSet::Spawn));
        fixed.add_systems(gridlayout_spawn_system.in_set(SimSet::Spawn));
        fixed.add_systems(tilemap_spawn_system.in_set(SimSet::Spawn));
        fixed.add_systems(check_pending_state.in_set(SimSet::Spawn));
        fixed.add_systems(
            (
                update_bevy_audio_cmds,
                forward_audio_cmds,
                poll_audio_messages,
                update_bevy_audio_messages,
            )
                .chain()
                .in_set(SimSet::AudioPump),
        );
        // update_bevy_render_asset_cmds + forward_render_asset_cmds moved off
        // `present` onto the tail of `fixed` in Phase 7c (SimSet::Bookkeeping):
        // with `present` now decimated to `[simulation] snapshot_hz` (see
        // `logic_thread_main`), asset loads must still reach the render
        // thread every sim tick, not just on a publish tick, or a texture
        // could sit queued for several ticks before a snapshot referencing
        // it is even built. See that block's own comment for the real
        // `.after()` edge between the pair.

        // --- FIXED: input-driven forces/movement (InputState is sampled once
        // per render frame in main_loop, before the accumulator loop, and held
        // constant across every fixed substep that reads it here) ---
        fixed.add_systems(input_simple_controller.in_set(SimSet::Controllers));
        fixed.add_systems(input_acceleration_controller.in_set(SimSet::Controllers));
        fixed.add_systems(mouse_controller.in_set(SimSet::Controllers));
        fixed.add_systems(
            particle_emitter_system
                .before(movement)
                .in_set(SimSet::Movement),
        );
        fixed.add_systems(movement.in_set(SimSet::Movement));
        fixed.add_systems(ttl_system.after(movement).in_set(SimSet::Movement));
        fixed.add_systems(tween_system::<MapPosition>.in_set(SimSet::Movement));
        fixed.add_systems(tween_system::<Rotation>.in_set(SimSet::Movement));
        fixed.add_systems(tween_system::<Scale>.in_set(SimSet::Movement));
        // propagate_transforms/collision_detector's old .after(movement)/
        // .after(tween_system::<T>) edges are now implied by
        // SimSet::Movement preceding SimSet::Transforms/Collision.
        fixed.add_systems(propagate_transforms.in_set(SimSet::Transforms));
        fixed.add_systems(
            cleanup_orphaned_global_transforms
                .after(propagate_transforms)
                .in_set(SimSet::Transforms),
        );
        fixed.add_systems(camera_follow_system.after(propagate_transforms).in_set(SimSet::Transforms));
        fixed.add_systems(collision_detector.in_set(SimSet::Collision));
        fixed.add_systems(stuck_to_entity_system.after(collision_detector).in_set(SimSet::Collision));
        fixed.add_systems(phase_system.after(collision_detector).in_set(SimSet::Collision));

        // --- FIXED: GUI (tween_system::<ScreenPosition> feeds GUI layout, not
        // collision, so it's grouped with the GUI chain rather than its
        // MapPosition/Rotation/Scale siblings above; since Phase 6d all four
        // TweenValue type parameters share FIXED cadence, so
        // LuaOnTweenFinished<ScreenPosition> no longer fires at a different
        // rate than the other three).
        fixed.add_systems(tween_system::<ScreenPosition>.in_set(SimSet::Gui));
        fixed.add_systems(
            (gui_button_spawn_system, gui_label_spawn_system, gui_image_spawn_system)
                .before(gui_layout_system)
                .in_set(SimSet::Gui),
        );
        fixed.add_systems(
            gui_layout_system
                .after(tween_system::<ScreenPosition>)
                .in_set(SimSet::Gui),
        );
        fixed.add_systems(gui_hit_test_system.after(gui_layout_system).in_set(SimSet::Gui));
        fixed.add_systems(
            gui_image_state_sync_system
                .after(gui_hit_test_system)
                .in_set(SimSet::Gui),
        );
        fixed.add_systems(gui_progressbar_signal_update_system.in_set(SimSet::Gui));

        #[cfg(feature = "lua")]
        if has_lua {
            fixed.add_systems(
                update_group_counts_system
                    .before(lua_phase_system)
                    .in_set(SimSet::PostCollision),
            );
            fixed.add_systems(
                lua_phase_system
                    .run_if(state_is_playing)
                    .in_set(SimSet::PostCollision),
            );
            fixed.add_systems(
                animation_controller
                    .after(lua_phase_system)
                    .in_set(SimSet::PostCollision),
            );
            fixed.add_systems(update_lua_timers.in_set(SimSet::PostCollision));
            // Mirrors the pre-thread-split engine's explicit
            // `.after(lua_plugin::update)` constraint on both systems: a
            // queued map/asset load is drained after on_update_<scene> has
            // had a chance to queue it this same tick, not before.
            // `.before(update_bevy_render_asset_cmds)` keeps a same-tick map
            // load's RenderAssetCmd forwarded to the render thread this same
            // tick rather than lagging one tick (both now share
            // SimSet::Bookkeeping, so this ordering needs an explicit edge).
            fixed.add_systems(
                process_lua_map_commands
                    .after(crate::lua_plugin::update)
                    .before(update_bevy_render_asset_cmds)
                    .in_set(SimSet::Bookkeeping),
            );
            fixed.add_systems(
                crate::lua_plugin::process_lua_asset_commands
                    .run_if(state_is_playing)
                    .after(crate::lua_plugin::update)
                    .before(update_bevy_render_asset_cmds)
                    .in_set(SimSet::Bookkeeping),
            );
            // Phase 6d: moved from VARIABLE to FIXED alongside everything
            // else. "Fires the substep after spawn" replaces the old "fires
            // the frame after spawn" -- a strictly tighter latency (up to
            // 1/240s instead of up to ~16ms at 60fps), not a regression; see
            // docs/plans/phase6d-variable-collapse.md.
            fixed.add_systems(
                lua_setup_entity_system
                    .run_if(state_is_playing)
                    .in_set(SimSet::Drain),
            );
        } else {
            fixed.add_systems(update_group_counts_system.in_set(SimSet::PostCollision));
            fixed.add_systems(animation_controller.in_set(SimSet::PostCollision));
        }

        #[cfg(not(feature = "lua"))]
        {
            // `has_lua` only exists to keep the build_schedules signature uniform
            // across feature combinations.
            let _ = has_lua;
            fixed.add_systems(update_group_counts_system.in_set(SimSet::PostCollision));
            fixed.add_systems(animation_controller.in_set(SimSet::PostCollision));
        }

        fixed.add_systems(animation.in_set(SimSet::Drain));
        fixed.add_systems(update_timers.in_set(SimSet::Drain));
        fixed.add_systems(update_world_signals_binding_system.in_set(SimSet::Bookkeeping));
        fixed.add_systems(
            dynamictext_size_system
                .after(update_world_signals_binding_system)
                .in_set(SimSet::Bookkeeping),
        );

        // update_hook/extra_systems (on_update/add_system/configure_schedule)
        // and fixed_update_hook/extra_fixed_systems (add_fixed_system/
        // configure_fixed_schedule) all target `fixed` -- see those methods'
        // doc comments for the once-per-sim-tick cadence. Each closure
        // supplies its own `.in_set(SimSet::X)` (see `on_update`/`add_system`/
        // `with_lua`'s hook installation for the concrete sets used);
        // `configure_schedule`/`configure_fixed_schedule` closures are free
        // to pick any `SimSet` (exported for exactly this).
        if let Some(update_hook) = update_hook {
            update_hook(&mut fixed);
        }
        if let Some(fixed_update_hook) = fixed_update_hook {
            fixed_update_hook(&mut fixed);
        }

        for extra in extra_systems {
            extra(&mut fixed);
        }
        for extra in extra_fixed_systems {
            extra(&mut fixed);
        }

        // Phase 6d: moved from VARIABLE to FIXED. Accepted consequence: a
        // scene switch resolved mid-substep-batch (e.g. substep 3 of an
        // 8-substep catch-up) runs the remaining substeps against the
        // newly-spawned scene, so the snapshot sent at the end of that frame
        // may show it partially constructed -- same accepted tradeoff as the
        // Lua-direct switch path (see lua_plugin::update's doc comment and
        // with_lua()'s update_hook installation above). `.add_scene()`
        // (use_scene_manager) and Lua's `.with_lua()` are mutually exclusive
        // (validate_builder rejects both switch_scene_hook and
        // use_scene_manager), so this and the Lua-direct path never run
        // together.
        if use_scene_manager {
            fixed.add_systems(scene_update_system.run_if(state_is_playing).in_set(SimSet::Drain));
            fixed.add_systems(
                scene_switch_poll
                    .run_if(state_is_playing)
                    .after(scene_update_system)
                    .in_set(SimSet::Drain),
            );
        }

        // Forwards RenderAssetCmd to the render thread (Phase 5e; the GL
        // drain itself, process_render_asset_cmds, now lives on the render
        // schedule). Phase 7c: this pair moved off `present` onto the tail
        // of `fixed` (SimSet::Bookkeeping) so it runs every sim tick,
        // independent of the now-decimated `present`/snapshot-publish rate
        // (see `logic_thread_main`'s doc comment) -- a tick's asset loads
        // must not sit queued for several ticks waiting for the next
        // publish. `.after(update_bevy_render_asset_cmds)` is a REAL
        // same-schedule edge (both land in `SimSet::Bookkeeping`); ordering
        // relative to menu/tilemap spawning (earlier `SimSet`s, via the
        // `configure_sets((...).chain())` pipeline) is implicit, but the Lua
        // asset/map command drains now share this same `SimSet` too (moved
        // here to run `.after(lua_plugin::update)`, restoring old-engine
        // ordering) and need their own explicit
        // `.before(update_bevy_render_asset_cmds)` edge so a same-tick load
        // still forwards this tick instead of lagging one.
        fixed.add_systems(update_bevy_render_asset_cmds.in_set(SimSet::Bookkeeping));
        fixed.add_systems(
            forward_render_asset_cmds
                .after(update_bevy_render_asset_cmds)
                .in_set(SimSet::Bookkeeping),
        );

        #[allow(unused_mut)] // only reassigned under #[cfg(feature = "lua")] below
        let mut drawable_snapshot_config = build_drawable_snapshot
            .after(gui_hit_test_system)
            .after(gui_image_state_sync_system)
            .after(gui_progressbar_signal_update_system)
            .after(dynamictext_size_system)
            .after(scene_switch_poll)
            .before(render_system);
        #[cfg(feature = "lua")]
        {
            drawable_snapshot_config = drawable_snapshot_config
                .after(crate::lua_plugin::update)
                .after(process_lua_map_commands);
        }
        present.add_systems(drawable_snapshot_config);

        // Tail of the logic `present` schedule (Phase 5e; renamed from
        // `variable` in Phase 6d): ship this frame's fully-settled snapshot to
        // the render thread. apply_gameconfig_changes + render_system live on
        // the render thread's own schedule (build_render_schedule).
        // Phase 7d: InputBindings became logic-thread-only (no render-side
        // mirror to refresh), so the send_input_bindings_on_change system
        // this schedule used to carry is gone.
        present.add_systems(send_drawable_snapshot.after(build_drawable_snapshot));

        fixed
            .initialize(world)
            .map_err(|err| format!("Failed to initialize fixed schedule: {err}"))?;
        present
            .initialize(world)
            .map_err(|err| format!("Failed to initialize present schedule: {err}"))?;

        Ok((fixed, present))
    }

    /// Build the render thread's single per-frame schedule (Phase 7f-1: every
    /// per-frame step, including the ones that used to be hand-written in
    /// `render_main_loop`, is now a system in this chain). `apply_gameconfig_changes`
    /// loses its old `run_if(state_is_playing)` gate — the render world has no
    /// `GameState`; the snapshot's config is seeded with the real loaded
    /// config at startup, so early application is a no-op, not a downgrade.
    fn build_render_schedule(world: &mut World) -> Result<Schedule, String> {
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
            .map_err(|err| format!("Failed to initialize render schedule: {err}"))?;
        Ok(schedule)
    }

    /// Render (main) thread loop (Phase 7f-1: shrunk to just running the
    /// schedule -- every step that used to be hand-written here is now a
    /// system in `build_render_schedule`'s chain, see that fn's doc comment).
    ///
    /// Shutdown ordering: window close (or `RenderMsg::Quit`) -> send
    /// `LogicMsg::Shutdown` -> join the logic thread (which runs
    /// `shutdown_audio` and drops `LuaRuntime` on its own thread) -> the
    /// render world drops here (`ImguiBridge` teardown with the GL context
    /// alive) -> `RaylibHandle` drops last, closing the window.
    fn render_main_loop(world: &mut World, schedule: &mut Schedule) {
        #[cfg(feature = "tracy")]
        let _tracy = tracy_client::Client::start();

        while !world
            .non_send::<raylib::RaylibHandle>()
            .window_should_close()
            && !world.resource::<QuitRequested>().0
            && crate::protocol::shutdown::running()
        {
            {
                crate::tracy::tracy_span!("render_schedule_run");
                schedule.run(world);
            }
            world.clear_trackers();
            crate::tracy::tracy_frame_mark!();
        }

        // Shutdown: stop the logic thread first (it owns the audio bridge and
        // LuaRuntime), then let the render world / window drop after return.
        shutdown_logic(world);
    }
}

/// Refreshes `WindowSize` from the OS before input sampling (Phase 7f-1;
/// first step of the render schedule, since `sample_and_send_input` needs
/// the up-to-date size for its letterbox math).
fn refresh_window_size(rl: NonSend<raylib::RaylibHandle>, mut window_size: ResMut<WindowSize>) {
    window_size.w = rl.get_screen_width();
    window_size.h = rl.get_screen_height();
}

/// Samples the raw device state once per render frame (the only system
/// touching the raylib handle for input; Phase 7d: no bindings, no edges
/// here -- the sim thread resolves all of that) and ships it (+ the
/// previous frame's imgui capture state, one-frame lag via
/// `PendingImguiCapture`) to the logic thread over the dedicated bounded
/// input channel. A momentarily full queue (sim stalled for 8+ render
/// frames) drops the sample rather than growing an unbounded backlog; a
/// disconnected channel (logic thread gone) sets `QuitRequested`.
fn sample_and_send_input(
    rl: NonSend<raylib::RaylibHandle>,
    window_size: Res<WindowSize>,
    capture: Res<PendingImguiCapture>,
    bridge: Res<LogicBridge>,
    mut quit: ResMut<QuitRequested>,
) {
    let raw = sample_raw_device_snapshot(&rl, window_size.w, window_size.h);
    let result = bridge.tx_input.try_send(InputSample {
        raw,
        capture: capture.0,
    });
    if crate::pacing::send_channel_disconnected(&result) {
        log::error!("Logic thread disconnected; shutting down");
        quit.0 = true;
    }
}

/// Drains logic->render messages once per frame: re-queues asset commands
/// into this world's `Messages<RenderAssetCmd>` for `process_render_asset_cmds`,
/// triggers `SwitchFullScreenEvent` (Phase 7d: F10 decisions arrive here,
/// bindings having moved sim-side), and sets `QuitRequested` on
/// `RenderMsg::Quit`. EXCLUSIVE system (`fn(&mut World)`, not a
/// `SystemParam`-based one): the fullscreen toggle's `world.trigger(...)` +
/// `world.flush()` must apply synchronously so it's visible to
/// `render_system` later in this same schedule run -- a regular system's
/// `Commands::trigger` would only apply at a scheduler-inserted sync point,
/// not deterministically before the next chained system.
fn pump_render_msgs(world: &mut World) {
    let msgs: Vec<RenderMsg> = {
        let bridge = world.resource::<LogicBridge>();
        bridge.rx_render.try_iter().collect()
    };

    let mut asset_cmds: Vec<RenderAssetCmd> = Vec::new();
    let mut toggle_fullscreen = false;
    for msg in msgs {
        match msg {
            RenderMsg::Asset(cmd) => asset_cmds.push(cmd),
            RenderMsg::ToggleFullscreen => toggle_fullscreen = true,
            RenderMsg::Quit => world.resource_mut::<QuitRequested>().0 = true,
        }
    }
    if toggle_fullscreen {
        world.trigger(SwitchFullScreenEvent {});
        world.flush();
    }
    if !asset_cmds.is_empty() {
        world
            .resource_mut::<Messages<RenderAssetCmd>>()
            .write_batch(asset_cmds);
    }
}

/// Reads the newest published snapshot off the triple buffer (Phase 7c;
/// latest-wins, no interpolation), cloning it into `DrawableSnapshot` if a
/// new one arrived since the last read (still a full clone, not
/// zero-copy). Runs after `update_bevy_render_asset_cmds`/
/// `process_render_asset_cmds` rather than before them (order changed from
/// pre-7f-1); safe because neither of those systems reads or writes
/// `DrawableSnapshot` (verified: no reference to it in
/// `src/systems/render_assets.rs`) -- only `apply_gameconfig_changes`/
/// `render_system` do, and both still run after this system either way, so
/// this frame's asset loads and this frame's snapshot are visible together
/// regardless of this reordering. This system's name is deliberately kept
/// stable for 7f-3/7f-4, which will expand its body into full mirror-entity
/// reconciliation without renaming it.
///
/// Phase 7f-2: in addition to the `DrawableSnapshot` clone-assign above
/// (kept as-is -- `render_system` still reads its 8 `Vec<...Entry>` fields
/// directly, and `DrawableSnapshot` itself is not shrunk by this phase, see
/// `src/resources/render_mirrors.rs`'s module doc), this system also fans
/// out each of `DrawableSnapshot`'s 10 "global" fields into its own
/// dedicated `Render*` resource (bundled as `SnapshotMirrors`, mirroring
/// `RenderResources`/`DebugResources`'s existing param-bundling pattern).
/// `render_system` reads those instead of `snapshot.<field>` for anything
/// other than the 8 Vecs. Unconditional, same as the `DrawableSnapshot`
/// assignment -- no `is_changed()`-style gating, since nothing downstream
/// needs it and it would be a behavior change (`apply_gameconfig_changes`
/// does its own `Local`-based diff independently either way).
///
/// `camera`/`world_time` are `Copy`, and `signals`/`active_scene` are
/// already `Arc`-wrapped in `DrawableSnapshot`, so mirroring those four is a
/// cheap copy/refcount bump. The other six fields
/// (`game_config`/`app_state`/`debug`/`post_process`/`gui_themes`/
/// `camera_follow`) are moved out of `new_snapshot` via `mem::take` rather
/// than `.clone()`d a second time -- `new_snapshot` already holds one deep
/// clone of each from the `output_buffer().clone()` above, so cloning again
/// into the mirror would deep-copy the same data twice per snapshot arrival
/// (notably `AppState::clone()`, which walks every typed entry, and
/// `DebugSnapshot`'s collider/position `Vec`s, which scale with live entity
/// count while F11 debug mode is on). `mem::take` leaves each of these six
/// fields at its `Default` value inside the render world's `DrawableSnapshot`
/// afterward, which is fine: nothing in the render world reads
/// `snapshot.<one of these six>` any more (confirmed -- this phase's whole
/// point is routing those reads through the mirrors instead); only the 8
/// `Vec<...Entry>` fields and the cheap `camera`/`world_time` copies above
/// are still consulted from `*snapshot` itself.
fn receive_snapshot(
    mut consumer: ResMut<SnapshotConsumer>,
    mut snapshot: ResMut<DrawableSnapshot>,
    mut mirrors: SnapshotMirrors,
) {
    if consumer.0.update() {
        let mut new_snapshot = consumer.0.output_buffer().clone();
        mirrors.camera.0 = new_snapshot.camera;
        mirrors.game_config.0 = std::mem::take(&mut new_snapshot.game_config);
        mirrors.signals.0 = new_snapshot.signals.clone();
        mirrors.app_state.0 = std::mem::take(&mut new_snapshot.app_state);
        mirrors.debug.0 = std::mem::take(&mut new_snapshot.debug);
        mirrors.active_scene.0 = new_snapshot.active_scene.clone();
        mirrors.world_time.0 = new_snapshot.world_time;
        mirrors.post_process.0 = std::mem::take(&mut new_snapshot.post_process);
        mirrors.gui_themes.0 = std::mem::take(&mut new_snapshot.gui_themes);
        mirrors.camera_follow.0 = std::mem::take(&mut new_snapshot.camera_follow);
        *snapshot = new_snapshot;
    }
}

/// Bundled write targets for `receive_snapshot`'s field fan-out (Phase
/// 7f-2), mirroring `RenderResources`/`DebugResources`'s existing
/// param-bundling convention (`src/systems/render/mod.rs`).
#[derive(SystemParam)]
struct SnapshotMirrors<'w> {
    camera: ResMut<'w, RenderCamera>,
    game_config: ResMut<'w, RenderGameConfig>,
    signals: ResMut<'w, RenderSignalSnapshot>,
    app_state: ResMut<'w, RenderAppState>,
    debug: ResMut<'w, RenderDebugSnapshot>,
    active_scene: ResMut<'w, RenderActiveScene>,
    world_time: ResMut<'w, RenderWorldTime>,
    post_process: ResMut<'w, RenderPostProcess>,
    gui_themes: ResMut<'w, RenderGuiThemes>,
    camera_follow: ResMut<'w, RenderCameraFollow>,
}

/// Diffs the render-owned mirrors (`ScreenSize`, `DebugOverlayConfig`)
/// against their previous-frame values and sends `LogicMsg`s on change;
/// drains `SignalIntents` queued by this frame's `GuiCallback`; refreshes
/// `PendingImguiCapture` for `sample_and_send_input`'s NEXT frame read (the
/// one-frame imgui-capture lag, unchanged since Phase 6e). The `Local<Option<T>>`s
/// replace the loop-local `last_screen_size`/`last_overlay_config` bindings
/// -- both are read AND written by this same system across repeated
/// `schedule.run()` calls, which is exactly what `Local<T>` is for; wrapped
/// in `Option` so this compiles without requiring `ScreenSize`/
/// `DebugOverlayConfig` to implement `Default` (neither does today) --
/// `Option<T>: Default` holds unconditionally. Consequence: frame 1 always
/// sees `None != Some(current)` and sends once even if nothing changed
/// since startup -- harmless, the logic thread already expects an initial
/// `ScreenSize`/`OverlayConfig` update.
#[allow(clippy::too_many_arguments)]
fn send_render_mirrors(
    screen_size: Res<ScreenSize>,
    mut last_screen_size: Local<Option<ScreenSize>>,
    overlay_config: Res<DebugOverlayConfig>,
    mut last_overlay_config: Local<Option<DebugOverlayConfig>>,
    mut intents: ResMut<SignalIntents>,
    bridge: Res<LogicBridge>,
    imgui: NonSend<ImguiBridge>,
    mut pending_capture: ResMut<PendingImguiCapture>,
) {
    if *last_screen_size != Some(*screen_size) {
        *last_screen_size = Some(*screen_size);
        let _ = bridge.tx_logic.send(LogicMsg::ScreenSize {
            w: screen_size.w,
            h: screen_size.h,
        });
    }
    if last_overlay_config.as_ref() != Some(&*overlay_config) {
        *last_overlay_config = Some(overlay_config.clone());
        let _ = bridge
            .tx_logic
            .send(LogicMsg::OverlayConfig(overlay_config.clone()));
    }
    let intents_taken = std::mem::take(&mut intents.0);
    if !intents_taken.is_empty() {
        let _ = bridge.tx_logic.send(LogicMsg::SignalIntents(intents_taken));
    }
    pending_capture.0 = imgui.capture_state();
}

/// Everything the logic thread needs to build the gameplay `World` and its
/// schedules inside its own closure (Phase 5e). Must be `Send`: hooks are
/// `Box<dyn FnOnce + Send>`, scene descriptors are fn pointers, and the
/// channel endpoints are crossbeam handles.
struct LogicInit {
    config: GameConfig,
    setup_hook: Option<HookRegistrar>,
    enter_play_hook: Option<HookRegistrar>,
    switch_scene_hook: Option<HookRegistrar>,
    update_hook: Option<UpdateRegistrar>,
    fixed_update_hook: Option<UpdateRegistrar>,
    extra_systems: Vec<UpdateRegistrar>,
    extra_fixed_systems: Vec<UpdateRegistrar>,
    extra_observers: Vec<ObserverRegistrar>,
    scenes: Vec<(String, SceneDescriptor)>,
    initial_scene: Option<String>,
    #[cfg(feature = "lua")]
    lua_script: Option<PathBuf>,
    /// Initial WindowSize mirror values (refreshed per-frame via `InputSample`).
    window_w: i32,
    window_h: i32,
    tx_render: Sender<RenderMsg>,
    rx_logic: Receiver<LogicMsg>,
    /// Receiver for the dedicated bounded input channel (Phase 7d).
    rx_input: Receiver<InputSample>,
    /// The sim thread's write end of the snapshot triple buffer (Phase 7c).
    /// `Option` so [`EngineBuilder::setup_logic_world`] can `.take()` it into
    /// the logic world's [`SnapshotPublisher`] resource -- `Input<T>` isn't
    /// `Clone`, unlike the `Sender`/`Receiver` fields above.
    snapshot_publisher: Option<SnapshotPublisher>,
}

/// Logic thread entry point. Startup errors can't propagate to
/// `EngineBuilder::try_run` (the thread is already detached from it), so they
/// are logged and converted into a `RenderMsg::Quit` so the render loop exits
/// instead of showing a frozen window.
fn logic_thread(init: LogicInit) {
    let tx_render = init.tx_render.clone();
    if let Err(err) = logic_thread_main(init) {
        log::error!("Logic thread failed: {err}");
        let _ = tx_render.send(RenderMsg::Quit);
    }
}

/// Cap on a single sim tick's real dt (Phase 7b): protects against a huge dt
/// after a debugger pause or long stall producing an unrealistic physics
/// step (tunneling through colliders, teleporting) on the next tick. The POC
/// reference implementation leaves this unclamped; this engine clamps it (a
/// 2026-07-13 decision) since a stall is far more likely in practice than in
/// the POC's demo loop.
const DT_CLAMP_SECONDS: f32 = 0.25;

/// Run one `sim` schedule tick and clear `InputState`'s one-shot edge flags
/// (`just_pressed`/`just_released`) afterward, so an edge delivered by the
/// current tick's input sample is consumed exactly once. `active` (held
/// state) is left untouched and freely re-readable every tick. Extracted
/// from `logic_thread_main` so this property stays independently testable
/// without a real `Pacer`/`Instant` drive.
fn run_sim_tick(world: &mut World, sim: &mut Schedule) {
    crate::tracy::tracy_span!("sim_schedule_run");
    sim.run(world);
    world.resource_mut::<InputState>().clear_edges();
}

/// The logic thread's `Pacer`-driven loop (Phase 7b): one `sim` tick per
/// `Pacer` wakeup at `[simulation] hz`, `dt` the real elapsed time since the
/// previous tick (clamped to [`DT_CLAMP_SECONDS`], scaled by `time_scale`
/// inside [`update_world_time`]). Unlike the old
/// `recv_timeout(FIXED_DT)` + accumulator/substep-cap model, there is no
/// catch-up: a stall simply produces one larger (clamped) dt on the next
/// tick rather than several replayed substeps -- strict fixed-step
/// determinism is consciously dropped on this branch (see
/// `docs/plans/phase7b-pacing-configurable-frequencies.md`).
///
/// A backlog of pending raw input samples (whenever `sim_hz` trails the
/// render frame rate, or a sim stall) is resolved sequentially, oldest to
/// newest, against `PrevRawSnapshot` (`resolve_input_backlog`, Phase 7d --
/// replaces the old merge-then-apply-once `merge_input_snapshots` model).
/// Non-input messages just update the logic-side mirrors/stores. The sim
/// still ticks every `Pacer` wakeup even when no input arrived (holding the
/// configured rate through a render stall).
///
/// `present` (build + publish this tick's [`DrawableSnapshot`]) no longer
/// runs once per received input sample (Phase 7c) — it's decimated to
/// `[simulation] snapshot_hz`, checked independently of whether input
/// arrived this tick (see [`snapshot_publish_due`]), so the render thread
/// keeps receiving fresh snapshots during input droughts too. Forwarding
/// queued `RenderAssetCmd`s (`forward_render_asset_cmds`) moved off
/// `present` onto the tail of `sim` for the same reason: asset loads must
/// reach the render thread every tick, not just on a publish tick.
fn logic_thread_main(mut init: LogicInit) -> Result<(), String> {
    let use_scene_manager = !init.scenes.is_empty();
    #[cfg(feature = "lua")]
    let has_lua = init.lua_script.is_some();
    #[cfg(not(feature = "lua"))]
    let has_lua = false;

    let sim_hz = init.config.sim_hz;
    let snapshot_hz = init.config.snapshot_hz;

    let mut world = EngineBuilder::setup_logic_world(&mut init)?;
    EngineBuilder::register_logic_systems(&mut init, &mut world, use_scene_manager)?;
    EngineBuilder::spawn_observers(&mut world, has_lua, std::mem::take(&mut init.extra_observers));

    let (mut sim, mut present) = EngineBuilder::build_logic_schedules(
        init.update_hook.take(),
        init.fixed_update_hook.take(),
        std::mem::take(&mut init.extra_systems),
        std::mem::take(&mut init.extra_fixed_systems),
        &mut world,
        has_lua,
        use_scene_manager,
    )?;

    let rx_logic = init.rx_logic;
    let rx_input = init.rx_input;
    let mut pacer = Pacer::new(sim_hz);
    // Phase 7c: a second, non-blocking `Pacer` decimates `present`/snapshot
    // publishing independently of the sim's own pacing above -- `due()`
    // never sleeps, it just reports whether a `snapshot_hz` period has
    // elapsed since it last fired.
    let mut snapshot_pacer = Pacer::new(snapshot_hz);
    // Reused across ticks (`.clear()` below) instead of a fresh `Vec` per
    // tick -- this drains at up to `sim_hz` (default 240/s) whenever input
    // is flowing, so keeping its allocation avoids reallocating on the hot
    // path.
    let mut input_backlog: Vec<RawDeviceSnapshot> = Vec::new();

    'main: loop {
        if !crate::protocol::shutdown::running() {
            break 'main;
        }
        let dt = pacer.tick().min(DT_CLAMP_SECONDS);

        // Drain the dedicated bounded input channel: this tick's backlog of
        // raw samples, oldest to newest -- resolve_input_backlog processes
        // them sequentially against PrevRawSnapshot (see its doc comment for
        // why merge-then-diff-once can't replace this). `capture` rides with
        // each sample; only the newest matters (one render-frame stale
        // either way, same as before Phase 7d).
        input_backlog.clear();
        let mut newest_capture: Option<ImguiCaptureState> = None;
        for sample in rx_input.try_iter() {
            newest_capture = Some(sample.capture);
            input_backlog.push(sample.raw);
        }

        // Drain everything else currently queued (non-blocking -- the Pacer
        // already did the waiting). Message kinds apply to their mirrors
        // immediately.
        let mut shutdown_requested = false;
        for msg in rx_logic.try_iter() {
            match msg {
                LogicMsg::ScreenSize { w, h } => {
                    let mut screen_size = world.resource_mut::<ScreenSize>();
                    screen_size.w = w;
                    screen_size.h = h;
                }
                LogicMsg::FontLoaded { key, metrics } => {
                    world
                        .resource_mut::<FontMetricsStore>()
                        .0
                        .insert(key, metrics);
                }
                LogicMsg::TextureLoaded { key, width, height } => {
                    world
                        .resource_mut::<TextureDimsStore>()
                        .insert(key, width, height);
                }
                LogicMsg::OverlayConfig(config) => {
                    *world.resource_mut::<DebugOverlayConfig>() = config;
                }
                LogicMsg::SignalIntents(intents) => {
                    world.resource_mut::<SignalIntents>().0.extend(intents);
                }
                LogicMsg::Shutdown => shutdown_requested = true,
            }
        }

        if shutdown_requested {
            // Mirrors the pre-7b behavior of exiting immediately on
            // Shutdown (no sim/present work runs after it) — the messages
            // loop above already applied everything in this batch to its
            // resource, including any SignalIntents, so flush those into
            // WorldSignals directly instead of running a full
            // (now-pointless) simulation tick just to reach
            // apply_signal_intents inside `sim`.
            let _ = world.run_system_once(apply_signal_intents);
            break 'main;
        }

        if crate::pacing::channel_disconnected(&rx_logic) {
            break 'main;
        }

        if let Some(newest) = input_backlog.last() {
            let mut window_size = world.resource_mut::<WindowSize>();
            window_size.w = newest.window_w;
            window_size.h = newest.window_h;
        }
        if let Some(capture) = newest_capture {
            world.resource_mut::<ImguiCaptureMirror>().0 = capture;
        }
        // Resolve bindings + edges (InputState + events) BEFORE the sim tick
        // that reads it, same order as before Phase 7b. A F10 edge (post
        // imgui-capture-mask) means the caller, not the resolver, ships
        // RenderMsg::ToggleFullscreen -- see resolve_input_backlog's doc
        // comment for why it stays free of channel sends.
        resolve_input_backlog(&mut world, &input_backlog);
        if world.resource::<InputState>().fullscreen_toggle.just_pressed {
            let tx_render = world.resource::<RenderTx>().0.clone();
            let _ = tx_render.send(RenderMsg::ToggleFullscreen);
        }

        // The sim ticks every Pacer wakeup regardless of whether new input
        // arrived this tick, holding the configured `sim_hz` through a
        // render stall (mirrors the old "Timeout => run FIXED only" arm).
        update_world_time(&mut world, dt);
        run_sim_tick(&mut world, &mut sim);

        // Phase 7c: `present` runs at the configured `snapshot_hz`, not once
        // per received input sample -- independent of whether input arrived
        // this tick, so the render thread keeps receiving fresh snapshots
        // during input droughts too. `present` sees the same real dt this
        // tick measured (no separate render-frame-delta override) -- that
        // dt is captured into the snapshot for render-side use (shader time
        // uniforms, perf panel), even on ticks that don't publish.
        if snapshot_pacer.due() {
            crate::tracy::tracy_span!("present_schedule_run");
            present.run(&mut world);
        }

        world.clear_trackers();
    }

    // Logic owns the audio bridge: stop the audio thread before this world
    // (and the LuaRuntime pinned to this thread) drops.
    shutdown_audio(&mut world);
    Ok(())
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: register a system into the world, mark it [`Persistent`], and insert
/// its ID into [`SystemsStore`].
fn register_persistent_system<M>(
    world: &mut World,
    store: &mut SystemsStore,
    name: &str,
    system: impl IntoSystem<(), (), M> + 'static,
) {
    let system_id = world.register_system(system);
    world.entity_mut(system_id.entity()).insert(Persistent);
    store.insert(name, system_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let builder = EngineBuilder::new();
        assert_eq!(builder.config_path, PathBuf::from("config.ini"));
        assert!(builder.title_override.is_none());
        assert!(builder.setup_hook.is_none());
        assert!(builder.enter_play_hook.is_none());
        assert!(builder.update_hook.is_none());
        assert!(builder.switch_scene_hook.is_none());
        assert!(builder.scenes.is_empty());
        assert!(builder.initial_scene.is_none());
    }

    #[test]
    fn test_builder_config() {
        let builder = EngineBuilder::new().config("custom.ini");
        assert_eq!(builder.config_path, PathBuf::from("custom.ini"));
    }

    #[test]
    fn test_builder_title() {
        let builder = EngineBuilder::new().title("My Game");
        assert_eq!(builder.title_override, Some("My Game".to_string()));
    }

    #[test]
    fn test_raylib_log_level_from_rust_log_defaults_to_info() {
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log(""),
            TraceLogLevel::LOG_INFO
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("mycrate=debug"),
            TraceLogLevel::LOG_INFO
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("nope"),
            TraceLogLevel::LOG_INFO
        );
    }

    #[test]
    fn test_raylib_log_level_from_rust_log_maps_supported_levels() {
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("trace"),
            TraceLogLevel::LOG_TRACE
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("debug"),
            TraceLogLevel::LOG_DEBUG
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("info"),
            TraceLogLevel::LOG_INFO
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("warning"),
            TraceLogLevel::LOG_WARNING
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("error"),
            TraceLogLevel::LOG_ERROR
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("off"),
            TraceLogLevel::LOG_NONE
        );
    }

    #[test]
    fn test_raylib_log_level_from_rust_log_uses_global_directive_only() {
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("warn,mycrate=debug"),
            TraceLogLevel::LOG_WARNING
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("mycrate=debug,trace"),
            TraceLogLevel::LOG_TRACE
        );
        assert_eq!(
            EngineBuilder::raylib_log_level_from_rust_log("info/foo,mycrate=debug"),
            TraceLogLevel::LOG_INFO
        );
    }

    #[test]
    fn test_builder_title_override_applied_to_config() {
        let mut config = GameConfig::new();
        assert_eq!(config.window_title, "Aberred Engine");
        // Simulate what run() does
        let title_override = Some("My Custom Title".to_string());
        if let Some(title) = &title_override {
            config.window_title = title.clone();
        }
        assert_eq!(config.window_title, "My Custom Title");
    }

    #[test]
    fn test_builder_config_path_applied_to_gameconfig() {
        let custom_path = PathBuf::from("/tmp/my_game.ini");
        let config = GameConfig::with_path(&custom_path);
        assert_eq!(config.config_path, custom_path);
    }

    fn dummy_setup() {}
    fn dummy_enter_play() {}
    fn dummy_update() {}
    fn dummy_switch_scene() {}

    // --- Phase 7b: input edge-latch (run_sim_tick) ---
    //
    // The old accumulator/substep-cap tests (`take_fixed_substeps_*`,
    // `run_fixed_substeps_*` across 0/1/8 substeps) are gone with the
    // machinery they exercised -- Phase 7b's paced loop runs exactly one
    // `sim` tick per `Pacer` wakeup, so "fires exactly once regardless of
    // batch size" collapses to "fires exactly once", covered below.

    /// Counts how many times `InputState.action_1`/`mouse_left_button` were
    /// observed with an edge set, for asserting "fires exactly once".
    #[derive(Resource, Default)]
    struct EdgeFireCounts {
        action_1_pressed: u32,
        action_1_released: u32,
        mouse_pressed: u32,
    }

    fn count_edges_system(input: Res<InputState>, mut counts: ResMut<EdgeFireCounts>) {
        if input.action_1.just_pressed {
            counts.action_1_pressed += 1;
        }
        if input.action_1.just_released {
            counts.action_1_released += 1;
        }
        if input.mouse_left_button.just_pressed {
            counts.mouse_pressed += 1;
        }
    }

    fn build_edge_test_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(InputState::default());
        world.insert_resource(EdgeFireCounts::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(count_edges_system);
        schedule.initialize(&mut world).expect("schedule init");
        (world, schedule)
    }

    #[test]
    fn run_sim_tick_fires_edge_exactly_once_and_clears_it() {
        // Covers both action_1 (rebindable digital input) and mouse_left_button
        // (raw, non-rebindable) in one pass -- clear_edges() treats every
        // digital field identically, so exercising two of them together is
        // enough to confirm the mechanism isn't field-specific.
        let (mut world, mut schedule) = build_edge_test_world();
        {
            let mut input = world.resource_mut::<InputState>();
            input.action_1.active = true;
            input.action_1.just_pressed = true;
            input.mouse_left_button.active = true;
            input.mouse_left_button.just_pressed = true;
        }

        run_sim_tick(&mut world, &mut schedule);

        let counts = world.resource::<EdgeFireCounts>();
        assert_eq!(counts.action_1_pressed, 1, "edge must fire exactly once");
        assert_eq!(counts.mouse_pressed, 1, "mouse edge must fire exactly once");

        let input = world.resource::<InputState>();
        assert!(input.action_1.active, "active/held state must never be cleared");
        assert!(input.mouse_left_button.active, "mouse active must be untouched");
        assert!(
            !input.action_1.just_pressed,
            "edge must be consumed (cleared) after the tick sees it"
        );
        assert!(!input.mouse_left_button.just_pressed, "mouse edge consumed");
    }

    #[test]
    fn run_sim_tick_delivers_press_and_release_in_same_sample() {
        let (mut world, mut schedule) = build_edge_test_world();
        {
            // A fast tap within one render frame: both edges present at once.
            let mut input = world.resource_mut::<InputState>();
            input.action_1.just_pressed = true;
            input.action_1.just_released = true;
        }

        run_sim_tick(&mut world, &mut schedule);

        let counts = world.resource::<EdgeFireCounts>();
        assert_eq!(counts.action_1_pressed, 1, "press edge must fire exactly once");
        assert_eq!(counts.action_1_released, 1, "release edge must fire exactly once");
    }

    // --- Phase 5e: channel enum round-trip smoke test ---

    #[test]
    fn logic_and_render_msgs_round_trip_across_a_thread() {
        let (tx_logic, rx_logic) = unbounded::<LogicMsg>();
        let (tx_render, rx_render) = unbounded::<RenderMsg>();

        let echo = std::thread::spawn(move || {
            // Receive until Shutdown, echoing a Quit back.
            loop {
                match rx_logic.recv().expect("sender alive") {
                    LogicMsg::Shutdown => break,
                    LogicMsg::ScreenSize { w, h } => assert_eq!((w, h), (320, 200)),
                    _ => {}
                }
            }
            let _ = tx_render.send(RenderMsg::Quit);
        });

        tx_logic.send(LogicMsg::ScreenSize { w: 320, h: 200 }).unwrap();
        tx_logic.send(LogicMsg::Shutdown).unwrap();

        echo.join().expect("echo thread should exit cleanly");
        assert!(matches!(rx_render.recv().unwrap(), RenderMsg::Quit));
    }

    // --- Phase 7d: dedicated bounded input channel round-trip smoke test ---

    #[test]
    fn input_sample_round_trips_across_a_thread() {
        let (tx_input, rx_input) = bounded::<InputSample>(8);

        let echo = std::thread::spawn(move || {
            let sample = rx_input.recv().expect("sender alive");
            assert_eq!(sample.raw.window_w, 800);
            assert_eq!(sample.raw.window_h, 600);
            assert!(sample.raw.is_key_down(raylib::ffi::KeyboardKey::KEY_SPACE as u32));
        });

        let mut raw = RawDeviceSnapshot {
            window_w: 800,
            window_h: 600,
            ..Default::default()
        };
        raw.set_key(raylib::ffi::KeyboardKey::KEY_SPACE as u32);
        tx_input
            .try_send(InputSample {
                raw,
                capture: ImguiCaptureState::default(),
            })
            .unwrap();

        echo.join().expect("echo thread should exit cleanly");
    }

    // --- Phase 7c: triple_buffer snapshot transport ---

    #[test]
    fn snapshot_triple_buffer_round_trips_across_a_thread() {
        let (snap_in, mut snap_out) =
            triple_buffer::TripleBuffer::new(&DrawableSnapshot::default()).split();
        let mut publisher = SnapshotPublisher(snap_in);

        let writer = std::thread::spawn(move || {
            for i in 0..8 {
                let mut snapshot = DrawableSnapshot::default();
                snapshot.camera.zoom = i as f32;
                publisher.0.write(snapshot);
            }
        });
        writer.join().expect("writer thread should exit cleanly");

        // Latest-wins: after the writer is done, the next read must observe
        // the LAST published value, not an intermediate one queued up
        // somewhere -- there's no queue to begin with.
        assert!(snap_out.update(), "a publish should be pending");
        assert_eq!(snap_out.output_buffer().camera.zoom, 7.0);

        // A second read with no new publish in between reports no update.
        assert!(!snap_out.update());
    }

    #[test]
    fn test_builder_hooks_set() {
        let builder = EngineBuilder::new()
            .on_setup(dummy_setup)
            .on_enter_play(dummy_enter_play)
            .on_update(dummy_update)
            .on_switch_scene(dummy_switch_scene);
        assert!(builder.setup_hook.is_some());
        assert!(builder.enter_play_hook.is_some());
        assert!(builder.update_hook.is_some());
        assert!(builder.switch_scene_hook.is_some());
    }

    #[test]
    fn test_register_persistent_system() {
        let mut world = World::new();
        let mut store = SystemsStore::new();

        fn test_system() {}

        register_persistent_system(&mut world, &mut store, "test", test_system);

        // System should be registered in the store
        let system_id = store.get("test");
        assert!(system_id.is_some());

        // System entity should be marked Persistent
        let entity = system_id.unwrap().entity();
        assert!(world.entity(entity).contains::<Persistent>());
    }

    #[cfg(feature = "lua")]
    #[test]
    fn test_builder_with_lua() {
        let builder = EngineBuilder::new().with_lua("assets/scripts/main.lua");
        assert_eq!(
            builder.lua_script,
            Some(PathBuf::from("assets/scripts/main.lua"))
        );
        assert!(builder.setup_hook.is_some());
        assert!(builder.enter_play_hook.is_some());
        assert!(builder.update_hook.is_some());
        assert!(builder.switch_scene_hook.is_some());
    }

    #[cfg(feature = "lua")]
    #[test]
    fn test_build_logic_schedules_without_lua_runtime_omits_lua_only_systems() {
        let mut world = World::new();
        let (fixed, _present) =
            EngineBuilder::build_logic_schedules(None, None, Vec::new(), Vec::new(), &mut world, false, false)
                .expect("build_logic_schedules should succeed without Lua runtime");
        let fixed_type_ids: Vec<_> = fixed
            .systems()
            .expect("build_logic_schedules initializes the fixed schedule")
            .map(|(_, system)| system.system_type())
            .collect();
        let phase_system_type = IntoSystem::into_system(phase_system).system_type();
        let animation_controller_type = IntoSystem::into_system(animation_controller).system_type();
        let lua_phase_system_type = IntoSystem::into_system(lua_phase_system).system_type();
        let update_lua_timers_type = IntoSystem::into_system(update_lua_timers).system_type();

        let phase_index = fixed_type_ids
            .iter()
            .position(|type_id| *type_id == phase_system_type)
            .expect("phase_system should be present in the fixed schedule");
        let animation_controller_index = fixed_type_ids
            .iter()
            .position(|type_id| *type_id == animation_controller_type)
            .expect("animation_controller should be present in the fixed schedule");

        assert!(
            animation_controller_index > phase_index,
            "animation_controller should still run after phase_system"
        );
        assert!(
            !fixed_type_ids.contains(&lua_phase_system_type),
            "lua_phase_system should be absent when has_lua is false"
        );
        assert!(
            !fixed_type_ids.contains(&update_lua_timers_type),
            "update_lua_timers should be absent when has_lua is false"
        );
    }

    #[cfg(feature = "lua")]
    #[test]
    fn test_build_logic_schedules_with_lua_orders_group_counts_before_lua_phase() {
        let mut world = World::new();
        let builder = EngineBuilder::new().with_lua("assets/scripts/main.lua");
        let (fixed, present) = EngineBuilder::build_logic_schedules(
            builder.update_hook,
            builder.fixed_update_hook,
            Vec::new(),
            Vec::new(),
            &mut world,
            true,
            false,
        )
        .expect("build_logic_schedules should succeed with has_lua=true");

        let fixed_type_ids: Vec<_> = fixed
            .systems()
            .expect("build_logic_schedules initializes the fixed schedule")
            .map(|(_, system)| system.system_type())
            .collect();
        let present_type_ids: Vec<_> = present
            .systems()
            .expect("build_logic_schedules initializes the present schedule")
            .map(|(_, system)| system.system_type())
            .collect();

        let index_of = |type_ids: &[std::any::TypeId], type_id, label| -> usize {
            type_ids
                .iter()
                .position(|t| *t == type_id)
                .unwrap_or_else(|| panic!("{label} should be present"))
        };

        let update_group_counts_index = index_of(
            &fixed_type_ids,
            IntoSystem::into_system(update_group_counts_system).system_type(),
            "update_group_counts_system",
        );
        let lua_phase_index = index_of(
            &fixed_type_ids,
            IntoSystem::into_system(lua_phase_system).system_type(),
            "lua_phase_system",
        );

        assert!(
            update_group_counts_index < lua_phase_index,
            "update_group_counts_system should run before lua_phase_system (both fixed-schedule)"
        );

        // Since Phase 6d, lua_plugin::update runs on the FIXED (240Hz) schedule
        // too, alongside update_group_counts_system/lua_phase_system -- and
        // since Phase 7c, `present` contains only build_drawable_snapshot/
        // send_drawable_snapshot (forward_render_asset_cmds moved to FIXED).
        let lua_update_type = IntoSystem::into_system(crate::lua_plugin::update).system_type();
        assert!(
            fixed_type_ids.contains(&lua_update_type),
            "lua_plugin::update should be present in the fixed schedule"
        );
        assert!(
            !present_type_ids.contains(&lua_update_type),
            "lua_plugin::update should not be present in the present schedule"
        );
        assert!(
            fixed_type_ids
                .iter()
                .position(|t| *t == lua_update_type)
                .unwrap()
                > lua_phase_index,
            "lua_plugin::update should run after lua_phase_system (ordered last among \
             Lua-touching FIXED systems each substep)"
        );
    }

    #[test]
    fn test_builder_chaining() {
        let builder = EngineBuilder::new()
            .config("test.ini")
            .title("Test Game")
            .on_setup(dummy_setup)
            .on_enter_play(dummy_enter_play)
            .on_update(dummy_update)
            .on_switch_scene(dummy_switch_scene);

        assert_eq!(builder.config_path, PathBuf::from("test.ini"));
        assert_eq!(builder.title_override, Some("Test Game".to_string()));
        assert!(builder.setup_hook.is_some());
        assert!(builder.enter_play_hook.is_some());
        assert!(builder.update_hook.is_some());
        assert!(builder.switch_scene_hook.is_some());
    }

    #[test]
    fn test_default_trait() {
        let builder = EngineBuilder::default();
        assert_eq!(builder.config_path, PathBuf::from("config.ini"));
        assert!(builder.title_override.is_none());
    }

    // --- SceneManager builder tests ---

    use crate::systems::GameCtx;
    use crate::systems::scene_dispatch::SceneDescriptor;

    fn dummy_scene_enter(_ctx: &mut GameCtx) {}
    fn dummy_scene_update(_ctx: &mut GameCtx, _dt: f32, _input: &InputState) {}

    fn make_descriptor() -> SceneDescriptor {
        SceneDescriptor {
            on_enter: dummy_scene_enter,
            on_update: Some(dummy_scene_update),
            on_exit: None,
            gui_callback: None,
            world_draw_callback: None,
        }
    }

    #[test]
    fn test_add_scene_stores_scenes() {
        let builder = EngineBuilder::new()
            .add_scene("menu", make_descriptor())
            .add_scene("level1", make_descriptor());
        assert_eq!(builder.scenes.len(), 2);
        assert_eq!(builder.scenes[0].0, "menu");
        assert_eq!(builder.scenes[1].0, "level1");
    }

    #[test]
    fn test_initial_scene_stored() {
        let builder = EngineBuilder::new()
            .add_scene("menu", make_descriptor())
            .initial_scene("menu");
        assert_eq!(builder.initial_scene, Some("menu".to_string()));
    }

    #[test]
    fn test_add_scene_conflicts_with_on_switch_scene() {
        let err = EngineBuilder::new()
            .add_scene("menu", make_descriptor())
            .initial_scene("menu")
            .on_switch_scene(dummy_switch_scene)
            .try_run()
            .expect_err("conflicting scene/switch_scene hooks should fail preflight");

        assert!(err.contains("EngineBuilder conflict: .add_scene() and .on_switch_scene()"));
    }

    #[test]
    fn test_add_scene_conflicts_with_on_enter_play() {
        let err = EngineBuilder::new()
            .add_scene("menu", make_descriptor())
            .initial_scene("menu")
            .on_enter_play(dummy_enter_play)
            .try_run()
            .expect_err("conflicting scene/enter_play hooks should fail preflight");

        assert!(err.contains("EngineBuilder conflict: .add_scene() and .on_enter_play()"));
    }

    #[test]
    fn test_add_scene_requires_initial_scene() {
        let err = EngineBuilder::new()
            .add_scene("menu", make_descriptor())
            .try_run()
            .expect_err("missing initial_scene should fail preflight");

        assert!(err.contains(".add_scene() requires .initial_scene"));
    }

    #[test]
    fn test_validate_required_systems_reports_missing_entries() {
        let systems_store = SystemsStore::new();
        let err = EngineBuilder::validate_required_systems(&systems_store, true)
            .expect_err("missing required systems should fail validation");

        assert!(err.contains("setup"));
        assert!(err.contains("enter_play"));
        assert!(err.contains("quit_game"));
        assert!(err.contains("switch_scene"));
    }
}
