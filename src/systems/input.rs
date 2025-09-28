use bevy_ecs::prelude::*;
use raylib::ffi::KeyboardKey;

use crate::events::switchdebug::SwitchDebugEvent;
use crate::resources::input::InputState;

pub fn update_input_state(mut input: ResMut<InputState>, rl: NonSendMut<raylib::RaylibHandle>) {
    // Update the input resource each frame
    let is_key_down = |key: KeyboardKey| rl.is_key_down(key);
    let is_key_pressed = |key: KeyboardKey| rl.is_key_pressed(key);

    input.maindirection_up.state = is_key_down(input.maindirection_up.key_binding);
    input.maindirection_left.state = is_key_down(input.maindirection_left.key_binding);
    input.maindirection_down.state = is_key_down(input.maindirection_down.key_binding);
    input.maindirection_right.state = is_key_down(input.maindirection_right.key_binding);
    // Arrow keys
    input.secondarydirection_up.state = is_key_down(input.secondarydirection_up.key_binding);
    input.secondarydirection_down.state = is_key_down(input.secondarydirection_down.key_binding);
    input.secondarydirection_left.state = is_key_down(input.secondarydirection_left.key_binding);
    input.secondarydirection_right.state = is_key_down(input.secondarydirection_right.key_binding);
    // Action special keys
    input.action_back.state = is_key_pressed(input.action_back.key_binding);
    input.action_1.state = is_key_pressed(input.action_1.key_binding);
    input.action_2.state = is_key_pressed(input.action_2.key_binding);
    input.mode_debug.state = is_key_pressed(input.mode_debug.key_binding);
    input.action_special.state = is_key_pressed(input.action_special.key_binding);
}

pub fn check_input(mut commands: Commands, input: Res<InputState>) {
    // React to the input resource this frame
    if input.maindirection_up.state {
        println!("W key pressed");
    }
    if input.maindirection_down.state {
        println!("S key pressed");
    }
    if input.maindirection_left.state {
        println!("A key pressed");
    }
    if input.maindirection_right.state {
        println!("D key pressed");
    }
    if input.secondarydirection_up.state {
        println!("Up arrow pressed");
    }
    if input.secondarydirection_down.state {
        println!("Down arrow pressed");
    }
    if input.secondarydirection_left.state {
        println!("Left arrow pressed");
    }
    if input.secondarydirection_right.state {
        println!("Right arrow pressed");
    }
    if input.action_back.state {
        println!("Esc pressed");
    }
    if input.action_1.state {
        println!("Space pressed");
    }
    if input.action_2.state {
        println!("Enter pressed");
    }
    if input.mode_debug.state {
        println!("F11 pressed");
        commands.trigger(SwitchDebugEvent {});
    }
    if input.action_special.state {
        println!("F12 pressed");
    }
}
