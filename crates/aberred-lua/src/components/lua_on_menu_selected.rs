//! Lua callback component fired when one of a menu's items is confirmed.
//!
//! [`lua_menu_selected_observer`](crate::systems::lua_menu_selected::lua_menu_selected_observer)
//! calls the named function when the menu triggers `MenuSelected`. A Lua menu
//! with a callback gets no `MenuActions`: the callback handles every item.
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_main_menu_select(ctx)
//!     -- ctx.menu_id, ctx.item_id, ctx.item_index
//! end
//! ```
//!
//! # Usage from Lua
//!
//! ```lua
//! engine.spawn()
//!     :with_menu({ { id = "play", label = "Play" } }, 100, 80, "arcade", 24, 30, true)
//!     :with_lua_menu_selected("on_main_menu_select")
//!     :build()
//! ```

use std::sync::Arc;

use bevy_ecs::prelude::Component;

/// Attaches a Lua callback to be called when one of the menu's items is confirmed.
#[derive(Component, Clone, Debug)]
pub struct LuaOnMenuSelected {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnMenuSelected {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}
