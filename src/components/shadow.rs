use bevy_ecs::prelude::Component;
use raylib::math::Vector2;
use raylib::prelude::Color;

/// Drop shadow component for rendering sprites and text.
///
/// When attached to an entity with a [`Sprite`](crate::components::sprite::Sprite)
/// or [`DynamicText`](crate::components::dynamictext::DynamicText), draws a
/// shadow pre-pass at `position + offset` before the main draw. The shadow
/// always uses `color` directly and bypasses entity shaders.
///
/// Works for both world-space and screen-space positions.
#[derive(Component, Clone, Copy, Debug)]
pub struct Shadow {
    /// World/screen-space displacement of the shadow from the entity position.
    pub offset: Vector2,
    /// Shadow color (typically semi-transparent black).
    pub color: Color,
}

impl Shadow {
    /// Create a shadow with explicit offset and RGBA color (0–255 each).
    pub fn new(dx: f32, dy: f32, r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            offset: Vector2 { x: dx, y: dy },
            color: Color::new(r, g, b, a),
        }
    }

    /// Create a shadow with offset and the default color (50% transparent black).
    pub fn default_color(dx: f32, dy: f32) -> Self {
        Self::new(dx, dy, 0, 0, 0, 128)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_fields() {
        let s = Shadow::new(3.0, 4.0, 10, 20, 30, 200);
        assert_eq!(s.offset.x, 3.0);
        assert_eq!(s.offset.y, 4.0);
        assert_eq!(s.color.r, 10);
        assert_eq!(s.color.g, 20);
        assert_eq!(s.color.b, 30);
        assert_eq!(s.color.a, 200);
    }

    #[test]
    fn default_color_is_half_alpha_black() {
        let s = Shadow::default_color(2.0, 2.0);
        assert_eq!(s.color.r, 0);
        assert_eq!(s.color.g, 0);
        assert_eq!(s.color.b, 0);
        assert_eq!(s.color.a, 128);
    }
}
