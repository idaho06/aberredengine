//! Screen-space position component.
//!
//! The [`ScreenPosition`] component stores an entity's position in screen
//! (pixel) coordinates. Use this for UI elements that should not move with
//! the camera.
//!
//! For world-space entities, see
//! [`MapPosition`](super::mapposition::MapPosition).

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Screen-space position (pivot) for an entity.
///
/// Used for UI elements that should remain fixed on screen regardless of
/// camera movement. The render system draws these after the world pass.
#[derive(Component, Clone, Copy, Debug)]
pub struct ScreenPosition {
    /// 2D coordinates in screen pixels.
    pub pos: Vector2,
}

impl Default for ScreenPosition {
    fn default() -> Self {
        Self {
            pos: Vector2 { x: 0.0, y: 0.0 },
        }
    }
}

impl ScreenPosition {
    /// Create a ScreenPosition from x and y.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            pos: Vector2 { x, y },
        }
    }

    /// Create a ScreenPosition from an existing Vector2.
    #[allow(dead_code)]
    pub fn from_vec(pos: Vector2) -> Self {
        Self { pos }
    }

    /// Get the underlying Vector2.
    #[allow(dead_code)]
    pub fn pos(&self) -> Vector2 {
        self.pos
    }

    /// X coordinate.
    #[allow(dead_code)]
    pub fn x(&self) -> f32 {
        self.pos.x
    }

    /// Y coordinate.
    #[allow(dead_code)]
    pub fn y(&self) -> f32 {
        self.pos.y
    }

    /// Set the entire position.
    #[allow(dead_code)]
    pub fn set_pos(&mut self, pos: Vector2) {
        self.pos = pos;
    }

    /// Set X coordinate.
    #[allow(dead_code)]
    pub fn set_x(&mut self, x: f32) {
        self.pos.x = x;
    }

    /// Set Y coordinate.
    #[allow(dead_code)]
    pub fn set_y(&mut self, y: f32) {
        self.pos.y = y;
    }

    /// Translate by delta.
    #[allow(dead_code)]
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.pos.x += dx;
        self.pos.y += dy;
    }

    /// Builder-style: return a copy with a different X.
    #[allow(dead_code)]
    pub fn with_x(mut self, x: f32) -> Self {
        self.pos.x = x;
        self
    }

    /// Builder-style: return a copy with a different Y.
    #[allow(dead_code)]
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
        let pos = ScreenPosition::new(10.0, 20.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));
    }

    #[test]
    fn test_new_with_negative_values() {
        let pos = ScreenPosition::new(-5.0, -10.0);
        assert!(approx_eq(pos.pos.x, -5.0));
        assert!(approx_eq(pos.pos.y, -10.0));
    }

    #[test]
    fn test_default_is_zero() {
        let pos = ScreenPosition::default();
        assert!(approx_eq(pos.pos.x, 0.0));
        assert!(approx_eq(pos.pos.y, 0.0));
    }

    #[test]
    fn test_from_vec() {
        let vec = Vector2 { x: 15.0, y: 25.0 };
        let pos = ScreenPosition::from_vec(vec);
        assert!(approx_eq(pos.pos.x, 15.0));
        assert!(approx_eq(pos.pos.y, 25.0));
    }

    #[test]
    fn test_pos_getter() {
        let pos = ScreenPosition::new(1.0, 2.0);
        let vec = pos.pos();
        assert!(approx_eq(vec.x, 1.0));
        assert!(approx_eq(vec.y, 2.0));
    }

    #[test]
    fn test_x_getter() {
        let pos = ScreenPosition::new(7.0, 8.0);
        assert!(approx_eq(pos.x(), 7.0));
    }

    #[test]
    fn test_y_getter() {
        let pos = ScreenPosition::new(7.0, 8.0);
        assert!(approx_eq(pos.y(), 8.0));
    }

    #[test]
    fn test_set_pos() {
        let mut pos = ScreenPosition::new(0.0, 0.0);
        pos.set_pos(Vector2 { x: 100.0, y: 200.0 });
        assert!(approx_eq(pos.pos.x, 100.0));
        assert!(approx_eq(pos.pos.y, 200.0));
    }

    #[test]
    fn test_set_x() {
        let mut pos = ScreenPosition::new(1.0, 2.0);
        pos.set_x(99.0);
        assert!(approx_eq(pos.pos.x, 99.0));
        assert!(approx_eq(pos.pos.y, 2.0));
    }

    #[test]
    fn test_set_y() {
        let mut pos = ScreenPosition::new(1.0, 2.0);
        pos.set_y(99.0);
        assert!(approx_eq(pos.pos.x, 1.0));
        assert!(approx_eq(pos.pos.y, 99.0));
    }

    #[test]
    fn test_translate() {
        let mut pos = ScreenPosition::new(10.0, 20.0);
        pos.translate(5.0, -3.0);
        assert!(approx_eq(pos.pos.x, 15.0));
        assert!(approx_eq(pos.pos.y, 17.0));
    }

    #[test]
    fn test_translate_with_zero() {
        let mut pos = ScreenPosition::new(10.0, 20.0);
        pos.translate(0.0, 0.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));
    }

    #[test]
    fn test_with_x_builder() {
        let pos = ScreenPosition::new(1.0, 2.0).with_x(50.0);
        assert!(approx_eq(pos.pos.x, 50.0));
        assert!(approx_eq(pos.pos.y, 2.0));
    }

    #[test]
    fn test_with_y_builder() {
        let pos = ScreenPosition::new(1.0, 2.0).with_y(50.0);
        assert!(approx_eq(pos.pos.x, 1.0));
        assert!(approx_eq(pos.pos.y, 50.0));
    }

    #[test]
    fn test_builder_chaining() {
        let pos = ScreenPosition::new(0.0, 0.0).with_x(10.0).with_y(20.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));
    }
}
