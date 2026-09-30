//! NonSend store for the audio thread's raw handles.

use raylib::ffi;
use rustc_hash::FxHashMap;

/// NonSend store for the audio thread's raw handles.
///
/// PORT NOTE: music is kept as raw `ffi::Music` (not the safe `Music<'a>`
/// wrapper) to avoid a self-referential borrow of the audio device living
/// inside a `World` value -- the same reason `fx`/FX aliases use raw `ffi`
/// handles too. The device itself is a local of `audio_thread`, declared
/// before the world so it drops after it (raylib requires the device to
/// outlive every Music/Sound handle); keeping it out of the store is what
/// lets the world be built and run without one (see `build_world`).
#[derive(Default)]
pub struct AudioStore {
    pub music: FxHashMap<String, ffi::Music>,
    pub fx: FxHashMap<String, ffi::Sound>,
}
