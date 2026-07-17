//! Headless test harness for the logic-thread `World`.
//!
//! [`TestWorld`] builds a REAL logic-thread `World` and its REAL `sim`/
//! `present` schedules by calling the exact same `EngineBuilder` functions
//! `logic_thread_main` calls (`setup_logic_world` -> `register_logic_systems`
//! -> `spawn_observers` -> `build_logic_schedules`) -- no raylib window, no
//! GL, no real audio thread (a stub `AudioBridge` is inserted instead, see
//! [`crate::protocol::endpoints::setup_audio_stub`]), and Lua only if
//! [`TestWorldBuilder::with_lua`] is used. This is deliberately NOT a
//! from-scratch minimal `World` (see `tests/engine_tick_integration.rs`'s
//! `make_world` for what that looks like at scale) -- if this harness ever
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
//! - `send_input`/[`resolve_input_backlog`](crate::systems::input::resolve_input_backlog):
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

use crate::components::persistent::Persistent;
use crate::engine_app::{
    EngineBuilder, HookRegistrar, LogicInit, ObserverRegistrar, UpdateRegistrar, hook_registrar,
    run_sim_tick,
};
use crate::protocol::audio::{AudioCmd, AudioMessage};
use crate::protocol::raw_input::RawDeviceSnapshot;
use crate::protocol::render_logic::{LogicMsg, RenderMsg};
use crate::protocol::snapshot::SnapshotPublisher;
use crate::resources::systemsstore as hook_keys;
use crate::resources::drawable_snapshot::DrawableSnapshot;
use crate::resources::fontmetrics::{FontMetrics, FontMetricsStore};
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use crate::resources::gameconfig::GameConfig;
use crate::resources::texturedims::TextureDimsStore;
use crate::systems::input::resolve_input_backlog;
use crate::systems::scene_dispatch::SceneDescriptor;
use crate::systems::time::update_world_time;

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
    snapshot_out: triple_buffer::Output<DrawableSnapshot>,
}

/// Default `setup` hook: immediately requests a transition to
/// [`GameStates::Playing`], so a default-built [`TestWorld`] reaches
/// `Playing` after its first [`TestWorld::tick`] with no game-specific setup
/// required. Override via [`TestWorldBuilder::on_setup`] for tests that need
/// their own setup behavior (and their own state transition).
fn default_test_setup(mut next_state: ResMut<NextGameState>) {
    next_state.set(GameStates::Playing);
}

/// Default `enter_play` hook: no-op.
fn default_test_enter_play() {}

/// Builder for [`TestWorld`], mirroring [`EngineBuilder`]'s registrar
/// surface (`on_setup`/`on_enter_play`/`on_update`/scenes/Lua) minus
/// anything window/render-related.
pub struct TestWorldBuilder {
    config: GameConfig,
    setup_hook: Option<HookRegistrar>,
    enter_play_hook: Option<HookRegistrar>,
    switch_scene_hook: Option<HookRegistrar>,
    update_hook: Option<UpdateRegistrar>,
    extra_systems: Vec<UpdateRegistrar>,
    extra_observers: Vec<ObserverRegistrar>,
    scenes: Vec<(String, SceneDescriptor)>,
    initial_scene: Option<String>,
    #[cfg(feature = "lua")]
    lua_script: Option<PathBuf>,
    window_w: i32,
    window_h: i32,
}

impl Default for TestWorldBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestWorldBuilder {
    /// Defaults: `GameConfig::new()`, a `setup` hook that immediately
    /// requests `Playing` (see [`default_test_setup`]), a no-op `enter_play`
    /// hook, no scenes, no Lua, an 800x600 nominal window size (the harness
    /// never opens a real window; this only seeds the logic world's
    /// `WindowSize` mirror).
    pub fn new() -> Self {
        Self {
            config: GameConfig::new(),
            setup_hook: Some(hook_registrar(hook_keys::SETUP, default_test_setup)),
            enter_play_hook: Some(hook_registrar(hook_keys::ENTER_PLAY, default_test_enter_play)),
            switch_scene_hook: None,
            update_hook: None,
            extra_systems: Vec::new(),
            extra_observers: Vec::new(),
            scenes: Vec::new(),
            initial_scene: None,
            #[cfg(feature = "lua")]
            lua_script: None,
            window_w: 800,
            window_h: 600,
        }
    }

    /// Override the default `GameConfig` (e.g. a custom `sim_hz`).
    pub fn config(mut self, config: GameConfig) -> Self {
        self.config = config;
        self
    }

    /// Replace the `setup` hook. Overriding this drops the default
    /// auto-transition-to-`Playing` behavior -- call
    /// `next_state.set(GameStates::Playing)` yourself (or drive the
    /// transition manually) if the test still needs to reach `Playing`.
    pub fn on_setup<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.setup_hook = Some(hook_registrar(hook_keys::SETUP, system));
        self
    }

    /// Replace the `enter_play` hook.
    pub fn on_enter_play<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.enter_play_hook = Some(hook_registrar(hook_keys::ENTER_PLAY, system));
        self
    }

    /// Register the `switch_scene` hook (required if any scene is added
    /// without using `add_scene`'s own `SceneManager` wiring).
    pub fn on_switch_scene<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.switch_scene_hook = Some(hook_registrar(hook_keys::SWITCH_SCENE, system));
        self
    }

    /// Register the `update` hook (runs once per sim tick, `SimSet::ScriptUpdate`).
    pub fn on_update<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(
                system
                    .run_if(crate::systems::gamestate::state_is_playing)
                    .in_set(crate::engine_app::SimSet::ScriptUpdate),
            );
        }));
        self
    }

    /// Add a system to the sim schedule, mirroring
    /// `EngineBuilder::add_system`: `.run_if(state_is_playing)`,
    /// `.in_set(SimSet::ScriptUpdate)`. Can be called multiple times.
    pub fn add_system<M>(mut self, system: impl IntoSystem<(), (), M> + Send + 'static) -> Self {
        self.extra_systems
            .push(Box::new(move |schedule: &mut Schedule| {
                schedule.add_systems(
                    system
                        .run_if(crate::systems::gamestate::state_is_playing)
                        .in_set(crate::engine_app::SimSet::ScriptUpdate),
                );
            }));
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
        self.extra_observers
            .push(Box::new(move |world: &mut World| {
                world.spawn((Observer::new(observer), Persistent));
            }));
        self
    }

    /// Register a named scene for `SceneManager`-based tests.
    pub fn add_scene(mut self, name: impl Into<String>, descriptor: SceneDescriptor) -> Self {
        self.scenes.push((name.into(), descriptor));
        self
    }

    /// Set the initial scene for `SceneManager`-based tests.
    pub fn initial_scene(mut self, name: impl Into<String>) -> Self {
        self.initial_scene = Some(name.into());
        self
    }

    #[cfg(feature = "lua")]
    /// Configure the harness for a Lua game, mirroring `EngineBuilder::with_lua`.
    pub fn with_lua(mut self, script_path: impl Into<PathBuf>) -> Self {
        use crate::lua_plugin;

        self.lua_script = Some(script_path.into());
        self.setup_hook = Some(hook_registrar(hook_keys::SETUP, lua_plugin::setup));
        self.enter_play_hook = Some(hook_registrar(hook_keys::ENTER_PLAY, lua_plugin::enter_play));
        self.update_hook = Some(Box::new(|schedule: &mut Schedule| {
            schedule.add_systems(
                lua_plugin::update
                    .run_if(crate::systems::gamestate::state_is_playing)
                    .in_set(crate::engine_app::SimSet::Bookkeeping),
            );
        }));
        self.switch_scene_hook = Some(hook_registrar(hook_keys::SWITCH_SCENE, lua_plugin::switch_scene));
        self
    }

    /// Build the [`TestWorld`], calling the same four production
    /// `EngineBuilder` functions `logic_thread_main` calls, in the same
    /// order.
    pub fn build(mut self) -> Result<TestWorld, crate::error::EngineError> {
        let use_scene_manager = !self.scenes.is_empty();

        let (_tx_logic, rx_logic) = unbounded::<LogicMsg>();
        let (tx_render, rx_render) = unbounded::<RenderMsg>();
        let (_tx_input, rx_input) = bounded(8);
        let (snap_in, snap_out) =
            triple_buffer::TripleBuffer::new(&DrawableSnapshot::default()).split();

        let mut init = LogicInit {
            config: self.config.clone(),
            setup_hook: self.setup_hook.take(),
            enter_play_hook: self.enter_play_hook.take(),
            switch_scene_hook: self.switch_scene_hook.take(),
            update_hook: self.update_hook.take(),
            extra_systems: std::mem::take(&mut self.extra_systems),
            extra_observers: std::mem::take(&mut self.extra_observers),
            scenes: std::mem::take(&mut self.scenes),
            initial_scene: self.initial_scene.take(),
            #[cfg(feature = "lua")]
            lua_script: self.lua_script.take(),
            window_w: self.window_w,
            window_h: self.window_h,
            tx_render,
            rx_logic,
            rx_input,
            snapshot_publisher: Some(SnapshotPublisher(snap_in)),
            stub_audio: true,
            audio_stub_ends: None,
        };

        #[cfg(feature = "lua")]
        let has_lua = init.lua_script.is_some();
        #[cfg(not(feature = "lua"))]
        let has_lua = false;

        let mut world = EngineBuilder::setup_logic_world(&mut init)?;
        let (audio_cmds, audio_msgs_tx) = init
            .audio_stub_ends
            .take()
            .expect("setup_logic_world always sets audio_stub_ends when stub_audio is true");

        EngineBuilder::register_logic_systems(&mut init, &mut world, use_scene_manager)?;
        EngineBuilder::spawn_observers(
            &mut world,
            has_lua,
            std::mem::take(&mut init.extra_observers),
        );

        let (sim, present) = EngineBuilder::build_logic_schedules(
            init.update_hook.take(),
            std::mem::take(&mut init.extra_systems),
            &mut world,
            has_lua,
            use_scene_manager,
        )?;

        Ok(TestWorld {
            world,
            sim,
            present,
            sent_to_render: rx_render,
            audio_cmds,
            audio_msgs_tx,
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
    /// drain, which the harness doesn't need): `update_world_time` ->
    /// `run_sim_tick` (which also clears `InputState`'s edge flags) ->
    /// `world.clear_trackers()`.
    pub fn tick(&mut self, n: u32, dt: f32) {
        for _ in 0..n {
            update_world_time(&mut self.world, dt);
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
            if matches!(self.world.resource::<GameState>().get(), GameStates::Playing) {
                return;
            }
            self.tick(1, dt);
        }
        assert!(
            matches!(self.world.resource::<GameState>().get(), GameStates::Playing),
            "TestWorld did not reach GameStates::Playing within {max_ticks} ticks"
        );
    }

    /// Feed one raw device sample straight into
    /// [`resolve_input_backlog`] -- see the module doc for why this bypasses
    /// the real `InputSample` channel.
    pub fn send_input(&mut self, sample: RawDeviceSnapshot) {
        resolve_input_backlog(&mut self.world, std::slice::from_ref(&sample));
    }

    /// Run the `present` schedule once and return the freshly published
    /// [`DrawableSnapshot`].
    pub fn present(&mut self) -> DrawableSnapshot {
        self.present.run(&mut self.world);
        self.snapshot_out.update();
        self.snapshot_out.output_buffer().clone()
    }

    /// Fake a render-side font-metrics reply
    /// (mirrors `LogicMsg::FontLoaded`), for testing consumers of
    /// [`FontMetricsStore`] without a real font/GL load.
    pub fn deliver_font_metrics(&mut self, key: &str, metrics: FontMetrics) {
        self.world
            .resource_mut::<FontMetricsStore>()
            .0
            .insert(key.to_string(), metrics);
    }

    /// Fake a render-side texture-dims reply (mirrors
    /// `LogicMsg::TextureLoaded`), for testing consumers of
    /// [`TextureDimsStore`] without a real texture/GL load.
    pub fn deliver_texture_dims(&mut self, key: &str, w: i32, h: i32) {
        self.world
            .resource_mut::<TextureDimsStore>()
            .insert(key.to_string(), w, h);
    }
}

impl Default for TestWorld {
    fn default() -> Self {
        Self::new()
    }
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
