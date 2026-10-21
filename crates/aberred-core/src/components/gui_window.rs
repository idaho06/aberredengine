//! Static themed GUI window panel.
//!
//! The [`GuiWindow`] component marks a screen-space entity as a themed
//! panel, rendered as a nine-patch background using [`GuiTheme`](crate::resources::guitheme::GuiTheme).
//! It carries panel rendering data only; child layout and interaction come
//! from other GUI components.

use std::sync::Arc;

use crate::math::Vec2;
use bevy_ecs::prelude::Component;

use crate::components::gui_themed::Themed;
use crate::resources::guitheme::DEFAULT_GUI_THEME_KEY;

/// Themed panel rendered as a nine-patch background at the entity's `ScreenPosition`.
/// `theme_key` selects which named theme in `GuiThemeStore` to render with
/// (default `"default"`).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct GuiWindow {
    pub size: Vec2,
    pub theme_key: Arc<str>,
}

impl GuiWindow {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vec2::new(width, height),
            theme_key: Arc::from(DEFAULT_GUI_THEME_KEY),
        }
    }

    pub fn with_theme_key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.theme_key = key.into();
        self
    }
}

impl Themed for GuiWindow {
    fn theme_key_mut(&mut self) -> &mut Arc<str> {
        &mut self.theme_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_uses_default_theme_key() {
        assert_eq!(
            &*GuiWindow::new(200.0, 150.0).theme_key,
            DEFAULT_GUI_THEME_KEY
        );
    }
}
