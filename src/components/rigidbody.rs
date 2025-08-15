use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct RigidBody {
    pub velocity: Vector2,
    // pub mass: f32, // for the future
}
impl RigidBody {
    /// Create a RigidBody with zero velocity.
    pub fn new() -> Self {
        Self {
            velocity: Vector2 { x: 0.0, y: 0.0 },
        }
    }

    /// Set the velocity of the RigidBody.
    pub fn set_velocity(&mut self, velocity: Vector2) {
        self.velocity = velocity;
    }

    /// Get the current velocity.
    pub fn velocity(&self) -> Vector2 {
        self.velocity
    }

    /// Translate the RigidBody by a delta vector.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.velocity.x += dx;
        self.velocity.y += dy;
    }
}
