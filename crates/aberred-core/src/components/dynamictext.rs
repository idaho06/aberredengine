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
//! - `aberred_render::resources::fontstore::FontStore` – font registry

use std::sync::Arc;

use crate::math::Vec2;
use bevy_ecs::prelude::Component;

use crate::math::Color;

/// Dynamic text component for rendering variable strings in the world or screen.
///
/// Unlike static sprite-based text, this component's content can be modified
/// at runtime via [`set_text`](DynamicText::set_text).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct DynamicText {
    /// The text content to render.
    pub text: Arc<str>,
    /// Font type
    pub font: Arc<str>,
    /// Font size in world units.
    pub font_size: f32,
    /// Color of the text.
    pub color: Color,
    /// Original configured text. Set on creation/update; never modified at runtime.
    /// Used by the editor to save the correct value regardless of runtime mutations.
    pub initial_text: Arc<str>,
    /// Original configured color. Set on creation/update; never modified at runtime.
    pub initial_color: Color,
    /// Size of the text bounding box
    size: Vec2,
}

impl DynamicText {
    /// Creates a new DynamicText component.
    ///
    /// The `size` field is initialized to zero and will be calculated
    /// by [`dynamictext_size_system`](crate::systems::dynamictext_size::dynamictext_size_system)
    /// on the first frame.
    pub fn new(
        content: impl Into<String>,
        font: impl Into<String>,
        font_size: f32,
        color: Color,
    ) -> Self {
        let text: Arc<str> = Arc::from(content.into());
        Self {
            initial_text: Arc::clone(&text),
            initial_color: color,
            text,
            font: Arc::from(font.into()),
            font_size,
            color,
            size: Vec2::ZERO,
        }
    }

    /// Returns the cached text bounding box size.
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Sets the cached text bounding box size.
    /// Used by [`dynamictext_size_system`](crate::systems::dynamictext_size::dynamictext_size_system).
    pub(crate) fn set_size(&mut self, size: Vec2) {
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

    #[test]
    fn set_text_reports_change() {
        let mut dt = DynamicText::new("old", "font", 12.0, Color::WHITE);
        assert!(dt.set_text("new"));
        assert_eq!(&*dt.text, "new");
        assert!(!dt.set_text("new"));
    }
}
