//! Per-frame keyboard input resource.
//!
//! Captures the subset of keyboard state the game cares about and exposes it
//! to systems via the [`InputState`] resource.  The hardware keys that trigger
//! each action are stored separately in
//! [`InputBindings`](crate::resources::input_bindings::InputBindings).
use bevy_ecs::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
/// Transient boolean key state for a single logical action.
///
/// Tracks whether the action is active this frame, was just pressed, or was
/// just released.  Hardware key assignments live in `InputBindings`, not here.
pub struct BoolState {
    /// Whether the action is currently active/held this frame.
    pub active: bool,
    /// Whether the action was just pressed this frame.
    pub just_pressed: bool,
    /// Whether the action was just released this frame.
    pub just_released: bool,
}

/// Resource capturing the per-frame keyboard state relevant to gameplay.
///
/// Fields are grouped by purpose: main movement (WASD), secondary movement
/// (arrow keys), and actions (escape/space/enter/F-keys).
#[derive(Resource, Debug, Clone, Default)]
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
    pub fullscreen_toggle: BoolState,
    pub action_special: BoolState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boolstate_default() {
        let bs = BoolState::default();
        assert!(!bs.active);
        assert!(!bs.just_pressed);
        assert!(!bs.just_released);
    }

    #[test]
    fn test_inputstate_default_all_inactive() {
        let input = InputState::default();
        assert!(!input.maindirection_up.active);
        assert!(!input.maindirection_down.active);
        assert!(!input.maindirection_left.active);
        assert!(!input.maindirection_right.active);
        assert!(!input.secondarydirection_up.active);
        assert!(!input.secondarydirection_down.active);
        assert!(!input.secondarydirection_left.active);
        assert!(!input.secondarydirection_right.active);
        assert!(!input.action_back.active);
        assert!(!input.action_1.active);
        assert!(!input.action_2.active);
        assert!(!input.mode_debug.active);
        assert!(!input.fullscreen_toggle.active);
        assert!(!input.action_special.active);
    }

    #[test]
    fn test_inputstate_no_just_pressed_on_default() {
        let input = InputState::default();
        assert!(!input.maindirection_up.just_pressed);
        assert!(!input.action_1.just_pressed);
        assert!(!input.action_back.just_released);
    }
}
