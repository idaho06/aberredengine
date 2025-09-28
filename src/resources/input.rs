use bevy_ecs::prelude::*;
use raylib::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct BoolState {
    pub state: bool,
    pub key_binding: KeyboardKey,
}

/// Resource capturing per-frame keyboard state we care about.
#[derive(Resource, Debug, Clone)]
pub struct InputState {
    pub maindirection_up: BoolState,
    pub maindirection_left: BoolState,
    pub maindirection_down: BoolState,
    pub maindirection_right: BoolState,
    // Arrow keys
    pub secondarydirection_up: BoolState,
    pub secondarydirection_down: BoolState,
    pub secondarydirection_left: BoolState,
    pub secondarydirection_right: BoolState,
    // Action special keys
    pub action_back: BoolState,
    pub action_1: BoolState,
    pub action_2: BoolState,
    pub mode_debug: BoolState,
    pub action_special: BoolState,
}

impl Default for BoolState {
    fn default() -> Self {
        Self {
            state: false,
            key_binding: KeyboardKey::KEY_NULL,
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            maindirection_up: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_W,
            },
            maindirection_left: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_A,
            },
            maindirection_down: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_S,
            },
            maindirection_right: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_D,
            },
            // Arrow keys
            secondarydirection_up: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_UP,
            },
            secondarydirection_down: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_DOWN,
            },
            secondarydirection_left: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_LEFT,
            },
            secondarydirection_right: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_RIGHT,
            },
            // Control keys
            action_back: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_ESCAPE,
            },
            action_1: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_SPACE,
            },
            action_2: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_ENTER,
            },
            mode_debug: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_F11,
            },
            action_special: BoolState {
                state: false,
                key_binding: KeyboardKey::KEY_F12,
            },
        }
    }
}
