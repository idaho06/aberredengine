//! Lua callback component fired when a clickable GUI widget is clicked.
//!
//! [`lua_gui_interactable_click_observer`](crate::systems::lua_gui_interactable_click::lua_gui_interactable_click_observer)
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
//! The last argument of `:with_gui_button()` / `:with_gui_image()` names the
//! callback; an empty name attaches none.
//!
//! ```lua
//! engine.spawn()
//!     :with_gui_button(80, 20, "Start", "on_start")
//!     :build()
//! ```

use std::sync::Arc;

use bevy_ecs::prelude::Component;

/// Attaches a Lua callback to be called when the entity's GUI widget is clicked.
#[derive(Component, Clone, Debug)]
pub struct LuaOnClick {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnClick {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}
