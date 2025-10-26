use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::inputcontrolled::InputControlled;
use crate::components::rigidbody::RigidBody;
use crate::resources::input::InputState;

pub fn input_simple_controller(
    mut query: Query<(&InputControlled, &mut RigidBody)>,
    input_state: Res<InputState>,
) {
    for (keyboard_controlled, mut rigidbody) in query.iter_mut() {
        // Reset velocity
        rigidbody.velocity = Vector2 { x: 0.0, y: 0.0 };

        // Update velocity based on input
        if input_state.maindirection_up.active {
            rigidbody.velocity += keyboard_controlled.up_velocity;
        }
        if input_state.maindirection_down.active {
            rigidbody.velocity += keyboard_controlled.down_velocity;
        }
        if input_state.maindirection_left.active {
            rigidbody.velocity += keyboard_controlled.left_velocity;
        }
        if input_state.maindirection_right.active {
            rigidbody.velocity += keyboard_controlled.right_velocity;
        }

        // Normalize diagonal movement
        if (input_state.maindirection_up.active || input_state.maindirection_down.active)
            && (input_state.maindirection_left.active || input_state.maindirection_right.active)
        {
            rigidbody.velocity.x *= 0.7071; // 1/sqrt(2)
            rigidbody.velocity.y *= 0.7071; // 1/sqrt(2)
        }
    }
}
