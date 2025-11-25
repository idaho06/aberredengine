//! Input systems.
//!
//! - [`update_input_state`] reads hardware input from Raylib each frame and
//!   writes the results into [`crate::resources::input::InputState`].
//! - Input events are emitted for key presses/releases (e.g., toggling debug
//!   mode via [`SwitchDebugEvent`](crate::events::switchdebug::SwitchDebugEvent)).
use bevy_ecs::prelude::*;
use raylib::ffi::KeyboardKey;

use crate::events::input::{InputAction, InputEvent};
use crate::events::switchdebug::SwitchDebugEvent;
use crate::resources::input::InputState;

/// Poll Raylib for keyboard input and update the `InputState` resource.
pub fn update_input_state(
    mut input: ResMut<InputState>,
    rl: NonSendMut<raylib::RaylibHandle>,
    mut commands: Commands,
) {
    // Update the input resource each frame
    let is_key_down = |key: KeyboardKey| rl.is_key_down(key);
    let is_key_pressed = |key: KeyboardKey| rl.is_key_pressed(key);
    let is_key_released = |key: KeyboardKey| rl.is_key_released(key);

    // WASD keys
    input.maindirection_up.active = is_key_down(input.maindirection_up.key_binding);
    input.maindirection_left.active = is_key_down(input.maindirection_left.key_binding);
    input.maindirection_down.active = is_key_down(input.maindirection_down.key_binding);
    input.maindirection_right.active = is_key_down(input.maindirection_right.key_binding);
    // Arrow keys
    input.secondarydirection_up.active = is_key_down(input.secondarydirection_up.key_binding);
    input.secondarydirection_down.active = is_key_down(input.secondarydirection_down.key_binding);
    input.secondarydirection_left.active = is_key_down(input.secondarydirection_left.key_binding);
    input.secondarydirection_right.active = is_key_down(input.secondarydirection_right.key_binding);
    // Action special keys
    input.action_back.active = is_key_down(input.action_back.key_binding);
    input.action_1.active = is_key_down(input.action_1.key_binding);
    input.action_2.active = is_key_down(input.action_2.key_binding);
    input.mode_debug.active = is_key_down(input.mode_debug.key_binding);
    input.action_special.active = is_key_down(input.action_special.key_binding);

    // Emit input events for actions that were just pressed or released
    if is_key_pressed(input.mode_debug.key_binding) {
        commands.trigger(SwitchDebugEvent {});
    }

    if is_key_pressed(input.action_special.key_binding) {
        input.action_special.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::Special,
            pressed: true,
        });
    } else {
        input.action_special.just_pressed = false;
    }
    if is_key_released(input.action_special.key_binding) {
        input.action_special.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::Special,
            pressed: false,
        });
    } else {
        input.action_special.just_released = false;
    }
    if is_key_pressed(input.action_1.key_binding) {
        input.action_1.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::Action1,
            pressed: true,
        });
    } else {
        input.action_1.just_pressed = false;
    }
    if is_key_released(input.action_1.key_binding) {
        input.action_1.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::Action1,
            pressed: false,
        });
    } else {
        input.action_1.just_released = false;
    }
    if is_key_pressed(input.action_2.key_binding) {
        input.action_2.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::Action2,
            pressed: true,
        });
    } else {
        input.action_2.just_pressed = false;
    }
    if is_key_released(input.action_2.key_binding) {
        input.action_2.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::Action2,
            pressed: false,
        });
    } else {
        input.action_2.just_released = false;
    }
    if is_key_pressed(input.action_back.key_binding) {
        input.action_back.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::Back,
            pressed: true,
        });
    } else {
        input.action_back.just_pressed = false;
    }
    if is_key_released(input.action_back.key_binding) {
        input.action_back.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::Back,
            pressed: false,
        });
    } else {
        input.action_back.just_released = false;
    }
    if is_key_pressed(input.maindirection_up.key_binding) {
        input.maindirection_up.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::MainDirectionUp,
            pressed: true,
        });
    } else {
        input.maindirection_up.just_pressed = false;
    }
    if is_key_released(input.maindirection_up.key_binding) {
        input.maindirection_up.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::MainDirectionUp,
            pressed: false,
        });
    } else {
        input.maindirection_up.just_released = false;
    }
    if is_key_pressed(input.maindirection_down.key_binding) {
        input.maindirection_down.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::MainDirectionDown,
            pressed: true,
        });
    } else {
        input.maindirection_down.just_pressed = false;
    }
    if is_key_released(input.maindirection_down.key_binding) {
        input.maindirection_down.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::MainDirectionDown,
            pressed: false,
        });
    } else {
        input.maindirection_down.just_released = false;
    }
    if is_key_pressed(input.maindirection_left.key_binding) {
        input.maindirection_left.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::MainDirectionLeft,
            pressed: true,
        });
    } else {
        input.maindirection_left.just_pressed = false;
    }
    if is_key_released(input.maindirection_left.key_binding) {
        input.maindirection_left.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::MainDirectionLeft,
            pressed: false,
        });
    } else {
        input.maindirection_left.just_released = false;
    }
    if is_key_pressed(input.maindirection_right.key_binding) {
        input.maindirection_right.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::MainDirectionRight,
            pressed: true,
        });
    } else {
        input.maindirection_right.just_pressed = false;
    }
    if is_key_released(input.maindirection_right.key_binding) {
        input.maindirection_right.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::MainDirectionRight,
            pressed: false,
        });
    } else {
        input.maindirection_right.just_released = false;
    }
    if is_key_pressed(input.secondarydirection_up.key_binding) {
        input.secondarydirection_up.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::SecondaryDirectionUp,
            pressed: true,
        });
    } else {
        input.secondarydirection_up.just_pressed = false;
    }
    if is_key_released(input.secondarydirection_up.key_binding) {
        input.secondarydirection_up.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::SecondaryDirectionUp,
            pressed: false,
        });
    } else {
        input.secondarydirection_up.just_released = false;
    }
    if is_key_pressed(input.secondarydirection_down.key_binding) {
        input.secondarydirection_down.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::SecondaryDirectionDown,
            pressed: true,
        });
    } else {
        input.secondarydirection_down.just_pressed = false;
    }
    if is_key_released(input.secondarydirection_down.key_binding) {
        input.secondarydirection_down.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::SecondaryDirectionDown,
            pressed: false,
        });
    } else {
        input.secondarydirection_down.just_released = false;
    }
    if is_key_pressed(input.secondarydirection_left.key_binding) {
        input.secondarydirection_left.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::SecondaryDirectionLeft,
            pressed: true,
        });
    } else {
        input.secondarydirection_left.just_pressed = false;
    }
    if is_key_released(input.secondarydirection_left.key_binding) {
        input.secondarydirection_left.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::SecondaryDirectionLeft,
            pressed: false,
        });
    } else {
        input.secondarydirection_left.just_released = false;
    }
    if is_key_pressed(input.secondarydirection_right.key_binding) {
        input.secondarydirection_right.just_pressed = true;
        commands.trigger(InputEvent {
            action: InputAction::SecondaryDirectionRight,
            pressed: true,
        });
    } else {
        input.secondarydirection_right.just_pressed = false;
    }
    if is_key_released(input.secondarydirection_right.key_binding) {
        input.secondarydirection_right.just_released = true;
        commands.trigger(InputEvent {
            action: InputAction::SecondaryDirectionRight,
            pressed: false,
        });
    } else {
        input.secondarydirection_right.just_released = false;
    }
}

// Example system that reacts to the current input state.
//
// This can be used as a place to trigger events such as
// [`SwitchDebugEvent`] when certain keys are pressed.
// TODO: Remove this example system when no longer needed.
/* pub fn check_input(mut commands: Commands, input: Res<InputState>) {
    // React to the input resource this frame
    if input.maindirection_up.active {
        println!("W key pressed");
    }
    if input.maindirection_down.active {
        println!("S key pressed");
    }
    if input.maindirection_left.active {
        println!("A key pressed");
    }
    if input.maindirection_right.active {
        println!("D key pressed");
    }
    if input.secondarydirection_up.active {
        println!("Up arrow pressed");
    }
    if input.secondarydirection_down.active {
        println!("Down arrow pressed");
    }
    if input.secondarydirection_left.active {
        println!("Left arrow pressed");
    }
    if input.secondarydirection_right.active {
        println!("Right arrow pressed");
    }
    if input.action_back.active {
        println!("Esc pressed");
    }
    if input.action_1.active {
        println!("Space pressed");
    }
    if input.action_2.active {
        println!("Enter pressed");
    }
    if input.mode_debug.active {
        println!("F11 pressed");
        // commands.trigger(SwitchDebugEvent {});
    }
    if input.action_special.active {
        println!("F12 pressed");
    }
} */
