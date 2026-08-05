//! Systems run by the audio thread's own `Schedule`: `drain_cmds`
//! -> `pump_music` -> `pump_fx`, chained, single-threaded. Each is an
//! exclusive `fn(&mut World)` system: `drain_cmds` needs simultaneous
//! spawn/despawn plus `NonSendMut<AudioStore>` access, which is awkward to
//! express through ordinary system params.

// Test to use

use std::ffi::CString;

use bevy_ecs::prelude::{Component, Entity, World};
use log::{debug, error, info};
use raylib::ffi;

use crate::components::audio::handles::{FfiHandle, MusicHandle};
use crate::components::audio::music_track::MusicTrack;
use crate::components::audio::playing_fx::PlayingFx;
use aberred_core::protocol::audio::{AudioCmd, AudioMessage};
use crate::resources::audio::channels::{CmdReceiver, MsgSender, ShouldExit};
use crate::resources::audio::store::AudioStore;

/// Drain all pending [`AudioCmd`]s non-blockingly and apply them to the
/// audio world.
pub fn drain_cmds(world: &mut World) {
    let cmds: Vec<AudioCmd> = world.resource::<CmdReceiver>().0.try_iter().collect();
    for cmd in cmds {
        handle_cmd(world, cmd);
    }
}

fn send(world: &World, msg: AudioMessage) {
    let _ = world.resource::<MsgSender>().0.send(msg);
}

/// Look up a loaded music stream's handle by id, independent of whether it
/// currently has a `MusicTrack` entity (a loaded-but-not-playing track has
/// none). Shared by every by-id music command except `LoadMusic`/
/// `UnloadMusic`, which mutate the map itself.
fn music_handle(world: &World, id: &str) -> Option<ffi::Music> {
    world.non_send::<AudioStore>().music.get(id).copied()
}

/// Look up a loaded FX sound's handle by id (`PlayFx`/`PlayFxPitched`).
fn fx_handle(world: &World, id: &str) -> Option<ffi::Sound> {
    world.non_send::<AudioStore>().fx.get(id).copied()
}

fn find_music_track(world: &mut World, id: &str) -> Option<Entity> {
    let mut q = world.query::<(Entity, &MusicTrack)>();
    q.iter(world).find(|(_, t)| t.id == id).map(|(e, _)| e)
}

fn despawn_music_track(world: &mut World, id: &str) {
    if let Some(entity) = find_music_track(world, id) {
        world.despawn(entity);
    }
}

fn set_music_track_paused(world: &mut World, id: &str, paused: bool) {
    if let Some(entity) = find_music_track(world, id)
        && let Some(mut track) = world.get_mut::<MusicTrack>(entity)
    {
        track.paused = paused;
    }
}

/// Snapshot of every current `MusicTrack` entity's data, needed whenever an
/// operation must act on all playing/paused tracks in one pass (avoids a
/// separate query per phase of the operation).
fn music_tracks(world: &mut World) -> Vec<(Entity, String, MusicHandle, bool, bool)> {
    // Optimization opportunity: return an iterator instead of a Vec, to avoid heap allocation.
    let mut q = world.query::<(Entity, &MusicTrack)>();
    q.iter(world)
        .map(|(e, t)| (e, t.id.clone(), t.music, t.looped, t.paused))
        .collect()
}

/// Snapshot of every current `PlayingFx` entity's alias, for the same
/// one-pass reason as [`music_tracks`].
fn playing_fx_entities(world: &mut World) -> Vec<(Entity, ffi::Sound)> {
    // Optimization opportunity: return an iterator instead of a Vec, to avoid heap allocation.
    let mut q = world.query::<(Entity, &PlayingFx)>();
    q.iter(world).map(|(e, f)| (e, f.alias.0)).collect()
}

/// Despawn every entity carrying component `T`.
pub(crate) fn despawn_all<T: Component>(world: &mut World) {
    let entities: Vec<Entity> = {
        let mut q = world.query::<(Entity, &T)>();
        q.iter(world).map(|(e, _)| e).collect()
    };
    for entity in entities {
        world.despawn(entity);
    }
}

/// Load a sound, spawn its alias, and start it playing -- shared by
/// `PlayFx`/`PlayFxPitched`, which differ only in whether a pitch override
/// is applied before playback starts.
fn spawn_fx_alias(world: &mut World, sound: ffi::Sound, pitch: Option<f32>) {
    let alias = unsafe { ffi::LoadSoundAlias(sound) };
    if let Some(pitch) = pitch {
        unsafe { ffi::SetSoundPitch(alias, pitch) };
    }
    unsafe { ffi::PlaySound(alias) };
    world.spawn(PlayingFx {
        alias: FfiHandle(alias),
    });
}

/// Stop and unload every currently-playing FX alias, despawning its
/// `PlayingFx` entity. Shared by `StopAllFx`/`UnloadAllFx`/`Shutdown`, which
/// differ only in whether the alias is stopped first (`StopAllFx` cuts off
/// audio still playing; `UnloadAllFx`/`Shutdown` unload without an explicit
/// stop).
pub(crate) fn unload_all_fx_aliases(world: &mut World, stop_first: bool) {
    for (entity, alias) in playing_fx_entities(world) {
        if stop_first {
            unsafe { ffi::StopSound(alias) };
        }
        unsafe { ffi::UnloadSoundAlias(alias) };
        world.despawn(entity);
    }
}

fn handle_cmd(world: &mut World, cmd: AudioCmd) {
    match cmd {
        AudioCmd::LoadMusic { id, path } => {
            let c_path = match CString::new(path.clone()) {
                Ok(s) => s,
                Err(e) => {
                    error!(
                        target: "audio", "load failed id='{}' path='{}' error='invalid path: {}'",
                        id, path, e
                    );
                    send(
                        world,
                        AudioMessage::MusicLoadFailed {
                            id,
                            error: format!("invalid path: {}", e),
                        },
                    );
                    return;
                }
            };
            let music = unsafe { ffi::LoadMusicStream(c_path.as_ptr()) };
            if music.stream.buffer.is_null() {
                error!(
                    target: "audio", "load failed id='{}' path='{}' error='failed to load'",
                    id, path
                );
                send(
                    world,
                    AudioMessage::MusicLoadFailed {
                        id,
                        error: "failed to load".to_string(),
                    },
                );
            } else {
                debug!(target: "audio", "loaded id='{}' path='{}'", id, path);
                world
                    .non_send_mut::<AudioStore>()
                    .music
                    .insert(id.clone(), music);
                send(world, AudioMessage::MusicLoaded { id });
            }
        }
        AudioCmd::PlayMusic {
            id,
            looped: want_loop,
        } => {
            if let Some(music) = music_handle(world, &id) {
                debug!(target: "audio", "play start id='{}' looped={}", id, want_loop);
                unsafe {
                    ffi::SeekMusicStream(music, 0.0);
                    ffi::PlayMusicStream(music);
                }
                despawn_music_track(world, &id);
                world.spawn(MusicTrack {
                    id: id.clone(),
                    music: FfiHandle(music),
                    looped: want_loop,
                    paused: false,
                });
                send(world, AudioMessage::MusicPlayStarted { id });
            }
        }
        AudioCmd::StopMusic { id } => {
            if let Some(music) = music_handle(world, &id) {
                debug!(target: "audio", "stop id='{}'", id);
                unsafe { ffi::StopMusicStream(music) };
                despawn_music_track(world, &id);
                send(world, AudioMessage::MusicStopped { id });
            }
        }
        AudioCmd::StopAllMusic => {
            debug!(target: "audio", "stop all");
            for (entity, id, music, ..) in music_tracks(world) {
                unsafe { ffi::StopMusicStream(music.0) };
                send(world, AudioMessage::MusicStopped { id });
                world.despawn(entity);
            }
        }
        AudioCmd::PauseMusic { id } => {
            if let Some(music) = music_handle(world, &id) {
                debug!(target: "audio", "pause id='{}'", id);
                unsafe { ffi::PauseMusicStream(music) };
                set_music_track_paused(world, &id, true);
                send(world, AudioMessage::MusicStopped { id });
            }
        }
        AudioCmd::ResumeMusic { id } => {
            if let Some(music) = music_handle(world, &id) {
                debug!(target: "audio", "resume id='{}'", id);
                unsafe { ffi::ResumeMusicStream(music) };
                set_music_track_paused(world, &id, false);
                send(world, AudioMessage::MusicPlayStarted { id });
            }
        }
        AudioCmd::VolumeMusic { id, vol } => {
            if let Some(music) = music_handle(world, &id) {
                debug!(target: "audio", "volume id='{}' vol={}", id, vol);
                unsafe { ffi::SetMusicVolume(music, vol) };
                send(world, AudioMessage::MusicVolumeChanged { id, vol });
            }
        }
        AudioCmd::UnloadMusic { id } => {
            let removed = world.non_send_mut::<AudioStore>().music.remove(&id);
            if let Some(music) = removed {
                debug!(target: "audio", "unload id='{}'", id);
                unsafe { ffi::UnloadMusicStream(music) };
                despawn_music_track(world, &id);
                send(world, AudioMessage::MusicUnloaded { id });
            }
        }
        AudioCmd::UnloadAllMusic => {
            debug!(target: "audio", "unload all");
            let mut store = world.non_send_mut::<AudioStore>();
            for (_, music) in store.music.drain() {
                unsafe { ffi::UnloadMusicStream(music) };
            }
            despawn_all::<MusicTrack>(world);
            send(world, AudioMessage::MusicUnloadedAll);
        }
        AudioCmd::LoadFx { id, path } => {
            let c_path = match CString::new(path.clone()) {
                Ok(s) => s,
                Err(e) => {
                    error!(
                        target: "audio", "fx load failed id='{}' path='{}' error='invalid path: {}'",
                        id, path, e
                    );
                    send(
                        world,
                        AudioMessage::FxLoadFailed {
                            id,
                            error: format!("invalid path: {}", e),
                        },
                    );
                    return;
                }
            };
            let sound = unsafe { ffi::LoadSound(c_path.as_ptr()) };
            if sound.stream.buffer.is_null() {
                error!(
                    target: "audio", "fx load failed id='{}' path='{}' error='failed to load'",
                    id, path
                );
                send(
                    world,
                    AudioMessage::FxLoadFailed {
                        id,
                        error: "failed to load".to_string(),
                    },
                );
            } else {
                debug!(target: "audio", "fx loaded id='{}' path='{}'", id, path);
                world
                    .non_send_mut::<AudioStore>()
                    .fx
                    .insert(id.clone(), sound);
                send(world, AudioMessage::FxLoaded { id });
            }
        }
        AudioCmd::PlayFx { id } => {
            if let Some(sound) = fx_handle(world, &id) {
                debug!(target: "audio", "fx play id='{}'", id);
                spawn_fx_alias(world, sound, None);
            } else {
                error!(target: "audio", "fx play failed id='{}' reason='not loaded'", id);
            }
        }
        AudioCmd::PlayFxPitched { id, pitch } => {
            if let Some(sound) = fx_handle(world, &id) {
                debug!(target: "audio", "fx play pitched id='{}' pitch={}", id, pitch);
                spawn_fx_alias(world, sound, Some(pitch));
            } else {
                error!(
                    target: "audio", "fx play pitched failed id='{}' reason='not loaded'",
                    id
                );
            }
        }
        AudioCmd::StopAllFx => {
            debug!(target: "audio", "fx stop all");
            unload_all_fx_aliases(world, true);
        }
        AudioCmd::UnloadFx { id } => {
            // Individual unload is a no-op with the SoundAlias approach --
            // sounds are kept loaded for the lifetime of the scene.
            debug!(
                target: "audio", "fx unload id='{}' (ignored - use UnloadAllFx instead)",
                id
            );
        }
        AudioCmd::UnloadAllFx => {
            debug!(target: "audio", "fx unload all");
            unload_all_fx_aliases(world, false);
            let mut store = world.non_send_mut::<AudioStore>();
            for (_, sound) in store.fx.drain() {
                unsafe { ffi::UnloadSound(sound) };
            }
            send(world, AudioMessage::FxUnloadedAll);
        }
        AudioCmd::Shutdown => {
            info!(target: "audio", "shutdown requested");
            debug!(target: "audio", "unload all");
            handle_cmd(world, AudioCmd::UnloadAllMusic);
            handle_cmd(world, AudioCmd::UnloadAllFx);
            world.resource_mut::<ShouldExit>().0 = true;
        }
    }
}

/// Pump music streams: `UpdateMusicStream` each unpaused `MusicTrack`, detect
/// end-of-track, restart looped tracks or emit `MusicFinished`. The stream
/// handle comes straight off the component (cached at `PlayMusic` time), so
/// this hot path -- it runs every audio tick regardless of commands -- never
/// needs to touch `AudioStore`.
pub fn pump_music(world: &mut World) {
    let mut ended: Vec<(Entity, String, bool)> = Vec::new();
    for (entity, id, music, looped, paused) in music_tracks(world) {
        if paused {
            continue;
        }
        let music = music.0;
        unsafe { ffi::UpdateMusicStream(music) };
        let len = unsafe { ffi::GetMusicTimeLength(music) };
        let played = unsafe { ffi::GetMusicTimePlayed(music) };
        if played >= len - 0.01 {
            ended.push((entity, id, looped));
        }
    }
    for (entity, id, looped) in ended {
        let music = match world.get::<MusicTrack>(entity) {
            Some(track) => track.music.0,
            None => continue,
        };
        if looped {
            debug!(target: "audio", "restarting looped id='{}'", id);
            unsafe {
                ffi::StopMusicStream(music);
                ffi::SeekMusicStream(music, 0.0);
                ffi::PlayMusicStream(music);
            }
            send(world, AudioMessage::MusicPlayStarted { id });
        } else {
            debug!(target: "audio", "finished id='{}'", id);
            unsafe { ffi::StopMusicStream(music) };
            world.despawn(entity);
            send(world, AudioMessage::MusicFinished { id });
        }
    }
}

/// Clean up finished sound aliases -- despawn `PlayingFx` entities whose
/// alias has stopped playing. No `AudioMessage` is sent here: FX completion
/// is silent by design, and the protocol (`AudioMessage`) intentionally has
/// no `FxFinished` variant.
pub fn pump_fx(world: &mut World) {
    for (entity, alias) in playing_fx_entities(world) {
        if !unsafe { ffi::IsSoundPlaying(alias) } {
            unsafe { ffi::UnloadSoundAlias(alias) };
            world.despawn(entity);
        }
    }
}
