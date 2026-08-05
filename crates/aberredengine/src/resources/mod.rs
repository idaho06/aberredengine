//! Render/audio/Lua-only resource re-exports.
//!
//! The bulk of the engine's resources now live in `aberred_core::resources`
//! (re-exported as `aberredengine::core::resources`). This module holds only
//! the thread-exclusive subset that can't live in core: resources used by
//! the audio thread's own `bevy_ecs::World`, resources used by the render
//! (main) thread, and the Lua runtime (`#[cfg(feature = "lua")]`).

pub mod audio;
#[cfg(feature = "lua")]
pub mod lua_runtime;
pub mod render;
