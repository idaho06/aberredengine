//! Render/Lua-only resource re-exports.
//!
//! The bulk of the engine's resources now live in `aberred_core::resources`
//! (re-exported as `aberredengine::core::resources`). This module holds only
//! the thread-exclusive subset that can't live in core: resources used by
//! the render (main) thread and the Lua runtime (`#[cfg(feature = "lua")]`).
//! Audio-thread-only resources live in the `aberred-audio` crate.

#[cfg(feature = "lua")]
pub mod lua_runtime;
pub mod render;
