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
    pub action_3: BoolState,
    pub mode_debug: BoolState,
    pub fullscreen_toggle: BoolState,
    pub action_special: BoolState,
    /// Mouse wheel scroll delta this frame. Positive = up, negative = down.
    pub scroll_y: f32,
    /// Mouse X in game/render-target space (letterbox-corrected). Range: 0.0..render_width.
    pub mouse_x: f32,
    /// Mouse Y in game/render-target space (letterbox-corrected). Range: 0.0..render_height.
    pub mouse_y: f32,
    /// Mouse X in world-space (after camera transform). Matches MapPosition coordinates.
    pub mouse_world_x: f32,
    /// Mouse Y in world-space (after camera transform). Matches MapPosition coordinates.
    pub mouse_world_y: f32,
    /// Raw left mouse button state. Unlike action_1/action_2/etc., this is
    /// NOT routed through InputBindings/InputAction rebinding — GUI hit
    /// testing always reacts to the literal left mouse button, same tier as
    /// mouse_x/mouse_y.
    pub mouse_left_button: BoolState,
}

impl BoolState {
    /// Clear the one-shot edge flags, leaving `active` (held state) untouched.
    pub fn clear_edge(&mut self) {
        self.just_pressed = false;
        self.just_released = false;
    }


    /// Force this action to read as not-held/not-just-pressed, without
    /// touching `just_released`. Used to mask input meant for the F11 debug
    /// imgui overlay -- `just_released` must never be suppressed, or
    /// gameplay sees a "stuck held" input once imgui grabs focus mid-press.
    pub fn force_inactive(&mut self) {
        self.active = false;
        self.just_pressed = false;
    }

    /// OR a `was -> now` transition's edges into this state (never clearing
    /// an edge an earlier diff already set) and take `now` as the held
    /// state. Used by the sim-side input resolver (`systems/input.rs`) to
    /// diff a backlogged raw device sample against the previous one, once
    /// per bound action.
    pub fn apply_edge(&mut self, was: bool, now: bool) {
        self.just_pressed |= now && !was;
        self.just_released |= !now && was;
        self.active = now;
    }
}

impl InputState {
    /// All digital `BoolState` fields, including `mouse_left_button`, as
    /// mutable references. Single source of truth for "every digital field"
    /// so `clear_edges` and its test enumerate the field list exactly once.
    fn bool_fields_mut(&mut self) -> [&mut BoolState; 16] {
        [
            &mut self.maindirection_up,
            &mut self.maindirection_left,
            &mut self.maindirection_down,
            &mut self.maindirection_right,
            &mut self.secondarydirection_up,
            &mut self.secondarydirection_down,
            &mut self.secondarydirection_left,
            &mut self.secondarydirection_right,
            &mut self.action_back,
            &mut self.action_1,
            &mut self.action_2,
            &mut self.action_3,
            &mut self.mode_debug,
            &mut self.fullscreen_toggle,
            &mut self.action_special,
            &mut self.mouse_left_button,
        ]
    }


    /// All keyboard-sourced digital fields (everything [`bool_fields_mut`](Self::bool_fields_mut)
    /// returns except `mouse_left_button`), as mutable references. Reused by
    /// `resolve_input_backlog`'s imgui keyboard-capture masking so that list
    /// isn't hand-duplicated in a second place -- see `bool_fields_mut`'s
    /// doc comment for why a single enumerated source matters.
    pub(crate) fn keyboard_bool_fields_mut(&mut self) -> [&mut BoolState; 15] {
        [
            &mut self.maindirection_up,
            &mut self.maindirection_left,
            &mut self.maindirection_down,
            &mut self.maindirection_right,
            &mut self.secondarydirection_up,
            &mut self.secondarydirection_down,
            &mut self.secondarydirection_left,
            &mut self.secondarydirection_right,
            &mut self.action_back,
            &mut self.action_1,
            &mut self.action_2,
            &mut self.action_3,
            &mut self.mode_debug,
            &mut self.fullscreen_toggle,
            &mut self.action_special,
        ]
    }

    /// Clear `just_pressed`/`just_released` on every digital field (including
    /// `mouse_left_button`), leaving `active` untouched. Called once per FIXED
    /// substep so an edge is delivered to exactly one substep regardless of
    /// how many substeps run in a given render frame.
    pub fn clear_edges(&mut self) {
        for bs in self.bool_fields_mut() {
            bs.clear_edge();
        }
    }
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
        assert!(!input.action_3.active);
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

    #[test]
    fn test_inputstate_mouse_left_button_default_inactive() {
        let input = InputState::default();
        assert!(!input.mouse_left_button.active);
        assert!(!input.mouse_left_button.just_pressed);
        assert!(!input.mouse_left_button.just_released);
    }

    #[test]
    fn test_clear_edges_zeroes_every_field_but_keeps_active() {
        let mut input = InputState::default();
        // Set active + both edges on every field, including mouse_left_button
        // (reuses the same field enumeration `clear_edges` itself uses).
        for bs in input.bool_fields_mut() {
            bs.active = true;
            bs.just_pressed = true;
            bs.just_released = true;
        }

        input.clear_edges();

        for bs in input.bool_fields_mut() {
            assert!(bs.active, "active must be untouched by clear_edges");
            assert!(!bs.just_pressed, "just_pressed must be cleared");
            assert!(!bs.just_released, "just_released must be cleared");
        }
    }

    #[test]
    fn test_clear_edge_leaves_active_untouched() {
        let mut bs = BoolState {
            active: true,
            just_pressed: true,
            just_released: true,
        };
        bs.clear_edge();
        assert!(bs.active);
        assert!(!bs.just_pressed);
        assert!(!bs.just_released);
    }
}
