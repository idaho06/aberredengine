//! Lua-priority GUI interactable click dispatch.
//!
//! Shadows [`aberred_core::systems::gui_interactable_click::gui_interactable_click_observer`]
//! with a variant that checks the entity's named Lua callback first, falling
//! back to its Rust fn-pointer callback -- mirrors
//! `aberred_core::systems::menu::menu_selection_observer`'s existing
//! priority chain. The Lua-aware body lives in
//! [`aberred_lua::systems::lua_gui_interactable_click`]. Under
//! `#[cfg(not(feature = "lua"))]` this resolves to core's Rust-only variant
//! instead.

#[cfg(feature = "lua")]
pub use aberred_lua::systems::lua_gui_interactable_click::gui_interactable_click_observer;
#[cfg(not(feature = "lua"))]
pub use aberred_core::systems::gui_interactable_click::gui_interactable_click_observer;
