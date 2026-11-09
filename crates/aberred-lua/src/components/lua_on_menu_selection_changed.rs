//! Lua callback component fired when a menu's highlight moves to another item.
//!
//! [`lua_menu_selection_changed_observer`](crate::systems::lua_menu_selection_changed::lua_menu_selection_changed_observer)
//! calls the named function when the menu triggers `MenuSelectionChanged`. It is independent
//! of [`LuaOnMenuSelected`](crate::components::lua_on_menu_selected::LuaOnMenuSelected) and of
//! `MenuActions`: a menu can have any combination of the three.
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_menu_hint(ctx)
//!     -- ctx.menu_id, ctx.item_id, ctx.item_index, ctx.previous_index
//! end
//! ```
//!
//! # Usage from Lua
//!
//! ```lua
//! engine.spawn()
//!     :with_menu({ { id = "play", label = "Play" } }, 100, 80, "arcade", 24, 30, true)
//!     :with_lua_menu_selection_changed("on_menu_hint")
//!     :build()
//! ```

use std::sync::Arc;

use bevy_ecs::prelude::Component;

/// Attaches a Lua callback to be called when the menu's highlight moves.
#[derive(Component, Clone, Debug)]
pub struct LuaOnMenuSelectionChanged {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnMenuSelectionChanged {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}
