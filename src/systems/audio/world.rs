//! Entry point of the dedicated audio thread: the audio thread
//! owns its own `bevy_ecs::World` + single-threaded `Schedule`, paced by a
//! [`Pacer`]/`audio_hz` loop.

use bevy_ecs::prelude::World;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, SingleThreadedExecutor};
use crossbeam_channel::{Receiver, Sender};
use log::info;
use raylib::core::audio::RaylibAudio;
use raylib::ffi;
use rustc_hash::FxHashMap;

use crate::pacing::Pacer;
use crate::protocol::audio::{AudioCmd, AudioMessage};

use crate::components::audio::music_track::MusicTrack;
use crate::resources::audio::channels::{CmdReceiver, MsgSender, ShouldExit};
use crate::resources::audio::store::AudioStore;

use super::systems::{despawn_all, drain_cmds, pump_fx, pump_music, unload_all_fx_aliases};

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
/// This function runs until it receives [`AudioCmd::Shutdown`] (or the
/// channel disconnects), at which point it unloads resources and exits
/// cleanly.
pub fn audio_thread(rx_cmd: Receiver<AudioCmd>, tx_evt: Sender<AudioMessage>, audio_hz: f64) {
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

    let mut world = World::new();
    world.insert_non_send(AudioStore {
        device,
        music: FxHashMap::default(),
        fx: FxHashMap::default(),
    });
    // Keep a clone of the receiver locally too (crossbeam `Receiver` is a
    // cheap `Clone`) so the disconnect check doesn't need to reach back into
    // the World's resource.
    world.insert_resource(CmdReceiver(rx_cmd.clone()));
    world.insert_resource(MsgSender(tx_evt));
    world.insert_resource(ShouldExit::default());

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor::new());
    schedule.add_systems((drain_cmds, pump_music, pump_fx).chain());
    schedule.initialize(&mut world).expect("audio schedule initialize");

    let mut pacer = Pacer::new(audio_hz);

    'run: loop {
        if !crate::protocol::shutdown::running() {
            break 'run;
        }
        pacer.tick();

        schedule.run(&mut world);

        if world.resource::<ShouldExit>().0 {
            break 'run;
        }
        if crate::pacing::channel_disconnected(&rx_cmd) {
            break 'run;
        }
        world.clear_trackers();
    }

    info!(
        target: "audio", "thread exiting (id={:?})",
        std::thread::current().id()
    );

    teardown(&mut world);
}

/// Drain and unload every remaining `PlayingFx`/`MusicTrack` entity before
/// the `AudioStore` (and its `device`) drops. A safety net, not the primary
/// unload path -- `Shutdown`/`UnloadAllFx`/`UnloadAllMusic` already unload
/// everything via `drain_cmds` in the common case. Reuses the same
/// `systems.rs` helpers those command handlers use, rather than
/// re-implementing the same queries here.
fn teardown(world: &mut World) {
    unload_all_fx_aliases(world, false);
    despawn_all::<MusicTrack>(world);

    // Unload any remaining loaded (not-currently-playing) assets so the
    // device drops with nothing outstanding.
    let mut store = world.non_send_mut::<AudioStore>();
    for (_, music) in store.music.drain() {
        unsafe { ffi::UnloadMusicStream(music) };
    }
    for (_, sound) in store.fx.drain() {
        unsafe { ffi::UnloadSound(sound) };
    }

    // `AudioStore` (and its `device`) drops when `world` drops at the end of
    // `audio_thread`, after every Music/Sound handle above has been unloaded.
}
