use std::path::PathBuf;

use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoObserverSystem;

use super::registrar::{HookRegistrar, ObserverRegistrar, UpdateRegistrar, hook_registrar};
use super::schedule::SimSet;
use crate::components::persistent::Persistent;
use crate::resources::systemsstore as hook_keys;
use crate::systems::gamestate::state_is_playing;
use crate::systems::scene_dispatch::SceneDescriptor;

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
    pub(super) config_path: PathBuf,
    pub(super) config_str: Option<&'static str>,
    pub(super) title_override: Option<String>,
    pub(super) setup_hook: Option<HookRegistrar>,
    pub(super) enter_play_hook: Option<HookRegistrar>,
    pub(super) update_hook: Option<UpdateRegistrar>,
    pub(super) switch_scene_hook: Option<HookRegistrar>,
    pub(super) scenes: Vec<(String, SceneDescriptor)>,
    pub(super) initial_scene: Option<String>,
    pub(super) extra_systems: Vec<UpdateRegistrar>,
    pub(super) extra_observers: Vec<ObserverRegistrar>,
    /// Name of the first `on_*` hook method explicitly called by the
    /// developer, tracked so `validate_builder` can detect a conflict with
    /// `.with_lua()` (which installs its own four hooks unconditionally)
    /// regardless of call order.
    pub(super) first_user_hook: Option<&'static str>,
    #[cfg(feature = "lua")]
    pub(super) lua_script: Option<PathBuf>,
    /// `Some(seed)` when `.deterministic(seed)` was called -- see that
    /// method's doc comment. Mutually exclusive with `.with_lua()`.
    pub(super) deterministic_seed: Option<u64>,
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
            switch_scene_hook: None,
            scenes: Vec::new(),
            initial_scene: None,
            extra_systems: Vec::new(),
            extra_observers: Vec::new(),
            first_user_hook: None,
            #[cfg(feature = "lua")]
            lua_script: None,
            deterministic_seed: None,
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
    /// The system is registered into [`SystemsStore`](crate::resources::systemsstore::SystemsStore)
    /// under the key `"setup"`.
    pub fn on_setup<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.setup_hook = Some(hook_registrar(hook_keys::SETUP, system));
        self.first_user_hook.get_or_insert("on_setup");
        self
    }

    /// Register the `enter_play` hook (called when transitioning to `Playing`).
    ///
    /// The system is registered into [`SystemsStore`](crate::resources::systemsstore::SystemsStore)
    /// under the key `"enter_play"`.
    pub fn on_enter_play<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.enter_play_hook = Some(hook_registrar(hook_keys::ENTER_PLAY, system));
        self.first_user_hook.get_or_insert("on_enter_play");
        self
    }

    /// Register the `update` hook.
    ///
    /// Runs once per sim tick (`[simulation] hz` in `config.ini`),
    /// in [`SimSet::ScriptUpdate`]. Treat it as idempotent/edge-triggered the
    /// same way Lua's `on_update_<scene>` must be
    /// (`.claude/context/system-order.md`): gate one-shot effects on an edge,
    /// not on "runs once per visible frame" -- the sim ticks faster than the
    /// render thread. The system is added with `.run_if(state_is_playing)`.
    pub fn on_update<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(system.run_if(state_is_playing).in_set(SimSet::ScriptUpdate));
        }));
        self.first_user_hook.get_or_insert("on_update");
        self
    }

    /// Register the `switch_scene` hook (called when a scene transition is requested).
    ///
    /// The system is registered into [`SystemsStore`](crate::resources::systemsstore::SystemsStore)
    /// under the key `"switch_scene"`.
    pub fn on_switch_scene<M>(
        mut self,
        system: impl IntoSystem<(), (), M> + Send + 'static,
    ) -> Self {
        self.switch_scene_hook = Some(hook_registrar(hook_keys::SWITCH_SCENE, system));
        self.first_user_hook.get_or_insert("on_switch_scene");
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
    /// This and [`add_system`](Self::add_system) are the only two extension
    /// points onto the sim schedule; use this one when you need ordering
    /// relative to the engine's own pipeline via [`SimSet`], e.g. a
    /// game-specific movement modifier that must run alongside
    /// [`movement`](crate::systems::movement::movement) and before
    /// [`collision_detector`](crate::systems::collision_detector::collision_detector).
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

    /// Register a named scene for [`SceneManager`](crate::resources::scenemanager::SceneManager)-based games.
    ///
    /// Scenes are stored and later inserted into a
    /// [`SceneManager`](crate::resources::scenemanager::SceneManager) resource
    /// at `.run()` time. Use with [`.initial_scene()`](Self::initial_scene) to
    /// specify which scene starts first.
    ///
    /// # Errors (at `.run()`/`.try_run()`)
    ///
    /// - If `.add_scene()` is combined with `.on_switch_scene()`, `.on_enter_play()`,
    ///   or `.with_lua()`
    /// - If `.add_scene()` is used without `.initial_scene()`, or `.initial_scene()`
    ///   names a scene that was never registered
    ///
    /// `.try_run()` returns these as an [`EngineError`](crate::error::EngineError);
    /// `.run()` prints the error to stderr and exits with a nonzero status. Neither panics.
    pub fn add_scene(mut self, name: impl Into<String>, descriptor: SceneDescriptor) -> Self {
        self.scenes.push((name.into(), descriptor));
        self
    }

    /// Set the initial scene for [`SceneManager`](crate::resources::scenemanager::SceneManager)-based games.
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

        self.setup_hook = Some(hook_registrar(hook_keys::SETUP, lua_plugin::setup));
        self.enter_play_hook = Some(hook_registrar(
            hook_keys::ENTER_PLAY,
            lua_plugin::enter_play,
        ));
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
        self.switch_scene_hook = Some(hook_registrar(
            hook_keys::SWITCH_SCENE,
            lua_plugin::switch_scene,
        ));
        self
    }

    /// Opt into deterministic mode: `SimRng` (`src/resources/sim_rng.rs`) is
    /// seeded from `seed` instead of entropy. Fixed-dt ticking and the
    /// single-threaded schedule executor are already unconditional for every
    /// game (determinism phases 01/02), so this is the last engine-side
    /// switch a deterministic game needs to flip.
    ///
    /// Mutually exclusive with [`.with_lua()`](Self::with_lua) -- Lua is
    /// outside the deterministic envelope
    /// (`docs/plans/determinism-00-overview.md`); combining both is rejected
    /// at `.run()`/`.try_run()` time as
    /// [`EngineError::LuaConflictsWithDeterministic`](crate::error::EngineError::LuaConflictsWithDeterministic).
    pub fn deterministic(mut self, seed: u64) -> Self {
        self.deterministic_seed = Some(seed);
        self
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
