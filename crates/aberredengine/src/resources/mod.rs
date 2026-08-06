//! Lua-only resource re-exports.
//!
//! The bulk of the engine's resources now live in `aberred_core::resources`
//! (re-exported as `aberredengine::core::resources`). This module holds only
//! the Lua runtime (`#[cfg(feature = "lua")]`) that can't live in core.
//! Audio-thread-only resources live in the `aberred-audio` crate;
//! render-thread-only resources live in the `aberred-render` crate
//! (re-exported as `aberredengine::render::resources`).

#[cfg(feature = "lua")]
pub mod lua_runtime;
