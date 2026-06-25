//! Themed button widget data.
//!
//! `GuiButton` carries the spawn-time data for a themed button (size,
//! caption, click callback name, disabled state). Hit-testing and click
//! dispatch run on a co-located `GuiInteractable` component (see
//! `guiinteractable.rs`), inserted by `gui_button_spawn_system`
//! (`systems/gui_spawn.rs`) reacting on `Added<GuiButton>` — mirrors how
//! `Menu` carries its own item data and `menu_spawn_system` reacts on
//! `Added<Menu>`. Querying `GuiButton` alone without `GuiInteractable` is
//! only valid for the one frame between insertion and the spawn system
//! running.
//!
//! See `docs/gui-system-architecture.md`.

use std::sync::Arc;

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

use crate::components::gui_themed::Themed;
use crate::resources::guitheme::DEFAULT_GUI_THEME_KEY;

/// Render this entity via `GuiTheme.button`'s nine-patch skin. Carries
/// everything needed to spawn itself: `gui_button_spawn_system` reacts on
/// `Added<GuiButton>` to insert the co-located `GuiInteractable` and spawn
/// the caption `DynamicText` child, the same way `Menu` carries its own item
/// data and `menu_spawn_system` reacts on `Added<Menu>`.
#[derive(Component, Clone, Debug)]
pub struct GuiButton {
    pub size: Vector2,
    /// Empty string = captionless button, no caption child spawned.
    pub caption: String,
    /// Lua callback name, checked first by the click dispatch chain. Empty
    /// string = no callback wired (`GuiInteractable.on_click_callback` stays
    /// `None`).
    pub callback_name: String,
    /// Authored disabled state, applied to the spawned `GuiInteractable.state`
    /// once at spawn time. Mutating this field after spawn has no further
    /// effect — toggle `GuiInteractable.state` directly for runtime
    /// enable/disable (see Open Item #1 in the design doc).
    pub disabled: bool,
    /// Selects which named theme in `GuiThemeStore` to render this button
    /// (and its caption) with. Default `"default"`.
    pub theme_key: Arc<str>,
}

impl GuiButton {
    pub fn new(width: f32, height: f32, caption: impl Into<String>) -> Self {
        Self {
            size: Vector2::new(width, height),
            caption: caption.into(),
            callback_name: String::new(),
            disabled: false,
            theme_key: Arc::from(DEFAULT_GUI_THEME_KEY),
        }
    }

    /// Lua-only constructor: sets `callback_name`, dispatched by name through
    /// the Lua-then-Rust callback chain. Rust callers should use `::new` and
    /// pair the entity with a pre-spawned `GuiInteractable::rust(...)`
    /// instead — `callback_name` has no effect once a `GuiInteractable` is
    /// already present (`insert_if_new`).
    #[cfg(feature = "lua")]
    pub fn with_lua_callback(
        width: f32,
        height: f32,
        caption: impl Into<String>,
        callback_name: impl Into<String>,
    ) -> Self {
        Self {
            callback_name: callback_name.into(),
            ..Self::new(width, height, caption)
        }
    }

    pub fn with_disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn with_theme_key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.theme_key = key.into();
        self
    }
}

impl Themed for GuiButton {
    fn theme_key_mut(&mut self) -> &mut Arc<str> {
        &mut self.theme_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guibutton_new_defaults() {
        let b = GuiButton::new(80.0, 24.0, "Start");
        assert!((b.size.x - 80.0).abs() < f32::EPSILON);
        assert!((b.size.y - 24.0).abs() < f32::EPSILON);
        assert_eq!(b.caption, "Start");
        assert!(b.callback_name.is_empty());
        assert!(!b.disabled);
    }

    #[cfg(feature = "lua")]
    #[test]
    fn test_guibutton_with_lua_callback() {
        let b = GuiButton::with_lua_callback(80.0, 24.0, "Start", "on_start_clicked");
        assert_eq!(b.caption, "Start");
        assert_eq!(b.callback_name, "on_start_clicked");
    }

    #[test]
    fn test_guibutton_with_disabled() {
        let b = GuiButton::new(80.0, 24.0, "Start").with_disabled();
        assert!(b.disabled);
    }
}
