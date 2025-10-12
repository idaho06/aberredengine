use bevy_ecs::prelude::*;
use raylib::ffi::KeyboardKey;

use crate::events::switchdebug::SwitchDebugEvent;
use crate::resources::input::InputState;

pub fn update_input_state(mut input: ResMut<InputState>, rl: NonSendMut<raylib::RaylibHandle>) {
    // Update the input resource each frame
    let is_key_down = |key: KeyboardKey| rl.is_key_down(key);
    let is_key_pressed = |key: KeyboardKey| rl.is_key_pressed(key);

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
    input.action_back.active = is_key_pressed(input.action_back.key_binding);
    input.action_1.active = is_key_pressed(input.action_1.key_binding);
    input.action_2.active = is_key_pressed(input.action_2.key_binding);
    input.mode_debug.active = is_key_pressed(input.mode_debug.key_binding);
    input.action_special.active = is_key_pressed(input.action_special.key_binding);
}

pub fn check_input(mut commands: Commands, input: Res<InputState>) {
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
        commands.trigger(SwitchDebugEvent {});
    }
    if input.action_special.active {
        println!("F12 pressed");
    }
}
