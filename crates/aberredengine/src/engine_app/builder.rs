use std::path::PathBuf;

use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoObserverSystem;

use super::registrar::{
    HookRegistrar, ObserverRegistrar, UpdateRegistrar, conditional_system_registrar,
    hook_registrar, scene_observer_registrar, scene_system_registrar, system_registrar,
};
use super::scene::SceneDescriptor;
#[cfg(any(doc, feature = "lua"))] // doc links, and with_lua's update hook
use super::schedule::SimSet;
use aberred_core::components::persistent::Persistent;
use aberred_core::events::scene::{SceneEntered, SceneExited};
use aberred_core::resources::systemsstore as hook_keys;
#[cfg(feature = "lua")]
use aberred_core::systems::gamestate::state_is_playing;
use aberred_core::systems::scene_dispatch::WorldDrawCallback;
use aberred_render::resources::scene_table::{GuiCallback, RenderSceneTable, SceneRender};

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
    /// `.add_scene_gui()` registrations, joined into the render table by scene name.
    pub(super) scene_guis: Vec<(&'static str, GuiCallback)>,
    /// `.add_scene_world_draw()` registrations, joined into the render table by scene name.
    pub(super) scene_world_draws: Vec<(&'static str, WorldDrawCallback)>,
    pub(super) initial_scene: Option<String>,
    /// Group names from `.track_group()`, tracked across scene switches.
    pub(super) tracked_groups: Vec<String>,
    pub(super) extra_systems: Vec<UpdateRegistrar>,
    pub(super) extra_observers: Vec<ObserverRegistrar>,
    /// `(method, scene)` for every scene-scoped call (`add_scene_system`,
    /// `on_scene_enter`, `on_scene_exit`), checked against `scenes` by
    /// `validate_builder`.
    pub(super) scene_refs: Vec<(&'static str, &'static str)>,
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
            enter_play_hook: None,
            update_hook: None,
            switch_scene_hook: None,
            scenes: Vec::new(),
            scene_guis: Vec::new(),
            scene_world_draws: Vec::new(),
            initial_scene: None,
            tracked_groups: Vec::new(),
            extra_systems: Vec::new(),
            extra_observers: Vec::new(),
            scene_refs: Vec::new(),
            first_user_hook: None,
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
        self.first_user_hook.get_or_insert("on_setup");
        self
    }

    /// Register the `enter_play` hook (called when transitioning to `Playing`).
    ///
    /// Optional. With `.add_scene()` the SceneManager supplies its own.
    ///
    /// The system is registered into [`SystemsStore`](aberred_core::resources::systemsstore::SystemsStore)
    /// under the key `"enter_play"`.
    pub fn on_enter_play<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.enter_play_hook = Some(hook_registrar(hook_keys::ENTER_PLAY, system));
        self.first_user_hook.get_or_insert("on_enter_play");
        self
    }

    /// Register the `switch_scene` hook (called when a scene transition is requested).
    ///
    /// The system is registered into [`SystemsStore`](aberred_core::resources::systemsstore::SystemsStore)
    /// under the key `"switch_scene"`.
    pub fn on_switch_scene<M>(
        mut self,
        system: impl IntoSystem<(), (), M> + Send + 'static,
    ) -> Self {
        self.switch_scene_hook = Some(hook_registrar(hook_keys::SWITCH_SCENE, system));
        self.first_user_hook.get_or_insert("on_switch_scene");
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
    /// it from the scene's `on_enter` callback **without** the [`Persistent`] component:
    ///
    /// ```rust,ignore
    /// fn my_scene_enter(ctx: &mut GameCtx) {
    ///     // No Persistent → cleaned up on scene switch by clean_all_entities
    ///     ctx.commands.spawn(Observer::new(on_my_event));
    /// }
    /// ```
    pub fn add_system<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.extra_systems.push(system_registrar(system));
        self
    }

    /// Add a system that runs only while `condition` holds.
    ///
    /// Like [`add_system`](Self::add_system) (once per sim tick, in
    /// [`SimSet::ScriptUpdate`], while `Playing`), plus `.run_if(condition)`.
    /// The condition is passed separately because a `.run_if(..)`-configured
    /// system can't be carried to the logic thread.
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
            .push(conditional_system_registrar(system, condition));
        self
    }

    /// Add a system that runs only while the scene `scene` is active.
    ///
    /// Short for [`add_system_if`](Self::add_system_if)`(system,`
    /// [`in_scene`](aberred_core::systems::scene_dispatch::in_scene)`(scene))`:
    /// once per sim tick, in [`SimSet::ScriptUpdate`], while `Playing`.
    /// `ScriptUpdate` runs before movement and collision, so the system sees
    /// the state the previous tick left. Can be called multiple times, also
    /// for the same scene.
    ///
    /// ```rust,ignore
    /// EngineBuilder::new()
    ///     .add_scene("level01", level01())
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
        self.extra_systems
            .push(scene_system_registrar(scene, system));
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
        mut self,
        scene: &'static str,
        observer: impl IntoObserverSystem<SceneEntered, B, M>,
    ) -> Self {
        self.scene_refs.push(("on_scene_enter", scene));
        self.extra_observers
            .push(scene_observer_registrar(scene, observer));
        self
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
        mut self,
        scene: &'static str,
        observer: impl IntoObserverSystem<SceneExited, B, M>,
    ) -> Self {
        self.scene_refs.push(("on_scene_exit", scene));
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

    /// Register a named scene for [`SceneManager`](aberred_core::resources::scenemanager::SceneManager)-based games.
    ///
    /// Scenes are stored and later inserted into a
    /// [`SceneManager`](aberred_core::resources::scenemanager::SceneManager) resource
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
    /// `.try_run()` returns these as an [`EngineError`](aberred_core::error::EngineError);
    /// `.run()` prints the error to stderr and exits with a nonzero status. Neither panics.
    pub fn add_scene(mut self, name: impl Into<String>, descriptor: SceneDescriptor) -> Self {
        self.scenes.push((name.into(), descriptor));
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
        self.scene_guis.push((scene, gui));
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
        self.scene_world_draws.push((scene, draw));
        self
    }

    /// The render thread's per-scene callback table: one entry per scene with
    /// a `.add_scene_gui()` or `.add_scene_world_draw()` callback.
    pub(super) fn render_scene_table(&self) -> RenderSceneTable {
        let mut table = RenderSceneTable::default();
        let guis = self
            .scene_guis
            .iter()
            .map(|&(scene, gui)| (scene, Some(gui), None));
        let draws = (self.scene_world_draws.iter()).map(|&(scene, draw)| (scene, None, Some(draw)));
        for (scene, gui, draw) in guis.chain(draws) {
            let render = table.0.entry(scene.to_owned()).or_insert(SceneRender {
                gui_callback: None,
                world_draw_callback: None,
            });
            render.gui_callback = gui.or(render.gui_callback);
            render.world_draw_callback = draw.or(render.world_draw_callback);
        }
        table
    }

    /// Set the initial scene for [`SceneManager`](aberred_core::resources::scenemanager::SceneManager)-based games.
    ///
    /// This scene's `on_enter` callback will be the first called when the
    /// game transitions to the `Playing` state.
    pub fn initial_scene(mut self, name: impl Into<String>) -> Self {
        self.initial_scene = Some(name.into());
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
        use aberred_lua::lua_plugin;

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
