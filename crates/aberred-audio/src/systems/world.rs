//! Entry point of the dedicated audio thread: the audio thread
//! owns its own `bevy_ecs::World` + single-threaded `Schedule`, paced by a
//! [`Pacer`]/`audio_hz` loop.

use bevy_ecs::prelude::World;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, SingleThreadedExecutor};
use crossbeam_channel::{Receiver, Sender};
use log::info;
use raylib::core::audio::RaylibAudio;

use aberred_core::pacing::{Pacer, StatsWindow};
use aberred_core::protocol::audio::{AudioCmd, AudioMessage};

use crate::components::music_track::MusicTrack;
use crate::resources::backend::{AudioBackend, RaylibBackend};
use crate::resources::channels::{CmdReceiver, MsgSender, ShouldExit};
use crate::resources::store::AudioStore;

use super::pipeline::{
    despawn_all, drain_cmds, pump_fx, pump_music, unload_all_fx_aliases, unload_all_music,
    unload_all_sounds,
};

/// Entry point of the dedicated audio thread.
///
/// Responsibilities:
/// - Initialize the Raylib audio device once for the life of the thread.
/// - Own all `Music`/`Sound` handles via the NonSend [`AudioStore`],
///   preventing use from other threads.
/// - React to [`AudioCmd`] inputs to load/unload and control playback.
/// - Emit [`AudioMessage`] outputs for state changes (loaded, started,
///   finished, etc.).
/// - Periodically pump music streams and detect when playback finishes.
///
/// Concurrency model:
/// - Uses `crossbeam_channel` for lock-free message passing.
/// - The loop is paced by a [`Pacer`] at `audio_hz`: each tick
///   drains all pending commands non-blockingly (`drain_cmds`), then pumps
///   music streams (`pump_music`) and cleans up finished sound aliases
///   (`pump_fx`) -- constant wakeups, no blocking while idle.
///
/// This function runs until the command channel disconnects (the engine drops
/// its sender at shutdown), at which point it applies any commands still
/// queued, unloads every resource and exits cleanly.
pub fn audio_thread(rx_cmd: Receiver<AudioCmd>, tx_evt: Sender<AudioMessage>, audio_hz: f64) {
    // Declared before `world` so it drops after it: raylib requires the
    // device to outlive every Music/Sound handle in the world's AudioStore.
    let device = match RaylibAudio::init_audio_device() {
        Ok(device) => device,
        Err(e) => {
            panic!("Failed to initialize audio device: {}", e);
        }
    };

    info!(
        target: "audio", "thread starting (id={:?})",
        std::thread::current().id()
    );

    let (mut world, mut schedule) = build_world(RaylibBackend, rx_cmd, tx_evt);

    let mut pacer = Pacer::new(audio_hz);
    // Rolls up schedule-run work time into a ThreadStats once per
    // ~1s window, shipped to the logic thread via AudioMessage::Stats for
    // the F11 perf panel.
    let mut stats_window = StatsWindow::new(audio_hz);

    'run: loop {
        if !aberred_core::protocol::shutdown::running() {
            break 'run;
        }
        pacer.tick();

        let tick_start = std::time::Instant::now();
        schedule.run(&mut world);
        let tick_work = tick_start.elapsed();
        if let Some(stats) = stats_window.record(tick_work) {
            let _ = world
                .resource::<MsgSender>()
                .0
                .send(AudioMessage::Stats { stats });
        }

        if world.resource::<ShouldExit>().0 {
            break 'run;
        }
        world.clear_trackers();
    }

    info!(
        target: "audio", "thread exiting (id={:?})",
        std::thread::current().id()
    );

    teardown::<RaylibBackend>(&mut world);
    drop(world);
    drop(device);
}

/// Build the audio world (NonSend [`AudioStore`] over `backend`,
/// command/message channel resources, exit flag) and its initialized
/// single-threaded `drain_cmds -> pump_music -> pump_fx` schedule. Opens no
/// audio device -- that is `audio_thread`'s job, which keeps the device
/// alive for as long as this world exists.
pub fn build_world<B: AudioBackend>(
    backend: B,
    rx_cmd: Receiver<AudioCmd>,
    tx_evt: Sender<AudioMessage>,
) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_non_send(AudioStore::new(backend));
    world.insert_resource(CmdReceiver(rx_cmd));
    world.insert_resource(MsgSender(tx_evt));
    world.insert_resource(ShouldExit::default());

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor::new());
    schedule.add_systems((drain_cmds::<B>, pump_music::<B>, pump_fx::<B>).chain());
    schedule
        .initialize(&mut world)
        .expect("audio schedule initialize");
    (world, schedule)
}

/// Drain and unload every remaining `PlayingFx`/`MusicTrack` entity before
/// the `AudioStore` (and then the device) drops. A safety net, not the primary
/// unload path when a game sends `UnloadAllFx`/`UnloadAllMusic` before exiting,
/// and the only one at engine shutdown, which closes the command channel. Reuses the same
/// `pipeline.rs` helpers those command handlers use, rather than
/// re-implementing the same queries here.
pub(crate) fn teardown<B: AudioBackend>(world: &mut World) {
    unload_all_fx_aliases::<B>(world, false);
    despawn_all::<MusicTrack<B::Music>>(world);

    // Unload any remaining loaded (not-currently-playing) assets so the
    // device drops with nothing outstanding.
    unload_all_music::<B>(world);
    unload_all_sounds::<B>(world);

    // `world` (and its `AudioStore`) drops at the end of `audio_thread`,
    // after every Music/Sound handle above has been unloaded and before the
    // device.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::backend::testing::RecordingBackend;

    /// The audio world and its schedule need no audio device -- only
    /// `audio_thread` itself opens one.
    #[test]
    fn build_world_runs_headless_and_exits_on_disconnect() {
        let (tx_cmd, rx_cmd) = crossbeam_channel::unbounded::<AudioCmd>();
        let (tx_evt, _rx_evt) = crossbeam_channel::unbounded::<AudioMessage>();
        let (mut world, mut schedule) = build_world(RecordingBackend::default(), rx_cmd, tx_evt);

        schedule.run(&mut world);
        assert!(!world.resource::<ShouldExit>().0);

        drop(tx_cmd);
        schedule.run(&mut world);
        assert!(world.resource::<ShouldExit>().0);
        let store = world.non_send::<AudioStore<RecordingBackend>>();
        assert!(store.music.is_empty() && store.fx.is_empty());
    }
}
