#[cfg(feature = "lua")]
use std::path::PathBuf;

use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use crossbeam_channel::{Receiver, Sender};

use super::builder::EngineBuilder;
use super::registrar::{HookRegistrar, ObserverRegistrar, UpdateRegistrar};
use crate::error::EngineError;
use crate::pacing::{Pacer, StatsWindow, TickCountdown};
#[cfg(any(test, feature = "test-support"))]
use crate::protocol::audio::{AudioCmd, AudioMessage};
use crate::protocol::endpoints::shutdown_audio;
use crate::protocol::endpoints::RenderTx;
use crate::protocol::raw_input::{InputSample, RawDeviceSnapshot};
use crate::protocol::render_logic::{LogicMsg, RenderMsg};
use crate::protocol::snapshot::SnapshotPublisher;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::fontmetrics::FontMetricsStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::input::InputState;
use crate::resources::render::imgui_bridge::ImguiCaptureState;
use crate::resources::rawinput::ImguiCaptureMirror;
use crate::resources::screensize::ScreenSize;
use crate::resources::signal_intents::SignalIntents;
use crate::resources::texturedims::TextureDimsStore;
use crate::resources::thread_stats::SimStats;
use crate::resources::windowsize::WindowSize;
use crate::systems::input::resolve_input_backlog;
use crate::systems::scene_dispatch::SceneDescriptor;
use crate::systems::signal_intents::apply_signal_intents;
use crate::systems::time::update_world_time;

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
    /// Test-harness-only: when `true`, [`EngineBuilder::setup_logic_world`]
    /// inserts a stub [`AudioBridge`](crate::protocol::endpoints::AudioBridge)
    /// (no real audio thread) via `setup_audio_stub` instead of `setup_audio`,
    /// stashing the stub's far ends into `audio_stub_ends` for the caller to
    /// retrieve. Always `false` in production (`try_run` never sets it).
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) stub_audio: bool,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) audio_stub_ends:
        Option<(Receiver<AudioCmd>, Sender<AudioMessage>)>,
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

/// Cap on a single sim tick's real dt: protects against a huge dt
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
pub(crate) fn run_sim_tick(world: &mut World, sim: &mut Schedule) {
    crate::tracy::tracy_span!("sim_schedule_run");
    sim.run(world);
    world.resource_mut::<InputState>().clear_edges();
}

/// The logic thread's `Pacer`-driven loop: one `sim` tick per
/// `Pacer` wakeup at `[simulation] hz`, `dt` the real elapsed time since the
/// previous tick (clamped to [`DT_CLAMP_SECONDS`], scaled by `time_scale`
/// inside [`update_world_time`]). There is no
/// catch-up: a stall simply produces one larger (clamped) dt on the next
/// tick rather than replayed substeps -- strict fixed-step
/// determinism is consciously not provided.
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

    let mut world = EngineBuilder::setup_logic_world(&mut init)?;
    EngineBuilder::register_logic_systems(&mut init, &mut world, use_scene_manager)?;
    EngineBuilder::spawn_observers(&mut world, has_lua, std::mem::take(&mut init.extra_observers));

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
        // either way).
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
                    world.resource_mut::<SignalIntents>().0.extend(intents);
                }
                LogicMsg::Shutdown => shutdown_requested = true,
            }
        }

        if shutdown_requested {
            // Exits immediately on Shutdown (no sim/present work runs
            // after it) — the messages loop above already applied
            // everything in this batch to its
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
        // that reads it. A F10 edge (post
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
        let tick_start = std::time::Instant::now();
        run_sim_tick(&mut world, &mut sim);
        let tick_work = tick_start.elapsed();

        backlog_sum += input_backlog.len() as u64;
        backlog_max = backlog_max.max(input_backlog.len() as u32);
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
    shutdown_audio(&mut world);
    Ok(())
}
