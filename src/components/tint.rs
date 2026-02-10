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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let t = Tint::new(100, 150, 200, 255);
        assert_eq!(t.color.r, 100);
        assert_eq!(t.color.g, 150);
        assert_eq!(t.color.b, 200);
        assert_eq!(t.color.a, 255);
    }

    #[test]
    fn test_default_is_white() {
        let t = Tint::default();
        assert_eq!(t.color.r, 255);
        assert_eq!(t.color.g, 255);
        assert_eq!(t.color.b, 255);
        assert_eq!(t.color.a, 255);
    }

    #[test]
    fn test_multiply_with_white_is_identity() {
        let t = Tint::new(100, 150, 200, 255);
        let result = t.multiply(Color::WHITE);
        assert_eq!(result.r, 100);
        assert_eq!(result.g, 150);
        assert_eq!(result.b, 200);
        assert_eq!(result.a, 255);
    }

    #[test]
    fn test_multiply_with_black_zeroes_out() {
        let t = Tint::new(100, 150, 200, 255);
        let result = t.multiply(Color::new(0, 0, 0, 0));
        assert_eq!(result.r, 0);
        assert_eq!(result.g, 0);
        assert_eq!(result.b, 0);
        assert_eq!(result.a, 0);
    }

    #[test]
    fn test_multiply_partial_values() {
        let t = Tint::new(255, 255, 255, 255);
        let result = t.multiply(Color::new(128, 64, 32, 255));
        assert_eq!(result.r, 128);
        assert_eq!(result.g, 64);
        assert_eq!(result.b, 32);
        assert_eq!(result.a, 255);
    }

    #[test]
    fn test_copy_trait() {
        let t = Tint::new(10, 20, 30, 40);
        let t2 = t;
        assert_eq!(t.color.r, 10);
        assert_eq!(t2.color.r, 10);
    }
}
