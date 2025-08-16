use bevy_ecs::prelude::*;

use crate::resources::input::InputState;

pub fn keyboard_input(_commands: Commands, input: Res<InputState>) {
    // React to the input resource this frame
    if input.w_pressed {
        println!("W key pressed");
    }
    if input.s_pressed {
        println!("S key pressed");
    }
    if input.a_pressed {
        println!("A key pressed");
    }
    if input.d_pressed {
        println!("D key pressed");
    }
    if input.up_pressed {
        println!("Up arrow pressed");
    }
    if input.down_pressed {
        println!("Down arrow pressed");
    }
    if input.left_pressed {
        println!("Left arrow pressed");
    }
    if input.right_pressed {
        println!("Right arrow pressed");
    }
    if input.esc_pressed {
        println!("Esc pressed");
    }
    if input.space_pressed {
        println!("Space pressed");
    }
    if input.enter_pressed {
        println!("Enter pressed");
    }
    if input.f11_pressed {
        println!("F11 pressed");
    }
    if input.f12_pressed {
        println!("F12 pressed");
    }
}
