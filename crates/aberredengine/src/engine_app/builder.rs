use std::path::PathBuf;

use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoObserverSystem;

use super::registrar::{
    HookRegistrar, ObserverRegistrar, UpdateRegistrar, hook_registrar, observer_registrar,
    playing_system, playing_system_if, scene_observer_registrar, scene_system,
};
#[cfg(doc)] // doc links
use super::schedule::SimSet;
use aberred_core::events::scene::{SceneEntered, SceneExited};
use aberred_core::resources::systemsstore as hook_keys;
use aberred_core::systems::scene_dispatch::WorldDrawCallback;
use aberred_render::resources::scene_table::{GuiCallback, RenderSceneTable, SceneRender};
use rustc_hash::FxHashMap;

/// Builder for bootstrapping the engine.
///
/// Handles world setup, window init, resources, system schedule, and main loop.
/// The developer supplies an optional `setup` hook, scenes (or the implicit
/// `"main"` scene) with their observers and systems, and any number of
/// per-frame systems ([`add_system`](Self::add_system),
/// [`configure_schedule`](Self::configure_schedule)) and persistent observers
/// ([`add_observer`](Self::add_observer)) for custom event handling.
#[must_use = "EngineBuilder does nothing until .run() is called"]
pub struct EngineBuilder {
    pub(super) config_path: PathBuf,
    pub(super) config_str: Option<&'static str>,
    pub(super) title_override: Option<String>,
    /// The user's `.on_setup()` hook. A Lua game's setup comes from
    /// `register_logic_systems`, so this never holds Lua's.
    pub(super) setup_hook: Option<HookRegistrar>,
    pub(super) scenes: Vec<String>,
    /// `.add_scene_gui()`/`.add_scene_world_draw()` registrations: the render
    /// thread's per-scene callback table.
    pub(super) scene_render: FxHashMap<String, SceneRender>,
    pub(super) initial_scene: Option<String>,
    /// `.loading_scene()`: the scene active during `Setup`.
    pub(super) loading_scene: Option<&'static str>,
    /// Group names from `.track_group()`, tracked across scene switches.
    pub(super) tracked_groups: Vec<String>,
    pub(super) extra_systems: Vec<UpdateRegistrar>,
    pub(super) extra_observers: Vec<ObserverRegistrar>,
    /// `(method, scene)` for every scene-scoped call (`add_scene_system`,
    /// `on_scene_enter`, `on_scene_exit`), checked against `scenes` by
    /// `validate_builder`.
    pub(super) scene_refs: Vec<(&'static str, &'static str)>,
    #[cfg(feature = "lua")]
    pub(super) lua_script: Option<PathBuf>,
    /// `Some(seed)` when `.deterministic(seed)` was called -- see that
    /// method's doc comment. Mutually exclusive with `.with_lua()`.
    pub(super) deterministic_seed: Option<u64>,
    /// `Some(path)` when `.record_replay(path, ..)` was called.
    pub(super) record_replay_path: Option<PathBuf>,
    /// User-supplied diagnostics string recorded into the replay header,
    /// from `.record_replay(path, game_version)`'s second argument.
    pub(super) replay_game_version: String,
    /// `Some(path)` when `.play_replay(path)` was called.
    pub(super) play_replay_path: Option<PathBuf>,
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
            scenes: Vec::new(),
            scene_render: FxHashMap::default(),
            initial_scene: None,
            loading_scene: None,
            tracked_groups: Vec::new(),
            extra_systems: Vec::new(),
            extra_observers: Vec::new(),
            scene_refs: Vec::new(),
            #[cfg(feature = "lua")]
            lua_script: None,
            deterministic_seed: None,
            record_replay_path: None,
            replay_game_version: String::new(),
            play_replay_path: None,
        }
    }

    /// Set a custom path for the config file (default: `"config.ini"`).
    ///
    /// The file is optional: a missing file means all defaults.
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

    /// Register the `setup` hook (called once when entering the `Setup` game state).
    ///
    /// Optional: omit it when there is nothing to load. After the hook runs, the
    /// engine moves to `Playing` on its own unless the hook requested another
    /// state through `NextGameState` (e.g. `Quitting`), which then wins.
    ///
    /// The system is registered into [`SystemsStore`](aberred_core::resources::systemsstore::SystemsStore)
    /// under the key `"setup"`.
    pub fn on_setup<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.setup_hook = Some(hook_registrar(hook_keys::SETUP, system));
        self
    }

    /// Add a per-tick system to the sim schedule.
    ///
    /// Runs once per sim tick (`[simulation] hz` in `config.ini`), in
    /// [`SimSet::ScriptUpdate`], with `.run_if(state_is_playing)`. Treat it as
    /// idempotent/edge-triggered the same way Lua's `on_update_<scene>` must be:
    /// gate one-shot effects on an edge, not on "runs once per visible frame"
    /// -- the sim ticks faster than the render thread. Can be called multiple
    /// times to register several systems.
    ///
    /// For custom ordering relative to other engine systems (e.g. `.after(movement)`)
    /// or for systems with different run conditions, use
    /// [`configure_schedule`](Self::configure_schedule) instead.
    ///
    /// # Scene-scoped (transient) observers
    ///
    /// If you need an observer that is only active within a specific scene, spawn
    /// it from the scene's [`SceneEntered`] observer **without** the [`Persistent`](aberred_core::components::persistent::Persistent) component:
    ///
    /// ```rust,ignore
    /// fn my_scene_enter(_: On<SceneEntered>, mut commands: Commands) {
    ///     // No Persistent → despawned with the scene on the next scene switch
    ///     commands.spawn(Observer::new(on_my_event));
    /// }
    /// ```
    pub fn add_system<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.extra_systems.push(playing_system(system));
        self
    }

    /// Add a system that runs only while `condition` holds.
    ///
    /// Like [`add_system`](Self::add_system) (once per sim tick, in
    /// [`SimSet::ScriptUpdate`], while `Playing`), plus `.run_if(condition)`.
    /// The condition is passed separately because a `.run_if(..)`-configured
    /// system can't be carried to the logic thread. It runs only while
    /// `Playing`; for a system tied to a scene, which also runs during `Setup`
    /// when that scene is active, use [`add_scene_system`](Self::add_scene_system).
    ///
    /// ```rust,ignore
    /// EngineBuilder::new()
    ///     .add_system_if(hud, in_scene("level01"))
    ///     .add_system_if(enemy_ai, in_scene("level01").or_else(in_scene("level02")))
    /// ```
    pub fn add_system_if<M, MC>(
        mut self,
        system: impl IntoSystem<(), (), M> + Send + 'static,
        condition: impl SystemCondition<MC> + Send + 'static,
    ) -> Self {
        self.extra_systems
            .push(playing_system_if(system, condition));
        self
    }

    /// Add a system that runs only while the scene `scene` is active.
    ///
    /// Runs once per sim tick, in [`SimSet::ScriptUpdate`], whenever `scene`
    /// is active ([`in_scene`](aberred_core::systems::scene_dispatch::in_scene)):
    /// during `Playing`, and during `Setup` for a loading scene. `ScriptUpdate`
    /// runs before movement and collision, so the system sees the state the
    /// previous tick left. Can be called multiple times, also for the same scene.
    ///
    /// ```rust,ignore
    /// EngineBuilder::new()
    ///     .add_scene("level01")
    ///     .add_scene_system("level01", enemy_waves)
    /// ```
    ///
    /// # Errors (at `.run()`/`.try_run()`)
    ///
    /// [`EngineError::SceneNotRegistered`](aberred_core::error::EngineError::SceneNotRegistered)
    /// if `scene` was never registered with [`add_scene`](Self::add_scene).
    pub fn add_scene_system<M>(
        mut self,
        scene: &'static str,
        system: impl IntoSystem<(), (), M> + Send + 'static,
    ) -> Self {
        self.scene_refs.push(("add_scene_system", scene));
        self.extra_systems.push(scene_system(scene, system));
        self
    }

    /// Observe [`SceneEntered`] for the scene `scene` only.
    ///
    /// The observer is attached to the scene's entity, so it fires each time
    /// `scene` becomes active, after the previous scene is torn down: the
    /// entities it spawns belong to `scene`. For every scene, use
    /// [`add_observer`](Self::add_observer) and read `name` from the event.
    ///
    /// ```rust,ignore
    /// fn spawn_level(_: On<SceneEntered>, mut commands: Commands) { /* … */ }
    ///
    /// EngineBuilder::new().on_scene_enter("level01", spawn_level)
    /// ```
    ///
    /// # Errors (at `.run()`/`.try_run()`)
    ///
    /// As [`add_scene_system`](Self::add_scene_system).
    pub fn on_scene_enter<B: Bundle, M>(
        self,
        scene: &'static str,
        observer: impl IntoObserverSystem<SceneEntered, B, M>,
    ) -> Self {
        self.on_scene_event("on_scene_enter", scene, observer)
    }

    /// Observe [`SceneExited`] for the scene `scene` only.
    ///
    /// Fires each time `scene` is left, before its entities are despawned, so
    /// the observer can still read them.
    ///
    /// # Errors (at `.run()`/`.try_run()`)
    ///
    /// As [`add_scene_system`](Self::add_scene_system).
    pub fn on_scene_exit<B: Bundle, M>(
        self,
        scene: &'static str,
        observer: impl IntoObserverSystem<SceneExited, B, M>,
    ) -> Self {
        self.on_scene_event("on_scene_exit", scene, observer)
    }

    fn on_scene_event<E: EntityEvent, B: Bundle, M>(
        mut self,
        method: &'static str,
        scene: &'static str,
        observer: impl IntoObserverSystem<E, B, M>,
    ) -> Self {
        self.scene_refs.push((method, scene));
        self.extra_observers
            .push(scene_observer_registrar(scene, observer));
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
    /// [`movement`](aberred_core::systems::movement::movement) and before
    /// [`collision_detector`](aberred_core::systems::collision_detector::collision_detector).
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
    /// The observer is spawned with the [`Persistent`](aberred_core::components::persistent::Persistent) component and therefore
    /// survives scene transitions. The observer function's first parameter must
    /// be `On<E>` where `E` is the event type.
    ///
    /// ```rust,ignore
    /// #[derive(Event)]
    /// struct TilemapLoaded { path: String }
    ///
    /// fn on_tilemap_loaded(trigger: On<TilemapLoaded>, mut commands: Commands) {
    ///     // react to the event …
    /// }
    ///
    /// EngineBuilder::new()
    ///     .add_observer(on_tilemap_loaded)
    ///     // …
    /// ```
    ///
    /// To trigger the event from a system or observer:
    /// ```rust,ignore
    /// commands.trigger(TilemapLoaded { path: "…".into() });
    /// ```
    pub fn add_observer<E: Event, B: Bundle, M>(
        mut self,
        observer: impl IntoObserverSystem<E, B, M>,
    ) -> Self {
        self.extra_observers.push(observer_registrar(observer));
        self
    }

    /// Register a named scene for [`SceneManager`](aberred_core::resources::scenemanager::SceneManager)-based games.
    ///
    /// A scene is a name: its behavior comes from observers of
    /// [`SceneEntered`]/[`SceneExited`] ([`on_scene_enter`](Self::on_scene_enter),
    /// [`on_scene_exit`](Self::on_scene_exit), [`add_observer`](Self::add_observer)) and
    /// from systems gated on it ([`add_scene_system`](Self::add_scene_system)). Each
    /// switch despawns every non-[`Persistent`](aberred_core::components::persistent::Persistent) entity. Use with
    /// [`.initial_scene()`](Self::initial_scene) to specify which scene starts first.
    ///
    /// # Errors (at `.run()`/`.try_run()`)
    ///
    /// - If `.add_scene()` is combined with `.with_lua()`
    /// - If `.add_scene()` is used without `.initial_scene()`, or `.initial_scene()`
    ///   names a scene that was never registered
    ///
    /// `.try_run()` returns these as an [`EngineError`](aberred_core::error::EngineError);
    /// `.run()` prints the error to stderr and exits with a nonzero status. Neither panics.
    pub fn add_scene(mut self, name: impl Into<String>) -> Self {
        self.scenes.push(name.into());
        self
    }

    /// Draw ImGui widgets every render frame while the scene `scene` is active.
    ///
    /// Runs on the render thread; see [`GuiCallback`] for what it can read and
    /// write. Calling it again for the same scene replaces the callback.
    ///
    /// # Errors (at `.run()`/`.try_run()`)
    ///
    /// As [`add_scene_system`](Self::add_scene_system).
    pub fn add_scene_gui(mut self, scene: &'static str, gui: GuiCallback) -> Self {
        self.scene_refs.push(("add_scene_gui", scene));
        self.scene_render
            .entry(scene.to_owned())
            .or_default()
            .gui_callback = Some(gui);
        self
    }

    /// Draw world-space overlays every render frame while the scene `scene` is
    /// active, inside the camera transform.
    ///
    /// Runs on the render thread; see
    /// [`WorldDrawCallback`] for what it can read. Calling it again for the same
    /// scene replaces the callback.
    ///
    /// # Errors (at `.run()`/`.try_run()`)
    ///
    /// As [`add_scene_system`](Self::add_scene_system).
    pub fn add_scene_world_draw(mut self, scene: &'static str, draw: WorldDrawCallback) -> Self {
        self.scene_refs.push(("add_scene_world_draw", scene));
        self.scene_render
            .entry(scene.to_owned())
            .or_default()
            .world_draw_callback = Some(draw);
        self
    }

    /// The render thread's per-scene callback table: one entry per scene with
    /// a `.add_scene_gui()` or `.add_scene_world_draw()` callback.
    pub(super) fn render_scene_table(&self) -> RenderSceneTable {
        RenderSceneTable(self.scene_render.clone())
    }

    /// Set the initial scene for [`SceneManager`](aberred_core::resources::scenemanager::SceneManager)-based games.
    ///
    /// This scene is entered (triggering [`SceneEntered`]) when the game
    /// transitions to the `Playing` state.
    pub fn initial_scene(mut self, name: impl Into<String>) -> Self {
        self.initial_scene = Some(name.into());
        self
    }

    /// Show the scene `scene` while `Setup` waits for assets.
    ///
    /// The engine enters it on entering `Setup`, right after the setup hook, so
    /// its [`SceneEntered`] observers see what the hook inserted and can queue
    /// the few assets the loading scene needs (a font); `Setup` waits for those
    /// too. Its
    /// [`add_scene_system`](Self::add_scene_system)s run every sim tick while
    /// loading, e.g. to show [`PendingAssets::len`](aberred_core::resources::pending_assets::PendingAssets::len).
    /// On `Playing` the engine leaves it for the initial scene, which tears it
    /// down along with everything spawned during `Setup`.
    ///
    /// `WorldTime` doesn't advance during `Setup`, so a loading scene shows
    /// progress rather than time-based animation.
    ///
    /// # Errors (at `.run()`/`.try_run()`)
    ///
    /// [`EngineError::SceneNotRegistered`](aberred_core::error::EngineError::SceneNotRegistered)
    /// if `scene` isn't registered with [`add_scene`](Self::add_scene), and
    /// [`EngineError::LoadingSceneIsInitialScene`](aberred_core::error::EngineError::LoadingSceneIsInitialScene)
    /// if it is the initial scene.
    pub fn loading_scene(mut self, scene: &'static str) -> Self {
        self.scene_refs.push(("loading_scene", scene));
        self.loading_scene = Some(scene);
        self
    }

    /// Track the entity count of group `name` for the whole game.
    ///
    /// The count is published to `WorldSignals` every sim tick (read it with
    /// `get_group_count(name)`), and the group stays tracked across scene
    /// switches, which otherwise reset group tracking. Names longer than
    /// `MAX_GROUP_NAME_LEN` bytes are a startup error
    /// ([`EngineError::GroupNameTooLong`](aberred_core::error::EngineError::GroupNameTooLong)).
    /// Can be called multiple times.
    pub fn track_group(mut self, name: impl Into<String>) -> Self {
        self.tracked_groups.push(name.into());
        self
    }

    /// Configure the builder for a Lua game.
    ///
    /// Sets up all four hooks to use `lua_plugin` functions and initialises the
    /// Lua runtime with the given script path.
    #[cfg(feature = "lua")]
    pub fn with_lua(mut self, script_path: impl Into<PathBuf>) -> Self {
        self.lua_script = Some(script_path.into());
        self
    }

    /// Opt into deterministic mode: `SimRng` (`src/resources/sim_rng.rs`) is
    /// seeded from `seed` instead of entropy. Fixed-dt ticking and the
    /// single-threaded schedule executor already apply to every game, so this
    /// is the engine switch that pins simulation randomness to a known seed.
    ///
    /// Mutually exclusive with `.with_lua()` -- Lua is
    /// outside the deterministic envelope, so combining both is rejected at
    /// `.run()`/`.try_run()` time as
    /// [`EngineError::LuaConflictsWithDeterministic`](aberred_core::error::EngineError::LuaConflictsWithDeterministic).
    pub fn deterministic(mut self, seed: u64) -> Self {
        self.deterministic_seed = Some(seed);
        self
    }

    /// Record this session's `TickInput` stream to a replay file.
    ///
    /// Requires `.deterministic(seed)` to already be set (the replay header
    /// needs a concrete seed) and is mutually exclusive with
    /// [`.play_replay()`](Self::play_replay) -- both checked at
    /// `.run()`/`.try_run()` time. `game_version` is a free-form
    /// diagnostics string written into the header, not validated on replay.
    pub fn record_replay(
        mut self,
        path: impl Into<PathBuf>,
        game_version: impl Into<String>,
    ) -> Self {
        self.record_replay_path = Some(path.into());
        self.replay_game_version = game_version.into();
        self
    }

    /// Play back a recorded replay file instead of live input.
    ///
    /// The seed comes from the replay file's header -- do not also call
    /// [`.deterministic()`](Self::deterministic) (rejected at
    /// `.run()`/`.try_run()` time as ambiguous). Mutually exclusive with
    /// [`.record_replay()`](Self::record_replay).
    pub fn play_replay(mut self, path: impl Into<PathBuf>) -> Self {
        self.play_replay_path = Some(path.into());
        self
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
