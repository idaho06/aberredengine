#[cfg(feature = "lua")]
use std::path::PathBuf;

use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use crossbeam_channel::{Receiver, Sender};

use super::builder::EngineBuilder;
use super::registrar::{HookRegistrar, ObserverRegistrar, UpdateRegistrar};
use super::replay::{ReplayPlayer, ReplayRecorder};
use aberred_core::error::EngineError;
use aberred_core::events::input::InputAction;
use aberred_core::pacing::{Pacer, StatsWindow, TickCountdown};
#[cfg(any(test, feature = "test-support"))]
use aberred_core::protocol::audio::{AudioCmd, AudioMessage};
use aberred_core::protocol::endpoints::RenderTx;
use aberred_core::protocol::endpoints::shutdown_audio;
use aberred_core::protocol::raw_input::InputSample;
use aberred_core::protocol::render_logic::{LogicMsg, RenderMsg, ReplayControl};
use aberred_core::protocol::snapshot::SnapshotPublisher;
use aberred_core::protocol::tick_input::TickInput;
use aberred_core::resources::debugoverlayconfig::DebugOverlayConfig;
use aberred_core::resources::deterministic_mode::DeterministicMode;
use aberred_core::resources::fontmetrics::FontMetricsStore;
use aberred_core::resources::gameconfig::GameConfig;
use aberred_core::resources::gamestate::{GameState, GameStates};
use aberred_core::resources::input::InputState;
use aberred_core::resources::loaded_assets::LoadedAssets;
use aberred_core::resources::rawinput::ImguiCaptureMirror;
use aberred_core::resources::screensize::ScreenSize;
use aberred_core::resources::signal_intents::SignalIntents;
use aberred_core::resources::texturedims::TextureDimsStore;
use aberred_core::resources::thread_stats::SimStats;
use aberred_core::resources::windowsize::WindowSize;
use aberred_core::resources::worldtime::WorldTime;
use aberred_core::systems::asset_tracking::settle_load;
use aberred_core::systems::input::resolve_input_backlog;
use aberred_core::systems::signal_intents::apply_signal_intents;
use aberred_core::systems::state_hash::hash_world_state;
use aberred_core::systems::time::update_world_time;

/// Runtime state a running replay-playback session's
/// `LogicMsg::ReplayControl` messages mutate. A `World` resource (inserted
/// unconditionally in `logic_thread_main`, harmless no-op when there's no
/// `TickInputSource::Replay` -- a stray `ReplayControl` message just has
/// nothing to affect) rather than a value threaded through
/// `collect_tick_input_live`/`drain_logic_messages` as an extra parameter --
/// matches how every other `LogicMsg`-driven cross-cutting concern
/// (`DebugOverlayConfig`, `ScreenSize`, `ImguiCaptureMirror`, ...) is
/// written: through `world`, which those functions already take.
#[derive(Resource, Default)]
struct ReplayRuntimeState {
    paused: bool,
    fast_forward: bool,
}

/// Where a tick's [`TickInput`] comes from: live device/channel collection,
/// or replay-file playback.
enum TickInputSource {
    Live,
    Replay(ReplayPlayer),
}

/// Everything the logic thread needs to build the gameplay `World` and its
/// schedules inside its own closure. Must be `Send`: hooks are
/// `Box<dyn FnOnce + Send>`, scene descriptors are fn pointers, and the
/// channel endpoints are crossbeam handles.
pub(crate) struct LogicInit {
    pub(crate) config: GameConfig,
    pub(crate) setup_hook: Option<HookRegistrar>,
    pub(crate) extra_systems: Vec<UpdateRegistrar>,
    pub(crate) extra_observers: Vec<ObserverRegistrar>,
    pub(crate) scenes: Vec<String>,
    pub(crate) initial_scene: Option<String>,
    pub(crate) loading_scene: Option<String>,
    /// Scene-persistent group names (`EngineBuilder::track_group`).
    pub(crate) tracked_groups: Vec<String>,
    #[cfg(feature = "lua")]
    pub(crate) lua_script: Option<PathBuf>,
    /// `Some(seed)` in deterministic mode (`EngineBuilder::deterministic`),
    /// `None` otherwise -- `setup_logic_world` seeds `SimRng` accordingly
    /// (see that resource's doc comment for the single construction path
    /// both branches funnel through).
    pub(crate) deterministic_seed: Option<u64>,
    /// Initial WindowSize mirror values (refreshed per-frame via `InputSample`).
    pub(crate) window_w: i32,
    pub(crate) window_h: i32,
    pub(crate) tx_render: Sender<RenderMsg>,
    pub(crate) rx_logic: Receiver<LogicMsg>,
    /// Receiver for the dedicated bounded input channel.
    pub(crate) rx_input: Receiver<InputSample>,
    /// The sim thread's write end of the snapshot triple buffer.
    /// `Option` so [`EngineBuilder::setup_logic_world`] can `.take()` it into
    /// the logic world's [`SnapshotPublisher`] resource -- `Input<T>` isn't
    /// `Clone`, unlike the `Sender`/`Receiver` fields above.
    pub(crate) snapshot_publisher: Option<SnapshotPublisher>,
    /// `Some` when `.play_replay(path)` was used -- `logic_thread_main`
    /// drives its `sim` ticks from this instead of live `rx_input`/
    /// `rx_logic` collection. `EngineBuilder` configures replay playback and
    /// replay recording as separate modes.
    pub(crate) replay_player: Option<ReplayPlayer>,
    /// `Some` when `.record_replay(path, ..)` was used.
    pub(crate) replay_recorder: Option<ReplayRecorder>,
    /// Test-harness-only: when `true`, [`EngineBuilder::setup_logic_world`]
    /// inserts a stub [`AudioBridge`](aberred_core::protocol::endpoints::AudioBridge)
    /// (no real audio thread) via `setup_audio_stub` instead of `setup_audio`,
    /// stashing the stub's far ends into `audio_stub_ends` for the caller to
    /// retrieve. Always `false` in production (`try_run` never sets it).
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) stub_audio: bool,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) audio_stub_ends: Option<(Receiver<AudioCmd>, Sender<AudioMessage>)>,
}

/// Logic thread entry point. Startup errors can't propagate to
/// `EngineBuilder::try_run` (the thread is already detached from it), so they
/// are logged and converted into a `RenderMsg::Quit` so the render loop exits
/// instead of showing a frozen window.
pub(super) fn logic_thread(init: LogicInit) {
    let tx_render = init.tx_render.clone();
    if let Err(err) = logic_thread_main(init) {
        log::error!("Logic thread failed: {err}");
        let _ = tx_render.send(RenderMsg::Quit);
    }
}

/// Run one `sim` schedule tick and clear `InputState`'s one-shot edge flags
/// (`just_pressed`/`just_released`) afterward, so an edge delivered by the
/// current tick's input sample is consumed exactly once. `active` (held
/// state) is left untouched and freely re-readable every tick. Extracted
/// from `logic_thread_main` so this property stays independently testable
/// without a real `Pacer`/`Instant` drive.
pub(crate) fn run_sim_tick(world: &mut World, sim: &mut Schedule) {
    aberred_core::tracy::tracy_span!("sim_schedule_run");
    sim.run(world);
    world.resource_mut::<InputState>().clear_edges();
}

/// Whether this tick is inside the deterministic envelope:
/// `GameState::Playing`, read at the top of the tick. Setup lasts a
/// wall-clock-dependent number of ticks (it waits for asset I/O), so
/// outside the envelope `WorldTime` doesn't advance and no replay entry is
/// recorded, played or checkpointed. `finish_setup` flips the state at the
/// end of a tick, so the first `Playing` tick is a whole one.
pub(crate) fn in_envelope(world: &World) -> bool {
    matches!(world.resource::<GameState>().get(), GameStates::Playing)
}

/// Keeps live input out of deterministic Setup ticks (see [`in_envelope`]).
/// Outside the envelope it moves `tick_input`'s latched facts
/// (`screen_size`, `capture`, `intents`) into `held` and drops its samples;
/// on the first `Playing` tick it folds `held` back in, so those facts are
/// applied, and recorded, there.
pub(crate) fn hold_back_setup_input(
    tick_input: &mut TickInput,
    held: &mut TickInput,
    playing: bool,
) {
    if playing {
        if !held.is_empty() {
            tick_input.absorb_latched(std::mem::take(held));
        }
    } else {
        held.capture = tick_input.capture.take().or(held.capture);
        held.screen_size = tick_input.screen_size.take().or(held.screen_size);
        held.intents.append(&mut tick_input.intents);
        tick_input.samples.clear();
    }
}

/// [`hold_back_setup_input`] in a `.deterministic()` game
/// ([`DeterministicMode`]); outside one, Setup input applies live.
pub(crate) fn hold_back_deterministic_setup_input(
    world: &World,
    tick_input: &mut TickInput,
    held: &mut TickInput,
) {
    if world.contains_resource::<DeterministicMode>() {
        hold_back_setup_input(tick_input, held, in_envelope(world));
    }
}

/// Collect stage (live source): drains `rx_input`/`rx_logic` for one tick.
/// Non-sim-visible messages (`FontLoaded`/`FontRemoved`/`FontRenamed`,
/// `TextureRemoved`/`TextureRenamed`, `OverlayConfig`) are applied directly
/// to `world` here rather than through `apply_tick_input`. Sim-visible facts
/// (raw samples, capture, `ScreenSize`, `SignalIntent`s) are written into
/// `out` instead of straight into `World`
/// resources, so `out` already holds every sim-visible fact for this tick
/// before `apply_tick_input` consumes it.
///
/// `TextureLoaded` dims land in `TextureDimsStore` directly rather than via
/// `out` — texture dims are not part of `TickInput`'s recorded envelope.
///
/// Returns what [`drain_logic_messages`] saw on `rx_logic`.
fn collect_tick_input_live(
    out: &mut TickInput,
    rx_input: &Receiver<InputSample>,
    rx_logic: &Receiver<LogicMsg>,
    world: &mut World,
) -> DrainOutcome {
    // Read before update_world_time increments frame_count later this tick,
    // so `out.tick` describes "the tick about to run," 0-indexed.
    out.reset(world.resource::<WorldTime>().frame_count);

    // Drain the dedicated bounded input channel: this tick's backlog of raw
    // samples, oldest to newest -- resolve_input_backlog (inside
    // apply_tick_input) processes them sequentially against
    // PrevRawSnapshot (see its doc comment for why merge-then-diff-once
    // can't replace this). `capture` rides with each sample; only the
    // newest matters (one render-frame stale either way).
    for sample in rx_input.try_iter() {
        out.capture = Some(sample.capture);
        out.samples.push(sample.raw);
    }

    drain_logic_messages(Some(out), rx_logic, world)
}

/// What one [`drain_logic_messages`] pass saw on `rx_logic`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrainOutcome {
    /// `LogicMsg::Shutdown` was in the batch.
    shutdown: bool,
    /// Every sender is gone and nothing is left queued. Detected by the
    /// draining `try_recv` itself (`pacing::drain_channel`), so no message
    /// is ever lost to a separate disconnect probe.
    disconnected: bool,
}

/// Applies every non-sim-visible `LogicMsg` (font/texture load/remove/
/// rename, overlay config, replay playback control) directly to `world`,
/// and folds sim-visible `ScreenSize`/`SignalIntent`s into `out`. Shared by
/// the live collector above, the replay path and the paused path in
/// `logic_thread_main` -- asset loads/overlay edits/playback control keep
/// happening for real in all three, regardless of where `out`'s other
/// fields came from.
///
/// `out` is `None` when the caller must NOT let live sim-visible facts reach
/// the sim: during replay playback (every sim-visible fact comes from the
/// file -- a live `LogicMsg::ScreenSize`, which `send_render_mirrors` emits
/// unconditionally on its first frame, would otherwise overwrite the
/// recorded one and guarantee divergence) and while playback is paused (no
/// tick runs, so there is nothing for them to apply to). The non-sim-visible
/// arms still run in both cases.
///
/// Returns whether `LogicMsg::Shutdown` was seen and whether the channel is
/// disconnected.
pub(crate) fn drain_logic_messages(
    mut out: Option<&mut TickInput>,
    rx_logic: &Receiver<LogicMsg>,
    world: &mut World,
) -> DrainOutcome {
    // Drain everything currently queued (non-blocking -- the Pacer already
    // did the waiting).
    let mut shutdown = false;
    let disconnected = aberred_core::pacing::drain_channel(rx_logic, |msg| {
        // Settled after the match, so observers of the load event see the
        // reply's data already stored.
        let change = msg.asset_change();
        let settled = msg.load_reply();
        match msg {
            LogicMsg::ScreenSize { w, h } => {
                if let Some(out) = out.as_deref_mut() {
                    out.screen_size = Some((w, h));
                }
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
            // No logic-side store mirrors shaders, and a failed load leaves
            // the stores untouched; both only go through the shared
            // asset_change/load_reply handling below.
            LogicMsg::ShaderLoaded { .. } | LogicMsg::AssetLoadFailed { .. } => {}
            LogicMsg::TextureRemoved { key } => {
                world.resource_mut::<TextureDimsStore>().remove(&key);
            }
            LogicMsg::FontRemoved { key } => {
                world.resource_mut::<FontMetricsStore>().0.remove(&key);
            }
            LogicMsg::TextureRenamed { old_key, new_key } => {
                world
                    .resource_mut::<TextureDimsStore>()
                    .rename(&old_key, new_key);
            }
            LogicMsg::FontRenamed { old_key, new_key } => {
                world
                    .resource_mut::<FontMetricsStore>()
                    .rename(&old_key, new_key);
            }
            LogicMsg::OverlayConfig(config) => {
                *world.resource_mut::<DebugOverlayConfig>() = config;
            }
            LogicMsg::SignalIntents(intents) => {
                if let Some(out) = out.as_deref_mut() {
                    out.intents.extend(intents);
                }
            }
            LogicMsg::Shutdown => shutdown = true,
            LogicMsg::ReplayControl(ReplayControl::Play) => {
                world.resource_mut::<ReplayRuntimeState>().paused = false;
            }
            LogicMsg::ReplayControl(ReplayControl::Pause) => {
                world.resource_mut::<ReplayRuntimeState>().paused = true;
            }
            LogicMsg::ReplayControl(ReplayControl::FastForward(on)) => {
                world.resource_mut::<ReplayRuntimeState>().fast_forward = on;
            }
        }
        if let Some(change) = change {
            world.resource_mut::<LoadedAssets>().apply(change);
        }
        if let Some(outcome) = settled {
            settle_load(world, outcome);
        }
    });
    DrainOutcome {
        shutdown,
        disconnected,
    }
}

/// Apply one [`TickInput`]'s sim-visible facts to `world`, ahead of that
/// tick's `sim` schedule run. This is the one function every `TickInput`
/// source must go through identically — live (`logic_thread_main`) and
/// replay playback (`ReplayPlayer`) — the determinism contract is "same
/// `TickInput` sequence in -> same state out".
///
/// Order matters: `ScreenSize`/`WindowSize`/`ImguiCaptureMirror` land first,
/// then [`resolve_input_backlog`] (which reads `WindowSize`/
/// `ScreenSize`/the capture mirror while resolving `tick_input.samples`),
/// then queued `SignalIntent`s are appended to the `SignalIntents` buffer
/// for `apply_signal_intents` (`SimSet::ApplyIntents`) to drain once `sim`
/// runs.
pub(crate) fn apply_tick_input(world: &mut World, tick_input: &TickInput) {
    if let Some((w, h)) = tick_input.screen_size {
        let mut screen_size = world.resource_mut::<ScreenSize>();
        screen_size.w = w;
        screen_size.h = h;
    }
    if let Some(newest) = tick_input.samples.last() {
        let mut window_size = world.resource_mut::<WindowSize>();
        window_size.w = newest.window_w;
        window_size.h = newest.window_h;
    }
    if let Some(capture) = tick_input.capture {
        world.resource_mut::<ImguiCaptureMirror>().0 = capture;
    }
    resolve_input_backlog(world, &tick_input.samples);
    world
        .resource_mut::<SignalIntents>()
        .0
        .extend(tick_input.intents.iter().cloned());
}

/// The logic thread's `Pacer`-driven loop: one `sim` tick per `Pacer` wakeup
/// at `[simulation] hz`. Every tick integrates the constant `1.0 / sim_hz`
/// (`Pacer::tick_fixed`, scaled by `time_scale` inside [`update_world_time`])
/// and a stall dilates game time instead of spiking dt -- no catch-up, no
/// replayed substeps.
///
/// A backlog of pending raw input samples (whenever `sim_hz` trails the
/// render frame rate, or a sim stall) is resolved sequentially, oldest to
/// newest, against `PrevRawSnapshot` (`resolve_input_backlog`).
/// Non-input messages just update the logic-side mirrors/stores. The sim
/// still ticks every `Pacer` wakeup even when no input arrived (holding the
/// configured rate through a render stall).
///
/// `present` (build + publish this tick's [`DrawableSnapshot`]) does not
/// run once per received input sample — it's decimated to
/// `[simulation] snapshot_skip`, a sim-tick countdown checked
/// independently of whether input arrived this tick, so the render thread
/// keeps receiving fresh snapshots during input droughts too. Forwarding
/// queued `RenderAssetCmd`s (`forward_render_asset_cmds`) runs on the tail
/// of `sim` rather than on `present`, for the same reason: asset loads must
/// reach the render thread every tick, not just on a publish tick.
fn logic_thread_main(mut init: LogicInit) -> Result<(), EngineError> {
    let sim_hz = init.config.sim_hz;
    let snapshot_skip = init.config.snapshot_skip;

    let mut world = EngineBuilder::setup_logic_world(&mut init)?;
    world.insert_resource(ReplayRuntimeState::default());
    let (mut sim, mut present) = EngineBuilder::init_logic_world(&mut init, &mut world)?;

    let rx_logic = init.rx_logic;
    let rx_input = init.rx_input;
    let mut pacer = Pacer::new(sim_hz);
    // Computed once and reused every tick -- derived from the Pacer's own
    // period rather than re-deriving `1.0 / sim_hz` independently, so
    // there's a single source of truth for the period (must be
    // bit-identical everywhere it's used, since dt is always this fixed
    // constant, never a measured value).
    let sim_period_f32 = pacer.period_secs_f32();
    // Decimates `present`/snapshot publishing independently of the sim's
    // own pacing above: a countdown of sim ticks rather than a second
    // wall-clock Pacer, so publish cadence scales with actual sim ticking
    // instead of holding an independent wall-clock rate. Fires on the first
    // tick, then every `snapshot_skip + 1` ticks thereafter.
    let mut present_countdown = TickCountdown::new(snapshot_skip);
    // Rolls up sim-tick work time (run_sim_tick only, not the
    // pacer's sleep) into SimStats once per ~1s window, for the F11 perf
    // panel. Input-backlog sum/max share this same window -- averaged
    // against ThreadStats::ticks on rollover rather than a separately
    // maintained tick counter, since record() is called exactly once per
    // sim tick below and would otherwise need to be kept in lockstep.
    let mut stats_window = StatsWindow::new(sim_hz);
    let mut backlog_sum: u64 = 0;
    let mut backlog_max: u32 = 0;
    // Reused across ticks instead of a fresh TickInput per tick -- see
    // TickInput::reset's doc comment for the allocation-reuse rationale.
    let mut tick_input = TickInput::default();
    // Latched input facts held back during deterministic Setup ticks; see
    // `hold_back_setup_input`.
    let mut held_setup_input = TickInput::default();

    // Replay wiring. `source`/`recorder` are independent -- `EngineBuilder`
    // exposes one mode at a time, but nothing here requires that.
    let mut source = match init.replay_player.take() {
        Some(player) => TickInputSource::Replay(player),
        None => TickInputSource::Live,
    };
    let mut recorder = init.replay_recorder.take();
    let mut replay_ended_sent = false;
    // Fires once per ~1s of sim ticks, same cadence as StatsWindow -- a
    // full-world hash every tick would be needlessly expensive; checkpoints
    // only need to catch divergence within about a second of it happening.
    // `want_checkpoints` short-circuits ahead of `due()` below, so outside a
    // recording/playback session this countdown never advances at all -- its
    // value is unused there either way.
    let mut checkpoint_countdown = TickCountdown::new((sim_hz.round() as u32).saturating_sub(1));
    let want_checkpoints = recorder.is_some() || matches!(source, TickInputSource::Replay(_));

    'main: loop {
        if !aberred_core::protocol::shutdown::running() {
            break 'main;
        }
        let replay_state = world.resource::<ReplayRuntimeState>();
        if replay_state.fast_forward {
            pacer.skip_to_now();
        } else {
            pacer.tick_fixed();
        }
        let dt = sim_period_f32;

        if replay_state.paused {
            // Still service the channels (shutdown, replay control, asset
            // replies) so a paused session stays responsive; no sim/present
            // work runs. `None`: no tick will run, so there is nothing for a
            // live ScreenSize/SignalIntent to apply to -- collecting them
            // here would only pile them into `tick_input` for the next
            // `reset` to discard.
            let drained = drain_logic_messages(None, &rx_logic, &mut world);
            if drained.shutdown || drained.disconnected {
                break 'main;
            }
            world.clear_trackers();
            continue 'main;
        }

        let playing = in_envelope(&world);

        // Collect stage: &tick_input holds every sim-visible fact this tick,
        // loss-free, once this call returns -- that's what makes it the one
        // thing the recorder has to capture (it does, just below the break
        // checks) and the one thing apply_tick_input has to consume.
        let drained = match &mut source {
            TickInputSource::Live => {
                collect_tick_input_live(&mut tick_input, &rx_input, &rx_logic, &mut world)
            }
            TickInputSource::Replay(player) => {
                // Live input is discarded outright during playback -- the
                // brainstorm doc's "drained and discarded by the sim" rule.
                // That covers rx_input here and rx_logic's sim-visible arms
                // via the `None` below; every sim-visible fact this tick
                // comes from the file, nothing merges on top of it.
                //
                // Deliberately AHEAD of the shutdown/disconnect breaks below
                // (unlike record_tick, which sits after them): on the final
                // tick of a session that breaks out, the player consumes one
                // entry that never gets simulated. Harmless while
                // record+play are mutually exclusive -- moving collect below
                // the breaks would instead desync the checkpoint stream from
                // the tick counter, which is worse.
                //
                // Setup runs live (its loads, its length) and reads no
                // entry: the recording starts at the first Playing tick.
                for _ in rx_input.try_iter() {}
                let tick = world.resource::<WorldTime>().frame_count;
                if playing {
                    player.collect(&mut tick_input, tick);
                } else {
                    tick_input.reset(tick);
                }
                if player.is_finished() && !replay_ended_sent {
                    replay_ended_sent = true;
                    let tx_render = world.resource::<RenderTx>().0.clone();
                    let _ = tx_render.send(RenderMsg::ReplayEnded);
                }
                drain_logic_messages(None, &rx_logic, &mut world)
            }
        };

        if drained.shutdown {
            // Exits immediately (no sim/present work runs after it).
            // collect_tick_input_live stashed this batch's SignalIntents
            // into tick_input rather than the SignalIntents resource
            // directly (apply_tick_input's job, which we're skipping) --
            // flush them explicitly before running apply_signal_intents
            // once, instead of running a full simulation tick just to
            // reach it inside `sim`.
            world
                .resource_mut::<SignalIntents>()
                .0
                .append(&mut tick_input.intents);
            let _ = world.run_system_once(apply_signal_intents);
            break 'main;
        }

        if drained.disconnected {
            break 'main;
        }

        // Recorder tap point: strictly after the break checks above, so the
        // file only ever contains ticks that actually ran (a batch carrying
        // both a SignalIntent and Shutdown would otherwise be recorded and
        // then replayed into a tick the original session never simulated).
        hold_back_deterministic_setup_input(&world, &mut tick_input, &mut held_setup_input);
        if playing && let Some(rec) = recorder.as_mut() {
            rec.record_tick(&tick_input);
        }

        // Apply stage. F10 edge (post imgui-capture-mask) means the
        // caller, not resolve_input_backlog, ships
        // RenderMsg::ToggleFullscreen -- see that fn's doc comment for why
        // it stays free of channel sends.
        apply_tick_input(&mut world, &tick_input);
        if world
            .resource::<InputState>()
            .action(InputAction::ToggleFullscreen)
            .just_pressed
        {
            let tx_render = world.resource::<RenderTx>().0.clone();
            let _ = tx_render.send(RenderMsg::ToggleFullscreen);
        }

        // The sim ticks every Pacer wakeup regardless of whether new input
        // arrived this tick, holding the configured `sim_hz` through a
        // render stall (mirrors the old "Timeout => run FIXED only" arm).
        if playing {
            update_world_time(&mut world, dt);
        }
        let tick_start = std::time::Instant::now();
        run_sim_tick(&mut world, &mut sim);
        let tick_work = tick_start.elapsed();

        if playing && want_checkpoints && checkpoint_countdown.due() {
            let hash = hash_world_state(&world);
            if let Some(rec) = recorder.as_mut() {
                rec.record_checkpoint(tick_input.tick, hash);
            }
            if let TickInputSource::Replay(player) = &mut source
                && let Some((expected, actual)) = player.verify_checkpoint(tick_input.tick, hash)
            {
                log::error!(
                    "Replay diverged at tick {}: expected hash {expected:#x}, got {actual:#x}",
                    tick_input.tick
                );
                let tx_render = world.resource::<RenderTx>().0.clone();
                let _ = tx_render.send(RenderMsg::ReplayDiverged {
                    tick: tick_input.tick,
                    expected,
                    actual,
                });
            }
        }

        backlog_sum += tick_input.samples.len() as u64;
        backlog_max = backlog_max.max(tick_input.samples.len() as u32);
        if let Some(thread) = stats_window.record(tick_work) {
            *world.resource_mut::<SimStats>() = SimStats {
                thread,
                input_backlog_avg: backlog_sum as f32 / thread.ticks as f32,
                input_backlog_max: backlog_max,
            };
            backlog_sum = 0;
            backlog_max = 0;
        }

        // `present` runs every `snapshot_skip + 1` sim ticks, not once
        // per received input sample -- independent of whether input arrived
        // this tick, so the render thread keeps receiving fresh snapshots
        // during input droughts too. `present` sees the same real dt this
        // tick measured (no separate render-frame-delta override) -- that
        // dt is captured into the snapshot for render-side use (shader time
        // uniforms, perf panel), even on ticks that don't publish.
        if present_countdown.due() {
            aberred_core::tracy::tracy_span!("present_schedule_run");
            present.run(&mut world);
        }

        world.clear_trackers();
    }

    // Logic owns the audio bridge: stop the audio thread before this world
    // (and the LuaRuntime pinned to this thread) drops.
    finalize_recorder(&world, recorder);
    shutdown_audio(&mut world);
    Ok(())
}

/// Finalize a `ReplayRecorder` (if one is active) on every
/// `logic_thread_main` exit path -- a file left without its closing
/// `ReplayEntry::End` is a truncated replay. The closing `final_hash` is
/// recomputed from the live world here rather than reused from the periodic
/// checkpoint cache -- shutdown can happen between checkpoints, and
/// `ReplayEntry::End` promises the true terminal world hash. `recorder` is
/// consumed (its `finish` takes `self`), hence a free fn rather than a
/// method taking `&mut Option<..>`.
fn finalize_recorder(world: &World, recorder: Option<ReplayRecorder>) {
    if let Some(rec) = recorder
        && let Err(e) = rec.finish(hash_world_state(world))
    {
        log::error!("replay recorder: failed to finalize replay file: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{self, BufReader, Read};
    use std::path::Path;

    use tempfile::NamedTempFile;

    use aberred_core::components::mapposition::MapPosition;
    use aberred_core::protocol::raw_input::{ImguiCaptureState, RawDeviceSnapshot};
    use aberred_core::protocol::replay::{
        REPLAY_FORMAT_VERSION, REPLAY_MAGIC, ReplayEntry, ReplayHeader,
    };
    use aberred_core::resources::pending_assets::PendingAssets;
    use aberred_core::resources::signal_intents::SignalIntent;
    use aberred_core::resources::sim_rng::SimRng;
    use aberred_core::resources::worldsignals::WorldSignals;

    fn test_replay_header() -> ReplayHeader {
        ReplayHeader {
            magic: REPLAY_MAGIC,
            format_version: REPLAY_FORMAT_VERSION,
            engine_build_id: "test".into(),
            seed: 1,
            sim_hz: 240.0,
            config_digest: 0,
            scene_id: "".into(),
            game_version: "test".into(),
        }
    }

    fn read_len_prefixed<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        Ok(Some(buf))
    }

    fn read_replay_end(path: &Path) -> ReplayEntry {
        let mut reader = BufReader::new(File::open(path).unwrap());
        let header = read_len_prefixed(&mut reader)
            .unwrap()
            .expect("replay file must contain a header");
        let _: ReplayHeader = postcard::from_bytes(&header).unwrap();

        let mut last = None;
        while let Some(bytes) = read_len_prefixed(&mut reader).unwrap() {
            last = Some(postcard::from_bytes::<ReplayEntry>(&bytes).unwrap());
        }

        last.expect("replay file must end with a ReplayEntry::End")
    }

    /// `collect_tick_input_live` stashes SignalIntents into `TickInput`
    /// rather than writing them straight to the `SignalIntents` resource, so
    /// the shutdown branch in `logic_thread_main` must explicitly flush
    /// `tick_input.intents` before running `apply_signal_intents` -- a
    /// `SignalIntent` queued in the same batch as `Shutdown` must not be
    /// silently dropped. This test exercises the collect half directly
    /// (the smallest unit that can regress): a batch containing both a
    /// `SignalIntents` message and `Shutdown` must report `shutdown` AND
    /// leave the intent recoverable in `tick_input.intents` (i.e. NOT
    /// already lost by the time the caller decides to shut down).
    #[test]
    fn shutdown_batch_preserves_signal_intents_in_tick_input() {
        let (tx_input, rx_input) = crossbeam_channel::unbounded::<InputSample>();
        let (tx_logic, rx_logic) = crossbeam_channel::unbounded::<LogicMsg>();
        drop(tx_input);

        tx_logic
            .send(LogicMsg::SignalIntents(vec![SignalIntent::SetFlag(
                "shutdown_batch_intent".into(),
            )]))
            .unwrap();
        tx_logic.send(LogicMsg::Shutdown).unwrap();

        let mut world = World::new();
        world.insert_resource(WorldTime::default());
        let mut tick_input = TickInput::default();
        let drained = collect_tick_input_live(&mut tick_input, &rx_input, &rx_logic, &mut world);

        assert!(drained.shutdown);
        assert_eq!(
            tick_input.intents,
            vec![SignalIntent::SetFlag("shutdown_batch_intent".into())],
            "a SignalIntent queued in the same batch as Shutdown must survive \
             into tick_input, so the shutdown branch can flush it into \
             SignalIntents before tearing down -- losing it here is exactly \
             the regression this test guards against"
        );
    }

    /// The `None` counterpart of the test above: during replay playback (and
    /// while paused) the same batch must leave `tick_input` completely alone.
    /// Live sim-visible facts merging on top of the recorded ones is what
    /// made every playback session diverge -- `send_render_mirrors` emits a
    /// `ScreenSize` on its first frame unconditionally, so this fired without
    /// anyone touching the window.
    #[test]
    fn draining_without_a_tick_input_discards_sim_visible_facts() {
        let (tx_logic, rx_logic) = crossbeam_channel::unbounded::<LogicMsg>();
        tx_logic
            .send(LogicMsg::SignalIntents(vec![SignalIntent::SetFlag(
                "live_click_during_playback".into(),
            )]))
            .unwrap();
        tx_logic
            .send(LogicMsg::ScreenSize { w: 1280, h: 720 })
            .unwrap();
        drop(tx_logic);

        let mut world = World::new();
        let mut tick_input = TickInput::default();
        tick_input.samples.push(Default::default());
        let recorded = tick_input.clone();

        let drained = drain_logic_messages(None, &rx_logic, &mut world);

        assert!(!drained.shutdown);
        assert_eq!(
            tick_input, recorded,
            "a live SignalIntent/ScreenSize must not reach the sim during \
             playback -- every sim-visible fact comes from the replay file"
        );
    }

    /// Render's shutdown sends `Shutdown` and then joins while still holding
    /// its sender, so a drain must never let a separate disconnect probe eat
    /// the last queued messages: they must be applied AND the disconnect
    /// reported by the same pass.
    #[test]
    fn drain_applies_messages_queued_before_disconnect_and_reports_both() {
        let (tx_logic, rx_logic) = crossbeam_channel::unbounded::<LogicMsg>();
        let mut world = World::new();
        world.init_resource::<FontMetricsStore>();
        world.init_resource::<PendingAssets>();
        world.init_resource::<LoadedAssets>();

        let first = drain_logic_messages(None, &rx_logic, &mut world);
        assert_eq!(first, DrainOutcome::default());

        tx_logic
            .send(LogicMsg::FontLoaded {
                key: "late".into(),
                metrics: Default::default(),
            })
            .unwrap();
        tx_logic.send(LogicMsg::Shutdown).unwrap();
        drop(tx_logic);

        let outcome = drain_logic_messages(None, &rx_logic, &mut world);
        assert_eq!(
            outcome,
            DrainOutcome {
                shutdown: true,
                disconnected: true
            }
        );
        assert!(world.resource::<FontMetricsStore>().0.contains_key("late"));
    }

    /// Deterministic Setup ticks run with no input; their latched facts land
    /// on the first `Playing` tick, which is the first one recorded.
    #[test]
    fn setup_input_is_held_back_until_the_first_playing_tick() {
        let mut held = TickInput::default();
        let mut tick_input = TickInput {
            tick: 0,
            samples: vec![sample_with_window_w(1)],
            capture: None,
            intents: vec![SignalIntent::SetFlag("during_setup".into())],
            screen_size: Some((320, 240)),
        };
        hold_back_setup_input(&mut tick_input, &mut held, false);
        assert!(tick_input.is_empty(), "a Setup tick sees no input");
        assert_eq!(tick_input.tick, 0);

        tick_input.screen_size = Some((640, 480));
        hold_back_setup_input(&mut tick_input, &mut held, false);
        assert!(tick_input.is_empty());

        tick_input.samples = vec![sample_with_window_w(2)];
        tick_input.intents = vec![SignalIntent::SetFlag("playing".into())];
        hold_back_setup_input(&mut tick_input, &mut held, true);
        assert_eq!(tick_input.screen_size, Some((640, 480)), "the latest size");
        assert_eq!(
            tick_input.intents,
            [
                SignalIntent::SetFlag("during_setup".into()),
                SignalIntent::SetFlag("playing".into())
            ]
        );
        let widths: Vec<i32> = tick_input.samples.iter().map(|s| s.window_w).collect();
        assert_eq!(widths, [2], "Setup samples are dropped");
        assert!(held.is_empty());
    }

    /// World with every resource the world-applied `LogicMsg` arms touch.
    fn drain_world() -> World {
        let mut world = World::new();
        world.init_resource::<FontMetricsStore>();
        world.init_resource::<PendingAssets>();
        world.init_resource::<LoadedAssets>();
        world.init_resource::<TextureDimsStore>();
        world.init_resource::<DebugOverlayConfig>();
        world.init_resource::<ReplayRuntimeState>();
        world.init_resource::<GameState>();
        world
    }

    fn drain_msgs(world: &mut World, msgs: Vec<LogicMsg>) -> DrainOutcome {
        let (tx, rx) = crossbeam_channel::unbounded::<LogicMsg>();
        for msg in msgs {
            tx.send(msg).unwrap();
        }
        drain_logic_messages(None, &rx, world)
    }

    /// Asset bookkeeping, overlay config and replay control apply to the
    /// world even with `out = None` (replay playback / paused), and a message
    /// behind `Shutdown` in the same batch is still applied.
    #[test]
    fn world_applied_messages_land_without_a_tick_input() {
        let mut world = drain_world();
        {
            let mut fonts = world.resource_mut::<FontMetricsStore>();
            fonts.0.insert("old_f".into(), Default::default());
            fonts.0.insert("gone_f".into(), Default::default());
        }
        {
            let mut dims = world.resource_mut::<TextureDimsStore>();
            dims.insert("old_t", 1, 2);
            dims.insert("gone_t", 3, 4);
        }
        let overlay = DebugOverlayConfig {
            show_collider_boxes: false,
            ..Default::default()
        };

        let outcome = drain_msgs(
            &mut world,
            vec![
                LogicMsg::FontLoaded {
                    key: "new_f".into(),
                    metrics: Default::default(),
                },
                LogicMsg::FontRemoved {
                    key: "gone_f".into(),
                },
                LogicMsg::FontRenamed {
                    old_key: "old_f".into(),
                    new_key: "ren_f".into(),
                },
                LogicMsg::TextureLoaded {
                    key: "new_t".into(),
                    width: 5,
                    height: 6,
                },
                LogicMsg::TextureRemoved {
                    key: "gone_t".into(),
                },
                LogicMsg::TextureRenamed {
                    old_key: "old_t".into(),
                    new_key: "ren_t".into(),
                },
                LogicMsg::OverlayConfig(overlay.clone()),
                LogicMsg::ReplayControl(ReplayControl::Pause),
                LogicMsg::ReplayControl(ReplayControl::FastForward(true)),
                LogicMsg::Shutdown,
                LogicMsg::FontLoaded {
                    key: "after_shutdown".into(),
                    metrics: Default::default(),
                },
            ],
        );

        assert!(outcome.shutdown);
        let mut fonts: Vec<&str> = world
            .resource::<FontMetricsStore>()
            .0
            .keys()
            .map(String::as_str)
            .collect();
        fonts.sort_unstable();
        assert_eq!(fonts, ["after_shutdown", "new_f", "ren_f"]);
        let dims = world.resource::<TextureDimsStore>();
        assert_eq!(dims.get("new_t"), Some((5, 6)));
        assert_eq!(dims.get("ren_t"), Some((1, 2)));
        assert_eq!(dims.get("old_t"), None);
        assert_eq!(dims.get("gone_t"), None);
        assert_eq!(*world.resource::<DebugOverlayConfig>(), overlay);
        let replay = world.resource::<ReplayRuntimeState>();
        assert!(replay.paused && replay.fast_forward);
    }

    #[test]
    fn replay_play_and_fast_forward_off_undo_pause_and_fast_forward() {
        let mut world = drain_world();
        *world.resource_mut::<ReplayRuntimeState>() = ReplayRuntimeState {
            paused: true,
            fast_forward: true,
        };
        drain_msgs(
            &mut world,
            vec![
                LogicMsg::ReplayControl(ReplayControl::Play),
                LogicMsg::ReplayControl(ReplayControl::FastForward(false)),
            ],
        );
        let replay = world.resource::<ReplayRuntimeState>();
        assert!(!replay.paused && !replay.fast_forward);
    }

    fn sample_with_window_w(window_w: i32) -> RawDeviceSnapshot {
        RawDeviceSnapshot {
            window_w,
            ..Default::default()
        }
    }

    #[test]
    fn collect_replaces_stale_tick_input_with_this_ticks_facts() {
        let (tx_input, rx_input) = crossbeam_channel::unbounded::<InputSample>();
        let (tx_logic, rx_logic) = crossbeam_channel::unbounded::<LogicMsg>();
        let newest_capture = ImguiCaptureState {
            mouse: false,
            keyboard: true,
        };
        for (w, capture) in [
            (
                1,
                ImguiCaptureState {
                    mouse: true,
                    keyboard: false,
                },
            ),
            (2, ImguiCaptureState::default()),
            (3, newest_capture),
        ] {
            tx_input
                .send(InputSample {
                    raw: sample_with_window_w(w),
                    capture,
                })
                .unwrap();
        }
        tx_logic
            .send(LogicMsg::ScreenSize { w: 320, h: 240 })
            .unwrap();
        tx_logic
            .send(LogicMsg::SignalIntents(vec![SignalIntent::SetFlag(
                "new".into(),
            )]))
            .unwrap();
        tx_logic
            .send(LogicMsg::TextureLoaded {
                key: "player".into(),
                width: 1,
                height: 1,
            })
            .unwrap();

        let mut world = drain_world();
        world.insert_resource(WorldTime {
            frame_count: 7,
            ..Default::default()
        });
        let mut tick_input = TickInput {
            tick: 99,
            samples: vec![sample_with_window_w(-1)],
            capture: Some(ImguiCaptureState::default()),
            intents: vec![SignalIntent::SetFlag("stale".into())],
            screen_size: Some((1, 1)),
        };

        collect_tick_input_live(&mut tick_input, &rx_input, &rx_logic, &mut world);

        assert_eq!(tick_input.tick, 7);
        let widths: Vec<i32> = tick_input.samples.iter().map(|s| s.window_w).collect();
        assert_eq!(
            widths,
            [1, 2, 3],
            "samples in arrival order, stale ones gone"
        );
        assert_eq!(tick_input.capture, Some(newest_capture));
        assert_eq!(tick_input.intents, [SignalIntent::SetFlag("new".into())]);
        assert_eq!(tick_input.screen_size, Some((320, 240)));
        assert_eq!(
            world.resource::<TextureDimsStore>().get("player"),
            Some((1, 1)),
            "world-applied replies land during collect"
        );
    }

    #[test]
    fn apply_tick_input_sets_mirrors_and_appends_intents() {
        let mut tw = crate::test_support::TestWorld::new();
        let world = &mut tw.world;
        world.resource_mut::<SignalIntents>().0 = vec![SignalIntent::SetFlag("queued".into())];
        let capture = ImguiCaptureState {
            mouse: true,
            keyboard: true,
        };

        apply_tick_input(
            world,
            &TickInput {
                tick: 0,
                samples: vec![sample_with_window_w(100), sample_with_window_w(200)],
                capture: Some(capture),
                intents: vec![SignalIntent::SetFlag("tick".into())],
                screen_size: Some((640, 360)),
            },
        );

        let check = |world: &World| {
            let screen = world.resource::<ScreenSize>();
            assert_eq!((screen.w, screen.h), (640, 360));
            assert_eq!(world.resource::<WindowSize>().w, 200, "newest sample wins");
            assert_eq!(world.resource::<ImguiCaptureMirror>().0, capture);
        };
        check(world);
        assert_eq!(
            world.resource::<SignalIntents>().0,
            [
                SignalIntent::SetFlag("queued".into()),
                SignalIntent::SetFlag("tick".into())
            ]
        );

        // An empty tick changes none of the mirrors and queues nothing.
        apply_tick_input(world, &TickInput::default());
        check(world);
        assert_eq!(world.resource::<SignalIntents>().0.len(), 2);
    }

    #[test]
    fn finalize_recorder_hashes_live_world_in_end_entry() {
        let file = NamedTempFile::new().unwrap();
        let recorder = ReplayRecorder::create(file.path(), &test_replay_header()).unwrap();
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world.insert_resource(WorldTime::default());
        world.insert_resource(SimRng::from_seed(1));
        let entity = world.spawn(MapPosition::new(1.0, 2.0)).id();

        let stale_checkpoint_hash = hash_world_state(&world);
        world
            .entity_mut(entity)
            .get_mut::<MapPosition>()
            .unwrap()
            .set_x(99.0);
        let expected_final_hash = hash_world_state(&world);
        assert_ne!(
            stale_checkpoint_hash, expected_final_hash,
            "the regression needs a world change after the stale checkpoint hash"
        );

        finalize_recorder(&world, Some(recorder));

        let ReplayEntry::End { final_hash, .. } = read_replay_end(file.path()) else {
            panic!("replay file must end with ReplayEntry::End");
        };
        assert_eq!(
            final_hash, expected_final_hash,
            "the replay trailer must hash the live shutdown world, not a stale checkpoint value"
        );
        assert_ne!(
            final_hash, stale_checkpoint_hash,
            "the replay trailer must not reuse the last periodic checkpoint hash when the world changed afterward"
        );
    }

    /// Without an active recorder nothing is hashed: an empty world (no
    /// resources `hash_world_state` needs) must not be touched.
    #[test]
    fn finalize_without_a_recorder_does_nothing() {
        finalize_recorder(&World::new(), None);
    }
}
