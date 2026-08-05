#[cfg(feature = "lua")]
use std::path::PathBuf;

use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use crossbeam_channel::{Receiver, Sender};

use super::builder::EngineBuilder;
use super::registrar::{HookRegistrar, ObserverRegistrar, UpdateRegistrar};
use super::replay::{ReplayPlayer, ReplayRecorder};
use crate::error::EngineError;
use crate::pacing::{Pacer, StatsWindow, TickCountdown};
#[cfg(any(test, feature = "test-support"))]
use crate::protocol::audio::{AudioCmd, AudioMessage};
use crate::protocol::endpoints::RenderTx;
use crate::protocol::endpoints::shutdown_audio;
use crate::protocol::raw_input::InputSample;
use crate::protocol::render_logic::{LogicMsg, RenderMsg, ReplayControl};
use crate::protocol::snapshot::SnapshotPublisher;
use crate::protocol::tick_input::TickInput;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::determinism_taint::DeterminismTaint;
use crate::resources::fontmetrics::FontMetricsStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::gamestate::{GameState, GameStates};
use crate::resources::input::InputState;
use crate::resources::rawinput::ImguiCaptureMirror;
use crate::resources::screensize::ScreenSize;
use crate::resources::signal_intents::SignalIntents;
use crate::resources::texturedims::TextureDimsStore;
use crate::resources::thread_stats::SimStats;
use crate::resources::windowsize::WindowSize;
use crate::resources::worldtime::WorldTime;
use crate::systems::input::resolve_input_backlog;
use crate::systems::scene_dispatch::SceneDescriptor;
use crate::systems::signal_intents::apply_signal_intents;
use crate::systems::state_hash::hash_world_state;
use crate::systems::time::update_world_time;

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
    pub(crate) enter_play_hook: Option<HookRegistrar>,
    pub(crate) switch_scene_hook: Option<HookRegistrar>,
    pub(crate) update_hook: Option<UpdateRegistrar>,
    pub(crate) extra_systems: Vec<UpdateRegistrar>,
    pub(crate) extra_observers: Vec<ObserverRegistrar>,
    pub(crate) scenes: Vec<(String, SceneDescriptor)>,
    pub(crate) initial_scene: Option<String>,
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
    /// inserts a stub [`AudioBridge`](crate::protocol::endpoints::AudioBridge)
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
    crate::tracy::tracy_span!("sim_schedule_run");
    sim.run(world);
    world.resource_mut::<InputState>().clear_edges();
}

/// In deterministic mode, treat a `TextureDimsStore` key arriving for the
/// *first* time while `GameState::Playing` as a determinism hazard: it means
/// texture metadata arrives during gameplay instead of before the playing
/// state begins. Logs an `error!` and marks [`DeterminismTaint`] without
/// blocking the insert. No-op outside deterministic mode.
fn guard_texture_preload(world: &mut World, key: &str, deterministic: bool) {
    if !deterministic {
        return;
    }
    if !matches!(world.resource::<GameState>().get(), GameStates::Playing) {
        return;
    }
    if world.resource::<TextureDimsStore>().get(key).is_some() {
        return;
    }
    log::error!(
        "Deterministic mode: TextureDimsStore gained new key {key:?} while \
         GameState::Playing -- likely a texture loaded outside the preload \
         window; session tainted"
    );
    world.resource_mut::<DeterminismTaint>().taint();
}

/// Collect stage (live source): drains `rx_input`/`rx_logic` for one tick.
/// Non-sim-visible messages (`FontLoaded`/`FontRemoved`/`FontRenamed`,
/// `TextureRemoved`/`TextureRenamed`, `OverlayConfig`) are applied directly
/// to `world` here rather than through `apply_tick_input`. Sim-visible facts
/// (raw samples, capture, `ScreenSize`, `SignalIntent`s, `TextureLoaded`
/// dims) are written into `out` instead of straight into `World`
/// resources, so `out` already holds every sim-visible fact for this tick
/// before `apply_tick_input` consumes it.
///
/// `TextureLoaded` dims land in `TextureDimsStore` directly rather than via
/// `out` — texture dims are not part of `TickInput`'s recorded envelope.
///
/// Returns `true` if `LogicMsg::Shutdown` was seen this batch.
fn collect_tick_input_live(
    out: &mut TickInput,
    rx_input: &Receiver<InputSample>,
    rx_logic: &Receiver<LogicMsg>,
    world: &mut World,
    deterministic: bool,
) -> bool {
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

    drain_logic_messages(Some(out), rx_logic, world, deterministic)
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
/// Returns `true` if `LogicMsg::Shutdown` was seen.
fn drain_logic_messages(
    mut out: Option<&mut TickInput>,
    rx_logic: &Receiver<LogicMsg>,
    world: &mut World,
    deterministic: bool,
) -> bool {
    // Drain everything currently queued (non-blocking -- the Pacer already
    // did the waiting).
    let mut shutdown_requested = false;
    for msg in rx_logic.try_iter() {
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
                guard_texture_preload(world, &key, deterministic);
                world
                    .resource_mut::<TextureDimsStore>()
                    .insert(key, width, height);
            }
            LogicMsg::TextureRemoved { key } => {
                world.resource_mut::<TextureDimsStore>().remove(&key);
            }
            LogicMsg::FontRemoved { key } => {
                world.resource_mut::<FontMetricsStore>().0.remove(&key);
            }
            LogicMsg::TextureRenamed { old_key, new_key } => {
                // Not guarded by guard_texture_preload: this renames an
                // already-loaded texture's key, not new texture metadata
                // arriving during gameplay. The preload guard applies only
                // to `TextureLoaded`.
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
            LogicMsg::Shutdown => shutdown_requested = true,
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
    }
    shutdown_requested
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
    let use_scene_manager = !init.scenes.is_empty();
    #[cfg(feature = "lua")]
    let has_lua = init.lua_script.is_some();
    #[cfg(not(feature = "lua"))]
    let has_lua = false;

    let sim_hz = init.config.sim_hz;
    let snapshot_skip = init.config.snapshot_skip;
    let deterministic = init.deterministic_seed.is_some();

    let mut world = EngineBuilder::setup_logic_world(&mut init)?;
    world.insert_resource(ReplayRuntimeState::default());
    EngineBuilder::register_logic_systems(&mut init, &mut world, use_scene_manager)?;
    EngineBuilder::spawn_observers(
        &mut world,
        has_lua,
        std::mem::take(&mut init.extra_observers),
    );

    let (mut sim, mut present) = EngineBuilder::build_logic_schedules(
        init.update_hook.take(),
        std::mem::take(&mut init.extra_systems),
        &mut world,
        has_lua,
        use_scene_manager,
    )?;

    let rx_logic = init.rx_logic;
    let rx_input = init.rx_input;
    let mut pacer = Pacer::new(sim_hz);
    // Computed once and reused every tick -- derived from the Pacer's own
    // period rather than re-deriving `1.0 / sim_hz` independently, so
    // there's a single source of truth for the period (must be
    // bit-identical everywhere it's used, determinism-01-fixed-timestep.md).
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
        if !crate::protocol::shutdown::running() {
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
            let shutdown_requested =
                drain_logic_messages(None, &rx_logic, &mut world, deterministic);
            if shutdown_requested {
                break 'main;
            }
            if crate::pacing::channel_disconnected(&rx_logic) {
                break 'main;
            }
            world.clear_trackers();
            continue 'main;
        }

        // Collect stage: &tick_input holds every sim-visible fact this tick,
        // loss-free, once this call returns -- that's what makes it the one
        // thing the recorder has to capture (it does, just below the break
        // checks) and the one thing apply_tick_input has to consume.
        let shutdown_requested = match &mut source {
            TickInputSource::Live => collect_tick_input_live(
                &mut tick_input,
                &rx_input,
                &rx_logic,
                &mut world,
                deterministic,
            ),
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
                for _ in rx_input.try_iter() {}
                player.collect(&mut tick_input, world.resource::<WorldTime>().frame_count);
                if player.is_finished() && !replay_ended_sent {
                    replay_ended_sent = true;
                    let tx_render = world.resource::<RenderTx>().0.clone();
                    let _ = tx_render.send(RenderMsg::ReplayEnded);
                }
                drain_logic_messages(None, &rx_logic, &mut world, deterministic)
            }
        };

        if shutdown_requested {
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

        if crate::pacing::channel_disconnected(&rx_logic) {
            break 'main;
        }

        // Recorder tap point: strictly after the break checks above, so the
        // file only ever contains ticks that actually ran (a batch carrying
        // both a SignalIntent and Shutdown would otherwise be recorded and
        // then replayed into a tick the original session never simulated).
        if let Some(rec) = recorder.as_mut() {
            rec.record_tick(&tick_input);
        }

        // Apply stage. F10 edge (post imgui-capture-mask) means the
        // caller, not resolve_input_backlog, ships
        // RenderMsg::ToggleFullscreen -- see that fn's doc comment for why
        // it stays free of channel sends.
        apply_tick_input(&mut world, &tick_input);
        if world
            .resource::<InputState>()
            .fullscreen_toggle
            .just_pressed
        {
            let tx_render = world.resource::<RenderTx>().0.clone();
            let _ = tx_render.send(RenderMsg::ToggleFullscreen);
        }

        // The sim ticks every Pacer wakeup regardless of whether new input
        // arrived this tick, holding the configured `sim_hz` through a
        // render stall (mirrors the old "Timeout => run FIXED only" arm).
        update_world_time(&mut world, dt);
        let tick_start = std::time::Instant::now();
        run_sim_tick(&mut world, &mut sim);
        let tick_work = tick_start.elapsed();

        if want_checkpoints && checkpoint_countdown.due() {
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
            crate::tracy::tracy_span!("present_schedule_run");
            present.run(&mut world);
        }

        world.clear_trackers();
    }

    // Logic owns the audio bridge: stop the audio thread before this world
    // (and the LuaRuntime pinned to this thread) drops.
    let tainted = world.resource::<DeterminismTaint>().is_tainted();
    finalize_recorder(&world, recorder, tainted);
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
/// method taking `&mut Option<..>`. `tainted` carries this session's
/// [`DeterminismTaint`] into the file, so a later playback can say up front
/// that the *recording* was already known non-reproducible.
fn finalize_recorder(world: &World, recorder: Option<ReplayRecorder>, tainted: bool) {
    if let Some(rec) = recorder
        && let Err(e) = rec.finish(hash_world_state(world), tainted)
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

    use crate::components::mapposition::MapPosition;
    use crate::protocol::replay::{REPLAY_FORMAT_VERSION, REPLAY_MAGIC, ReplayEntry, ReplayHeader};
    use crate::resources::signal_intents::SignalIntent;
    use crate::resources::sim_rng::SimRng;
    use crate::resources::worldsignals::WorldSignals;

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

    /// Regression test for the bug this refactor could have introduced:
    /// once collect_tick_input_live stashes SignalIntents into `TickInput`
    /// instead of writing them straight to the `SignalIntents` resource,
    /// the shutdown branch in `logic_thread_main` must explicitly flush
    /// `tick_input.intents` before running `apply_signal_intents` -- a
    /// `SignalIntent` queued in the same batch as `Shutdown` must not be
    /// silently dropped. This test exercises the collect half directly
    /// (the smallest unit that can regress): a batch containing both a
    /// `SignalIntents` message and `Shutdown` must report `shutdown_requested
    /// == true` AND leave the intent recoverable in `tick_input.intents`
    /// (i.e. NOT already lost by the time the caller decides to shut down).
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
        let shutdown_requested =
            collect_tick_input_live(&mut tick_input, &rx_input, &rx_logic, &mut world, false);

        assert!(shutdown_requested);
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

        let shutdown_requested = drain_logic_messages(None, &rx_logic, &mut world, false);

        assert!(!shutdown_requested);
        assert_eq!(
            tick_input, recorded,
            "a live SignalIntent/ScreenSize must not reach the sim during \
             playback -- every sim-visible fact comes from the replay file"
        );
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

        finalize_recorder(&world, Some(recorder), false);

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

    fn world_with_state(state: GameStates) -> World {
        let mut world = World::new();
        let mut game_state = GameState::new();
        game_state.set(state);
        world.insert_resource(game_state);
        world.insert_resource(TextureDimsStore::default());
        world.insert_resource(DeterminismTaint::default());
        world
    }

    #[test]
    fn preload_guard_taints_on_new_key_while_playing_in_deterministic_mode() {
        let mut world = world_with_state(GameStates::Playing);
        guard_texture_preload(&mut world, "late_texture", true);
        assert!(world.resource::<DeterminismTaint>().is_tainted());
    }

    #[test]
    fn preload_guard_ignores_non_deterministic_mode() {
        let mut world = world_with_state(GameStates::Playing);
        guard_texture_preload(&mut world, "late_texture", false);
        assert!(!world.resource::<DeterminismTaint>().is_tainted());
    }

    #[test]
    fn preload_guard_ignores_non_playing_state() {
        let mut world = world_with_state(GameStates::Setup);
        guard_texture_preload(&mut world, "late_texture", true);
        assert!(!world.resource::<DeterminismTaint>().is_tainted());
    }

    #[test]
    fn preload_guard_ignores_already_known_key() {
        let mut world = world_with_state(GameStates::Playing);
        world
            .resource_mut::<TextureDimsStore>()
            .insert("known_texture", 32, 32);
        guard_texture_preload(&mut world, "known_texture", true);
        assert!(!world.resource::<DeterminismTaint>().is_tainted());
    }
}
