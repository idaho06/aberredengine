//! Audio system implementation backed by a dedicated thread and Raylib.
//!
//! This module hosts the background audio thread and the systems that bridge
//! it with the ECS world:
//! - [`audio_thread`] runs on its own OS thread, owns the Raylib audio device,
//!   and processes [`AudioCmd`](crate::events::audio::AudioCmd) messages,
//!   emitting [`AudioMessage`](crate::events::audio::AudioMessage) responses.
//! - [`poll_audio_events`] non-blockingly drains the audio thread's event
//!   receiver into Bevy ECS' message queue each frame.
//! - [`update_bevy_audio_events`] advances the ECS message queue so newly
//!   written messages become readable by message subscribers.
//!
//! The design keeps Raylib audio API calls isolated to a single thread, while
//! the main game thread communicates via lock-free channels.
//!
//! Notes
//! - The audio thread must be created once via
//!   [`crate::resources::audio::setup_audio`] and joined/terminated via
//!   [`crate::resources::audio::shutdown_audio`].
//! - All file I/O (load) and control (play/stop/pause/volume) happen on the
//!   audio thread in response to commands.
//! - Music streaming requires periodic `update_stream()` calls; this loop takes
//!   care of it while tracks are playing.
//!
//! See also: [`crate::events::audio`] and [`crate::resources::audio`].

use crate::events::audio::{AudioCmd, AudioMessage};
use crate::resources::audio::AudioBridge;
use bevy_ecs::prelude::Messages;
use bevy_ecs::{
    prelude::{MessageWriter, Res},
    system::ResMut,
};
use crossbeam_channel::{Receiver, Sender};
use raylib::core::audio::{Music, RaylibAudio, Sound};
use rustc_hash::{FxHashMap, FxHashSet};

// FxPlayingState removed; we now track only the set of FX ids considered playing.

/// Drain any pending events from the audio thread and enqueue them into the
/// ECS [`Messages<AudioMessage>`] mailbox.
///
/// This is a non-blocking system function intended to run each frame on the
/// main thread. It ensures that messages produced by the audio thread become
/// available to ECS message readers and systems that consume
/// [`AudioMessage`].
///
/// It does not mutate world state beyond writing messages.
pub fn poll_audio_messages(bridge: Res<AudioBridge>, mut writer: MessageWriter<AudioMessage>) {
    writer.write_batch(bridge.rx_msg.try_iter());
}

/// Advance the ECS message queue for [`AudioMessage`].
///
/// Bevy ECS' [`Messages`] API requires calling `update()` once per frame to
/// make messages written this frame visible to readers in the same frame.
/// Run this after [`poll_audio_messages`] in your schedule.
pub fn update_bevy_audio_messages(mut events: ResMut<Messages<AudioMessage>>) {
    events.update();
}

/// Forward ECS AudioCmd messages to the audio thread via the AudioBridge sender.
pub fn forward_audio_cmds(
    bridge: Res<AudioBridge>,
    mut reader: bevy_ecs::prelude::MessageReader<AudioCmd>,
) {
    for cmd in reader.read() {
        // Forward clone to crossbeam channel; ignore send error on shutdown
        let _ = bridge.tx_cmd.send(cmd.clone());
    }
}

/// Advance the ECS message queue for AudioCmd so same-frame readers can observe writes.
pub fn update_bevy_audio_cmds(mut msgs: ResMut<Messages<AudioCmd>>) {
    msgs.update();
}

/// Entry point of the dedicated audio thread.
///
/// Responsibilities:
/// - Initialize the Raylib audio device once for the life of the thread.
/// - Own all `Music` and `Sound` handles, preventing use from other threads.
/// - React to [`AudioCmd`] inputs to load/unload and control playback.
/// - Emit [`AudioMessage`] outputs for state changes (loaded, started,
///   finished, etc.).
/// - Periodically pump music streams and detect when playback finishes.
///
/// Concurrency model:
/// - Uses `crossbeam_channel` for lock-free message passing.
/// - The loop non-blockingly drains commands, performs required Raylib calls,
///   and sleeps briefly between iterations to avoid busy-waiting.
///
/// This function blocks until it receives [`AudioCmd::Shutdown`], at which
/// point it unloads resources and exits cleanly.
pub fn audio_thread(rx_cmd: Receiver<AudioCmd>, tx_evt: Sender<AudioMessage>) {
    let audio = match RaylibAudio::init_audio_device() {
        Ok(device) => device,
        Err(e) => {
            panic!("Failed to initialize audio device: {}", e);
        }
    };

    eprintln!(
        "[audio] thread starting (id={:?})",
        std::thread::current().id()
    );

    let mut musics: FxHashMap<String, Music> = FxHashMap::default();
    let mut playing: FxHashSet<String> = FxHashSet::default();
    let mut looped: FxHashSet<String> = FxHashSet::default();
    let mut sounds: FxHashMap<String, Sound> = FxHashMap::default();
    let mut fx_playing: FxHashSet<String> = FxHashSet::default();

    'run: loop {
        // 1) Drain commands
        for cmd in rx_cmd.try_iter() {
            match cmd {
                AudioCmd::LoadMusic { id, path } => match audio.new_music(&path) {
                    Ok(music) => {
                        // log then insert/send
                        eprintln!("[audio] loaded id='{}' path='{}'", id, path);
                        musics.insert(id.clone(), music);
                        let _ = tx_evt.send(AudioMessage::MusicLoaded { id });
                    }
                    Err(e) => {
                        eprintln!(
                            "[audio] load failed id='{}' path='{}' error='{}'",
                            id, path, e
                        );
                        let _ = tx_evt.send(AudioMessage::MusicLoadFailed {
                            id,
                            error: e.to_string(),
                        });
                    }
                },
                AudioCmd::PlayMusic {
                    id,
                    looped: want_loop,
                } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] play start id='{}' looped={}", id, want_loop);
                        music.seek_stream(0.0);
                        music.play_stream();
                        playing.insert(id.clone());
                        if want_loop {
                            looped.insert(id.clone());
                        } else {
                            looped.remove(&id);
                        }
                        let _ = tx_evt.send(AudioMessage::MusicPlayStarted { id });
                    }
                }
                AudioCmd::StopMusic { id } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] stop id='{}'", id);
                        music.stop_stream();
                        playing.remove(&id);
                        looped.remove(&id);
                        let _ = tx_evt.send(AudioMessage::MusicStopped { id });
                    }
                }
                AudioCmd::PauseMusic { id } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] pause id='{}'", id);
                        music.pause_stream();
                        playing.remove(&id);
                        let _ = tx_evt.send(AudioMessage::MusicStopped { id });
                    }
                }
                AudioCmd::ResumeMusic { id } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] resume id='{}'", id);
                        music.resume_stream();
                        playing.insert(id.clone());
                        let _ = tx_evt.send(AudioMessage::MusicPlayStarted { id });
                    }
                }
                AudioCmd::VolumeMusic { id, vol } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] volume id='{}' vol={}", id, vol);
                        music.set_volume(vol);
                        let _ = tx_evt.send(AudioMessage::MusicVolumeChanged { id, vol });
                    }
                }
                AudioCmd::UnloadMusic { id } => {
                    if let Some(music) = musics.remove(&id) {
                        eprintln!("[audio] unload id='{}'", id);
                        drop(music);
                        let _ = tx_evt.send(AudioMessage::MusicUnloaded { id });
                    }
                }
                AudioCmd::UnloadAllMusic => {
                    eprintln!("[audio] unload all");
                    musics.clear();
                    playing.clear();
                    looped.clear();
                    let _ = tx_evt.send(AudioMessage::MusicUnloadedAll);
                }
                AudioCmd::LoadFx { id, path } => match audio.new_sound(&path) {
                    Ok(sound) => {
                        eprintln!("[audio] fx loaded id='{}' path='{}'", id, path);
                        sounds.insert(id.clone(), sound);
                        let _ = tx_evt.send(AudioMessage::FxLoaded { id });
                    }
                    Err(e) => {
                        eprintln!(
                            "[audio] fx load failed id='{}' path='{}' error='{}'",
                            id, path, e
                        );
                        let _ = tx_evt.send(AudioMessage::FxLoadFailed {
                            id,
                            error: e.to_string(),
                        });
                    }
                },
                AudioCmd::PlayFx { id } => {
                    if let Some(sound) = sounds.get(&id) {
                        eprintln!("[audio] fx play id='{}'", id);
                        sound.play();
                        fx_playing.insert(id.clone());
                    } else {
                        eprintln!("[audio] fx play failed id='{}' reason='not loaded'", id);
                    }
                }
                AudioCmd::UnloadFx { id } => {
                    if let Some(sound) = sounds.remove(&id) {
                        eprintln!("[audio] fx unload id='{}'", id);
                        drop(sound);
                        fx_playing.remove(&id);
                        let _ = tx_evt.send(AudioMessage::FxUnloaded { id });
                    }
                }
                AudioCmd::UnloadAllFx => {
                    eprintln!("[audio] fx unload all");
                    sounds.clear();
                    fx_playing.clear();
                    let _ = tx_evt.send(AudioMessage::FxUnloadedAll);
                }
                AudioCmd::Shutdown => {
                    eprintln!("[audio] shutdown requested");
                    // unload all locally before exiting
                    eprintln!("[audio] unload all");
                    musics.clear();
                    playing.clear();
                    looped.clear();
                    let _ = tx_evt.send(AudioMessage::MusicUnloadedAll);
                    sounds.clear();
                    fx_playing.clear();
                    let _ = tx_evt.send(AudioMessage::FxUnloadedAll);
                    break 'run;
                }
            }
        }
        // 2) Pump streaming + detect ends
        //    `update_stream()` must be called regularly while playing.
        //    If a track ended and isn't looped, emit Finished exactly once.
        let mut ended: Vec<String> = Vec::new();
        for id in playing.iter() {
            if let Some(music) = musics.get(id) {
                if music.is_stream_playing() {
                    music.update_stream();
                } else {
                    // Not currently playing; check if naturally finished.
                    // time_played >= time_lenght - epsilon
                    let len = music.get_time_length();
                    let played = music.get_time_played();
                    if played >= len - 0.01 {
                        ended.push(id.clone());
                    }
                }
            }
        }
        for id in ended.iter() {
            if looped.contains(id) {
                // Restart
                if let Some(music) = musics.get(id) {
                    eprintln!("[audio] restarting looped id='{}'", id);
                    music.seek_stream(0.0);
                    music.play_stream();
                    let _ = tx_evt.send(AudioMessage::MusicPlayStarted { id: id.clone() });
                }
            } else {
                eprintln!("[audio] finished id='{}'", id);
                playing.remove(id);
                let _ = tx_evt.send(AudioMessage::MusicFinished { id: id.clone() });
            }
        }

        // FX end detection: if an id is tracked as playing and Raylib reports it
        // is no longer playing (or the sound handle is missing), emit FxFinished
        // once and stop tracking it.
        let mut fx_ended: Vec<String> = Vec::new();
        for id in fx_playing.iter() {
            let still_playing = sounds
                .get(id)
                .map(|sound| sound.is_playing())
                .unwrap_or(false);
            if !still_playing {
                fx_ended.push(id.clone());
            }
        }

        for id in fx_ended.iter() {
            eprintln!("[audio] fx finished id='{}'", id);
            fx_playing.remove(id);
            let _ = tx_evt.send(AudioMessage::FxFinished { id: id.clone() });
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    } // 'run

    eprintln!(
        "[audio] thread exiting (id={:?})",
        std::thread::current().id()
    );

    // On exit, musics and sounds drop before `audio`, satisfying lifetimes
}
