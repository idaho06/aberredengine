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
use raylib::ffi::TraceLogLevel;

use crate::components::mapposition::MapPosition;
use crate::components::screenposition::ScreenPosition;
use crate::components::persistent::Persistent;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::events::gamestate::GameStateChangedEvent;
use crate::events::gamestate::observe_gamestate_change_event;
use crate::events::switchdebug::switch_debug_observer;
use crate::events::switchfullscreen::switch_fullscreen_observer;
use crate::resources::animationstore::AnimationStore;
use crate::resources::appstate::AppState;
use crate::resources::audio::{setup_audio, shutdown_audio};
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
use crate::resources::imgui_bridge::ImguiBridge;
use crate::resources::input::InputState;
use crate::resources::input_bindings::InputBindings;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::rendertarget::RenderTarget;
use crate::resources::scenemanager::SceneManager;
use crate::resources::screensize::ScreenSize;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::windowsize::WindowSize;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::drawable_snapshot::{build_drawable_snapshot, DrawableSnapshot};
use crate::resources::worldtime::{FIXED_DT, WorldTime};
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
use crate::resources::rawinput::LatestInputSnapshot;
use crate::systems::input::{apply_input_snapshot, sample_input_snapshot};
use crate::systems::inputaccelerationcontroller::input_acceleration_controller;
use crate::systems::inputsimplecontroller::input_simple_controller;
use crate::systems::mapspawn::spawn_map_observer;
use crate::systems::menu::menu_selection_observer;
use crate::systems::menu::{menu_controller_observer, menu_despawn, menu_spawn_system};
use crate::systems::mousecontroller::mouse_controller;
use crate::systems::movement::movement;
use crate::systems::particleemitter::particle_emitter_system;
use crate::systems::phase::phase_system;
use crate::systems::propagate_transforms::{
    cleanup_orphaned_global_transforms, propagate_transforms,
};
use crate::systems::render::render_system;
use crate::systems::rust_collision::rust_collision_observer;
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
type HookRegistrar = Box<dyn FnOnce(&mut World, &mut SystemsStore)>;

/// Closure that adds a game-update system to the [`Schedule`].
/// Deferred until `run()` when the schedule is being built.
type UpdateRegistrar = Box<dyn FnOnce(&mut Schedule)>;

/// Closure that spawns an observer entity into the [`World`].
/// Deferred until `run()` when the world exists.
type ObserverRegistrar = Box<dyn FnOnce(&mut World)>;

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

    /// Register the `update` hook (runs every frame when `state_is_playing`).
    ///
    /// The system is added to the schedule with
    /// `.run_if(state_is_playing).after(check_pending_state)`.
    pub fn on_update<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(system.run_if(state_is_playing).after(check_pending_state));
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

    /// Add a per-frame system to the schedule.
    ///
    /// The system is added with `.run_if(state_is_playing).after(check_pending_state)`,
    /// matching the behaviour of [`.on_update()`](Self::on_update). Can be called
    /// multiple times to register several systems.
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
                schedule.add_systems(system.run_if(state_is_playing).after(check_pending_state));
            }));
        self
    }

    /// Add systems to the per-frame schedule with full control over ordering and
    /// run conditions.
    ///
    /// The closure receives a `&mut Schedule` and can call `schedule.add_systems(…)`
    /// with any configuration. No automatic constraints are applied — the developer
    /// is responsible for `.run_if()`, `.after()`, `.before()` etc.
    ///
    /// ```rust,ignore
    /// .configure_schedule(|schedule| {
    ///     schedule.add_systems(
    ///         my_system
    ///             .run_if(state_is_playing)
    ///             .after(movement)
    ///             .before(render_system),
    ///     );
    /// })
    /// ```
    pub fn configure_schedule(mut self, f: impl FnOnce(&mut Schedule) + 'static) -> Self {
        self.extra_systems.push(Box::new(f));
        self
    }


    /// Add a system to the fixed (240 Hz) simulation schedule.
    ///
    /// Use this for custom Rust-side physics/gameplay logic that needs a
    /// deterministic, render-rate-independent tick -- e.g. a game-specific
    /// movement modifier that must run alongside [`movement`](crate::systems::movement::movement)
    /// and before [`collision_detector`](crate::systems::collision_detector::collision_detector).
    ///
    /// The system is added with `.run_if(state_is_playing)`, matching
    /// [`.add_system()`](Self::add_system). For custom ordering relative to
    /// other fixed-schedule systems, use
    /// [`configure_fixed_schedule`](Self::configure_fixed_schedule) instead.
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

    /// Add systems to the fixed (240 Hz) simulation schedule with full control
    /// over ordering and run conditions.
    ///
    /// Mirrors [`configure_schedule`](Self::configure_schedule), but targets
    /// the fixed schedule instead of the once-per-render-frame one.
    pub fn configure_fixed_schedule(mut self, f: impl FnOnce(&mut Schedule) + 'static) -> Self {
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
        // lua_plugin::update runs on the VARIABLE (once-per-render-frame) schedule.
        // lua_phase_system/camera_follow_system now live on the FIXED schedule, so
        // ordering against them is no longer expressible (or needed) here -- every
        // fixed substep for this frame has already completed by the time the
        // variable schedule runs, so Lua always reads this frame's settled phase
        // and camera state regardless.
        self.update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(
                lua_plugin::update
                    .run_if(state_is_playing)
                    .after(check_pending_state)
                    .before(render_system),
            );
        }));
        // lua_plugin::fixed_update runs on the FIXED (240Hz) schedule, once per
        // substep, before movement/collision -- see lua_plugin::fixed_update's
        // doc comment and docs/render-simulation-separation-brainstorm.md for
        // why substep-synchronized Lua logic (e.g. clearing collision-derived
        // flags) needs this instead of on_update_<scene>.
        self.fixed_update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(
                lua_plugin::fixed_update
                    .run_if(state_is_playing)
                    .before(movement),
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
    pub fn try_run(mut self) -> Result<(), String> {
        log::info!("Hello, world! This is the Aberred Engine!");

        let use_scene_manager = !self.scenes.is_empty();
        #[cfg(feature = "lua")]
        let has_lua = self.lua_script.is_some();
        #[cfg(not(feature = "lua"))]
        let has_lua = false;

        self.validate_builder(use_scene_manager)?;
        let config = self.load_config()?;
        let (rl, thread, render_target) = Self::setup_window(&config)?;

        let update_hook = self.update_hook.take();
        let fixed_update_hook = self.fixed_update_hook.take();
        let extra_systems = std::mem::take(&mut self.extra_systems);
        let extra_fixed_systems = std::mem::take(&mut self.extra_fixed_systems);
        let extra_observers = std::mem::take(&mut self.extra_observers);

        let mut world = self.setup_world(config, rl, thread, render_target)?;
        self.register_systems(&mut world, use_scene_manager)?;
        Self::spawn_observers(&mut world, has_lua, extra_observers);

        let (mut fixed, mut variable) = Self::build_schedules(
            update_hook,
            fixed_update_hook,
            extra_systems,
            extra_fixed_systems,
            &mut world,
            has_lua,
            use_scene_manager,
        )?;
        Self::main_loop(&mut world, &mut fixed, &mut variable);

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

    fn setup_world(
        &self,
        config: GameConfig,
        rl: raylib::RaylibHandle,
        thread: raylib::RaylibThread,
        render_target: RenderTarget,
    ) -> Result<World, String> {
        let render_width = config.render_width;
        let render_height = config.render_height;
        let window_width = rl.get_screen_width();
        let window_height = rl.get_screen_height();

        let mut world = World::new();
        world.insert_resource(WorldTime::default().with_time_scale(1.0));
        world.insert_resource(WorldSignals::default());
        world.insert_resource(AppState::default());
        world.insert_resource(TrackedGroups::default());
        world.insert_resource(ScreenSize {
            w: render_width as i32,
            h: render_height as i32,
        });
        world.insert_resource(WindowSize {
            w: window_width,
            h: window_height,
        });
        world.insert_resource(config);
        world.insert_resource(InputState::default());
        world.insert_resource(InputBindings::default());
        world.insert_resource(LatestInputSnapshot::default());
        world.insert_non_send(render_target);

        setup_audio(&mut world);

        world.insert_resource(GameState::new());
        world.insert_resource(NextGameState::new());
        world.insert_non_send(FontStore::new());
        world.insert_resource(FontMetricsStore::default());
        world.insert_resource(FontMetricsWarnCache::default());
        let imgui_bridge = ImguiBridge::new_dark()
            .map_err(|err| format!("Failed to initialize imgui bridge: {err}"))?;
        world.insert_non_send(imgui_bridge);
        world.insert_non_send(ShaderStore::new());
        world.insert_resource(TextureStore::new());
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

        #[cfg(feature = "lua")]
        if let Some(ref script_path) = self.lua_script {
            let lua_runtime =
                LuaRuntime::new().map_err(|err| format!("Failed to create Lua runtime: {err}"))?;
            if let Err(e) = lua_runtime.run_script(script_path.to_str().unwrap_or("")) {
                log::error!("Failed to load Lua script: {}", e);
            }
            world.insert_non_send(lua_runtime);
        }

        world.insert_non_send(rl);
        world.insert_non_send(thread);
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

    fn register_systems(self, world: &mut World, use_scene_manager: bool) -> Result<(), String> {
        let mut systems_store = SystemsStore::new();
        #[cfg(feature = "lua")]
        let requires_switch_scene =
            use_scene_manager || self.switch_scene_hook.is_some() || self.lua_script.is_some();
        #[cfg(not(feature = "lua"))]
        let requires_switch_scene = use_scene_manager || self.switch_scene_hook.is_some();

        if let Some(hook) = self.setup_hook {
            hook(world, &mut systems_store);
        }
        if let Some(hook) = self.enter_play_hook {
            hook(world, &mut systems_store);
        }
        if let Some(hook) = self.switch_scene_hook {
            hook(world, &mut systems_store);
        }

        if use_scene_manager {
            let mut scene_manager = SceneManager::new();
            scene_manager.initial_scene = self.initial_scene;
            for (name, descriptor) in self.scenes {
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
        world.spawn((Observer::new(switch_fullscreen_observer), Persistent));
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

    /// Build the two schedules that make up a frame: `fixed` (run 0-8 times
    /// per render frame at a constant `FIXED_DT` -- deterministic core
    /// simulation: movement, collision, phases, animation) and `variable`
    /// (run exactly once per render frame -- scripted gameplay logic, GUI,
    /// scene lifecycle, and rendering). See
    /// `docs/render-simulation-separation-brainstorm.md` and
    /// `.claude/context/system-order.md` for the rationale behind the split
    /// and the full list of which system lives where.
    ///
    /// `bevy_ecs` cannot express `.after()`/`.before()` across two separate
    /// `Schedule`s, so edges that used to cross what is now the fixed/variable
    /// boundary are dropped here (the accumulator loop in `main_loop` already
    /// guarantees every fixed substep for a frame completes before `variable`
    /// runs, which is the ordering those edges used to express).
    fn build_schedules(
        update_hook: Option<UpdateRegistrar>,
        fixed_update_hook: Option<UpdateRegistrar>,
        extra_systems: Vec<UpdateRegistrar>,
        extra_fixed_systems: Vec<UpdateRegistrar>,
        world: &mut World,
        has_lua: bool,
        use_scene_manager: bool,
    ) -> Result<(Schedule, Schedule), String> {
        let mut fixed = Schedule::default();
        let mut variable = Schedule::default();

        // --- VARIABLE: input/state bookkeeping, one-shot spawns ---
        // (apply_gameconfig_changes is registered further down, after
        // build_drawable_snapshot -- it reads config from the snapshot.)
        variable.add_systems(menu_spawn_system);
        variable.add_systems(gridlayout_spawn_system);
        variable.add_systems(tilemap_spawn_system);
        variable.add_systems(check_pending_state);
        variable.add_systems(
            (
                update_bevy_audio_cmds,
                forward_audio_cmds,
                poll_audio_messages,
                update_bevy_audio_messages,
            )
                .chain(),
        );

        // --- FIXED: input-driven forces/movement (InputState is sampled once
        // per render frame in main_loop, before the accumulator loop, and held
        // constant across every fixed substep that reads it here) ---
        fixed.add_systems(input_simple_controller);
        fixed.add_systems(input_acceleration_controller);
        fixed.add_systems(mouse_controller);
        fixed.add_systems(particle_emitter_system.before(movement));
        fixed.add_systems(movement);
        fixed.add_systems(ttl_system.after(movement));
        fixed.add_systems(tween_system::<MapPosition>);
        fixed.add_systems(tween_system::<Rotation>);
        fixed.add_systems(tween_system::<Scale>);
        fixed.add_systems(
            propagate_transforms
                .after(movement)
                .after(tween_system::<MapPosition>)
                .after(tween_system::<Rotation>)
                .after(tween_system::<Scale>)
                .before(collision_detector),
        );
        fixed.add_systems(
            cleanup_orphaned_global_transforms
                .after(propagate_transforms)
                .before(collision_detector),
        );
        fixed.add_systems(camera_follow_system.after(propagate_transforms));
        fixed.add_systems(collision_detector.after(mouse_controller).after(movement));
        fixed.add_systems(stuck_to_entity_system.after(collision_detector));
        fixed.add_systems(phase_system.after(collision_detector));

        // --- VARIABLE: GUI (tween_system::<ScreenPosition> feeds GUI layout,
        // not collision, so it moves here unlike its MapPosition/Rotation/Scale
        // siblings above; its LuaOnTweenFinished<ScreenPosition> callback now
        // fires once per render frame while the other three fire on FIXED) ---
        variable.add_systems(tween_system::<ScreenPosition>);
        variable.add_systems(
            (gui_button_spawn_system, gui_label_spawn_system, gui_image_spawn_system)
                .before(gui_layout_system),
        );
        variable.add_systems(
            gui_layout_system
                .after(tween_system::<ScreenPosition>)
                .before(render_system),
        );
        variable.add_systems(gui_hit_test_system.after(gui_layout_system).before(render_system));
        variable.add_systems(
            gui_image_state_sync_system
                .after(gui_hit_test_system)
                .before(render_system),
        );
        variable.add_systems(gui_progressbar_signal_update_system.before(render_system));

        #[cfg(feature = "lua")]
        if has_lua {
            fixed.add_systems(update_group_counts_system.before(lua_phase_system));
            fixed.add_systems(lua_phase_system.run_if(state_is_playing).after(collision_detector));
            fixed.add_systems(
                animation_controller
                    .after(lua_phase_system)
                    .after(phase_system),
            );
            fixed.add_systems(update_lua_timers);
            variable.add_systems(
                process_lua_map_commands
                    .after(crate::lua_plugin::update)
                    .before(render_system),
            );
            variable.add_systems(
                crate::lua_plugin::process_lua_asset_commands
                    .run_if(state_is_playing)
                    .after(crate::lua_plugin::update),
            );
            variable.add_systems(
                lua_setup_entity_system
                    .run_if(state_is_playing)
                    .after(check_pending_state),
            );
        } else {
            fixed.add_systems(update_group_counts_system);
            fixed.add_systems(animation_controller.after(phase_system));
        }

        #[cfg(not(feature = "lua"))]
        {
            // `has_lua` only exists to keep the build_schedules signature uniform
            // across feature combinations.
            let _ = has_lua;
            fixed.add_systems(update_group_counts_system);
            fixed.add_systems(animation_controller.after(phase_system));
        }

        fixed.add_systems(animation.after(animation_controller));
        fixed.add_systems(update_timers);
        variable.add_systems(update_world_signals_binding_system);
        variable.add_systems(dynamictext_size_system.after(update_world_signals_binding_system));

        if let Some(update_hook) = update_hook {
            update_hook(&mut variable);
        }
        if let Some(fixed_update_hook) = fixed_update_hook {
            fixed_update_hook(&mut fixed);
        }

        // Apply user-registered extra systems (add_system / configure_schedule
        // target VARIABLE; add_fixed_system / configure_fixed_schedule target FIXED)
        for extra in extra_systems {
            extra(&mut variable);
        }
        for extra in extra_fixed_systems {
            extra(&mut fixed);
        }

        if use_scene_manager {
            variable.add_systems(
                scene_update_system
                    .run_if(state_is_playing)
                    .after(check_pending_state),
            );
            variable.add_systems(
                scene_switch_poll
                    .run_if(state_is_playing)
                    .after(scene_update_system),
            );
        }

        // Populates DrawableSnapshot from this frame's fully-settled VARIABLE
        // state -- GUI hit-test/layout, Lua on_update entity repositioning,
        // camera/config commands, and scene switches are all VARIABLE-schedule,
        // so this must run after them (not on FIXED, where it ran in an
        // earlier iteration of Phase 3 -- see the "capture cadence" note in
        // docs/render-simulation-separation-brainstorm.md for why that left
        // GUI/camera/config a full frame stale). `.after(...)` on a system not
        // scheduled in this configuration (e.g. lua-only edges when has_lua is
        // false) is a vacuous constraint, not an error.
        //
        // This is an enumerated stand-in for "after everything in VARIABLE
        // that can still change this frame's drawable state" -- bevy_ecs 0.19
        // has no `.after_all()` primitive and this codebase doesn't yet use
        // `SystemSet`/`configure_sets` to express that generically. If a
        // future VARIABLE-schedule system starts writing snapshot-relevant
        // state (a new GUI widget's state-sync system, a new Lua command
        // queue mutating rendered state), it must also be added to this list
        // -- nothing enforces that automatically.
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
        variable.add_systems(drawable_snapshot_config);

        // Applies window/render-target settings from the snapshot's
        // GameConfig copy (Phase 4) -- must run after the snapshot is built
        // (fresh config, including this frame's Lua config commands) and
        // before render_system (so a RenderTarget resize is visible the same
        // frame). Known trade-off: a render-size change updates ScreenSize
        // here, *after* this frame's gui_hit_test_system already ran -- GUI
        // hit-testing uses the old resolution for one frame on the rare
        // resize frame. Inherent to the Phase 5 thread split anyway.
        variable.add_systems(
            apply_gameconfig_changes
                .run_if(state_is_playing)
                .after(build_drawable_snapshot)
                .before(render_system),
        );

        variable.add_systems(render_system);

        fixed
            .initialize(world)
            .map_err(|err| format!("Failed to initialize fixed schedule: {err}"))?;
        variable
            .initialize(world)
            .map_err(|err| format!("Failed to initialize variable schedule: {err}"))?;

        Ok((fixed, variable))
    }

    /// Runs the fixed-step accumulator loop described in
    /// `docs/render-simulation-separation-brainstorm.md`: `fixed` (240 Hz,
    /// `FIXED_DT`) runs 0-8 times per render frame to drain a real-time
    /// accumulator, then `variable` runs exactly once. No interpolation in
    /// this pass -- `variable`/`render_system` simply displays whatever
    /// simulation state the fixed loop last produced, so a fast render frame
    /// (accumulator < FIXED_DT) can redraw the same state twice, and a slow
    /// one catches up via multiple fixed substeps.
    fn main_loop(world: &mut World, fixed: &mut Schedule, variable: &mut Schedule) {
        /// Spiral-of-death cap: at most this many fixed substeps run per
        /// render frame. If a stall needs more, the accumulator keeps the
        /// remainder rather than trying to catch up all at once.
        const MAX_FIXED_STEPS_PER_FRAME: u32 = 8;

        #[cfg(feature = "tracy")]
        let _tracy = tracy_client::Client::start();

        // Built once and reused every frame (mirrors `fixed`/`variable`) rather
        // than `world.run_system_once(apply_input_snapshot)`, which would build
        // and initialize a fresh temporary system on every single call.
        let mut input_schedule = Schedule::default();
        input_schedule.add_systems(apply_input_snapshot);
        input_schedule
            .initialize(world)
            .expect("input_schedule should initialize: apply_input_snapshot has no unusual system requirements");

        let mut accumulator: f32 = 0.0;

        while !world
            .non_send::<raylib::RaylibHandle>()
            .window_should_close()
        {
            let dt = world
                .non_send::<raylib::RaylibHandle>()
                .get_frame_time();

            // update_world_time is called directly (not via a schedule) because
            // WorldTime::delta must be available to all systems without ordering
            // constraints on every delta-reading system. Called once per render
            // frame: sets elapsed/frame_count and the frame-rate delta; the delta
            // is temporarily overridden below for each fixed substep.
            update_world_time(world, dt);
            let frame_delta = world.resource::<WorldTime>().delta;

            // Sample raw input once per render frame, held constant across every
            // fixed substep below -- not part of either schedule, since running it
            // inside `fixed` would resample/skip input unpredictably 0-8x per
            // frame, and running it inside `variable` would leave `fixed`'s input
            // controllers reading a stale InputState from the previous frame.
            //
            // Split along the future thread boundary (Phase 5a):
            // `sample_input_snapshot` is the render-thread half (raylib polls,
            // no ECS writes), called inline here; `apply_input_snapshot` is the
            // logic-thread half (InputState + camera-dependent mouse-world +
            // event triggers), run via the schedule. Phase 5e replaces the
            // direct resource write with a channel send/receive.
            {
                let snapshot = {
                    let rl = world.non_send::<raylib::RaylibHandle>();
                    let bindings = world.resource::<InputBindings>();
                    let window_size = world.resource::<WindowSize>();
                    let screen_size = world.resource::<ScreenSize>();
                    sample_input_snapshot(rl, bindings, window_size, screen_size)
                };
                world.resource_mut::<LatestInputSnapshot>().0 = snapshot;
            }
            input_schedule.run(world);

            accumulator += frame_delta;

            // time_scale doesn't change during the substep loop below (no FIXED
            // system mutates it), so the scaled fixed delta is computed and
            // written once per render frame rather than re-read/re-written on
            // every substep.
            let time_scale = world.resource::<WorldTime>().time_scale;
            world.resource_mut::<WorldTime>().delta = FIXED_DT * time_scale;

            let mut steps_run = 0;
            while accumulator >= FIXED_DT && steps_run < MAX_FIXED_STEPS_PER_FRAME {
                crate::tracy::tracy_span!("fixed_schedule_run");
                fixed.run(world);
                accumulator -= FIXED_DT;
                steps_run += 1;
            }
            // Spiral-of-death cap: if MAX_FIXED_STEPS_PER_FRAME wasn't enough to
            // drain the accumulator, stop -- the remainder is kept (not reset to
            // zero) so a temporary stall doesn't get silently discarded, it just
            // takes a couple of frames to fully catch up.

            // Restore the real frame delta for the variable schedule (render,
            // on_update, GUI) before it runs.
            world.resource_mut::<WorldTime>().delta = frame_delta;

            {
                crate::tracy::tracy_span!("variable_schedule_run");
                variable.run(world);
            }

            world.clear_trackers();
            crate::tracy::tracy_frame_mark!();

            let (new_w, new_h) = {
                let rl = world.non_send::<raylib::RaylibHandle>();
                (rl.get_screen_width(), rl.get_screen_height())
            };
            {
                let mut window_size = world.resource_mut::<WindowSize>();
                window_size.w = new_w;
                window_size.h = new_h;
            }
        }
        shutdown_audio(world);
    }
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
    fn test_build_schedules_without_lua_runtime_omits_lua_only_systems() {
        let mut world = World::new();
        let (fixed, _variable) =
            EngineBuilder::build_schedules(None, None, Vec::new(), Vec::new(), &mut world, false, false)
                .expect("build_schedules should succeed without Lua runtime");
        let fixed_type_ids: Vec<_> = fixed
            .systems()
            .expect("build_schedules initializes the fixed schedule")
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
    fn test_build_schedules_with_lua_orders_group_counts_before_lua_phase() {
        let mut world = World::new();
        let builder = EngineBuilder::new().with_lua("assets/scripts/main.lua");
        let (fixed, variable) = EngineBuilder::build_schedules(
            builder.update_hook,
            builder.fixed_update_hook,
            Vec::new(),
            Vec::new(),
            &mut world,
            true,
            false,
        )
        .expect("build_schedules should succeed with has_lua=true");

        let fixed_type_ids: Vec<_> = fixed
            .systems()
            .expect("build_schedules initializes the fixed schedule")
            .map(|(_, system)| system.system_type())
            .collect();
        let variable_type_ids: Vec<_> = variable
            .systems()
            .expect("build_schedules initializes the variable schedule")
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

        // lua_plugin::update runs once per render frame (variable schedule), while
        // update_group_counts_system/lua_phase_system run on the fixed 240Hz
        // schedule -- these are no longer comparable by position in a shared
        // system list, so just assert each lives in the schedule it should.
        let lua_update_type = IntoSystem::into_system(crate::lua_plugin::update).system_type();
        assert!(
            variable_type_ids.contains(&lua_update_type),
            "lua_plugin::update should be present in the variable schedule"
        );
        assert!(
            !fixed_type_ids.contains(&lua_update_type),
            "lua_plugin::update should not be present in the fixed schedule"
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
