//! NonSend store for the audio thread's device and raw handles.

use raylib::core::audio::RaylibAudio;
use raylib::ffi;
use rustc_hash::FxHashMap;

/// NonSend store for the audio thread's device and raw handles.
///
/// PORT NOTE: music is kept as raw `ffi::Music` (not the safe `Music<'a>`
/// wrapper) to avoid a self-referential borrow of `device` living inside a
/// `World` value -- the same reason `fx`/FX aliases use raw `ffi` handles
/// too. `device` is the last field so it drops last (raylib requires the
/// device to outlive every Music/Sound handle).
pub struct AudioStore {
    pub music: FxHashMap<String, ffi::Music>,
    pub fx: FxHashMap<String, ffi::Sound>,
    // Never read directly -- held only so it drops last (Rust drops struct
    // fields in declaration order), after every Music/Sound handle above.
    #[allow(dead_code)]
    pub device: RaylibAudio,
}
