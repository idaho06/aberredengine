//! Per-frame keyboard input resource.
//!
//! Captures the subset of keyboard state the game cares about and exposes it
//! to systems via the [`InputState`] resource. Defaults use WASD for primary
//! movement and arrow keys for secondary directions.
use bevy_ecs::prelude::*;
use raylib::prelude::*;

#[derive(Debug, Clone, Copy)]
/// Boolean key state with an associated keyboard binding.
pub struct BoolState {
    /// Whether the key is currently active/pressed this frame.
    pub active: bool,
    /// Whether the key was just pressed this frame.
    pub just_pressed: bool,
    /// Whether the key was just released this frame.
    pub just_released: bool,

    /// The key bound to this action.
    pub key_binding: KeyboardKey,
}

/// Resource capturing the per-frame keyboard state relevant to gameplay.
///
/// Fields are grouped by purpose: main movement (WASD), secondary movement
/// (arrow keys), and actions (escape/space/enter/F-keys).
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
            active: false,
            just_pressed: false,
            just_released: false,
            key_binding: KeyboardKey::KEY_NULL,
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            maindirection_up: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_W,
            },
            maindirection_left: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_A,
            },
            maindirection_down: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_S,
            },
            maindirection_right: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_D,
            },
            // Arrow keys
            secondarydirection_up: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_UP,
            },
            secondarydirection_down: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_DOWN,
            },
            secondarydirection_left: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_LEFT,
            },
            secondarydirection_right: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_RIGHT,
            },
            // Control keys
            action_back: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_ESCAPE,
            },
            action_1: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_SPACE,
            },
            action_2: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_ENTER,
            },
            mode_debug: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_F11,
            },
            action_special: BoolState {
                active: false,
                just_pressed: false,
                just_released: false,
                key_binding: KeyboardKey::KEY_F12,
            },
        }
    }
}
