//! Acceleration-based input controller.
//!
//! Reads the shared [`InputState`](crate::resources::input::InputState) and
//! applies directional accelerations to entities with an
//! [`AccelerationControlled`](crate::components::inputcontrolled::AccelerationControlled)
//! component. Unlike the simple velocity controller, this provides smooth,
//! physics-like movement with momentum and gradual speed changes.
//!
//! Diagonal movement is normalized to maintain consistent acceleration magnitude.

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::inputcontrolled::AccelerationControlled;
use crate::components::rigidbody::RigidBody;
use crate::resources::input::InputState;

/// Update each controlled entity's `RigidBody` acceleration based on input.
///
/// When no input is pressed, acceleration is set to zero (friction handles
/// deceleration). When input is pressed, acceleration is set to the configured
/// directional values.
pub fn input_acceleration_controller(
    mut query: Query<(&AccelerationControlled, &mut RigidBody)>,
    input_state: Res<InputState>,
) {
    for (accel_controlled, mut rigidbody) in query.iter_mut() {
        // Reset acceleration - friction in movement system handles deceleration
        rigidbody.acceleration = Vector2 { x: 0.0, y: 0.0 };

        // Accumulate acceleration based on input
        if input_state.maindirection_up.active {
            rigidbody.acceleration += accel_controlled.up_acceleration;
        }
        if input_state.maindirection_down.active {
            rigidbody.acceleration += accel_controlled.down_acceleration;
        }
        if input_state.maindirection_left.active {
            rigidbody.acceleration += accel_controlled.left_acceleration;
        }
        if input_state.maindirection_right.active {
            rigidbody.acceleration += accel_controlled.right_acceleration;
        }

        // Normalize diagonal acceleration to maintain consistent magnitude
        if (input_state.maindirection_up.active || input_state.maindirection_down.active)
            && (input_state.maindirection_left.active || input_state.maindirection_right.active)
        {
            rigidbody.acceleration.x *= 0.7071; // 1/sqrt(2)
            rigidbody.acceleration.y *= 0.7071; // 1/sqrt(2)
        }
    }
}
