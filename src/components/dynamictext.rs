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

/// Dynamic text component for rendering variable strings in the world or screen.
///
/// Unlike static sprite-based text, this component's content can be modified
/// at runtime via [`set_content`](DynamicText::set_content).
#[derive(Component, Clone, Debug)]
pub struct DynamicText {
    /// The text content to render.
    pub content: Arc<str>,
    /// Font type
    pub font: Arc<str>,
    /// Font size in world units.
    pub font_size: f32,
    /// Color of the text.
    pub color: raylib::prelude::Color,
}

impl DynamicText {
    /// Creates a new DynamicText component.
    pub fn new(
        content: impl Into<String>,
        font: impl Into<String>,
        font_size: f32,
        color: raylib::prelude::Color,
    ) -> Self {
        Self {
            content: Arc::from(content.into()),
            font: Arc::from(font.into()),
            font_size,
            color,
        }
    }
    /// Updates the text content only if changed.
    pub fn set_content(&mut self, new_content: impl AsRef<str>) {
        let new_content_ref = new_content.as_ref();
        if &*self.content != new_content_ref {
            self.content = Arc::from(new_content_ref);
        }
    }
}
