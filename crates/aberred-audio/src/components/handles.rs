//! `Copy` FFI handle wrapper used by audio-world components.

use raylib::ffi;

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
