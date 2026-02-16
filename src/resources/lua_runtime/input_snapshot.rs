//! Input snapshot for Lua callbacks.
//!
//! This module provides [`InputSnapshot`], a frozen snapshot of the input state
//! that is passed to Lua callbacks. This replaces the old approach of caching
//! individual input values in RefCells.
//!
//! # Design
//!
//! The input is organized into two categories:
//! - `digital` - Boolean button states (pressed/just_pressed/just_released)
//! - `analog` - Float axis values (reserved for future gamepad support)
//!
//! This structure mirrors the Lua table that will be passed to callbacks:
//! ```lua
//! input.digital.up.pressed
//! input.digital.action_1.just_released
//! input.analog.move_x  -- future
//! ```

use crate::resources::input::InputState;

/// State of a single digital input button.
#[derive(Debug, Clone, Copy, Default)]
pub struct DigitalButtonState {
    /// Whether the button is currently held down.
    pub pressed: bool,
    /// Whether the button was just pressed this frame.
    pub just_pressed: bool,
    /// Whether the button was just released this frame.
    pub just_released: bool,
}

impl DigitalButtonState {
    /// Create from a BoolState.
    pub fn from_bool_state(state: &crate::resources::input::BoolState) -> Self {
        Self {
            pressed: state.active,
            just_pressed: state.just_pressed,
            just_released: state.just_released,
        }
    }
}

/// All digital input states.
#[derive(Debug, Clone, Default)]
pub struct DigitalInputs {
    pub up: DigitalButtonState,
    pub down: DigitalButtonState,
    pub left: DigitalButtonState,
    pub right: DigitalButtonState,
    pub action_1: DigitalButtonState,
    pub action_2: DigitalButtonState,
    pub back: DigitalButtonState,
    pub special: DigitalButtonState,
}

/// Analog input values (reserved for future gamepad support).
#[derive(Debug, Clone, Default)]
pub struct AnalogInputs {
    // Future: move_x, move_y, look_x, look_y, trigger_left, trigger_right
    // For now, this is empty but the structure exists for Lua API compatibility
}

/// Frozen snapshot of all input state for a single frame.
///
/// This is created once per frame from [`InputState`] and passed to Lua callbacks.
/// The structure is designed to be easily convertible to a Lua table.
#[derive(Debug, Clone, Default)]
pub struct InputSnapshot {
    pub digital: DigitalInputs,
    #[allow(dead_code)] // Reserved for future use
    pub analog: AnalogInputs,
}

impl InputSnapshot {
    /// Create a new input snapshot from the current input state.
    ///
    /// This combines main direction (WASD) and secondary direction (arrows)
    /// inputs into unified directional inputs (up/down/left/right).
    pub fn from_input_state(input: &InputState) -> Self {
        Self {
            digital: DigitalInputs {
                // Combine WASD and arrow keys for directional input
                up: DigitalButtonState {
                    pressed: input.maindirection_up.active || input.secondarydirection_up.active,
                    just_pressed: input.maindirection_up.just_pressed
                        || input.secondarydirection_up.just_pressed,
                    just_released: input.maindirection_up.just_released
                        || input.secondarydirection_up.just_released,
                },
                down: DigitalButtonState {
                    pressed: input.maindirection_down.active
                        || input.secondarydirection_down.active,
                    just_pressed: input.maindirection_down.just_pressed
                        || input.secondarydirection_down.just_pressed,
                    just_released: input.maindirection_down.just_released
                        || input.secondarydirection_down.just_released,
                },
                left: DigitalButtonState {
                    pressed: input.maindirection_left.active
                        || input.secondarydirection_left.active,
                    just_pressed: input.maindirection_left.just_pressed
                        || input.secondarydirection_left.just_pressed,
                    just_released: input.maindirection_left.just_released
                        || input.secondarydirection_left.just_released,
                },
                right: DigitalButtonState {
                    pressed: input.maindirection_right.active
                        || input.secondarydirection_right.active,
                    just_pressed: input.maindirection_right.just_pressed
                        || input.secondarydirection_right.just_pressed,
                    just_released: input.maindirection_right.just_released
                        || input.secondarydirection_right.just_released,
                },
                action_1: DigitalButtonState::from_bool_state(&input.action_1),
                action_2: DigitalButtonState::from_bool_state(&input.action_2),
                back: DigitalButtonState::from_bool_state(&input.action_back),
                special: DigitalButtonState::from_bool_state(&input.action_special),
            },
            analog: AnalogInputs::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::input::BoolState;
    use raylib::prelude::KeyboardKey;

    fn default_input() -> InputState {
        InputState::default()
    }

    fn bool_state_pressed(key: KeyboardKey) -> BoolState {
        BoolState {
            active: true,
            just_pressed: true,
            just_released: false,
            key_binding: key,
        }
    }

    #[test]
    fn test_from_default_input_all_unpressed() {
        let snap = InputSnapshot::from_input_state(&default_input());
        assert!(!snap.digital.up.pressed);
        assert!(!snap.digital.down.pressed);
        assert!(!snap.digital.left.pressed);
        assert!(!snap.digital.right.pressed);
        assert!(!snap.digital.action_1.pressed);
        assert!(!snap.digital.action_2.pressed);
        assert!(!snap.digital.back.pressed);
        assert!(!snap.digital.special.pressed);
    }

    #[test]
    fn test_wasd_maps_to_directional() {
        let mut input = default_input();
        input.maindirection_up.active = true;
        input.maindirection_up.just_pressed = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.up.pressed);
        assert!(snap.digital.up.just_pressed);
        assert!(!snap.digital.down.pressed);
    }

    #[test]
    fn test_arrows_maps_to_directional() {
        let mut input = default_input();
        input.secondarydirection_left.active = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.left.pressed);
        assert!(!snap.digital.right.pressed);
    }

    #[test]
    fn test_combined_wasd_and_arrows() {
        let mut input = default_input();
        // Neither WASD nor arrow pressed
        input.maindirection_up.active = false;
        input.secondarydirection_up.active = false;
        input.maindirection_up.just_pressed = false;
        input.secondarydirection_up.just_pressed = true; // arrow just pressed
        let snap = InputSnapshot::from_input_state(&input);
        assert!(!snap.digital.up.pressed);
        assert!(snap.digital.up.just_pressed); // OR of both
    }

    #[test]
    fn test_wasd_or_arrows_pressed_means_combined_pressed() {
        let mut input = default_input();
        input.maindirection_right.active = true;
        input.secondarydirection_right.active = false;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.right.pressed);
    }

    #[test]
    fn test_action_buttons_map_directly() {
        let mut input = default_input();
        input.action_1 = bool_state_pressed(KeyboardKey::KEY_SPACE);
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.action_1.pressed);
        assert!(snap.digital.action_1.just_pressed);
        assert!(!snap.digital.action_1.just_released);
    }

    #[test]
    fn test_back_maps_from_action_back() {
        let mut input = default_input();
        input.action_back.active = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.back.pressed);
    }

    #[test]
    fn test_special_maps_from_action_special() {
        let mut input = default_input();
        input.action_special.active = true;
        input.action_special.just_released = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.special.pressed);
        assert!(snap.digital.special.just_released);
    }

    #[test]
    fn test_digital_button_state_from_bool_state() {
        let bs = BoolState {
            active: true,
            just_pressed: false,
            just_released: true,
            key_binding: KeyboardKey::KEY_SPACE,
        };
        let dbs = DigitalButtonState::from_bool_state(&bs);
        assert!(dbs.pressed);
        assert!(!dbs.just_pressed);
        assert!(dbs.just_released);
    }

    #[test]
    fn test_digital_button_state_default() {
        let dbs = DigitalButtonState::default();
        assert!(!dbs.pressed);
        assert!(!dbs.just_pressed);
        assert!(!dbs.just_released);
    }
}
