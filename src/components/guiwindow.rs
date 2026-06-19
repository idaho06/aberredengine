//! Static themed GUI window panel.
//!
//! The [`GuiWindow`] component marks a screen-space entity as a themed
//! panel, rendered as a nine-patch background using [`GuiTheme`](crate::resources::guitheme::GuiTheme).
//! v1 is a standalone, static panel — no children, layout, or interaction yet.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Themed panel rendered as a nine-patch background at the entity's `ScreenPosition`.
#[derive(Component, Clone, Copy, Debug)]
pub struct GuiWindow {
    pub size: Vector2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guiwindow_construction() {
        let w = GuiWindow {
            size: Vector2::new(200.0, 150.0),
        };
        assert!((w.size.x - 200.0).abs() < f32::EPSILON);
        assert!((w.size.y - 150.0).abs() < f32::EPSILON);
    }
}
