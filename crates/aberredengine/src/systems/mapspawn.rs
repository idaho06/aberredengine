//! Lua-only map-spawning glue.
//!
//! [`aberred_core::systems::mapspawn::spawn_map`] cannot attach
//! `LuaSetup`/`LuaOnAnimationEnd` itself -- those component types are
//! Lua-only and `aberred-core` cannot name them. The Lua-aware
//! `spawn_map_observer` (which wraps core's `spawn_map`) and
//! `process_lua_map_commands` live in [`crate::systems::lua_mapspawn`] (a
//! separate file so they can move to `aberred-lua` without dragging this
//! file's `--no-default-features` re-export along with them). Under
//! `#[cfg(not(feature = "lua"))]` `spawn_map_observer` resolves to core's
//! Lua-free variant instead; `process_lua_map_commands` has no Rust-only
//! counterpart and simply doesn't exist in that build.

#[cfg(feature = "lua")]
pub use crate::systems::lua_mapspawn::{process_lua_map_commands, spawn_map_observer};
#[cfg(not(feature = "lua"))]
pub use aberred_core::systems::mapspawn::spawn_map_observer;
