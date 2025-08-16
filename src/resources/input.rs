use bevy_ecs::prelude::*;
use raylib::prelude::*;

/// Resource capturing per-frame keyboard state we care about.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct InputState {
    pub w_pressed: bool,
    pub a_pressed: bool,
    pub s_pressed: bool,
    pub d_pressed: bool,
    // Arrow keys
    pub up_pressed: bool,
    pub down_pressed: bool,
    pub left_pressed: bool,
    pub right_pressed: bool,
    // Common control keys
    pub esc_pressed: bool,
    pub space_pressed: bool,
    pub enter_pressed: bool,
    pub f11_pressed: bool,
    pub f12_pressed: bool,
}

/// Update the InputState resource from the current Raylib keyboard state.
/// Call this once per frame before running the ECS update schedule.
pub fn update_input_state(world: &mut World, rl: &RaylibHandle) {
    let mut input = world.resource_mut::<InputState>();
    input.w_pressed = rl.is_key_down(KeyboardKey::KEY_W);
    input.a_pressed = rl.is_key_down(KeyboardKey::KEY_A);
    input.s_pressed = rl.is_key_down(KeyboardKey::KEY_S);
    input.d_pressed = rl.is_key_down(KeyboardKey::KEY_D);
    // Arrow keys
    input.up_pressed = rl.is_key_down(KeyboardKey::KEY_UP);
    input.down_pressed = rl.is_key_down(KeyboardKey::KEY_DOWN);
    input.left_pressed = rl.is_key_down(KeyboardKey::KEY_LEFT);
    input.right_pressed = rl.is_key_down(KeyboardKey::KEY_RIGHT);
    // Control keys
    input.esc_pressed = rl.is_key_pressed(KeyboardKey::KEY_ESCAPE);
    input.space_pressed = rl.is_key_pressed(KeyboardKey::KEY_SPACE);
    input.enter_pressed = rl.is_key_pressed(KeyboardKey::KEY_ENTER);
    input.f11_pressed = rl.is_key_pressed(KeyboardKey::KEY_F11);
    input.f12_pressed = rl.is_key_pressed(KeyboardKey::KEY_F12);
}
