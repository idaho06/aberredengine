//! Headless test harness for the logic-thread `World`.
//!
//! [`TestWorld`] builds a REAL logic-thread `World` and its REAL `sim`/
//! `present` schedules by calling the exact same `EngineBuilder` functions
//! `logic_thread_main` calls (`setup_logic_world` -> `init_logic_world`)
//! -- no raylib window, no
//! GL, no real audio thread (a stub `AudioBridge` is inserted instead, see
//! [`aberred_core::protocol::endpoints::setup_audio_stub`]), and Lua only if
//! [`TestWorldBuilder::with_lua`] is used. This is deliberately NOT a
//! from-scratch minimal `World` like the ones per-system unit tests build
//! -- if this harness ever
//! forks the production construction path instead of calling into it, its
//! green tests become lies about what the real engine does.
//!
//! Available under `#[cfg(test)]` and the `test-support` Cargo feature (never
//! enabled by default -- downstream release builds must not compile this
//! module in).
//!
//! # What's real vs. stubbed
//!
//! - `world`, `sim`, `present`: the real production `World`/`Schedule`s.
//! - `audio_cmds`/`audio_msgs_tx`: a stub `AudioBridge` far end -- no audio
//!   thread runs, but outgoing `AudioCmd`s can be inspected and fake
//!   `AudioMessage` replies injected.
//! - `sent_to_render`: a plain `RenderMsg` channel the harness owns instead
//!   of a real render thread.
//! - `send_input`/[`resolve_input_backlog`](aberred_core::systems::input::resolve_input_backlog):
//!   the harness calls this function DIRECTLY rather than routing a sample
//!   through the real bounded `InputSample` channel -- that channel only
//!   exists because [`LogicInit`] requires an `rx_input` field, and is never
//!   driven by the harness. Don't mistake `send_input` for a real
//!   cross-thread round trip.
//! - `present()`: `SnapshotPublisher` (inside `world`) is write-only; the
//!   harness holds the paired `Output<DrawableSnapshot>` itself
//!   (`snapshot_out`) to read back what `present` just published.

use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoObserverSystem;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
#[cfg(feature = "lua")]
use std::path::PathBuf;

use crate::engine_app::{
    EngineBuilder, HookRegistrar, LogicInit, ObserverRegistrar, UpdateRegistrar, apply_tick_input,
    drain_logic_messages, ensure_main_scene, hold_back_deterministic_setup_input, hook_registrar,
    in_envelope, observer_registrar, playing_system, playing_system_if, run_sim_tick,
    scene_observer_registrar, scene_system,
};
use aberred_core::events::scene::{SceneEntered, SceneExited};
use aberred_core::protocol::audio::{AudioCmd, AudioMessage};
use aberred_core::protocol::raw_input::RawDeviceSnapshot;
use aberred_core::protocol::render_logic::{LogicMsg, RenderMsg};
use aberred_core::protocol::snapshot::SnapshotPublisher;
use aberred_core::protocol::tick_input::TickInput;
use aberred_core::resources::drawable_snapshot::DrawableSnapshot;
use aberred_core::resources::fontmetrics::FontMetrics;
use aberred_core::resources::gameconfig::GameConfig;
use aberred_core::resources::gamestate::{GameState, GameStates};
use aberred_core::resources::systemsstore as hook_keys;
use aberred_core::systems::input::resolve_input_backlog;
use aberred_core::systems::time::update_world_time;

/// A headless logic-thread `World` plus its `sim`/`present` schedules.
///
/// See the module doc for what's real vs. stubbed.
pub struct TestWorld {
    pub world: World,
    sim: Schedule,
    present: Schedule,
    /// `RenderMsg`s the logic world would have sent to a real render thread
    /// (asset commands, fullscreen toggle, quit).
    pub sent_to_render: Receiver<RenderMsg>,
    /// Outgoing `AudioCmd`s -- no real audio thread consumes these.
    pub audio_cmds: Receiver<AudioCmd>,
    /// Inject a fake `AudioMessage` as if the (nonexistent) audio thread
    /// replied.
    pub audio_msgs_tx: Sender<AudioMessage>,
    /// Setup input held back for the first `Playing` tick (deterministic
    /// mode), as `logic_thread_main` does.
    held_setup_input: TickInput,
    snapshot_out: triple_buffer::Output<DrawableSnapshot>,
}

/// Builder for [`TestWorld`], mirroring [`EngineBuilder`]'s registrar
/// surface (`on_setup`/`add_system`/scenes/Lua) minus
/// anything window/render-related.
pub struct TestWorldBuilder {
    config: GameConfig,
    setup_hook: Option<HookRegistrar>,
    extra_systems: Vec<UpdateRegistrar>,
    extra_observers: Vec<ObserverRegistrar>,
    scenes: Vec<String>,
    initial_scene: Option<String>,
    loading_scene: Option<&'static str>,
    tracked_groups: Vec<String>,
    #[cfg(feature = "lua")]
    lua_script: Option<PathBuf>,
    deterministic_seed: Option<u64>,
    window_w: i32,
    window_h: i32,
}

impl Default for TestWorldBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestWorldBuilder {
    /// Defaults: `GameConfig::new()`, no hooks (like [`EngineBuilder`], the
    /// engine moves from `Setup` to `Playing` on its own), no scenes, no Lua,
    /// an 800x600 nominal window size (the harness never opens a real window;
    /// this only seeds the logic world's `WindowSize` mirror).
    pub fn new() -> Self {
        Self {
            config: GameConfig::new(),
            setup_hook: None,
            extra_systems: Vec::new(),
            extra_observers: Vec::new(),
            scenes: Vec::new(),
            initial_scene: None,
            loading_scene: None,
            tracked_groups: Vec::new(),
            #[cfg(feature = "lua")]
            lua_script: None,
            deterministic_seed: None,
            window_w: 800,
            window_h: 600,
        }
    }

    /// Override the default `GameConfig` (e.g. a custom `sim_hz`).
    pub fn config(mut self, config: GameConfig) -> Self {
        self.config = config;
        self
    }

    /// Register the `setup` hook. As with [`EngineBuilder::on_setup`], the
    /// engine moves to `Playing` after it runs unless it requested another state.
    pub fn on_setup<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.setup_hook = Some(hook_registrar(hook_keys::SETUP, system));
        self
    }

    /// Add a system to the sim schedule, mirroring
    /// `EngineBuilder::add_system`: `.run_if(state_is_playing)`,
    /// `.in_set(SimSet::ScriptUpdate)`. Can be called multiple times.
    pub fn add_system<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.extra_systems.push(playing_system(system));
        self
    }

    /// Add a conditional system, mirroring `EngineBuilder::add_system_if`.
    pub fn add_system_if<M, MC>(
        mut self,
        system: impl IntoSystem<(), (), M> + Send + 'static,
        condition: impl SystemCondition<MC> + Send + 'static,
    ) -> Self {
        self.extra_systems
            .push(playing_system_if(system, condition));
        self
    }

    /// Add a scene-scoped system, mirroring `EngineBuilder::add_scene_system`.
    pub fn add_scene_system<M>(
        mut self,
        scene: &'static str,
        system: impl IntoSystem<(), (), M> + Send + 'static,
    ) -> Self {
        self.extra_systems.push(scene_system(scene, system));
        self
    }

    /// Observe one scene's `SceneEntered`, mirroring `EngineBuilder::on_scene_enter`.
    pub fn on_scene_enter<B: Bundle, M>(
        mut self,
        scene: &'static str,
        observer: impl IntoObserverSystem<SceneEntered, B, M>,
    ) -> Self {
        self.extra_observers
            .push(scene_observer_registrar(scene, observer));
        self
    }

    /// Observe one scene's `SceneExited`, mirroring `EngineBuilder::on_scene_exit`.
    pub fn on_scene_exit<B: Bundle, M>(
        mut self,
        scene: &'static str,
        observer: impl IntoObserverSystem<SceneExited, B, M>,
    ) -> Self {
        self.extra_observers
            .push(scene_observer_registrar(scene, observer));
        self
    }

    /// Add systems to the sim schedule with full control over ordering and
    /// run conditions, mirroring `EngineBuilder::configure_schedule`.
    pub fn configure_schedule(mut self, f: impl FnOnce(&mut Schedule) + Send + 'static) -> Self {
        self.extra_systems.push(Box::new(f));
        self
    }

    /// Add a persistent observer for a custom (or engine) event, mirroring
    /// `EngineBuilder::add_observer`.
    pub fn add_observer<E: Event, B: Bundle, M>(
        mut self,
        observer: impl IntoObserverSystem<E, B, M>,
    ) -> Self {
        self.extra_observers.push(observer_registrar(observer));
        self
    }

    /// Register a named scene for `SceneManager`-based tests.
    pub fn add_scene(mut self, name: impl Into<String>) -> Self {
        self.scenes.push(name.into());
        self
    }

    /// Set the initial scene for `SceneManager`-based tests.
    pub fn initial_scene(mut self, name: impl Into<String>) -> Self {
        self.initial_scene = Some(name.into());
        self
    }

    /// Set the scene shown while `Setup` waits for assets, mirroring
    /// `EngineBuilder::loading_scene`.
    pub fn loading_scene(mut self, scene: &'static str) -> Self {
        self.loading_scene = Some(scene);
        self
    }

    /// Track a scene-persistent group, mirroring [`EngineBuilder::track_group`].
    pub fn track_group(mut self, name: impl Into<String>) -> Self {
        self.tracked_groups.push(name.into());
        self
    }

    /// Opt into deterministic mode, mirroring `EngineBuilder::deterministic`:
    /// `SimRng` is seeded from `seed` instead of entropy.
    pub fn deterministic(mut self, seed: u64) -> Self {
        self.deterministic_seed = Some(seed);
        self
    }

    #[cfg(feature = "lua")]
    /// Configure the harness for a Lua game, mirroring `EngineBuilder::with_lua`.
    pub fn with_lua(mut self, script_path: impl Into<PathBuf>) -> Self {
        self.lua_script = Some(script_path.into());
        self
    }

    /// Build the [`TestWorld`], calling the same production `EngineBuilder`
    /// functions `logic_thread_main` calls (`setup_logic_world`, then
    /// `init_logic_world`).
    pub fn build(mut self) -> Result<TestWorld, aberred_core::error::EngineError> {
        #[cfg(feature = "lua")]
        let has_lua = self.lua_script.is_some();
        #[cfg(not(feature = "lua"))]
        let has_lua = false;
        ensure_main_scene(&mut self.scenes, &mut self.initial_scene, has_lua);

        let (_tx_logic, rx_logic) = unbounded::<LogicMsg>();
        let (tx_render, rx_render) = unbounded::<RenderMsg>();
        let (_tx_input, rx_input) = bounded(64);
        let (snap_in, snap_out) =
            triple_buffer::TripleBuffer::new(&DrawableSnapshot::default()).split();

        let mut init = LogicInit {
            config: self.config.clone(),
            setup_hook: self.setup_hook.take(),
            extra_systems: std::mem::take(&mut self.extra_systems),
            extra_observers: std::mem::take(&mut self.extra_observers),
            scenes: std::mem::take(&mut self.scenes),
            initial_scene: self.initial_scene.take(),
            loading_scene: self.loading_scene,
            tracked_groups: std::mem::take(&mut self.tracked_groups),
            #[cfg(feature = "lua")]
            lua_script: self.lua_script.take(),
            deterministic_seed: self.deterministic_seed,
            window_w: self.window_w,
            window_h: self.window_h,
            tx_render,
            rx_logic,
            rx_input,
            snapshot_publisher: Some(SnapshotPublisher(snap_in)),
            replay_player: None,
            replay_recorder: None,
            stub_audio: true,
            audio_stub_ends: None,
        };

        let mut world = EngineBuilder::setup_logic_world(&mut init)?;
        let (audio_cmds, audio_msgs_tx) = init
            .audio_stub_ends
            .take()
            .expect("setup_logic_world always sets audio_stub_ends when stub_audio is true");

        let (sim, present) = EngineBuilder::init_logic_world(&mut init, &mut world)?;

        Ok(TestWorld {
            world,
            sim,
            present,
            sent_to_render: rx_render,
            audio_cmds,
            audio_msgs_tx,
            held_setup_input: TickInput::default(),
            snapshot_out: snap_out,
        })
    }
}

impl TestWorld {
    /// Build a [`TestWorld`] with all defaults (see [`TestWorldBuilder::new`]).
    pub fn new() -> Self {
        TestWorldBuilder::new()
            .build()
            .expect("default TestWorld construction cannot fail")
    }

    pub fn builder() -> TestWorldBuilder {
        TestWorldBuilder::new()
    }

    /// Run `n` sim ticks with the given `dt`, mirroring
    /// `logic_thread_main`'s per-tick loop body (minus the `Pacer`/channel
    /// drain, which the harness doesn't need): `update_world_time` (only
    /// while `Playing`) ->
    /// `run_sim_tick` (which also clears `InputState`'s edge flags) ->
    /// `world.clear_trackers()`.
    pub fn tick(&mut self, n: u32, dt: f32) {
        for _ in 0..n {
            if in_envelope(&self.world) {
                update_world_time(&mut self.world, dt);
            }
            run_sim_tick(&mut self.world, &mut self.sim);
            self.world.clear_trackers();
        }
    }

    /// Tick with `dt` until [`GameState`] reaches [`GameStates::Playing`],
    /// or panic after `max_ticks` ticks without reaching it. Useful for
    /// consumer tests that don't want to reason about exact Setup/EnterPlay
    /// transition timing -- pass a generous bound (a handful of ticks is
    /// normally enough with the default setup hook).
    pub fn tick_to_play(&mut self, dt: f32, max_ticks: u32) {
        for _ in 0..max_ticks {
            if matches!(
                self.world.resource::<GameState>().get(),
                GameStates::Playing
            ) {
                return;
            }
            self.tick(1, dt);
        }
        assert!(
            matches!(
                self.world.resource::<GameState>().get(),
                GameStates::Playing
            ),
            "TestWorld did not reach GameStates::Playing within {max_ticks} ticks"
        );
    }

    /// Feed one raw device sample straight into
    /// [`resolve_input_backlog`] -- see the module doc for why this bypasses
    /// the real `InputSample` channel.
    pub fn send_input(&mut self, sample: RawDeviceSnapshot) {
        resolve_input_backlog(&mut self.world, std::slice::from_ref(&sample));
    }

    /// Apply one [`TickInput`], then advance one tick via [`Self::tick`] --
    /// together the same sequence `logic_thread_main`'s per-tick loop runs
    /// post-collect. Does not drain any channel -- build the `TickInput`
    /// yourself (it's a plain public struct) and pass it in, mirroring how
    /// `tick` doesn't drain a channel either. Determinism-roadmap round-trip/
    /// replay tests use this to drive a `TestWorld` from a recorded
    /// `TickInput` sequence the same way the real logic thread would.
    pub fn apply_tick_input(&mut self, tick_input: &TickInput, dt: f32) {
        let mut tick_input = tick_input.clone();
        hold_back_deterministic_setup_input(
            &self.world,
            &mut tick_input,
            &mut self.held_setup_input,
        );
        apply_tick_input(&mut self.world, &tick_input);
        self.tick(1, dt);
    }

    /// Run the `present` schedule once and return the freshly published
    /// [`DrawableSnapshot`].
    pub fn present(&mut self) -> DrawableSnapshot {
        self.present.run(&mut self.world);
        self.snapshot_out.update();
        self.snapshot_out.output_buffer().clone()
    }

    /// Deliver one render-thread message through the logic thread's real
    /// message drain, as if the render thread had sent it. Load replies
    /// (`TextureLoaded`, `AssetLoadFailed`, ...) settle `PendingAssets` and
    /// trigger their load event before this returns.
    pub fn deliver_logic_msg(&mut self, msg: LogicMsg) {
        let (tx, rx) = unbounded();
        tx.send(msg).expect("receiver is alive");
        drain_logic_messages(None, &rx, &mut self.world);
    }

    /// Fake a render-side font load reply (`LogicMsg::FontLoaded`), for
    /// testing consumers of `FontMetricsStore` without a real font/GL load.
    pub fn deliver_font_metrics(&mut self, key: &str, metrics: FontMetrics) {
        self.deliver_logic_msg(LogicMsg::FontLoaded {
            key: key.to_owned(),
            metrics,
        });
    }

    /// Fake a render-side texture load reply (`LogicMsg::TextureLoaded`), for
    /// testing consumers of `TextureDimsStore` without a real texture/GL
    /// load.
    pub fn deliver_texture_dims(&mut self, key: &str, w: i32, h: i32) {
        self.deliver_logic_msg(LogicMsg::TextureLoaded {
            key: key.to_owned(),
            width: w,
            height: h,
        });
    }
}

impl Default for TestWorld {
    fn default() -> Self {
        Self::new()
    }
}

// --- Replay test wrappers -------------------------------------------------
//
// `hash_world_state`/`ReplayRecorder`/`ReplayPlayer`/`validate_replay_header`
// are crate-internal (`pub(crate)`, not real public API a downstream game
// should see) -- these thin wrappers give `tests/determinism.rs` (an
// external integration-test crate, unlike this module) a way to exercise
// them without promoting the internals themselves.

/// Full-world deterministic state hash -- see `crate::systems::state_hash`.
pub fn hash_world_state(world: &World) -> u64 {
    aberred_core::systems::state_hash::hash_world_state(world)
}

/// Write `ticks` (one entry per tick, in order, no checkpoints) to a replay
/// file at `path`.
pub fn write_tick_inputs_to_replay(
    path: &std::path::Path,
    header: aberred_core::protocol::replay::ReplayHeader,
    ticks: &[TickInput],
) -> std::io::Result<()> {
    let mut rec = crate::engine_app::ReplayRecorder::create(path, &header)?;
    for ti in ticks {
        rec.record_tick(ti);
    }
    rec.finish(0)
}

/// Read `n` tick entries back from a replay file written by
/// [`write_tick_inputs_to_replay`], returning the header plus the
/// reconstructed `TickInput` sequence. `n` must match how many ticks were
/// written.
pub fn read_tick_inputs_from_replay(
    path: &std::path::Path,
    n: usize,
) -> (aberred_core::protocol::replay::ReplayHeader, Vec<TickInput>) {
    let (header, mut player) = crate::engine_app::ReplayPlayer::open_header(path)
        .expect("read_tick_inputs_from_replay: failed to open replay file");
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut ti = TickInput::default();
        player.collect(&mut ti, i as u64);
        out.push(ti);
    }
    (header, out)
}

/// Validate a replay header against a `GameConfig`, mirroring
/// `EngineBuilder::try_run`'s own check.
pub fn validate_replay_header(
    header: &aberred_core::protocol::replay::ReplayHeader,
    config: &GameConfig,
) -> Result<(), aberred_core::error::EngineError> {
    crate::engine_app::validate_replay_header(header, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_test_world_builds_and_ticks_without_panicking() {
        let mut tw = TestWorld::new();
        tw.tick(1, 1.0 / 60.0);
    }

    #[test]
    fn default_test_world_reaches_playing_quickly() {
        let mut tw = TestWorld::new();
        tw.tick_to_play(1.0 / 60.0, 8);
        assert!(matches!(
            tw.world.resource::<GameState>().get(),
            GameStates::Playing
        ));
    }
}
