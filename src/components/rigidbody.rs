//! Simple kinematic body component.
//!
//! The [`RigidBody`] component stores velocity for an entity. Movement systems
//! integrate position from velocity each frame. Input systems can set velocity
//! directly.
//!
//! For physics-based simulation, extend this with mass, forces, or use a
//! dedicated physics library.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Simple kinematic body storing per-entity velocity.
///
/// Intended to be updated by input/physics systems and consumed by movement
/// systems to update [`MapPosition`](super::mapposition::MapPosition).
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
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn velocity(&self) -> Vector2 {
        self.velocity
    }

    /// Translate the RigidBody by a delta vector.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.velocity.x += dx;
        self.velocity.y += dy;
    }
}
