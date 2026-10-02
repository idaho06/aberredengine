//! Input-controlled movement components.
//!
//! This module provides components that describe how entities respond to
//! input:
//! - [`InputControlled`] – keyboard-driven directional movement (instant velocity)
//! - [`AccelerationControlled`] – keyboard-driven acceleration-based movement (smooth physics)
//! - [`MouseControlled`] – mouse position tracking
//!
//! Systems in [`crate::systems::inputsimplecontroller`],
//! [`crate::systems::inputaccelerationcontroller`], and
//! [`crate::systems::mousecontroller`] read these components to update
//! entity positions or velocities.

use crate::math::Vec2;
use bevy_ecs::prelude::Component;

/// Movement intent derived from player keyboard input.
///
/// Each field stores the velocity to apply when the corresponding directional
/// input is active. A system should read the current input state and update an
/// entity's velocity or position accordingly.
#[derive(Component, Clone, Copy, Debug)]
pub struct InputControlled {
    /// Velocity when moving up.
    pub up_velocity: Vec2,
    /// Velocity when moving down.
    pub down_velocity: Vec2,
    /// Velocity when moving left.
    pub left_velocity: Vec2,
    /// Velocity when moving right.
    pub right_velocity: Vec2,
}

impl InputControlled {
    /// Moves at `speed` world units per second in each of the four directions
    /// (up is negative Y).
    pub fn symmetric(speed: f32) -> Self {
        Self {
            up_velocity: Vec2::new(0.0, -speed),
            down_velocity: Vec2::new(0.0, speed),
            left_velocity: Vec2::new(-speed, 0.0),
            right_velocity: Vec2::new(speed, 0.0),
        }
    }
}

/// Movement controlled by mouse position.
///
/// When attached to an entity, systems will update the entity's position
/// to follow the mouse cursor on the enabled axes.
#[derive(Component, Clone, Copy, Debug)]
pub struct MouseControlled {
    /// Follow mouse X axis.
    pub follow_x: bool,
    /// Follow mouse Y axis.
    pub follow_y: bool,
}

/// Acceleration-based movement from player keyboard input.
///
/// Unlike [`InputControlled`] which sets velocity directly, this component
/// provides acceleration values that accumulate into velocity over time,
/// creating smooth, physics-like movement with momentum.
///
/// Requires a [`RigidBody`](super::rigidbody::RigidBody) with appropriate
/// `friction` and optionally `max_speed` configured for best results.
#[derive(Component, Clone, Copy, Debug)]
pub struct AccelerationControlled {
    /// Acceleration when moving up (typically negative Y).
    pub up_acceleration: Vec2,
    /// Acceleration when moving down (typically positive Y).
    pub down_acceleration: Vec2,
    /// Acceleration when moving left (typically negative X).
    pub left_acceleration: Vec2,
    /// Acceleration when moving right (typically positive X).
    pub right_acceleration: Vec2,
}

impl AccelerationControlled {
    /// Create an AccelerationControlled with symmetric acceleration magnitude.
    ///
    /// # Arguments
    /// * `accel` - Acceleration magnitude in world units per second squared
    pub fn symmetric(accel: f32) -> Self {
        Self {
            up_acceleration: Vec2 { x: 0.0, y: -accel },
            down_acceleration: Vec2 { x: 0.0, y: accel },
            left_acceleration: Vec2 { x: -accel, y: 0.0 },
            right_acceleration: Vec2 { x: accel, y: 0.0 },
        }
    }
}

// TODO: MouseDeltaControlled component for relative mouse movement (e.g., for camera control)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_input_points_each_direction_with_the_given_speed() {
        let c = InputControlled::symmetric(100.0);
        assert_eq!(c.up_velocity, Vec2::new(0.0, -100.0));
        assert_eq!(c.down_velocity, Vec2::new(0.0, 100.0));
        assert_eq!(c.left_velocity, Vec2::new(-100.0, 0.0));
        assert_eq!(c.right_velocity, Vec2::new(100.0, 0.0));
    }
}
