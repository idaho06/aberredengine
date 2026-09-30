//! The audio FFI surface the pipeline calls through, so the command/pump
//! logic runs unchanged against a recording fake in tests.
//!
//! [`RaylibBackend`] is the production implementation: one raylib FFI call
//! per method, no logic of its own (the only branch is the null-buffer check
//! that turns a failed load into `None`). It never opens the audio device --
//! `audio_thread` owns that separately and keeps it alive for as long as the
//! world holding the backend exists.

use std::ffi::CStr;

use raylib::ffi;

use crate::components::handles::{FfiHandle, MusicHandle, SoundHandle};

/// Every audio FFI operation the audio world performs. Handles are opaque
/// `Copy` values the backend hands out and later receives back.
pub trait AudioBackend: 'static {
    type Music: Copy + Send + Sync + 'static;
    type Sound: Copy + Send + Sync + 'static;

    /// `None` when the stream fails to load.
    fn load_music(&mut self, path: &CStr) -> Option<Self::Music>;
    fn unload_music(&mut self, music: Self::Music);
    fn play_music(&mut self, music: Self::Music);
    fn stop_music(&mut self, music: Self::Music);
    fn pause_music(&mut self, music: Self::Music);
    fn resume_music(&mut self, music: Self::Music);
    fn seek_music(&mut self, music: Self::Music, position: f32);
    fn set_music_volume(&mut self, music: Self::Music, volume: f32);
    fn update_music(&mut self, music: Self::Music);
    fn music_time_length(&mut self, music: Self::Music) -> f32;
    fn music_time_played(&mut self, music: Self::Music) -> f32;

    /// `None` when the sound fails to load.
    fn load_sound(&mut self, path: &CStr) -> Option<Self::Sound>;
    fn unload_sound(&mut self, sound: Self::Sound);
    fn load_sound_alias(&mut self, sound: Self::Sound) -> Self::Sound;
    fn unload_sound_alias(&mut self, alias: Self::Sound);
    fn set_sound_pitch(&mut self, sound: Self::Sound, pitch: f32);
    fn play_sound(&mut self, sound: Self::Sound);
    fn stop_sound(&mut self, sound: Self::Sound);
    fn is_sound_playing(&mut self, sound: Self::Sound) -> bool;
}

/// Production [`AudioBackend`]: raylib's audio FFI.
#[derive(Default)]
pub struct RaylibBackend;

impl AudioBackend for RaylibBackend {
    type Music = MusicHandle;
    type Sound = SoundHandle;

    fn load_music(&mut self, path: &CStr) -> Option<MusicHandle> {
        let music = unsafe { ffi::LoadMusicStream(path.as_ptr()) };
        (!music.stream.buffer.is_null()).then_some(FfiHandle(music))
    }
    fn unload_music(&mut self, music: MusicHandle) {
        unsafe { ffi::UnloadMusicStream(music.0) }
    }
    fn play_music(&mut self, music: MusicHandle) {
        unsafe { ffi::PlayMusicStream(music.0) }
    }
    fn stop_music(&mut self, music: MusicHandle) {
        unsafe { ffi::StopMusicStream(music.0) }
    }
    fn pause_music(&mut self, music: MusicHandle) {
        unsafe { ffi::PauseMusicStream(music.0) }
    }
    fn resume_music(&mut self, music: MusicHandle) {
        unsafe { ffi::ResumeMusicStream(music.0) }
    }
    fn seek_music(&mut self, music: MusicHandle, position: f32) {
        unsafe { ffi::SeekMusicStream(music.0, position) }
    }
    fn set_music_volume(&mut self, music: MusicHandle, volume: f32) {
        unsafe { ffi::SetMusicVolume(music.0, volume) }
    }
    fn update_music(&mut self, music: MusicHandle) {
        unsafe { ffi::UpdateMusicStream(music.0) }
    }
    fn music_time_length(&mut self, music: MusicHandle) -> f32 {
        unsafe { ffi::GetMusicTimeLength(music.0) }
    }
    fn music_time_played(&mut self, music: MusicHandle) -> f32 {
        unsafe { ffi::GetMusicTimePlayed(music.0) }
    }

    fn load_sound(&mut self, path: &CStr) -> Option<SoundHandle> {
        let sound = unsafe { ffi::LoadSound(path.as_ptr()) };
        (!sound.stream.buffer.is_null()).then_some(FfiHandle(sound))
    }
    fn unload_sound(&mut self, sound: SoundHandle) {
        unsafe { ffi::UnloadSound(sound.0) }
    }
    fn load_sound_alias(&mut self, sound: SoundHandle) -> SoundHandle {
        FfiHandle(unsafe { ffi::LoadSoundAlias(sound.0) })
    }
    fn unload_sound_alias(&mut self, alias: SoundHandle) {
        unsafe { ffi::UnloadSoundAlias(alias.0) }
    }
    fn set_sound_pitch(&mut self, sound: SoundHandle, pitch: f32) {
        unsafe { ffi::SetSoundPitch(sound.0, pitch) }
    }
    fn play_sound(&mut self, sound: SoundHandle) {
        unsafe { ffi::PlaySound(sound.0) }
    }
    fn stop_sound(&mut self, sound: SoundHandle) {
        unsafe { ffi::StopSound(sound.0) }
    }
    fn is_sound_playing(&mut self, sound: SoundHandle) -> bool {
        unsafe { ffi::IsSoundPlaying(sound.0) }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use rustc_hash::FxHashSet;

    /// Length every fake music stream reports, in seconds.
    pub(crate) const MUSIC_LEN: f32 = 10.0;

    /// Test [`AudioBackend`]: hands out `u32` handles (1, 2, ...) and logs
    /// every state-changing call in order. Queries (`music_time_*`,
    /// `is_sound_playing`) are answered from the knobs below, not logged.
    #[derive(Default)]
    pub(crate) struct RecordingBackend {
        pub calls: Vec<String>,
        next_handle: u32,
        /// Loads of these paths fail (`None`).
        pub fail_paths: FxHashSet<String>,
        /// Music handle -> seconds played (default 0).
        pub played: rustc_hash::FxHashMap<u32, f32>,
        /// Sound/alias handles that have stopped playing.
        pub finished: FxHashSet<u32>,
    }

    impl RecordingBackend {
        fn load(&mut self, what: &str, path: &CStr) -> Option<u32> {
            let path = path.to_string_lossy().into_owned();
            let handle = (!self.fail_paths.contains(&path)).then(|| {
                self.next_handle += 1;
                self.next_handle
            });
            match handle {
                Some(h) => self.calls.push(format!("{what}({path}) -> {h}")),
                None => self.calls.push(format!("{what}({path}) -> None")),
            }
            handle
        }

        fn log(&mut self, call: String) {
            self.calls.push(call);
        }
    }

    impl AudioBackend for RecordingBackend {
        type Music = u32;
        type Sound = u32;

        fn load_music(&mut self, path: &CStr) -> Option<u32> {
            self.load("load_music", path)
        }
        fn unload_music(&mut self, m: u32) {
            self.log(format!("unload_music({m})"))
        }
        fn play_music(&mut self, m: u32) {
            self.log(format!("play_music({m})"))
        }
        fn stop_music(&mut self, m: u32) {
            self.log(format!("stop_music({m})"))
        }
        fn pause_music(&mut self, m: u32) {
            self.log(format!("pause_music({m})"))
        }
        fn resume_music(&mut self, m: u32) {
            self.log(format!("resume_music({m})"))
        }
        fn seek_music(&mut self, m: u32, position: f32) {
            self.log(format!("seek_music({m}, {position})"))
        }
        fn set_music_volume(&mut self, m: u32, volume: f32) {
            self.log(format!("set_music_volume({m}, {volume})"))
        }
        fn update_music(&mut self, m: u32) {
            self.log(format!("update_music({m})"))
        }
        fn music_time_length(&mut self, _: u32) -> f32 {
            MUSIC_LEN
        }
        fn music_time_played(&mut self, m: u32) -> f32 {
            self.played.get(&m).copied().unwrap_or(0.0)
        }

        fn load_sound(&mut self, path: &CStr) -> Option<u32> {
            self.load("load_sound", path)
        }
        fn unload_sound(&mut self, s: u32) {
            self.log(format!("unload_sound({s})"))
        }
        fn load_sound_alias(&mut self, s: u32) -> u32 {
            self.next_handle += 1;
            let alias = self.next_handle;
            self.log(format!("load_sound_alias({s}) -> {alias}"));
            alias
        }
        fn unload_sound_alias(&mut self, a: u32) {
            self.log(format!("unload_sound_alias({a})"))
        }
        fn set_sound_pitch(&mut self, s: u32, pitch: f32) {
            self.log(format!("set_sound_pitch({s}, {pitch})"))
        }
        fn play_sound(&mut self, s: u32) {
            self.log(format!("play_sound({s})"))
        }
        fn stop_sound(&mut self, s: u32) {
            self.log(format!("stop_sound({s})"))
        }
        fn is_sound_playing(&mut self, s: u32) -> bool {
            !self.finished.contains(&s)
        }
    }
}
