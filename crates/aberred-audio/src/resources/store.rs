//! NonSend store for the audio thread's backend and loaded handles.

use rustc_hash::FxHashMap;

use super::backend::AudioBackend;

/// NonSend store for the audio thread's [`AudioBackend`] and the handles of
/// every loaded music stream / sound, keyed by id.
///
/// PORT NOTE: with the raylib backend, handles are raw `ffi::Music`/
/// `ffi::Sound` (not the safe `Music<'a>` wrappers) to avoid a
/// self-referential borrow of the audio device living inside a `World`
/// value. The device itself is a local of `audio_thread`, declared before
/// the world so it drops after it (raylib requires the device to outlive
/// every Music/Sound handle); keeping it out of the store is what lets the
/// world be built and run without one (see `build_world`).
pub struct AudioStore<B: AudioBackend> {
    pub backend: B,
    pub music: FxHashMap<String, B::Music>,
    pub fx: FxHashMap<String, B::Sound>,
}

impl<B: AudioBackend> AudioStore<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            music: FxHashMap::default(),
            fx: FxHashMap::default(),
        }
    }
}
