//! Dynamic text component for runtime text rendering.
//!
//! The [`DynamicText`] component allows rendering text that can change at
//! runtime. It references a font by key and stores the text content, size,
//! and color.
//!
//! # Positioning
//!
//! Position the text using one of:
//! - [`MapPosition`](super::mapposition::MapPosition) for world-space (moves with camera)
//! - [`ScreenPosition`](super::screenposition::ScreenPosition) for UI/screen-space (fixed on screen)
//!
//! # Reactive Updates
//!
//! Combine with [`SignalBinding`](super::signalbinding::SignalBinding) to automatically
//! update text content when signal values change (e.g., score, lives).
//!
//! # Example
//!
//! ```ignore
//! // Static UI text
//! commands.spawn((
//!     ScreenPosition::new(10.0, 20.0),
//!     DynamicText::new("Score:", "arcade", 24.0, Color::WHITE),
//! ));
//!
//! // Reactive score display
//! commands.spawn((
//!     ScreenPosition::new(100.0, 20.0),
//!     DynamicText::new("0", "arcade", 24.0, Color::WHITE),
//!     SignalBinding::new("score"),
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::components::signalbinding::SignalBinding`] – binds text to signal values
//! - [`crate::resources::fontstore::FontStore`] – font registry

use std::sync::Arc;

use bevy_ecs::prelude::Component;
use raylib::math::Vector2;

/// Dynamic text component for rendering variable strings in the world or screen.
///
/// Unlike static sprite-based text, this component's content can be modified
/// at runtime via [`set_content`](DynamicText::set_content).
#[derive(Component, Clone, Debug)]
pub struct DynamicText {
    /// The text content to render.
    pub text: Arc<str>,
    /// Font type
    pub font: Arc<str>,
    /// Font size in world units.
    pub font_size: f32,
    /// Color of the text.
    pub color: raylib::prelude::Color,
    /// Size of the text bounding box
    size: Vector2,
}

impl DynamicText {
    /// Creates a new DynamicText component.
    ///
    /// The `size` field is initialized to zero and will be calculated
    /// by [`dynamictext_size_system`](crate::systems::dynamictext_size_system)
    /// on the first frame.
    pub fn new(
        content: impl Into<String>,
        font: impl Into<String>,
        font_size: f32,
        color: raylib::prelude::Color,
    ) -> Self {
        Self {
            text: Arc::from(content.into()),
            font: Arc::from(font.into()),
            font_size,
            color,
            size: Vector2::zero(),
        }
    }

    /// Returns the cached text bounding box size.
    pub fn size(&self) -> Vector2 {
        self.size
    }

    /// Sets the cached text bounding box size.
    /// Used by [`dynamictext_size_system`](crate::systems::dynamictext_size_system).
    pub(crate) fn set_size(&mut self, size: Vector2) {
        self.size = size;
    }
    /// Updates the text content only if changed.
    /// Returns `true` if the content was actually modified.
    pub fn set_text(&mut self, new_text: impl AsRef<str>) -> bool {
        let new_text_ref = new_text.as_ref();
        if &*self.text != new_text_ref {
            self.text = Arc::from(new_text_ref);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use raylib::prelude::Color;

    #[test]
    fn test_new_stores_fields() {
        let dt = DynamicText::new("Hello", "arcade", 16.0, Color::WHITE);
        assert_eq!(&*dt.text, "Hello");
        assert_eq!(&*dt.font, "arcade");
        assert_eq!(dt.font_size, 16.0);
        assert_eq!(dt.color, Color::WHITE);
    }

    #[test]
    fn test_new_size_is_zero() {
        let dt = DynamicText::new("test", "font", 12.0, Color::RED);
        assert_eq!(dt.size().x, 0.0);
        assert_eq!(dt.size().y, 0.0);
    }

    #[test]
    fn test_set_size() {
        let mut dt = DynamicText::new("test", "font", 12.0, Color::RED);
        dt.set_size(Vector2 { x: 100.0, y: 20.0 });
        assert_eq!(dt.size().x, 100.0);
        assert_eq!(dt.size().y, 20.0);
    }

    #[test]
    fn test_set_text_changed() {
        let mut dt = DynamicText::new("old", "font", 12.0, Color::WHITE);
        assert!(dt.set_text("new"));
        assert_eq!(&*dt.text, "new");
    }

    #[test]
    fn test_set_text_unchanged() {
        let mut dt = DynamicText::new("same", "font", 12.0, Color::WHITE);
        assert!(!dt.set_text("same"));
    }

    #[test]
    fn test_set_text_empty_to_nonempty() {
        let mut dt = DynamicText::new("", "font", 12.0, Color::WHITE);
        assert!(dt.set_text("content"));
        assert_eq!(&*dt.text, "content");
    }

    #[test]
    fn test_new_accepts_string_types() {
        let dt = DynamicText::new(String::from("hi"), String::from("myfont"), 8.0, Color::BLUE);
        assert_eq!(&*dt.text, "hi");
        assert_eq!(&*dt.font, "myfont");
    }
}
