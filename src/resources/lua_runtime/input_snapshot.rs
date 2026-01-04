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
                    pressed: input.maindirection_down.active || input.secondarydirection_down.active,
                    just_pressed: input.maindirection_down.just_pressed
                        || input.secondarydirection_down.just_pressed,
                    just_released: input.maindirection_down.just_released
                        || input.secondarydirection_down.just_released,
                },
                left: DigitalButtonState {
                    pressed: input.maindirection_left.active || input.secondarydirection_left.active,
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