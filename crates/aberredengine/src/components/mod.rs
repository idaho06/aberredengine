//! Render/Lua-only component re-exports.
//!
//! The bulk of the engine's components now live in `aberred_core::components`
//! (re-exported as `aberredengine::core::components`). This module holds only
//! the thread-exclusive subset that can't live in core: components used by
//! the render (main) thread and Lua-callback components
//! (`#[cfg(feature = "lua")]`). Audio-thread-only components live in the
//! `aberred-audio` crate.

#[cfg(feature = "lua")]
pub mod lua_on_animation_end;
#[cfg(feature = "lua")]
pub mod lua_on_tween_finished;
#[cfg(feature = "lua")]
pub mod luacollision;
#[cfg(feature = "lua")]
pub mod luaphase;
#[cfg(feature = "lua")]
pub mod luasetup;
#[cfg(feature = "lua")]
pub mod luatimer;
pub mod render;
