//! Simple kinematic body component with optional acceleration forces.
//!
//! The [`RigidBody`] component stores velocity and acceleration for an entity.
//! Movement systems integrate acceleration into velocity, then velocity into
//! position each frame. Input systems can set either velocity directly (for
//! instant movement) or acceleration (for smooth, physics-like movement).
//!
//! The `friction` field provides velocity damping - useful for gradual slowdown
//! when no input is applied.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Kinematic body storing per-entity velocity and acceleration.
///
/// Intended to be updated by input/physics systems and consumed by movement
/// systems to update [`MapPosition`](super::mapposition::MapPosition).
///
/// # Fields
/// - `velocity` - Current velocity in world units per second
/// - `acceleration` - Current acceleration in world units per second squared
/// - `friction` - Velocity damping factor (0.0 = no friction, higher = more drag)
/// - `max_speed` - Optional maximum speed clamp
#[derive(Component, Clone, Copy, Debug)]
pub struct RigidBody {
    /// Current velocity in world units per second.
    pub velocity: Vector2,
    /// Current acceleration in world units per second squared.
    pub acceleration: Vector2,
    /// Velocity damping factor. Applied as: velocity *= (1 - friction * delta).
    /// Typical values: 0.0 (no friction) to 10.0 (heavy drag).
    pub friction: f32,
    /// Optional maximum speed. If set, velocity magnitude is clamped to this value.
    pub max_speed: Option<f32>,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self::new()
    }
}

impl RigidBody {
    /// Create a RigidBody with zero velocity and acceleration.
    pub fn new() -> Self {
        Self {
            velocity: Vector2 { x: 0.0, y: 0.0 },
            acceleration: Vector2 { x: 0.0, y: 0.0 },
            friction: 0.0,
            max_speed: None,
        }
    }

    /// Create a RigidBody with acceleration physics enabled.
    ///
    /// # Arguments
    /// * `friction` - Velocity damping (0.0 = none, ~5.0 = responsive, ~10.0 = heavy)
    /// * `max_speed` - Optional velocity magnitude limit
    pub fn with_physics(friction: f32, max_speed: Option<f32>) -> Self {
        Self {
            velocity: Vector2 { x: 0.0, y: 0.0 },
            acceleration: Vector2 { x: 0.0, y: 0.0 },
            friction,
            max_speed,
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

    /// Set the acceleration of the RigidBody.
    pub fn set_acceleration(&mut self, acceleration: Vector2) {
        self.acceleration = acceleration;
    }

    /// Get the current acceleration.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn acceleration(&self) -> Vector2 {
        self.acceleration
    }

    /// Apply an instantaneous force to acceleration.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn apply_force(&mut self, force: Vector2) {
        self.acceleration += force;
    }

    /// Translate the RigidBody velocity by a delta vector.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.velocity.x += dx;
        self.velocity.y += dy;
    }
}
