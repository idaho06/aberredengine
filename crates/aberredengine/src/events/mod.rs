//! Lua-only event re-exports.
//!
//! The bulk of the engine's events now live in `aberred_core::events`
//! (re-exported as `aberredengine::core::events`). This module holds only
//! Lua timer callback events (`#[cfg(feature = "lua")]`) that can't live in
//! core. Render-thread-only events live in the `aberred-render` crate
//! (re-exported as `aberredengine::render::events`).

#[cfg(feature = "lua")]
pub mod luatimer;
