//! Lua callback component fired when a clickable GUI widget is clicked.
//!
//! [`lua_gui_clicked_observer`](crate::systems::lua_gui_clicked::lua_gui_clicked_observer)
//! calls the named function when the widget triggers `GuiClicked`.
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_start(ctx)
//!     -- ctx.entity_id
//! end
//! ```
//!
//! # Usage from Lua
//!
//! `:with_lua_gui_clicked()` names the callback of a `:with_gui_button()` /
//! `:with_gui_image()` widget; without it the widget dispatches nothing.
//!
//! ```lua
//! engine.spawn()
//!     :with_gui_button(80, 20, "Start")
//!     :with_lua_gui_clicked("on_start")
//!     :build()
//! ```

use std::sync::Arc;

use bevy_ecs::prelude::Component;

/// Attaches a Lua callback to be called when the entity's GUI widget is clicked.
#[derive(Component, Clone, Debug)]
pub struct LuaOnGuiClicked {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnGuiClicked {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}
