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

use crate::components::mapposition::MapPosition;
use crate::components::persistent::Persistent;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::events::gamestate::GameStateChangedEvent;
use crate::events::gamestate::observe_gamestate_change_event;
use crate::events::switchdebug::switch_debug_observer;
use crate::events::switchfullscreen::switch_fullscreen_observer;
use crate::resources::animationstore::AnimationStore;
use crate::resources::audio::{setup_audio, shutdown_audio};
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use crate::resources::group::TrackedGroups;
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
use crate::resources::worldtime::WorldTime;
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
use crate::systems::input::update_input_state;
use crate::systems::inputaccelerationcontroller::input_acceleration_controller;
use crate::systems::inputsimplecontroller::input_simple_controller;
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
use crate::systems::mapspawn::spawn_map_observer;
use crate::systems::rust_collision::rust_collision_observer;
use crate::systems::scene_dispatch::{
    SceneDescriptor, scene_enter_play, scene_switch_poll, scene_switch_system, scene_update_system,
};
use crate::systems::signalbinding::update_world_signals_binding_system;
use crate::systems::stuckto::stuck_to_entity_system;
use crate::systems::time::update_world_time;
use crate::systems::timer::{timer_observer, update_timers};
use crate::systems::ttl::ttl_system;
use crate::systems::tween::tween_system;
use raylib::prelude::{Camera2D, Vector2};

#[cfg(feature = "lua")]
use crate::resources::lua_runtime::LuaRuntime;
#[cfg(feature = "lua")]
use crate::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use crate::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use crate::systems::luatimer::{lua_timer_observer, update_lua_timers};

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
    title_override: Option<String>,
    setup_hook: Option<HookRegistrar>,
    enter_play_hook: Option<HookRegistrar>,
    update_hook: Option<UpdateRegistrar>,
    switch_scene_hook: Option<HookRegistrar>,
    scenes: Vec<(String, SceneDescriptor)>,
    initial_scene: Option<String>,
    extra_systems: Vec<UpdateRegistrar>,
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
            title_override: None,
            setup_hook: None,
            enter_play_hook: None,
            update_hook: None,
            switch_scene_hook: None,
            scenes: Vec::new(),
            initial_scene: None,
            extra_systems: Vec::new(),
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
        self.extra_systems.push(Box::new(move |schedule: &mut Schedule| {
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
        self.extra_observers.push(Box::new(move |world: &mut World| {
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
        // Lua update needs .after(lua_phase_system) in addition to the base constraints
        self.update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(
                lua_plugin::update
                    .run_if(state_is_playing)
                    .after(check_pending_state)
                    .after(lua_phase_system),
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
    pub fn run(mut self) {
        log::info!("Hello, world! This is the Aberred Engine!");

        let use_scene_manager = !self.scenes.is_empty();
        #[cfg(feature = "lua")]
        let has_lua = self.lua_script.is_some();
        #[cfg(not(feature = "lua"))]
        let has_lua = false;

        self.validate_builder(use_scene_manager);
        let config = self.load_config();
        let (rl, thread, render_target) = Self::setup_window(&config);

        let update_hook = self.update_hook.take();
        let extra_systems = std::mem::take(&mut self.extra_systems);
        let extra_observers = std::mem::take(&mut self.extra_observers);

        let mut world = self.setup_world(config, rl, thread, render_target);
        self.register_systems(&mut world, use_scene_manager);
        Self::spawn_observers(&mut world, has_lua, extra_observers);

        let mut update =
            Self::build_schedule(update_hook, extra_systems, &mut world, has_lua, use_scene_manager);
        Self::main_loop(&mut world, &mut update);
    }

    fn validate_builder(&self, use_scene_manager: bool) {
        if use_scene_manager {
            if self.switch_scene_hook.is_some() {
                panic!(
                    "EngineBuilder conflict: .add_scene() and .on_switch_scene() cannot be used \
                     together. Use .add_scene() for SceneManager-based games, or \
                     .on_switch_scene() for full manual control — not both."
                );
            }
            if self.enter_play_hook.is_some() {
                panic!(
                    "EngineBuilder conflict: .add_scene() and .on_enter_play() cannot be used \
                     together. SceneManager owns the enter_play hook. Use .on_setup() for \
                     asset loading instead."
                );
            }
            if self.initial_scene.is_none() {
                panic!(
                    "EngineBuilder: .add_scene() requires .initial_scene(\"name\") to specify \
                     which scene to enter first."
                );
            }
        }
    }

    fn load_config(&self) -> GameConfig {
        let mut config = GameConfig::with_path(&self.config_path);
        config.load_from_file().ok(); // ignore errors, use defaults

        if let Some(title) = &self.title_override {
            config.window_title = title.clone();
        }

        config
    }

    fn setup_window(
        config: &GameConfig,
    ) -> (raylib::RaylibHandle, raylib::RaylibThread, RenderTarget) {
        let (mut rl, thread) = raylib::init()
            .size(config.window_width as i32, config.window_height as i32)
            .resizable()
            .title(&config.window_title)
            .imgui_theme(raylib::imgui::ImGuiTheme::Dark)
            .build();
        rl.set_target_fps(config.target_fps);
        rl.set_exit_key(None);

        let render_target =
            RenderTarget::new(&mut rl, &thread, config.render_width, config.render_height)
                .expect("Failed to create render target");

        (rl, thread, render_target)
    }

    fn setup_world(
        &self,
        config: GameConfig,
        rl: raylib::RaylibHandle,
        thread: raylib::RaylibThread,
        render_target: RenderTarget,
    ) -> World {
        let render_width = config.render_width;
        let render_height = config.render_height;
        let window_width = rl.get_screen_width();
        let window_height = rl.get_screen_height();

        let mut world = World::new();
        world.insert_resource(WorldTime::default().with_time_scale(1.0));
        world.insert_resource(WorldSignals::default());
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
        world.insert_non_send_resource(render_target);

        setup_audio(&mut world);

        world.insert_resource(GameState::new());
        world.insert_resource(NextGameState::new());
        world.insert_non_send_resource(FontStore::new());
        world.insert_non_send_resource(ShaderStore::new());
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

        #[cfg(feature = "lua")]
        if let Some(ref script_path) = self.lua_script {
            let lua_runtime = LuaRuntime::new().expect("Failed to create Lua runtime");
            if let Err(e) = lua_runtime.run_script(script_path.to_str().unwrap_or("")) {
                log::error!("Failed to load Lua script: {}", e);
            }
            world.insert_non_send_resource(lua_runtime);
        }

        world.insert_non_send_resource(rl);
        world.insert_non_send_resource(thread);
        world.spawn((Observer::new(observe_gamestate_change_event), Persistent));

        world
    }

    fn register_systems(self, world: &mut World, use_scene_manager: bool) {
        let mut systems_store = SystemsStore::new();

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

        world.insert_resource(systems_store);
        world.flush();

        {
            let mut next_state = world.resource_mut::<NextGameState>();
            next_state.set(GameStates::Setup);
        }
        world.trigger(GameStateChangedEvent {});
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
        #[cfg(feature = "lua")]
        if has_lua {
            world.spawn((Observer::new(lua_timer_observer), Persistent));
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

    fn build_schedule(
        update_hook: Option<UpdateRegistrar>,
        extra_systems: Vec<UpdateRegistrar>,
        world: &mut World,
        has_lua: bool,
        use_scene_manager: bool,
    ) -> Schedule {
        let mut update = Schedule::default();
        update.add_systems(apply_gameconfig_changes.run_if(state_is_playing));
        update.add_systems(menu_spawn_system);
        update.add_systems(gridlayout_spawn_system);
        update.add_systems(update_input_state);
        update.add_systems(check_pending_state);
        update.add_systems(update_group_counts_system);
        update.add_systems(
            (
                update_bevy_audio_cmds,
                forward_audio_cmds,
                poll_audio_messages,
                update_bevy_audio_messages,
            )
                .chain(),
        );
        update.add_systems(input_simple_controller);
        update.add_systems(input_acceleration_controller);
        update.add_systems(mouse_controller);
        update.add_systems(stuck_to_entity_system.after(collision_detector));
        update.add_systems(tween_system::<MapPosition>);
        update.add_systems(tween_system::<Rotation>);
        update.add_systems(tween_system::<Scale>);
        update.add_systems(particle_emitter_system.before(movement));
        update.add_systems(movement);
        update.add_systems(ttl_system.after(movement));
        update.add_systems(
            propagate_transforms
                .after(movement)
                .after(tween_system::<MapPosition>)
                .after(tween_system::<Rotation>)
                .after(tween_system::<Scale>)
                .before(collision_detector),
        );
        update.add_systems(
            cleanup_orphaned_global_transforms
                .after(propagate_transforms)
                .before(collision_detector),
        );
        update.add_systems(
            camera_follow_system
                .after(propagate_transforms)
                .before(render_system),
        );
        update.add_systems(collision_detector.after(mouse_controller).after(movement));
        update.add_systems(phase_system.after(collision_detector));

        #[cfg(feature = "lua")]
        if has_lua {
            update.add_systems(lua_phase_system.after(collision_detector));
            update.add_systems(
                animation_controller
                    .after(lua_phase_system)
                    .after(phase_system),
            );
            update.add_systems(update_lua_timers);
        } else {
            update.add_systems(animation_controller.after(phase_system));
        }

        #[cfg(not(feature = "lua"))]
        {
            // `has_lua` only exists to keep the build_schedule signature uniform
            // across feature combinations.
            let _ = has_lua;
            update.add_systems(animation_controller.after(phase_system));
        }

        update.add_systems(animation.after(animation_controller));
        update.add_systems(update_timers);
        update.add_systems(update_world_signals_binding_system);
        update.add_systems(dynamictext_size_system.after(update_world_signals_binding_system));

        if let Some(update_hook) = update_hook {
            update_hook(&mut update);
        }

        // Apply user-registered extra systems (add_system / configure_schedule)
        for extra in extra_systems {
            extra(&mut update);
        }

        if use_scene_manager {
            update.add_systems(
                scene_update_system
                    .run_if(state_is_playing)
                    .after(check_pending_state),
            );
            update.add_systems(
                scene_switch_poll
                    .run_if(state_is_playing)
                    .after(scene_update_system),
            );
        }

        update.add_systems(render_system.after(collision_detector));

        update
            .initialize(world)
            .expect("Failed to initialize schedule");

        update
    }

    fn main_loop(world: &mut World, update: &mut Schedule) {
        while !world
            .non_send_resource::<raylib::RaylibHandle>()
            .window_should_close()
            && !world.resource::<WorldSignals>().has_flag("quit_game")
        {
            let dt = world
                .non_send_resource::<raylib::RaylibHandle>()
                .get_frame_time();

            // update_world_time is called directly (not via the schedule) because
            // WorldTime::delta must be available to all systems in the update pass.
            // Scheduling it would require ordering constraints on every delta-reading system.
            update_world_time(world, dt);

            update.run(world);

            world.clear_trackers();

            let (new_w, new_h) = {
                let rl = world.non_send_resource::<raylib::RaylibHandle>();
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
    fn test_build_schedule_without_lua_runtime_omits_lua_only_systems() {
        let mut world = World::new();
        let schedule = EngineBuilder::build_schedule(None, Vec::new(), &mut world, false, false);
        let system_type_ids: Vec<_> = schedule
            .systems()
            .expect("build_schedule initializes the schedule")
            .map(|(_, system)| system.type_id())
            .collect();
        let phase_system_type = IntoSystem::into_system(phase_system).type_id();
        let animation_controller_type = IntoSystem::into_system(animation_controller).type_id();
        let lua_phase_system_type = IntoSystem::into_system(lua_phase_system).type_id();
        let update_lua_timers_type = IntoSystem::into_system(update_lua_timers).type_id();

        let phase_index = system_type_ids
            .iter()
            .position(|type_id| *type_id == phase_system_type)
            .expect("phase_system should be present");
        let animation_controller_index = system_type_ids
            .iter()
            .position(|type_id| *type_id == animation_controller_type)
            .expect("animation_controller should be present");

        assert!(
            animation_controller_index > phase_index,
            "animation_controller should still run after phase_system"
        );
        assert!(
            !system_type_ids.contains(&lua_phase_system_type),
            "lua_phase_system should be absent when has_lua is false"
        );
        assert!(
            !system_type_ids.contains(&update_lua_timers_type),
            "update_lua_timers should be absent when has_lua is false"
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
    #[should_panic(expected = "EngineBuilder conflict: .add_scene() and .on_switch_scene()")]
    fn test_add_scene_conflicts_with_on_switch_scene() {
        EngineBuilder::new()
            .add_scene("menu", make_descriptor())
            .initial_scene("menu")
            .on_switch_scene(dummy_switch_scene)
            .run();
    }

    #[test]
    #[should_panic(expected = "EngineBuilder conflict: .add_scene() and .on_enter_play()")]
    fn test_add_scene_conflicts_with_on_enter_play() {
        EngineBuilder::new()
            .add_scene("menu", make_descriptor())
            .initial_scene("menu")
            .on_enter_play(dummy_enter_play)
            .run();
    }

    #[test]
    #[should_panic(expected = ".add_scene() requires .initial_scene")]
    fn test_add_scene_requires_initial_scene() {
        EngineBuilder::new()
            .add_scene("menu", make_descriptor())
            .run();
    }
}
