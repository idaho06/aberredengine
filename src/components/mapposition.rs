use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

#[derive(Component, Clone, Copy, Debug)]
pub struct MapPosition {
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
