//! Lua-priority shadow systems.
//!
//! The bulk of the engine's systems now live in `aberred_core::systems`
//! (re-exported as `aberredengine::core::systems`). This module holds four
//! "shadow" modules (`collision_rule_index`, `gui_interactable_click`,
//! `mapspawn`, `menu`) that re-export one function per module unconditionally
//! from core, then override it under `#[cfg(feature = "lua")]` with a
//! Lua-priority variant from the `aberred-lua` crate -- core keeps a
//! Rust-only-priority version of each of those four functions
//! unconditionally, since it cannot name `LuaRuntime`/Lua-only component
//! types at all. Every other Lua-callback system/command-processing module
//! now lives in the `aberred-lua` crate (re-exported as
//! `aberredengine::lua::systems`). The audio thread's own `bevy_ecs::World`
//! and its systems live in the `aberred-audio` crate; render (main) thread
//! systems live in the `aberred-render` crate (re-exported as
//! `aberredengine::render::systems`).

pub mod collision_rule_index;
pub mod gui_interactable_click;
pub mod mapspawn;
pub mod menu;
