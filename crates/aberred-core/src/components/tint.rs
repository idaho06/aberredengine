//! Color tint component for rendering sprites and text.
//!
//! The [`Tint`] component applies color modulation to entities during rendering:
//! - For sprites: replaces `Color::WHITE` in draw calls
//! - For text: multiplies with the existing `DynamicText.color`

use bevy_ecs::prelude::Component;

use crate::math::Color;

/// Color tint component for rendering modulation.
///
/// When attached to an entity with a [`Sprite`](crate::components::sprite::Sprite),
/// the tint color replaces `Color::WHITE` in draw calls.
///
/// When attached to an entity with [`DynamicText`](crate::components::dynamictext::DynamicText),
/// the tint color is multiplied with the text's existing color.
#[derive(Component, Clone, Debug, Copy, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_white() {
        assert_eq!(Tint::default().color, Color::WHITE);
    }

    #[test]
    fn multiply_is_componentwise_over_255() {
        let cases = [
            (Tint::new(100, 150, 200, 255), Color::WHITE, Color::new(100, 150, 200, 255)),
            (Tint::new(100, 150, 200, 255), Color::new(0, 0, 0, 0), Color::new(0, 0, 0, 0)),
            (Tint::default(), Color::new(128, 64, 32, 255), Color::new(128, 64, 32, 255)),
            (Tint::new(128, 128, 128, 128), Color::new(128, 255, 0, 255), Color::new(64, 128, 0, 128)),
        ];
        for (tint, other, expected) in cases {
            assert_eq!(tint.multiply(other), expected, "{tint:?} * {other:?}");
        }
    }
}
