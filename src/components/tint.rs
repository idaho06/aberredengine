//! Color tint component for rendering sprites and text.
//!
//! The [`Tint`] component applies color modulation to entities during rendering:
//! - For sprites: replaces `Color::WHITE` in draw calls
//! - For text: multiplies with the existing `DynamicText.color`

use bevy_ecs::prelude::Component;
use raylib::prelude::Color;

/// Color tint component for rendering modulation.
///
/// When attached to an entity with a [`Sprite`](crate::components::sprite::Sprite),
/// the tint color replaces `Color::WHITE` in draw calls.
///
/// When attached to an entity with [`DynamicText`](crate::components::dynamictext::DynamicText),
/// the tint color is multiplied with the text's existing color.
#[derive(Component, Clone, Debug, Copy)]
pub struct Tint {
    pub color: Color,
}

impl Tint {
    /// Create a new Tint with the specified RGBA values.
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            color: Color::new(r, g, b, a),
        }
    }

    /// Multiply this tint with another color (component-wise).
    ///
    /// Used for text rendering where the tint modulates the text's base color.
    pub fn multiply(&self, other: Color) -> Color {
        Color::new(
            ((self.color.r as u16 * other.r as u16) / 255) as u8,
            ((self.color.g as u16 * other.g as u16) / 255) as u8,
            ((self.color.b as u16 * other.b as u16) / 255) as u8,
            ((self.color.a as u16 * other.a as u16) / 255) as u8,
        )
    }
}

impl Default for Tint {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
        }
    }
}
