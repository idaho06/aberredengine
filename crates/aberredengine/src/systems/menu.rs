//! Lua-priority menu selection dispatch.
//!
//! Shadows `aberred_core::systems::menu::menu_selection_observer` with a
//! variant that checks the menu's `on_select_callback` (Lua) first --
//! `aberred-core` cannot name `LuaRuntime`. The Lua-aware body lives in
//! [`aberred_lua::systems::lua_menu`]. Under `#[cfg(not(feature = "lua"))]`
//! this resolves to core's Rust-only variant instead.

pub use aberred_core::systems::menu::{menu_controller_observer, menu_despawn, menu_spawn_system};

#[cfg(feature = "lua")]
pub use aberred_lua::systems::lua_menu::menu_selection_observer;
#[cfg(not(feature = "lua"))]
pub use aberred_core::systems::menu::menu_selection_observer;
