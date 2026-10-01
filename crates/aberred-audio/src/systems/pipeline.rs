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
use log::{debug, error, info, warn};

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
                // Only a playing/paused stream has a MusicTrack for
                // `pump_music` to update; resuming one that was never played
                // (or was stopped) would start a stream nothing pumps.
                if find_music_track::<B>(world, &id).is_none() {
                    warn!(target: "audio", "resume ignored id='{}' reason='not playing or paused'", id);
                    return;
                }
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
                // Same order as a reload: stop and drop the track (which
                // caches `music`) before unloading the stream.
                store::<B>(world).backend.stop_music(music);
                despawn_music_track::<B>(world, &id);
                store::<B>(world).backend.unload_music(music);
                send(world, AudioMessage::MusicUnloaded { id });
            }
        }
        AudioCmd::UnloadAllMusic => {
            debug!(target: "audio", "unload all");
            // Same order as a single UnloadMusic: stop and drop every
            // playing/paused track before any stream is unloaded.
            for (entity, _, music, ..) in music_tracks::<B>(world) {
                store::<B>(world).backend.stop_music(music);
                world.despawn(entity);
            }
            unload_all_music::<B>(world);
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

        /// `(id, looped, paused)` of every `MusicTrack`, sorted by id.
        fn tracks(&mut self) -> Vec<(String, bool, bool)> {
            let mut q = self.world.query::<&MusicTrack<u32>>();
            let mut tracks: Vec<_> = q
                .iter(&self.world)
                .map(|t| (t.id.clone(), t.looped, t.paused))
                .collect();
            tracks.sort();
            tracks
        }

        /// `(alias, source_id)` of every `PlayingFx`, sorted by alias.
        fn fx_aliases(&mut self) -> Vec<(u32, String)> {
            let mut q = self.world.query::<&PlayingFx<u32>>();
            let mut aliases: Vec<_> = q
                .iter(&self.world)
                .map(|f| (f.alias, f.source_id.clone()))
                .collect();
            aliases.sort();
            aliases
        }

        /// Run `cmds` for their side effects, discarding replies and calls.
        fn setup(&mut self, cmds: impl IntoIterator<Item = AudioCmd>) {
            self.tick(cmds);
            self.take_calls();
        }
    }

    fn load_music(id: &str) -> AudioCmd {
        AudioCmd::LoadMusic {
            id: id.into(),
            path: format!("{id}.ogg"),
        }
    }

    fn play_music(id: &str, looped: bool) -> AudioCmd {
        AudioCmd::PlayMusic {
            id: id.into(),
            looped,
        }
    }

    fn track(id: &str, looped: bool, paused: bool) -> (String, bool, bool) {
        (id.into(), looped, paused)
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
    fn reloading_music_stops_and_unloads_the_old_stream_without_a_stopped_reply() {
        let mut h = Harness::new();
        h.setup([load_music("theme"), play_music("theme", true)]);

        let replies = h.tick([load_music("theme")]);

        assert_eq!(replies, [r#"MusicLoaded { id: "theme" }"#]);
        assert_eq!(
            h.take_calls(),
            [
                "load_music(theme.ogg) -> 2",
                "stop_music(1)",
                "unload_music(1)"
            ]
        );
        assert_eq!(h.tracks(), [], "the old stream's track is gone");
        assert_eq!(h.store().music.get("theme"), Some(&2));
    }

    #[test]
    fn music_load_failures_reply_load_failed_and_store_nothing() {
        let mut h = Harness::new();
        h.store().backend.fail_paths.insert("bad.ogg".into());

        let replies = h.tick([
            AudioCmd::LoadMusic {
                id: "nul".into(),
                path: "a\0b.ogg".into(),
            },
            AudioCmd::LoadMusic {
                id: "bad".into(),
                path: "bad.ogg".into(),
            },
        ]);

        assert_eq!(replies.len(), 2);
        assert!(
            replies[0].starts_with(r#"MusicLoadFailed { id: "nul", error: "invalid path: "#),
            "{}",
            replies[0]
        );
        assert_eq!(
            replies[1],
            r#"MusicLoadFailed { id: "bad", error: "failed to load" }"#
        );
        assert_eq!(
            h.take_calls(),
            ["load_music(bad.ogg) -> None"],
            "no FFI for a NUL path"
        );
        assert!(h.store().music.is_empty());
    }

    #[test]
    fn play_music_rewinds_starts_and_replaces_the_track() {
        let mut h = Harness::new();
        h.setup([load_music("theme")]);

        let replies = h.tick([play_music("theme", true)]);
        assert_eq!(replies, [r#"MusicPlayStarted { id: "theme" }"#]);
        assert_eq!(
            h.take_calls(),
            ["seek_music(1, 0)", "play_music(1)", "update_music(1)"],
            "rewind, start, then pumped in the same tick"
        );
        assert_eq!(h.tracks(), [track("theme", true, false)]);

        h.tick([play_music("theme", false)]);
        assert_eq!(
            h.tracks(),
            [track("theme", false, false)],
            "one track per id"
        );
    }

    #[test]
    fn by_id_music_commands_on_an_unknown_id_are_silent_no_ops() {
        let mut h = Harness::new();
        let replies = h.tick([
            play_music("ghost", false),
            AudioCmd::StopMusic { id: "ghost".into() },
            AudioCmd::PauseMusic { id: "ghost".into() },
            AudioCmd::ResumeMusic { id: "ghost".into() },
            AudioCmd::VolumeMusic {
                id: "ghost".into(),
                vol: 0.5,
            },
            AudioCmd::UnloadMusic { id: "ghost".into() },
        ]);
        assert_eq!(replies, Vec::<String>::new());
        assert_eq!(h.take_calls(), Vec::<String>::new());
    }

    #[test]
    fn stop_music_replies_stopped_even_when_the_stream_was_not_playing() {
        let mut h = Harness::new();
        h.setup([load_music("theme")]);

        let replies = h.tick([AudioCmd::StopMusic { id: "theme".into() }]);

        assert_eq!(replies, [r#"MusicStopped { id: "theme" }"#]);
        assert_eq!(h.take_calls(), ["stop_music(1)"]);
    }

    #[test]
    fn stop_all_music_stops_only_playing_tracks_and_keeps_them_loaded() {
        let mut h = Harness::new();
        h.setup([load_music("a"), load_music("b"), play_music("a", false)]);

        let replies = h.tick([AudioCmd::StopAllMusic]);

        assert_eq!(replies, [r#"MusicStopped { id: "a" }"#]);
        assert_eq!(h.take_calls(), ["stop_music(1)"]);
        assert_eq!(h.tracks(), []);
        assert_eq!(h.store().music.len(), 2);
    }

    #[test]
    fn pause_and_resume_toggle_the_track_and_its_pumping() {
        let mut h = Harness::new();
        h.setup([load_music("theme"), play_music("theme", false)]);

        let replies = h.tick([AudioCmd::PauseMusic { id: "theme".into() }]);
        // Pinned as-is: pause reuses the stop reply.
        assert_eq!(replies, [r#"MusicStopped { id: "theme" }"#]);
        assert_eq!(
            h.take_calls(),
            ["pause_music(1)"],
            "a paused track is not pumped"
        );
        assert_eq!(h.tracks(), [track("theme", false, true)]);

        let replies = h.tick([AudioCmd::ResumeMusic { id: "theme".into() }]);
        assert_eq!(replies, [r#"MusicPlayStarted { id: "theme" }"#]);
        assert_eq!(h.take_calls(), ["resume_music(1)", "update_music(1)"]);
        assert_eq!(h.tracks(), [track("theme", false, false)]);
    }

    #[test]
    fn resume_without_a_track_is_ignored() {
        let mut h = Harness::new();
        h.setup([
            load_music("never_played"),
            load_music("stopped"),
            play_music("stopped", false),
            AudioCmd::StopMusic {
                id: "stopped".into(),
            },
        ]);

        let replies = h.tick([
            AudioCmd::ResumeMusic {
                id: "never_played".into(),
            },
            AudioCmd::ResumeMusic {
                id: "stopped".into(),
            },
        ]);

        // Nothing would ever pump such a stream, so claiming it started
        // playing would be a lie.
        assert_eq!(replies, Vec::<String>::new());
        assert_eq!(h.take_calls(), Vec::<String>::new());
        assert_eq!(h.tracks(), []);
    }

    #[test]
    fn volume_music_sets_the_volume_and_echoes_it() {
        let mut h = Harness::new();
        h.setup([load_music("theme")]);

        let replies = h.tick([AudioCmd::VolumeMusic {
            id: "theme".into(),
            vol: 0.5,
        }]);

        assert_eq!(replies, [r#"MusicVolumeChanged { id: "theme", vol: 0.5 }"#]);
        assert_eq!(h.take_calls(), ["set_music_volume(1, 0.5)"]);
    }

    #[test]
    fn unload_music_stops_the_stream_before_unloading_and_drops_the_track() {
        let mut h = Harness::new();
        h.setup([load_music("theme"), play_music("theme", false)]);

        let replies = h.tick([AudioCmd::UnloadMusic { id: "theme".into() }]);

        assert_eq!(replies, [r#"MusicUnloaded { id: "theme" }"#]);
        // Same order as a reload: stop, drop the track, then unload.
        assert_eq!(h.take_calls(), ["stop_music(1)", "unload_music(1)"]);
        assert_eq!(h.tracks(), []);
        assert!(h.store().music.is_empty());
    }

    #[test]
    fn unload_all_music_stops_playing_streams_then_unloads_every_stream() {
        let mut h = Harness::new();
        assert_eq!(h.tick([AudioCmd::UnloadAllMusic]), ["MusicUnloadedAll"]);

        h.setup([load_music("a"), load_music("b"), play_music("a", true)]);
        let replies = h.tick([AudioCmd::UnloadAllMusic]);

        assert_eq!(replies, ["MusicUnloadedAll"]);
        let mut calls = h.take_calls();
        assert_eq!(
            calls.first().map(String::as_str),
            Some("stop_music(1)"),
            "only the playing stream is stopped, before any unload: {calls:?}"
        );
        calls.remove(0);
        calls.sort();
        assert_eq!(calls, ["unload_music(1)", "unload_music(2)"]);
        assert_eq!(h.tracks(), []);
        assert!(h.store().music.is_empty());
    }

    fn load_fx(id: &str) -> AudioCmd {
        AudioCmd::LoadFx {
            id: id.into(),
            path: format!("{id}.wav"),
        }
    }

    fn play_fx(id: &str) -> AudioCmd {
        AudioCmd::PlayFx { id: id.into() }
    }

    #[test]
    fn load_fx_stores_the_sound_and_replies_loaded() {
        let mut h = Harness::new();
        let replies = h.tick([load_fx("hit")]);
        assert_eq!(replies, [r#"FxLoaded { id: "hit" }"#]);
        assert_eq!(h.take_calls(), ["load_sound(hit.wav) -> 1"]);
        assert_eq!(h.store().fx.get("hit"), Some(&1));
    }

    #[test]
    fn fx_load_failures_reply_load_failed_and_store_nothing() {
        let mut h = Harness::new();
        h.store().backend.fail_paths.insert("bad.wav".into());

        let replies = h.tick([
            AudioCmd::LoadFx {
                id: "nul".into(),
                path: "a\0b.wav".into(),
            },
            load_fx("bad"),
        ]);

        assert_eq!(replies.len(), 2);
        assert!(
            replies[0].starts_with(r#"FxLoadFailed { id: "nul", error: "invalid path: "#),
            "{}",
            replies[0]
        );
        assert_eq!(
            replies[1],
            r#"FxLoadFailed { id: "bad", error: "failed to load" }"#
        );
        assert_eq!(h.take_calls(), ["load_sound(bad.wav) -> None"]);
        assert!(h.store().fx.is_empty());
    }

    #[test]
    fn reloading_fx_drops_only_that_sounds_aliases_before_unloading_it() {
        let mut h = Harness::new();
        // hit = 1, jump = 2, their aliases 3 and 4.
        h.setup([
            load_fx("hit"),
            load_fx("jump"),
            play_fx("hit"),
            play_fx("jump"),
        ]);

        let replies = h.tick([load_fx("hit")]);

        assert_eq!(replies, [r#"FxLoaded { id: "hit" }"#]);
        assert_eq!(
            h.take_calls(),
            [
                "load_sound(hit.wav) -> 5",
                "stop_sound(3)",
                "unload_sound_alias(3)",
                "unload_sound(1)"
            ]
        );
        assert_eq!(h.fx_aliases(), [(4, "jump".to_string())]);
        assert_eq!(h.store().fx.get("hit"), Some(&5));
    }

    #[test]
    fn play_fx_spawns_a_playing_alias_and_sends_no_reply() {
        let mut h = Harness::new();
        h.setup([load_fx("hit")]);

        let replies = h.tick([
            play_fx("hit"),
            AudioCmd::PlayFxPitched {
                id: "hit".into(),
                pitch: 1.5,
            },
        ]);

        assert_eq!(replies, Vec::<String>::new());
        assert_eq!(
            h.take_calls(),
            [
                "load_sound_alias(1) -> 2",
                "play_sound(2)",
                "load_sound_alias(1) -> 3",
                "set_sound_pitch(3, 1.5)",
                "play_sound(3)"
            ],
            "pitch is applied before playback starts"
        );
        assert_eq!(
            h.fx_aliases(),
            [(2, "hit".to_string()), (3, "hit".to_string())]
        );
    }

    #[test]
    fn play_fx_of_an_unloaded_sound_is_silent() {
        let mut h = Harness::new();
        let replies = h.tick([
            play_fx("ghost"),
            AudioCmd::PlayFxPitched {
                id: "ghost".into(),
                pitch: 2.0,
            },
        ]);
        assert_eq!(replies, Vec::<String>::new());
        assert_eq!(h.take_calls(), Vec::<String>::new());
        assert_eq!(h.fx_aliases(), []);
    }

    #[test]
    fn stop_all_fx_stops_aliases_but_keeps_sounds_loaded() {
        let mut h = Harness::new();
        h.setup([load_fx("hit"), play_fx("hit"), play_fx("hit")]);

        let replies = h.tick([AudioCmd::StopAllFx]);

        assert_eq!(replies, Vec::<String>::new());
        let calls = h.take_calls();
        for alias in [2, 3] {
            let stop = calls
                .iter()
                .position(|c| *c == format!("stop_sound({alias})"));
            let unload = calls
                .iter()
                .position(|c| *c == format!("unload_sound_alias({alias})"));
            assert!(stop.is_some() && stop < unload, "{calls:?}");
        }
        assert_eq!(calls.len(), 4, "{calls:?}");
        assert_eq!(h.fx_aliases(), []);
        assert_eq!(h.store().fx.get("hit"), Some(&1), "the sound stays loaded");
    }

    #[test]
    fn unload_all_fx_unloads_aliases_without_stopping_then_every_sound() {
        let mut h = Harness::new();
        h.setup([load_fx("hit"), play_fx("hit")]);

        let replies = h.tick([AudioCmd::UnloadAllFx]);

        assert_eq!(replies, ["FxUnloadedAll"]);
        assert_eq!(h.take_calls(), ["unload_sound_alias(2)", "unload_sound(1)"]);
        assert_eq!(h.fx_aliases(), []);
        assert!(h.store().fx.is_empty());
    }

    #[test]
    fn unload_fx_by_id_is_a_no_op() {
        let mut h = Harness::new();
        h.setup([load_fx("hit"), play_fx("hit")]);

        // Pinned as-is: per-id unload is ignored and FxUnloaded is never sent.
        let replies = h.tick([AudioCmd::UnloadFx { id: "hit".into() }]);

        assert_eq!(replies, Vec::<String>::new());
        assert_eq!(h.take_calls(), Vec::<String>::new());
        assert_eq!(h.fx_aliases(), [(2, "hit".to_string())]);
        assert_eq!(h.store().fx.get("hit"), Some(&1));
    }

    #[test]
    fn shutdown_unloads_everything_replies_twice_and_requests_exit() {
        let mut h = Harness::new();
        h.setup([
            load_music("theme"),
            play_music("theme", true),
            load_fx("hit"),
            play_fx("hit"),
        ]);

        let replies = h.tick([AudioCmd::Shutdown]);

        assert_eq!(replies, ["MusicUnloadedAll", "FxUnloadedAll"]);
        assert!(h.world.resource::<ShouldExit>().0);
        assert_eq!(h.tracks(), []);
        assert_eq!(h.fx_aliases(), []);
        assert!(h.store().music.is_empty() && h.store().fx.is_empty());
    }

    #[test]
    fn a_command_queued_before_disconnect_is_applied_before_exiting() {
        let mut h = Harness::new();
        h.tx_cmd.take().unwrap().send(load_music("theme")).unwrap();

        let replies = h.tick([]);

        assert_eq!(replies, [r#"MusicLoaded { id: "theme" }"#]);
        assert!(h.world.resource::<ShouldExit>().0);
    }

    /// Sets how far `theme` (music handle 1) has played.
    fn set_played(h: &mut Harness, seconds: f32) {
        h.store().backend.played.insert(1, seconds);
    }

    #[test]
    fn pump_music_only_ends_a_track_within_10ms_of_its_length() {
        use crate::resources::backend::testing::MUSIC_LEN;
        let mut h = Harness::new();
        h.setup([load_music("theme"), play_music("theme", false)]);

        set_played(&mut h, MUSIC_LEN - 0.02);
        assert_eq!(h.tick([]), Vec::<String>::new());
        assert_eq!(h.take_calls(), ["update_music(1)"]);

        set_played(&mut h, MUSIC_LEN - 0.005);
        assert_eq!(h.tick([]), [r#"MusicFinished { id: "theme" }"#]);
        assert_eq!(h.take_calls(), ["update_music(1)", "stop_music(1)"]);
        assert_eq!(h.tracks(), []);
    }

    #[test]
    fn pump_music_restarts_a_looped_track_and_reports_it_started() {
        use crate::resources::backend::testing::MUSIC_LEN;
        let mut h = Harness::new();
        h.setup([load_music("theme"), play_music("theme", true)]);
        set_played(&mut h, MUSIC_LEN);

        let replies = h.tick([]);

        assert_eq!(replies, [r#"MusicPlayStarted { id: "theme" }"#]);
        assert_eq!(
            h.take_calls(),
            [
                "update_music(1)",
                "stop_music(1)",
                "seek_music(1, 0)",
                "play_music(1)"
            ]
        );
        assert_eq!(h.tracks(), [track("theme", true, false)]);
    }

    #[test]
    fn pump_music_skips_paused_tracks() {
        use crate::resources::backend::testing::MUSIC_LEN;
        let mut h = Harness::new();
        h.setup([
            load_music("theme"),
            play_music("theme", false),
            AudioCmd::PauseMusic { id: "theme".into() },
        ]);
        set_played(&mut h, MUSIC_LEN);

        assert_eq!(h.tick([]), Vec::<String>::new());
        assert_eq!(h.take_calls(), Vec::<String>::new());
        assert_eq!(h.tracks(), [track("theme", false, true)]);
    }

    #[test]
    fn pump_fx_silently_unloads_only_finished_aliases() {
        let mut h = Harness::new();
        h.setup([load_fx("hit"), play_fx("hit"), play_fx("hit")]);
        h.store().backend.finished.insert(2);

        assert_eq!(h.tick([]), Vec::<String>::new());
        assert_eq!(h.take_calls(), ["unload_sound_alias(2)"]);
        assert_eq!(h.fx_aliases(), [(3, "hit".to_string())]);
    }

    #[test]
    fn teardown_unloads_every_alias_stream_and_sound() {
        let mut h = Harness::new();
        h.setup([
            load_music("theme"),
            play_music("theme", false),
            load_fx("hit"),
            play_fx("hit"),
        ]);

        crate::systems::world::teardown::<RecordingBackend>(&mut h.world);

        // Pinned as-is: tracks are despawned without a stop_music first.
        assert_eq!(
            h.take_calls(),
            [
                "unload_sound_alias(3)",
                "unload_music(1)",
                "unload_sound(2)"
            ]
        );
        assert_eq!(h.tracks(), []);
        assert_eq!(h.fx_aliases(), []);
        assert!(h.store().music.is_empty() && h.store().fx.is_empty());
    }

    #[test]
    fn drain_cmds_requests_exit_once_every_sender_is_gone() {
        let mut h = Harness::new();
        h.tick([]);
        assert!(
            !h.world.resource::<ShouldExit>().0,
            "live sender: keep running"
        );

        h.tx_cmd = None;
        h.tick([]);
        assert!(h.world.resource::<ShouldExit>().0, "disconnected: exit");
    }
}
