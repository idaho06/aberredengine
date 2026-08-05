//! Render/audio/Lua-only system re-exports.
//!
//! The bulk of the engine's systems now live in `aberred_core::systems`
//! (re-exported as `aberredengine::core::systems`). This module holds the
//! thread-exclusive subset that can't live in core: the audio thread's own
//! `bevy_ecs::World` and its systems, render (main) thread systems,
//! Lua-callback systems/command processing (`#[cfg(feature = "lua")]`), and
//! four "shadow" modules (`collision_rule_index`, `gui_interactable_click`,
//! `mapspawn`, `menu`) that `pub use aberred_core::systems::<name>::*;` and
//! then override just the one function per module that needs a Lua-name
//! lookup `#[cfg(feature = "lua")]` -- core keeps a Rust-only-priority
//! version of each of those four functions unconditionally, since it cannot
//! name `LuaRuntime`/Lua-only component types at all.

pub mod collision_rule_index;
pub mod gui_interactable_click;
#[cfg(feature = "lua")]
pub mod lua_animation_finished;
#[cfg(feature = "lua")]
pub mod lua_collision;
#[cfg(feature = "lua")]
pub mod lua_commands;
#[cfg(feature = "lua")]
pub mod lua_setup_entity;
#[cfg(feature = "lua")]
pub mod lua_tween_finished;
#[cfg(feature = "lua")]
pub mod luaphase;
#[cfg(feature = "lua")]
pub mod luatimer;
pub mod mapspawn;
pub mod menu;
pub mod render;
