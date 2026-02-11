//! Audio system implementation backed by a dedicated thread and Raylib.
//!
//! This module hosts the background audio thread and the systems that bridge
//! it with the ECS world:
//! - [`audio_thread`] runs on its own OS thread, owns the Raylib audio device,
//!   and processes [`AudioCmd`] messages, emitting [`AudioMessage`] responses.
//! - [`poll_audio_messages`] non-blockingly drains the audio thread's event
//!   receiver into Bevy ECS' message queue each frame.
//! - [`update_bevy_audio_messages`] advances the ECS message queue so newly
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
use raylib::core::audio::{Music, RaylibAudio};
use raylib::ffi;
use rustc_hash::{FxHashMap, FxHashSet};
use std::ffi::CString;
use log::{info, error, debug};

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
pub fn update_bevy_audio_messages(mut msgs: ResMut<Messages<AudioMessage>>) {
    msgs.update();
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

    info!(
        target: "audio", "thread starting (id={:?})",
        std::thread::current().id()
    );

    let mut musics: FxHashMap<String, Music> = FxHashMap::default();
    let mut playing: FxHashSet<String> = FxHashSet::default();
    let mut looped: FxHashSet<String> = FxHashSet::default();
    let mut sounds: FxHashMap<String, ffi::Sound> = FxHashMap::default();
    let mut active_aliases: Vec<ffi::Sound> = Vec::new();

    'run: loop {
        // 1) Drain commands
        for cmd in rx_cmd.try_iter() {
            match cmd {
                AudioCmd::LoadMusic { id, path } => match audio.new_music(&path) {
                    Ok(music) => {
                        // log then insert/send
                        info!(target: "audio", "loaded id='{}' path='{}'", id, path);
                        musics.insert(id.clone(), music);
                        let _ = tx_evt.send(AudioMessage::MusicLoaded { id });
                    }
                    Err(e) => {
                        error!(
                            target: "audio", "load failed id='{}' path='{}' error='{}'",
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
                        debug!(target: "audio", "play start id='{}' looped={}", id, want_loop);
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
                        debug!(target: "audio", "stop id='{}'", id);
                        music.stop_stream();
                        playing.remove(&id);
                        looped.remove(&id);
                        let _ = tx_evt.send(AudioMessage::MusicStopped { id });
                    }
                }
                AudioCmd::StopAllMusic => {
                    debug!(target: "audio", "stop all");
                    for id in playing.drain() {
                        if let Some(music) = musics.get(&id) {
                            music.stop_stream();
                            let _ = tx_evt.send(AudioMessage::MusicStopped { id: id.clone() });
                        }
                    }
                    looped.clear();
                }
                AudioCmd::PauseMusic { id } => {
                    if let Some(music) = musics.get(&id) {
                        debug!(target: "audio", "pause id='{}'", id);
                        music.pause_stream();
                        playing.remove(&id);
                        let _ = tx_evt.send(AudioMessage::MusicStopped { id });
                    }
                }
                AudioCmd::ResumeMusic { id } => {
                    if let Some(music) = musics.get(&id) {
                        debug!(target: "audio", "resume id='{}'", id);
                        music.resume_stream();
                        playing.insert(id.clone());
                        let _ = tx_evt.send(AudioMessage::MusicPlayStarted { id });
                    }
                }
                AudioCmd::VolumeMusic { id, vol } => {
                    if let Some(music) = musics.get(&id) {
                        debug!(target: "audio", "volume id='{}' vol={}", id, vol);
                        music.set_volume(vol);
                        let _ = tx_evt.send(AudioMessage::MusicVolumeChanged { id, vol });
                    }
                }
                AudioCmd::UnloadMusic { id } => {
                    if let Some(music) = musics.remove(&id) {
                        debug!(target: "audio", "unload id='{}'", id);
                        drop(music);
                        let _ = tx_evt.send(AudioMessage::MusicUnloaded { id });
                    }
                }
                AudioCmd::UnloadAllMusic => {
                    debug!(target: "audio", "unload all");
                    musics.clear();
                    playing.clear();
                    looped.clear();
                    let _ = tx_evt.send(AudioMessage::MusicUnloadedAll);
                }
                AudioCmd::LoadFx { id, path } => {
                    let c_path = match CString::new(path.clone()) {
                        Ok(s) => s,
                        Err(e) => {
                            error!(
                                target: "audio", "fx load failed id='{}' path='{}' error='invalid path: {}'",
                                id, path, e
                            );
                            let _ = tx_evt.send(AudioMessage::FxLoadFailed {
                                id,
                                error: format!("invalid path: {}", e),
                            });
                            continue;
                        }
                    };
                    let sound = unsafe { ffi::LoadSound(c_path.as_ptr()) };
                    if sound.stream.buffer.is_null() {
                        error!(
                            target: "audio", "fx load failed id='{}' path='{}' error='failed to load'",
                            id, path
                        );
                        let _ = tx_evt.send(AudioMessage::FxLoadFailed {
                            id,
                            error: "failed to load".to_string(),
                        });
                    } else {
                        info!(target: "audio", "fx loaded id='{}' path='{}'", id, path);
                        sounds.insert(id.clone(), sound);
                        let _ = tx_evt.send(AudioMessage::FxLoaded { id });
                    }
                }
                AudioCmd::PlayFx { id } => {
                    if let Some(sound) = sounds.get(&id) {
                        debug!(target: "audio", "fx play id='{}'", id);
                        let alias = unsafe { ffi::LoadSoundAlias(*sound) };
                        unsafe { ffi::PlaySound(alias) };
                        active_aliases.push(alias);
                    } else {
                        error!(target: "audio", "fx play failed id='{}' reason='not loaded'", id);
                    }
                }
                AudioCmd::UnloadFx { id } => {
                    // Individual unload is a no-op with SoundAlias approach
                    // Sounds are kept loaded for the lifetime of the scene
                    debug!(
                        target: "audio", "fx unload id='{}' (ignored - use UnloadAllFx instead)",
                        id
                    );
                }
                AudioCmd::UnloadAllFx => {
                    debug!(target: "audio", "fx unload all");
                    // First unload all active aliases
                    for alias in active_aliases.drain(..) {
                        unsafe { ffi::UnloadSoundAlias(alias) };
                    }
                    // Then unload all base sounds
                    for (_, sound) in sounds.drain() {
                        unsafe { ffi::UnloadSound(sound) };
                    }
                    let _ = tx_evt.send(AudioMessage::FxUnloadedAll);
                }
                AudioCmd::Shutdown => {
                    info!(target: "audio", "shutdown requested");
                    // unload all locally before exiting
                    debug!(target: "audio", "unload all");
                    musics.clear();
                    playing.clear();
                    looped.clear();
                    let _ = tx_evt.send(AudioMessage::MusicUnloadedAll);
                    // Clean up aliases first
                    for alias in active_aliases.drain(..) {
                        unsafe { ffi::UnloadSoundAlias(alias) };
                    }
                    // Then unload base sounds
                    for (_, sound) in sounds.drain() {
                        unsafe { ffi::UnloadSound(sound) };
                    }
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
                music.update_stream();
                let len = music.get_time_length();
                let played = music.get_time_played();
                if played >= len - 0.01 && !looped.contains(id) {
                    ended.push(id.clone());
                }
            }
        }
        for id in ended.iter() {
            if looped.contains(id) {
                // Restart
                if let Some(music) = musics.get(id) {
                    debug!(target: "audio", "restarting looped id='{}'", id);
                    music.stop_stream();
                    //music.seek_stream(0.0);
                    music.play_stream();
                    let _ = tx_evt.send(AudioMessage::MusicPlayStarted { id: id.clone() });
                }
            } else {
                debug!(target: "audio", "finished id='{}'", id);
                if let Some(music) = musics.get(id) {
                    music.stop_stream();
                };
                playing.remove(id);
                let _ = tx_evt.send(AudioMessage::MusicFinished { id: id.clone() });
            }
        }

        // Clean up finished sound aliases - unload those that have stopped playing
        active_aliases.retain(|alias| {
            let still_playing = unsafe { ffi::IsSoundPlaying(*alias) };
            if !still_playing {
                unsafe { ffi::UnloadSoundAlias(*alias) };
            }
            still_playing
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
    } // 'run

    info!(
        target: "audio", "thread exiting (id={:?})",
        std::thread::current().id()
    );

    // On exit, musics and sounds drop before `audio`, satisfying lifetimes
}
