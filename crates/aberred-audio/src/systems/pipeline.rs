//! Systems run by the audio thread's own `Schedule`: `drain_cmds`
//! -> `pump_music` -> `pump_fx`, chained, single-threaded. Each is an
//! exclusive `fn(&mut World)` system: `drain_cmds` needs simultaneous
//! spawn/despawn plus `NonSendMut<AudioStore>` access, which is awkward to
//! express through ordinary system params.
//!
//! Every system is generic over the [`AudioBackend`] that performs the FFI
//! calls (`RaylibBackend` in production, a recording fake in tests); the
//! handles it hands out are what `AudioStore`/`MusicTrack`/`PlayingFx` hold.

use std::ffi::CString;

use bevy_ecs::prelude::{Component, Entity, World};
use bevy_ecs::world::Mut;
use log::{debug, error, info};

use crate::components::music_track::MusicTrack;
use crate::components::playing_fx::PlayingFx;
use crate::resources::backend::AudioBackend;
use crate::resources::channels::{CmdReceiver, MsgSender, ShouldExit};
use crate::resources::store::AudioStore;
use aberred_core::protocol::audio::{AudioCmd, AudioMessage};

/// Drain all pending [`AudioCmd`]s non-blockingly and apply them to the
/// audio world. Once every sender is gone (and nothing is left queued) it
/// sets [`ShouldExit`] -- detected by the draining `try_recv` itself, so a
/// command that lands mid-drain is still applied, never lost.
pub fn drain_cmds<B: AudioBackend>(world: &mut World) {
    aberred_core::tracy::tracy_span!("audio_drain_cmds");
    let mut cmds: Vec<AudioCmd> = Vec::new();
    let disconnected =
        aberred_core::pacing::drain_channel(&world.resource::<CmdReceiver>().0, |cmd| {
            cmds.push(cmd)
        });
    for cmd in cmds {
        handle_cmd::<B>(world, cmd);
    }
    if disconnected {
        world.resource_mut::<ShouldExit>().0 = true;
    }
}

fn send(world: &World, msg: AudioMessage) {
    let _ = world.resource::<MsgSender>().0.send(msg);
}

/// The audio world's store (backend + loaded handles).
fn store<B: AudioBackend>(world: &mut World) -> Mut<'_, AudioStore<B>> {
    world.non_send_mut::<AudioStore<B>>()
}

/// Look up a loaded music stream's handle by id, independent of whether it
/// currently has a `MusicTrack` entity (a loaded-but-not-playing track has
/// none). Shared by every by-id music command except `LoadMusic`/
/// `UnloadMusic`, which mutate the map itself.
fn music_handle<B: AudioBackend>(world: &World, id: &str) -> Option<B::Music> {
    world.non_send::<AudioStore<B>>().music.get(id).copied()
}

/// Look up a loaded FX sound's handle by id (`PlayFx`/`PlayFxPitched`).
fn fx_handle<B: AudioBackend>(world: &World, id: &str) -> Option<B::Sound> {
    world.non_send::<AudioStore<B>>().fx.get(id).copied()
}

fn find_music_track<B: AudioBackend>(world: &mut World, id: &str) -> Option<Entity> {
    let mut q = world.query::<(Entity, &MusicTrack<B::Music>)>();
    q.iter(world).find(|(_, t)| t.id == id).map(|(e, _)| e)
}

fn despawn_music_track<B: AudioBackend>(world: &mut World, id: &str) {
    if let Some(entity) = find_music_track::<B>(world, id) {
        world.despawn(entity);
    }
}

fn set_music_track_paused<B: AudioBackend>(world: &mut World, id: &str, paused: bool) {
    if let Some(entity) = find_music_track::<B>(world, id)
        && let Some(mut track) = world.get_mut::<MusicTrack<B::Music>>(entity)
    {
        track.paused = paused;
    }
}

/// One `MusicTrack` entity's data, snapshotted.
type TrackSnapshot<M> = (Entity, String, M, bool, bool);

/// Snapshot of every current `MusicTrack` entity's data, needed whenever an
/// operation must act on all playing/paused tracks in one pass (avoids a
/// separate query per phase of the operation).
fn music_tracks<B: AudioBackend>(world: &mut World) -> Vec<TrackSnapshot<B::Music>> {
    // Optimization opportunity: return an iterator instead of a Vec, to avoid heap allocation.
    let mut q = world.query::<(Entity, &MusicTrack<B::Music>)>();
    q.iter(world)
        .map(|(e, t)| (e, t.id.clone(), t.music, t.looped, t.paused))
        .collect()
}

/// Snapshot of every current `PlayingFx` entity's alias, for the same
/// one-pass reason as [`music_tracks`].
fn playing_fx_entities<B: AudioBackend>(world: &mut World) -> Vec<(Entity, B::Sound)> {
    // Optimization opportunity: return an iterator instead of a Vec, to avoid heap allocation.
    let mut q = world.query::<(Entity, &PlayingFx<B::Sound>)>();
    q.iter(world).map(|(e, f)| (e, f.alias)).collect()
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
fn spawn_fx_alias<B: AudioBackend>(
    world: &mut World,
    sound: B::Sound,
    source_id: String,
    pitch: Option<f32>,
) {
    let mut s = store::<B>(world);
    let alias = s.backend.load_sound_alias(sound);
    if let Some(pitch) = pitch {
        s.backend.set_sound_pitch(alias, pitch);
    }
    s.backend.play_sound(alias);
    world.spawn(PlayingFx { alias, source_id });
}

/// Stop and unload every currently-playing FX alias, despawning its
/// `PlayingFx` entity. Shared by `StopAllFx`/`UnloadAllFx`/`Shutdown`, which
/// differ only in whether the alias is stopped first (`StopAllFx` cuts off
/// audio still playing; `UnloadAllFx`/`Shutdown` unload without an explicit
/// stop).
pub(crate) fn unload_all_fx_aliases<B: AudioBackend>(world: &mut World, stop_first: bool) {
    for (entity, alias) in playing_fx_entities::<B>(world) {
        let mut s = store::<B>(world);
        if stop_first {
            s.backend.stop_sound(alias);
        }
        s.backend.unload_sound_alias(alias);
        world.despawn(entity);
    }
}

/// Stop and unload every live alias of the FX loaded under `id`, despawning
/// its `PlayingFx` entity. Must run before the source sound itself is
/// unloaded, since aliases share its sample data.
fn unload_fx_aliases_of<B: AudioBackend>(world: &mut World, id: &str) {
    let aliases: Vec<(Entity, B::Sound)> = {
        let mut q = world.query::<(Entity, &PlayingFx<B::Sound>)>();
        q.iter(world)
            .filter(|(_, f)| f.source_id == id)
            .map(|(e, f)| (e, f.alias))
            .collect()
    };
    for (entity, alias) in aliases {
        let mut s = store::<B>(world);
        s.backend.stop_sound(alias);
        s.backend.unload_sound_alias(alias);
        world.despawn(entity);
    }
}

fn handle_cmd<B: AudioBackend>(world: &mut World, cmd: AudioCmd) {
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
            let loaded = store::<B>(world).backend.load_music(&c_path);
            let Some(music) = loaded else {
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
                return;
            };
            debug!(target: "audio", "loaded id='{}' path='{}'", id, path);
            let previous = store::<B>(world).music.insert(id.clone(), music);
            if let Some(old) = previous {
                // Reload over a live id: raw FFI handles don't unload on
                // drop. Stop + despawn the track (which caches `old`)
                // before unloading, so `pump_music` never touches a
                // freed stream.
                debug!(target: "audio", "unloading previous stream id='{}'", id);
                store::<B>(world).backend.stop_music(old);
                despawn_music_track::<B>(world, &id);
                store::<B>(world).backend.unload_music(old);
            }
            send(world, AudioMessage::MusicLoaded { id });
        }
        AudioCmd::PlayMusic {
            id,
            looped: want_loop,
        } => {
            if let Some(music) = music_handle::<B>(world, &id) {
                debug!(target: "audio", "play start id='{}' looped={}", id, want_loop);
                let mut s = store::<B>(world);
                s.backend.seek_music(music, 0.0);
                s.backend.play_music(music);
                despawn_music_track::<B>(world, &id);
                world.spawn(MusicTrack {
                    id: id.clone(),
                    music,
                    looped: want_loop,
                    paused: false,
                });
                send(world, AudioMessage::MusicPlayStarted { id });
            }
        }
        AudioCmd::StopMusic { id } => {
            if let Some(music) = music_handle::<B>(world, &id) {
                debug!(target: "audio", "stop id='{}'", id);
                store::<B>(world).backend.stop_music(music);
                despawn_music_track::<B>(world, &id);
                send(world, AudioMessage::MusicStopped { id });
            }
        }
        AudioCmd::StopAllMusic => {
            debug!(target: "audio", "stop all");
            for (entity, id, music, ..) in music_tracks::<B>(world) {
                store::<B>(world).backend.stop_music(music);
                send(world, AudioMessage::MusicStopped { id });
                world.despawn(entity);
            }
        }
        AudioCmd::PauseMusic { id } => {
            if let Some(music) = music_handle::<B>(world, &id) {
                debug!(target: "audio", "pause id='{}'", id);
                store::<B>(world).backend.pause_music(music);
                set_music_track_paused::<B>(world, &id, true);
                send(world, AudioMessage::MusicStopped { id });
            }
        }
        AudioCmd::ResumeMusic { id } => {
            if let Some(music) = music_handle::<B>(world, &id) {
                debug!(target: "audio", "resume id='{}'", id);
                store::<B>(world).backend.resume_music(music);
                set_music_track_paused::<B>(world, &id, false);
                send(world, AudioMessage::MusicPlayStarted { id });
            }
        }
        AudioCmd::VolumeMusic { id, vol } => {
            if let Some(music) = music_handle::<B>(world, &id) {
                debug!(target: "audio", "volume id='{}' vol={}", id, vol);
                store::<B>(world).backend.set_music_volume(music, vol);
                send(world, AudioMessage::MusicVolumeChanged { id, vol });
            }
        }
        AudioCmd::UnloadMusic { id } => {
            let removed = store::<B>(world).music.remove(&id);
            if let Some(music) = removed {
                debug!(target: "audio", "unload id='{}'", id);
                store::<B>(world).backend.unload_music(music);
                despawn_music_track::<B>(world, &id);
                send(world, AudioMessage::MusicUnloaded { id });
            }
        }
        AudioCmd::UnloadAllMusic => {
            debug!(target: "audio", "unload all");
            unload_all_music::<B>(world);
            despawn_all::<MusicTrack<B::Music>>(world);
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
            let loaded = store::<B>(world).backend.load_sound(&c_path);
            let Some(sound) = loaded else {
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
                return;
            };
            debug!(target: "audio", "fx loaded id='{}' path='{}'", id, path);
            let previous = store::<B>(world).fx.insert(id.clone(), sound);
            if let Some(old) = previous {
                // Reload over a live id: raw FFI handles don't unload on
                // drop. Drop the old sound's aliases first -- they share
                // its sample data.
                debug!(target: "audio", "unloading previous fx id='{}'", id);
                unload_fx_aliases_of::<B>(world, &id);
                store::<B>(world).backend.unload_sound(old);
            }
            send(world, AudioMessage::FxLoaded { id });
        }
        AudioCmd::PlayFx { id } => {
            if let Some(sound) = fx_handle::<B>(world, &id) {
                debug!(target: "audio", "fx play id='{}'", id);
                spawn_fx_alias::<B>(world, sound, id, None);
            } else {
                error!(target: "audio", "fx play failed id='{}' reason='not loaded'", id);
            }
        }
        AudioCmd::PlayFxPitched { id, pitch } => {
            if let Some(sound) = fx_handle::<B>(world, &id) {
                debug!(target: "audio", "fx play pitched id='{}' pitch={}", id, pitch);
                spawn_fx_alias::<B>(world, sound, id, Some(pitch));
            } else {
                error!(
                    target: "audio", "fx play pitched failed id='{}' reason='not loaded'",
                    id
                );
            }
        }
        AudioCmd::StopAllFx => {
            debug!(target: "audio", "fx stop all");
            unload_all_fx_aliases::<B>(world, true);
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
            unload_all_fx_aliases::<B>(world, false);
            unload_all_sounds::<B>(world);
            send(world, AudioMessage::FxUnloadedAll);
        }
        AudioCmd::Shutdown => {
            info!(target: "audio", "shutdown requested");
            debug!(target: "audio", "unload all");
            handle_cmd::<B>(world, AudioCmd::UnloadAllMusic);
            handle_cmd::<B>(world, AudioCmd::UnloadAllFx);
            world.resource_mut::<ShouldExit>().0 = true;
        }
    }
}

/// Unload every loaded music stream and empty `AudioStore::music`.
pub(crate) fn unload_all_music<B: AudioBackend>(world: &mut World) {
    let mut s = store::<B>(world);
    let s = &mut *s;
    for (_, music) in s.music.drain() {
        s.backend.unload_music(music);
    }
}

/// Unload every loaded sound and empty `AudioStore::fx`.
pub(crate) fn unload_all_sounds<B: AudioBackend>(world: &mut World) {
    let mut s = store::<B>(world);
    let s = &mut *s;
    for (_, sound) in s.fx.drain() {
        s.backend.unload_sound(sound);
    }
}

/// Pump music streams: update each unpaused `MusicTrack`, detect
/// end-of-track, restart looped tracks or emit `MusicFinished`. The stream
/// handle comes straight off the component (cached at `PlayMusic` time), so
/// this hot path -- it runs every audio tick regardless of commands -- never
/// looks it up in `AudioStore::music`.
pub fn pump_music<B: AudioBackend>(world: &mut World) {
    aberred_core::tracy::tracy_span!("pump_music");
    let mut ended: Vec<(Entity, String, bool)> = Vec::new();
    for (entity, id, music, looped, paused) in music_tracks::<B>(world) {
        if paused {
            continue;
        }
        let mut s = store::<B>(world);
        s.backend.update_music(music);
        let len = s.backend.music_time_length(music);
        let played = s.backend.music_time_played(music);
        if played >= len - 0.01 {
            ended.push((entity, id, looped));
        }
    }
    for (entity, id, looped) in ended {
        let music = match world.get::<MusicTrack<B::Music>>(entity) {
            Some(track) => track.music,
            None => continue,
        };
        if looped {
            debug!(target: "audio", "restarting looped id='{}'", id);
            let mut s = store::<B>(world);
            s.backend.stop_music(music);
            s.backend.seek_music(music, 0.0);
            s.backend.play_music(music);
            send(world, AudioMessage::MusicPlayStarted { id });
        } else {
            debug!(target: "audio", "finished id='{}'", id);
            store::<B>(world).backend.stop_music(music);
            world.despawn(entity);
            send(world, AudioMessage::MusicFinished { id });
        }
    }
}

/// Clean up finished sound aliases -- despawn `PlayingFx` entities whose
/// alias has stopped playing. No `AudioMessage` is sent here: FX completion
/// is silent by design, and the protocol (`AudioMessage`) intentionally has
/// no `FxFinished` variant.
pub fn pump_fx<B: AudioBackend>(world: &mut World) {
    aberred_core::tracy::tracy_span!("pump_fx");
    for (entity, alias) in playing_fx_entities::<B>(world) {
        let mut s = store::<B>(world);
        if !s.backend.is_sound_playing(alias) {
            s.backend.unload_sound_alias(alias);
            world.despawn(entity);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::Schedule;
    use crossbeam_channel::{Receiver, Sender};

    use super::*;
    use crate::resources::backend::testing::RecordingBackend;
    use crate::systems::world::build_world;

    /// The real audio world + schedule over a [`RecordingBackend`].
    struct Harness {
        world: World,
        schedule: Schedule,
        tx_cmd: Option<Sender<AudioCmd>>,
        rx_evt: Receiver<AudioMessage>,
    }

    impl Harness {
        fn new() -> Self {
            let (tx_cmd, rx_cmd) = crossbeam_channel::unbounded::<AudioCmd>();
            let (tx_evt, rx_evt) = crossbeam_channel::unbounded::<AudioMessage>();
            let (world, schedule) = build_world(RecordingBackend::default(), rx_cmd, tx_evt);
            Self {
                world,
                schedule,
                tx_cmd: Some(tx_cmd),
                rx_evt,
            }
        }

        /// Queue `cmds`, run one audio tick, return the replies (as `Debug`
        /// strings -- `AudioMessage` has no `PartialEq`).
        fn tick(&mut self, cmds: impl IntoIterator<Item = AudioCmd>) -> Vec<String> {
            for cmd in cmds {
                self.tx_cmd.as_ref().unwrap().send(cmd).unwrap();
            }
            self.schedule.run(&mut self.world);
            self.rx_evt.try_iter().map(|m| format!("{m:?}")).collect()
        }

        fn store(&mut self) -> &mut AudioStore<RecordingBackend> {
            self.world
                .get_non_send_mut::<AudioStore<RecordingBackend>>()
                .unwrap()
                .into_inner()
        }

        /// Backend calls since the last `take_calls`.
        fn take_calls(&mut self) -> Vec<String> {
            std::mem::take(&mut self.store().backend.calls)
        }
    }

    #[test]
    fn load_music_stores_the_handle_and_replies_loaded() {
        let mut h = Harness::new();
        let replies = h.tick([AudioCmd::LoadMusic {
            id: "theme".into(),
            path: "theme.ogg".into(),
        }]);
        assert_eq!(replies, [r#"MusicLoaded { id: "theme" }"#]);
        assert_eq!(h.take_calls(), ["load_music(theme.ogg) -> 1"]);
        assert_eq!(h.store().music.get("theme"), Some(&1));
    }

    #[test]
    fn drain_cmds_requests_exit_once_every_sender_is_gone() {
        let mut h = Harness::new();
        h.tick([]);
        assert!(!h.world.resource::<ShouldExit>().0, "live sender: keep running");

        h.tx_cmd = None;
        h.tick([]);
        assert!(h.world.resource::<ShouldExit>().0, "disconnected: exit");
    }
}
