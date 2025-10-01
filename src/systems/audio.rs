use core::panic;

use crate::events::audio::{AudioCmd, AudioMessage};
use crate::resources::audio::AudioBridge;
use bevy_ecs::prelude::Messages;
use bevy_ecs::{
    prelude::{MessageWriter, Res},
    system::ResMut,
};
use crossbeam_channel::{Receiver, Sender};
use raylib::core::audio::{Music, RaylibAudio};
use rustc_hash::{FxHashMap, FxHashSet};

pub fn poll_audio_events(bridge: Res<AudioBridge>, mut writer: MessageWriter<AudioMessage>) {
    writer.write_batch(bridge.rx_evt.try_iter());
}

pub fn update_bevy_audio_events(mut events: ResMut<Messages<AudioMessage>>) {
    events.update();
}

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

    'run: loop {
        // 1) Drain commands
        for cmd in rx_cmd.try_iter() {
            match cmd {
                AudioCmd::Load { id, path } => match audio.new_music(&path) {
                    Ok(music) => {
                        // log then insert/send
                        eprintln!("[audio] loaded id='{}' path='{}'", id, path);
                        musics.insert(id.clone(), music);
                        let _ = tx_evt.send(AudioMessage::Loaded { id });
                    }
                    Err(e) => {
                        eprintln!(
                            "[audio] load failed id='{}' path='{}' error='{}'",
                            id, path, e
                        );
                        let _ = tx_evt.send(AudioMessage::LoadFailed {
                            id,
                            error: e.to_string(),
                        });
                    }
                },
                AudioCmd::Play {
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
                        let _ = tx_evt.send(AudioMessage::PlayStarted { id });
                    }
                }
                AudioCmd::Stop { id } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] stop id='{}'", id);
                        music.stop_stream();
                        playing.remove(&id);
                        looped.remove(&id);
                        let _ = tx_evt.send(AudioMessage::Stopped { id });
                    }
                }
                AudioCmd::Pause { id } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] pause id='{}'", id);
                        music.pause_stream();
                        playing.remove(&id);
                        let _ = tx_evt.send(AudioMessage::Stopped { id });
                    }
                }
                AudioCmd::Resume { id } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] resume id='{}'", id);
                        music.resume_stream();
                        playing.insert(id.clone());
                        let _ = tx_evt.send(AudioMessage::PlayStarted { id });
                    }
                }
                AudioCmd::Volume { id, vol } => {
                    if let Some(music) = musics.get(&id) {
                        eprintln!("[audio] volume id='{}' vol={}", id, vol);
                        music.set_volume(vol);
                        let _ = tx_evt.send(AudioMessage::VolumeChanged { id, vol });
                    }
                }
                AudioCmd::Unload { id } => {
                    if let Some(music) = musics.remove(&id) {
                        eprintln!("[audio] unload id='{}'", id);
                        drop(music);
                        let _ = tx_evt.send(AudioMessage::Unloaded { id });
                    }
                }
                AudioCmd::UnloadAll => {
                    eprintln!("[audio] unload all");
                    musics.clear();
                    playing.clear();
                    looped.clear();
                    let _ = tx_evt.send(AudioMessage::UnloadedAll);
                }
                AudioCmd::Shutdown => {
                    eprintln!("[audio] shutdown requested");
                    // unload all locally before exiting
                    eprintln!("[audio] unload all");
                    musics.clear();
                    playing.clear();
                    looped.clear();
                    let _ = tx_evt.send(AudioMessage::UnloadedAll);
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
                    let _ = tx_evt.send(AudioMessage::PlayStarted { id: id.clone() });
                }
            } else {
                eprintln!("[audio] finished id='{}'", id);
                playing.remove(id);
                let _ = tx_evt.send(AudioMessage::Finished { id: id.clone() });
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    } // 'run

    eprintln!(
        "[audio] thread exiting (id={:?})",
        std::thread::current().id()
    );

    // On exit, musics drop before `audio`, satisfying lifetimes
}
