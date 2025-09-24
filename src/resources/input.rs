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
pub fn update_input_state(world: &mut World) {
    // Snapshot keyboard state first to end the immutable borrow before mutably borrowing InputState.
    let (w, a, s, d, up, down, left, right, esc, space, enter, f11, f12) = {
        let rl = world.non_send_resource::<RaylibHandle>();
        (
            rl.is_key_down(KeyboardKey::KEY_W),
            rl.is_key_down(KeyboardKey::KEY_A),
            rl.is_key_down(KeyboardKey::KEY_S),
            rl.is_key_down(KeyboardKey::KEY_D),
            // Arrow keys
            rl.is_key_down(KeyboardKey::KEY_UP),
            rl.is_key_down(KeyboardKey::KEY_DOWN),
            rl.is_key_down(KeyboardKey::KEY_LEFT),
            rl.is_key_down(KeyboardKey::KEY_RIGHT),
            // Control keys
            rl.is_key_pressed(KeyboardKey::KEY_ESCAPE),
            rl.is_key_pressed(KeyboardKey::KEY_SPACE),
            rl.is_key_pressed(KeyboardKey::KEY_ENTER),
            rl.is_key_pressed(KeyboardKey::KEY_F11),
            rl.is_key_pressed(KeyboardKey::KEY_F12),
        )
    };

    let mut input = world.resource_mut::<InputState>();
    input.w_pressed = w;
    input.a_pressed = a;
    input.s_pressed = s;
    input.d_pressed = d;
    // Arrow keys
    input.up_pressed = up;
    input.down_pressed = down;
    input.left_pressed = left;
    input.right_pressed = right;
    // Control keys
    input.esc_pressed = esc;
    input.space_pressed = space;
    input.enter_pressed = enter;
    input.f11_pressed = f11;
    input.f12_pressed = f12;
}

//TODO: Create a proper system for input state to be added to the update schedule
// pub fn input_system(mut input: ResMut<InputState>, rl: NonSendMut<raylib::RaylibHandle>) {
