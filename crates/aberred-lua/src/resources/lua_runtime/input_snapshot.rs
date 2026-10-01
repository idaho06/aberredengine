//! Input snapshot for Lua callbacks.
//!
//! This module provides [`InputSnapshot`], a frozen snapshot of the input state
//! that is passed to Lua callbacks.
//!
//! # Design
//!
//! The input is organized into two categories:
//! - `digital` - Boolean button states (pressed/just_pressed/just_released)
//! - `analog` - Float axis values for mouse/scroll input: wheel delta, and
//!   cursor position in both game-space and world-space
//!
//! This structure mirrors the Lua table that will be passed to callbacks:
//! ```lua
//! input.digital.up.pressed
//! input.digital.action_1.just_released
//! input.analog.mouse_world_x
//! ```

use aberred_core::events::input::InputAction;
use aberred_core::resources::input::InputState;

/// State of a single digital input button.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
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
    pub fn from_bool_state(state: &aberred_core::resources::input::BoolState) -> Self {
        Self {
            pressed: state.active,
            just_pressed: state.just_pressed,
            just_released: state.just_released,
        }
    }
}

/// Master list of the digital buttons exposed to Lua's `input.digital` table
/// and mirrored in [`DigitalInputs`] / `InputCtxTables` (runtime.rs). Each row:
///   (field_name, mapping)
/// where mapping is one of:
///   combined(MainAction, SecondaryAction) — OR of two `InputAction` states
///   direct(Action)                        — 1:1 from one `InputAction` state
///   raw(input_state_field)                — 1:1 from a non-action `InputState`
///                                           field (`mouse_left_button`)
///
/// `for_each_digital_button!(my_cb)` expands to a single
/// `my_cb!{ (field, mapping), (field, mapping), ... }` call carrying every
/// row as one comma-separated argument list — one macro invocation, so a
/// consumer can produce a complete item or expression from it directly
/// (a struct definition, a struct-literal expression, or a `$(...)* `
/// repetition of statements), since Rust does not support splicing
/// multiple separate macro invocations into a struct's field list.
macro_rules! for_each_digital_button {
    ($cb:ident) => {
        $cb! {
            (up,              combined(MainDirectionUp,    SecondaryDirectionUp)),
            (down,            combined(MainDirectionDown,  SecondaryDirectionDown)),
            (left,            combined(MainDirectionLeft,  SecondaryDirectionLeft)),
            (right,           combined(MainDirectionRight, SecondaryDirectionRight)),
            (action_1,        direct(Action1)),
            (action_2,        direct(Action2)),
            (action_3,        direct(Action3)),
            (back,            direct(Back)),
            (special,         direct(Special)),
            (main_up,         direct(MainDirectionUp)),
            (main_down,       direct(MainDirectionDown)),
            (main_left,       direct(MainDirectionLeft)),
            (main_right,      direct(MainDirectionRight)),
            (secondary_up,    direct(SecondaryDirectionUp)),
            (secondary_down,  direct(SecondaryDirectionDown)),
            (secondary_left,  direct(SecondaryDirectionLeft)),
            (secondary_right, direct(SecondaryDirectionRight)),
            (debug,           direct(ToggleDebug)),
            (fullscreen,      direct(ToggleFullscreen)),
            (mouse_left,      raw(mouse_left_button)),
        }
    };
}
pub(crate) use for_each_digital_button;

macro_rules! digital_inputs_fields {
    ($(($field:ident, $($m:tt)*)),* $(,)?) => {
        /// All digital input states — one field per `for_each_digital_button!`
        /// row (grouping: combined directional / raw WASD / raw arrows /
        /// action buttons / function keys / mouse).
        #[derive(Debug, Clone, Default, PartialEq)]
        pub struct DigitalInputs {
            $( pub $field: DigitalButtonState, )*
        }
    };
}
for_each_digital_button! {digital_inputs_fields}

/// Analog input values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnalogInputs {
    /// Mouse wheel scroll delta this frame. Positive = up, negative = down. Zero if no scroll.
    pub scroll_y: f32,
    /// Mouse X in game/render-target space (letterbox-corrected). Range: 0..render_width.
    /// Matches ScreenPosition entity coordinates. Use for HUD hit-testing.
    pub mouse_x: f32,
    /// Mouse Y in game/render-target space (letterbox-corrected). Range: 0..render_height.
    pub mouse_y: f32,
    /// Mouse X in world-space (after camera transform). Matches MapPosition coordinates.
    pub mouse_world_x: f32,
    /// Mouse Y in world-space (after camera transform). Matches MapPosition coordinates.
    pub mouse_world_y: f32,
    /// Whether pad 0 is connected this tick.
    pub gamepad_connected: bool,
    /// Pad 0's left stick X axis (-1.0..1.0).
    pub pad_left_x: f32,
    /// Pad 0's left stick Y axis (-1.0..1.0).
    pub pad_left_y: f32,
    /// Pad 0's right stick X axis (-1.0..1.0).
    pub pad_right_x: f32,
    /// Pad 0's right stick Y axis (-1.0..1.0).
    pub pad_right_y: f32,
    /// Pad 0's left trigger pressure (-1.0..1.0, raylib convention).
    pub pad_lt: f32,
    /// Pad 0's right trigger pressure (-1.0..1.0, raylib convention).
    pub pad_rt: f32,
}

/// Frozen snapshot of all input state for a single frame.
///
/// This is created once per frame from [`InputState`] and passed to Lua callbacks.
/// The structure is designed to be easily convertible to a Lua table.
#[derive(Debug, Clone, Default, PartialEq)]
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
        macro_rules! digital_value {
            (combined($main:ident, $secondary:ident)) => {{
                let main = input.action(InputAction::$main);
                let secondary = input.action(InputAction::$secondary);
                DigitalButtonState {
                    pressed: main.active || secondary.active,
                    just_pressed: main.just_pressed || secondary.just_pressed,
                    just_released: main.just_released || secondary.just_released,
                }
            }};
            (direct($action:ident)) => {
                DigitalButtonState::from_bool_state(input.action(InputAction::$action))
            };
            (raw($field:ident)) => {
                DigitalButtonState::from_bool_state(&input.$field)
            };
        }
        macro_rules! digital_from_input {
            ($(($field:ident, $($m:tt)*)),* $(,)?) => {
                DigitalInputs {
                    $( $field: digital_value!($($m)*), )*
                }
            };
        }
        Self {
            digital: for_each_digital_button! {digital_from_input},
            analog: AnalogInputs {
                scroll_y: input.scroll_y,
                mouse_x: input.mouse_x,
                mouse_y: input.mouse_y,
                mouse_world_x: input.mouse_world_x,
                mouse_world_y: input.mouse_world_y,
                gamepad_connected: input.gamepad_connected,
                // Indices match raylib's GamepadAxis ordinal order (LX, LY,
                // RX, RY, LT, RT) -- see RawGamepad's doc comment.
                pad_left_x: input.gamepad_axes[0],
                pad_left_y: input.gamepad_axes[1],
                pad_right_x: input.gamepad_axes[2],
                pad_right_y: input.gamepad_axes[3],
                pad_lt: input.gamepad_axes[4],
                pad_rt: input.gamepad_axes[5],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::resources::input::BoolState;

    fn default_input() -> InputState {
        InputState::default()
    }

    fn bool_state_pressed() -> BoolState {
        BoolState {
            active: true,
            just_pressed: true,
            just_released: false,
        }
    }

    #[test]
    fn test_wasd_maps_to_directional() {
        let mut input = default_input();
        input.action_mut(InputAction::MainDirectionUp).active = true;
        input.action_mut(InputAction::MainDirectionUp).just_pressed = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.up.pressed);
        assert!(snap.digital.up.just_pressed);
        assert!(!snap.digital.down.pressed);
    }

    #[test]
    fn test_arrows_maps_to_directional() {
        let mut input = default_input();
        input.action_mut(InputAction::SecondaryDirectionLeft).active = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.left.pressed);
        assert!(!snap.digital.right.pressed);
    }

    #[test]
    fn test_combined_wasd_and_arrows() {
        let mut input = default_input();
        // Neither WASD nor arrow held (defaults); only the arrow's edge fires.
        input
            .action_mut(InputAction::SecondaryDirectionUp)
            .just_pressed = true; // arrow just pressed
        let snap = InputSnapshot::from_input_state(&input);
        assert!(!snap.digital.up.pressed);
        assert!(snap.digital.up.just_pressed); // OR of both
    }

    #[test]
    fn test_wasd_or_arrows_pressed_means_combined_pressed() {
        let mut input = default_input();
        input.action_mut(InputAction::MainDirectionRight).active = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.right.pressed);
    }

    #[test]
    fn test_action_buttons_map_directly() {
        let mut input = default_input();
        *input.action_mut(InputAction::Action1) = bool_state_pressed();
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.action_1.pressed);
        assert!(snap.digital.action_1.just_pressed);
        assert!(!snap.digital.action_1.just_released);
    }

    #[test]
    fn test_back_maps_from_action_back() {
        let mut input = default_input();
        input.action_mut(InputAction::Back).active = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.back.pressed);
    }

    #[test]
    fn test_special_maps_from_action_special() {
        let mut input = default_input();
        input.action_mut(InputAction::Special).active = true;
        input.action_mut(InputAction::Special).just_released = true;
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
        };
        let dbs = DigitalButtonState::from_bool_state(&bs);
        assert!(dbs.pressed);
        assert!(!dbs.just_pressed);
        assert!(dbs.just_released);
    }

    #[test]
    fn test_main_direction_fields_populated() {
        let mut input = default_input();
        input.action_mut(InputAction::MainDirectionUp).active = true;
        input.action_mut(InputAction::MainDirectionUp).just_pressed = true;
        input.action_mut(InputAction::MainDirectionLeft).active = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.main_up.pressed);
        assert!(snap.digital.main_up.just_pressed);
        assert!(!snap.digital.main_down.pressed);
        assert!(snap.digital.main_left.pressed);
        assert!(!snap.digital.main_right.pressed);
    }

    #[test]
    fn test_secondary_direction_fields_populated() {
        let mut input = default_input();
        input.action_mut(InputAction::SecondaryDirectionDown).active = true;
        input
            .action_mut(InputAction::SecondaryDirectionRight)
            .active = true;
        input
            .action_mut(InputAction::SecondaryDirectionRight)
            .just_released = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(!snap.digital.secondary_up.pressed);
        assert!(snap.digital.secondary_down.pressed);
        assert!(!snap.digital.secondary_left.pressed);
        assert!(snap.digital.secondary_right.pressed);
        assert!(snap.digital.secondary_right.just_released);
    }

    #[test]
    fn test_raw_and_combined_independent() {
        // Only arrow up pressed — combined up is true, main_up is false
        let mut input = default_input();
        input.action_mut(InputAction::SecondaryDirectionUp).active = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.up.pressed); // combined
        assert!(!snap.digital.main_up.pressed); // WASD raw — not pressed
        assert!(snap.digital.secondary_up.pressed); // arrow raw — pressed
    }

    #[test]
    fn test_debug_field_populated() {
        let mut input = default_input();
        input.action_mut(InputAction::ToggleDebug).active = true;
        input.action_mut(InputAction::ToggleDebug).just_pressed = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.debug.pressed);
        assert!(snap.digital.debug.just_pressed);
    }

    #[test]
    fn test_fullscreen_field_populated() {
        let mut input = default_input();
        input.action_mut(InputAction::ToggleFullscreen).active = true;
        input
            .action_mut(InputAction::ToggleFullscreen)
            .just_released = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.fullscreen.pressed);
        assert!(snap.digital.fullscreen.just_released);
        assert!(!snap.digital.fullscreen.just_pressed);
    }

    #[test]
    fn test_action3_field_populated() {
        let mut input = default_input();
        input.action_mut(InputAction::Action3).active = true;
        input.action_mut(InputAction::Action3).just_pressed = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.action_3.pressed);
        assert!(snap.digital.action_3.just_pressed);
        assert!(!snap.digital.action_3.just_released);
    }

    #[test]
    fn test_scroll_y_propagated() {
        let mut input = default_input();
        input.scroll_y = 1.5;
        let snap = InputSnapshot::from_input_state(&input);
        assert_eq!(snap.analog.scroll_y, 1.5);
    }

    #[test]
    fn test_mouse_screen_pos_propagated() {
        let mut input = default_input();
        input.mouse_x = 320.0;
        input.mouse_y = 180.0;
        let snap = InputSnapshot::from_input_state(&input);
        assert_eq!(snap.analog.mouse_x, 320.0);
        assert_eq!(snap.analog.mouse_y, 180.0);
    }

    #[test]
    fn test_mouse_world_pos_propagated() {
        let mut input = default_input();
        input.mouse_world_x = -150.0;
        input.mouse_world_y = 75.5;
        let snap = InputSnapshot::from_input_state(&input);
        assert_eq!(snap.analog.mouse_world_x, -150.0);
        assert_eq!(snap.analog.mouse_world_y, 75.5);
    }

    #[test]
    fn test_mouse_left_button_field_populated() {
        let mut input = default_input();
        input.mouse_left_button.active = true;
        input.mouse_left_button.just_pressed = true;
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.digital.mouse_left.pressed);
        assert!(snap.digital.mouse_left.just_pressed);
        assert!(!snap.digital.mouse_left.just_released);
    }

    #[test]
    fn test_gamepad_fields_propagated() {
        let mut input = default_input();
        input.gamepad_connected = true;
        input.gamepad_axes = [0.1, -0.2, 0.3, -0.4, 0.5, -0.6];
        let snap = InputSnapshot::from_input_state(&input);
        assert!(snap.analog.gamepad_connected);
        assert_eq!(snap.analog.pad_left_x, 0.1);
        assert_eq!(snap.analog.pad_left_y, -0.2);
        assert_eq!(snap.analog.pad_right_x, 0.3);
        assert_eq!(snap.analog.pad_right_y, -0.4);
        assert_eq!(snap.analog.pad_lt, 0.5);
        assert_eq!(snap.analog.pad_rt, -0.6);
    }
}
