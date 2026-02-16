//! Acceleration-based input controller.
//!
//! Reads the shared [`InputState`](crate::resources::input::InputState) and
//! applies directional accelerations to entities with an
//! [`AccelerationControlled`](crate::components::inputcontrolled::AccelerationControlled)
//! component. Unlike the simple velocity controller, this provides smooth,
//! physics-like movement with momentum and gradual speed changes.
//!
//! This system manages an "input" force on the RigidBody that is updated each
//! frame based on the current input state. Other forces (gravity, wind, etc.)
//! remain unaffected.
//!
//! Diagonal movement is normalized to maintain consistent acceleration magnitude.

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::inputcontrolled::AccelerationControlled;
use crate::components::rigidbody::RigidBody;
use crate::resources::input::InputState;

/// The force name used by the input acceleration controller.
pub const INPUT_FORCE_NAME: &str = "input";

/// Update each controlled entity's `RigidBody` "input" force based on input.
///
/// When no input is pressed, the input force is set to zero (friction handles
/// deceleration). When input is pressed, the input force is set to the
/// accumulated directional accelerations from the component.
pub fn input_acceleration_controller(
    mut query: Query<(&AccelerationControlled, &mut RigidBody)>,
    input_state: Res<InputState>,
) {
    for (accel_controlled, mut rigidbody) in query.iter_mut() {
        // Calculate acceleration from input
        let mut acceleration = Vector2 { x: 0.0, y: 0.0 };

        if input_state.maindirection_up.active {
            acceleration += accel_controlled.up_acceleration;
        }
        if input_state.maindirection_down.active {
            acceleration += accel_controlled.down_acceleration;
        }
        if input_state.maindirection_left.active {
            acceleration += accel_controlled.left_acceleration;
        }
        if input_state.maindirection_right.active {
            acceleration += accel_controlled.right_acceleration;
        }

        // Normalize diagonal acceleration to maintain consistent magnitude
        if (input_state.maindirection_up.active || input_state.maindirection_down.active)
            && (input_state.maindirection_left.active || input_state.maindirection_right.active)
        {
            acceleration.x *= std::f32::consts::FRAC_1_SQRT_2;
            acceleration.y *= std::f32::consts::FRAC_1_SQRT_2;
        }

        // Update the "input" force on the rigidbody
        rigidbody.add_force(INPUT_FORCE_NAME, acceleration);
    }
}
