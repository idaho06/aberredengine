//! Lua-only system re-exports.
//!
//! The bulk of the engine's systems now live in `aberred_core::systems`
//! (re-exported as `aberredengine::core::systems`). This module holds the
//! Lua-callback systems/command processing (`#[cfg(feature = "lua")]`) that
//! can't live in core, plus four "shadow" modules (`collision_rule_index`,
//! `gui_interactable_click`, `mapspawn`, `menu`) that
//! `pub use aberred_core::systems::<name>::*;` and then override just the
//! one function per module that needs a Lua-name lookup
//! `#[cfg(feature = "lua")]` -- core keeps a Rust-only-priority version of
//! each of those four functions unconditionally, since it cannot name
//! `LuaRuntime`/Lua-only component types at all. The audio thread's own
//! `bevy_ecs::World` and its systems live in the `aberred-audio` crate;
//! render (main) thread systems live in the `aberred-render` crate
//! (re-exported as `aberredengine::render::systems`).

pub mod collision_rule_index;
pub mod gui_interactable_click;
#[cfg(feature = "lua")]
pub mod lua_animation_finished;
#[cfg(feature = "lua")]
pub mod lua_collision;
#[cfg(feature = "lua")]
pub mod lua_collision_rule_index;
#[cfg(feature = "lua")]
pub mod lua_commands;
#[cfg(feature = "lua")]
pub mod lua_gui_interactable_click;
#[cfg(feature = "lua")]
pub mod lua_mapspawn;
#[cfg(feature = "lua")]
pub mod lua_menu;
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
