//! World-space position component.
//!
//! The [`MapPosition`] component stores an entity's position in world
//! coordinates. It serves as the pivot point used by rendering and collision
//! systems.
//!
//! For screen-space UI elements, see
//! [`ScreenPosition`](super::screenposition::ScreenPosition).

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// World-space position (pivot) for an entity.
///
/// This position commonly represents the pivot used by other components such
/// as [`Sprite`](super::sprite::Sprite) and [`BoxCollider`](super::boxcollider::BoxCollider)
/// to compute rendering and collision bounds.
#[derive(Component, Clone, Copy, Debug)]
pub struct MapPosition {
    /// 2D coordinates in world units.
    pub pos: Vector2,
}

impl MapPosition {
    /// Create a MapPosition from x and y.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            pos: Vector2 { x, y },
        }
    }

    /// Create a MapPosition from an existing Vector2.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_vec(pos: Vector2) -> Self {
        Self { pos }
    }

    /// Get the underlying Vector2.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn pos(&self) -> Vector2 {
        self.pos
    }

    /// X coordinate.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn x(&self) -> f32 {
        self.pos.x
    }

    /// Y coordinate.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn y(&self) -> f32 {
        self.pos.y
    }

    /// Set the entire position.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_pos(&mut self, pos: Vector2) {
        self.pos = pos;
    }

    /// Set X coordinate.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_x(&mut self, x: f32) {
        self.pos.x = x;
    }

    /// Set Y coordinate.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_y(&mut self, y: f32) {
        self.pos.y = y;
    }

    /// Translate by delta.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.pos.x += dx;
        self.pos.y += dy;
    }

    /// Builder-style: return a copy with a different X.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_x(mut self, x: f32) -> Self {
        self.pos.x = x;
        self
    }

    /// Builder-style: return a copy with a different Y.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_y(mut self, y: f32) -> Self {
        self.pos.y = y;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_new_creates_correct_position() {
        let pos = MapPosition::new(10.0, 20.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));
    }

    #[test]
    fn test_new_with_zero() {
        let pos = MapPosition::new(0.0, 0.0);
        assert!(approx_eq(pos.pos.x, 0.0));
        assert!(approx_eq(pos.pos.y, 0.0));
    }

    #[test]
    fn test_new_with_negative_values() {
        let pos = MapPosition::new(-5.0, -10.0);
        assert!(approx_eq(pos.pos.x, -5.0));
        assert!(approx_eq(pos.pos.y, -10.0));
    }

    #[test]
    fn test_from_vec() {
        let vec = Vector2 { x: 15.0, y: 25.0 };
        let pos = MapPosition::from_vec(vec);
        assert!(approx_eq(pos.pos.x, 15.0));
        assert!(approx_eq(pos.pos.y, 25.0));
    }

    #[test]
    fn test_pos_getter() {
        let pos = MapPosition::new(1.0, 2.0);
        let vec = pos.pos();
        assert!(approx_eq(vec.x, 1.0));
        assert!(approx_eq(vec.y, 2.0));
    }

    #[test]
    fn test_x_getter() {
        let pos = MapPosition::new(7.0, 8.0);
        assert!(approx_eq(pos.x(), 7.0));
    }

    #[test]
    fn test_y_getter() {
        let pos = MapPosition::new(7.0, 8.0);
        assert!(approx_eq(pos.y(), 8.0));
    }

    #[test]
    fn test_set_pos() {
        let mut pos = MapPosition::new(0.0, 0.0);
        pos.set_pos(Vector2 { x: 100.0, y: 200.0 });
        assert!(approx_eq(pos.pos.x, 100.0));
        assert!(approx_eq(pos.pos.y, 200.0));
    }

    #[test]
    fn test_set_x() {
        let mut pos = MapPosition::new(1.0, 2.0);
        pos.set_x(99.0);
        assert!(approx_eq(pos.pos.x, 99.0));
        assert!(approx_eq(pos.pos.y, 2.0)); // y unchanged
    }

    #[test]
    fn test_set_y() {
        let mut pos = MapPosition::new(1.0, 2.0);
        pos.set_y(99.0);
        assert!(approx_eq(pos.pos.x, 1.0)); // x unchanged
        assert!(approx_eq(pos.pos.y, 99.0));
    }

    #[test]
    fn test_translate() {
        let mut pos = MapPosition::new(10.0, 20.0);
        pos.translate(5.0, -3.0);
        assert!(approx_eq(pos.pos.x, 15.0));
        assert!(approx_eq(pos.pos.y, 17.0));
    }

    #[test]
    fn test_translate_with_zero() {
        let mut pos = MapPosition::new(10.0, 20.0);
        pos.translate(0.0, 0.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));
    }

    #[test]
    fn test_with_x_builder() {
        let pos = MapPosition::new(1.0, 2.0).with_x(50.0);
        assert!(approx_eq(pos.pos.x, 50.0));
        assert!(approx_eq(pos.pos.y, 2.0));
    }

    #[test]
    fn test_with_y_builder() {
        let pos = MapPosition::new(1.0, 2.0).with_y(50.0);
        assert!(approx_eq(pos.pos.x, 1.0));
        assert!(approx_eq(pos.pos.y, 50.0));
    }

    #[test]
    fn test_builder_chaining() {
        let pos = MapPosition::new(0.0, 0.0).with_x(10.0).with_y(20.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));
    }
}
