//! Types owned by the audio thread's own `bevy_ecs::World` (Phase 7e).
//!
//! Nothing here is ever inserted into or read from the sim/logic world --
//! the only contract between the two worlds is `AudioCmd`/`AudioMessage`
//! (`crate::protocol::audio`), carried over the channels wrapped by
//! `CmdReceiver`/`MsgSender` below.

use bevy_ecs::prelude::{Component, Resource};
use crossbeam_channel::{Receiver, Sender};
use raylib::core::audio::RaylibAudio;
use raylib::ffi;
use rustc_hash::FxHashMap;

use crate::protocol::audio::{AudioCmd, AudioMessage};

/// NonSend store for the audio thread's device and raw handles.
///
/// PORT NOTE: music is kept as raw `ffi::Music` (not the safe `Music<'a>`
/// wrapper) to avoid a self-referential borrow of `device` living inside a
/// `World` value -- the same reason `fx`/FX aliases already used raw `ffi`
/// handles before this phase. `device` is the last field so it drops last
/// (raylib requires the device to outlive every Music/Sound handle).
pub struct AudioStore {
    pub music: FxHashMap<String, ffi::Music>,
    pub fx: FxHashMap<String, ffi::Sound>,
    // Never read directly -- held only so it drops last (Rust drops struct
    // fields in declaration order), after every Music/Sound handle above.
    #[allow(dead_code)]
    pub device: RaylibAudio,
}

/// Wraps a `Copy` FFI handle (`ffi::Sound`, `ffi::Music`, ...) so it
/// satisfies `Component`'s unconditional `Send + Sync` bound. SAFETY: these
/// handles hold raw FFI pointers that are only ever touched from the single
/// audio thread that owns this whole `World` -- they never actually cross a
/// thread boundary. Asserting Send+Sync here just satisfies bevy_ecs's
/// compile-time requirement (unlike `NonSend` resources, `Component` cannot
/// opt out of it), not a real cross-thread access pattern.
#[derive(Clone, Copy)]
pub struct FfiHandle<T: Copy>(pub T);
unsafe impl<T: Copy> Send for FfiHandle<T> {}
unsafe impl<T: Copy> Sync for FfiHandle<T> {}

pub type SoundHandle = FfiHandle<ffi::Sound>;
pub type MusicHandle = FfiHandle<ffi::Music>;

/// One entity per live FX alias (`LoadSoundAlias`). The alias handle is
/// `Copy`, so it's stored directly on the component -- no separate
/// entity-keyed map is needed in `AudioStore`.
#[derive(Component)]
pub struct PlayingFx {
    pub alias: SoundHandle,
}

/// One entity per music id that is currently playing-or-paused. A loaded
/// but not (yet) playing track has no `MusicTrack` entity -- it lives only
/// in `AudioStore::music`, matching the pre-7e `playing`/`looped` set
/// semantics (any number of tracks can be loaded; only playing/paused ones
/// are pumped). The stream handle is cached here (rather than re-looked-up
/// from `AudioStore::music` by `id` every tick) since `pump_music` runs
/// every audio tick regardless of commands.
#[derive(Component)]
pub struct MusicTrack {
    pub id: String,
    pub music: MusicHandle,
    pub looped: bool,
    pub paused: bool,
}

#[derive(Resource)]
pub struct CmdReceiver(pub Receiver<AudioCmd>);

#[derive(Resource)]
pub struct MsgSender(pub Sender<AudioMessage>);

#[derive(Resource, Default)]
pub struct ShouldExit(pub bool);
