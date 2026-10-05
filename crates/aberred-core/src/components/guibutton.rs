//! Themed button widget data.
//!
//! `GuiButton` carries the spawn-time data for a themed button (size,
//! caption, disabled state, theme). Hit-testing and click
//! dispatch run on a co-located `GuiInteractable` component (see
//! `guiinteractable.rs`), inserted by `gui_button_spawn_system`
//! (`systems/gui_spawn.rs`) reacting on `Added<GuiButton>` — mirrors how
//! `Menu` carries its own item data and `menu_spawn_system` reacts on
//! `Added<Menu>`. Querying `GuiButton` alone without `GuiInteractable` is
//! only valid for the one frame between insertion and the spawn system
//! running.

use std::sync::Arc;

use crate::math::Vec2;
use bevy_ecs::prelude::Component;

use crate::components::gui_themed::Themed;
use crate::resources::guitheme::DEFAULT_GUI_THEME_KEY;

/// Render this entity via `GuiTheme.button`'s nine-patch skin. Carries
/// everything needed to spawn itself: `gui_button_spawn_system` reacts on
/// `Added<GuiButton>` to insert the co-located `GuiInteractable` and spawn
/// the caption `DynamicText` child, the same way `Menu` carries its own item
/// data and `menu_spawn_system` reacts on `Added<Menu>`.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct GuiButton {
    pub size: Vec2,
    /// Empty string = captionless button, no caption child spawned.
    pub caption: String,
    /// Authored disabled state, applied to the spawned `GuiInteractable.state`
    /// once at spawn time. Mutating this field after spawn has no further
    /// effect — toggle `GuiInteractable.state` directly for runtime
    /// enable/disable.
    pub disabled: bool,
    /// Selects which named theme in `GuiThemeStore` to render this button
    /// (and its caption) with. Default `"default"`.
    pub theme_key: Arc<str>,
}

impl GuiButton {
    pub fn new(width: f32, height: f32, caption: impl Into<String>) -> Self {
        Self {
            size: Vec2::new(width, height),
            caption: caption.into(),
            disabled: false,
            theme_key: Arc::from(DEFAULT_GUI_THEME_KEY),
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
