//! Simple input-to-velocity controller.
//!
//! Reads the shared [`InputState`] and
//! applies directional velocities to entities with an
//! [`InputControlled`]
//! component. Diagonal movement is normalized to maintain constant speed.
use bevy_ecs::prelude::*;
use crate::math::Vec2;

use crate::components::inputcontrolled::InputControlled;
use crate::components::rigidbody::RigidBody;
use crate::events::input::InputAction;
use crate::resources::input::InputState;

/// Update each controlled entity's `RigidBody` velocity based on input.
pub fn input_simple_controller(
    mut query: Query<(&InputControlled, &mut RigidBody)>,
    input_state: Res<InputState>,
) {
    let up = input_state.action(InputAction::MainDirectionUp).active;
    let down = input_state.action(InputAction::MainDirectionDown).active;
    let left = input_state.action(InputAction::MainDirectionLeft).active;
    let right = input_state.action(InputAction::MainDirectionRight).active;
    for (keyboard_controlled, mut rigidbody) in query.iter_mut() {
        // Reset velocity
        rigidbody.velocity = Vec2 { x: 0.0, y: 0.0 };

        // Update velocity based on input
        if up {
            rigidbody.velocity += keyboard_controlled.up_velocity;
        }
        if down {
            rigidbody.velocity += keyboard_controlled.down_velocity;
        }
        if left {
            rigidbody.velocity += keyboard_controlled.left_velocity;
        }
        if right {
            rigidbody.velocity += keyboard_controlled.right_velocity;
        }

        // Normalize diagonal movement
        if (up || down) && (left || right) {
            rigidbody.velocity.x *= std::f32::consts::FRAC_1_SQRT_2;
            rigidbody.velocity.y *= std::f32::consts::FRAC_1_SQRT_2;
        }
    }
}
