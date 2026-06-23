//! Static themed GUI window panel.
//!
//! The [`GuiWindow`] component marks a screen-space entity as a themed
//! panel, rendered as a nine-patch background using [`GuiTheme`](crate::resources::guitheme::GuiTheme).
//! v1 is a standalone, static panel — no children, layout, or interaction yet.

use std::sync::Arc;

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

use crate::resources::guitheme::DEFAULT_GUI_THEME_KEY;

/// Themed panel rendered as a nine-patch background at the entity's `ScreenPosition`.
/// `theme_key` selects which named theme in `GuiThemeStore` to render with
/// (default `"default"`); see `docs/gui-system-architecture.md` Roadmap #2.
#[derive(Component, Clone, Debug)]
pub struct GuiWindow {
    pub size: Vector2,
    pub theme_key: Arc<str>,
}

impl GuiWindow {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vector2::new(width, height),
            theme_key: Arc::from(DEFAULT_GUI_THEME_KEY),
        }
    }

    pub fn with_theme_key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.theme_key = key.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guiwindow_construction() {
        let w = GuiWindow::new(200.0, 150.0);
        assert!((w.size.x - 200.0).abs() < f32::EPSILON);
        assert!((w.size.y - 150.0).abs() < f32::EPSILON);
        assert_eq!(&*w.theme_key, "default");
    }

    #[test]
    fn test_guiwindow_with_theme_key() {
        let w = GuiWindow::new(200.0, 150.0).with_theme_key("dark");
        assert_eq!(&*w.theme_key, "dark");
    }
}
