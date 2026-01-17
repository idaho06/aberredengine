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
