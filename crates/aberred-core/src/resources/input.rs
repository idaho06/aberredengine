//! Per-frame keyboard input resource.
//!
//! Captures the subset of keyboard state the game cares about and exposes it
//! to systems via the [`InputState`] resource.  The hardware keys that trigger
//! each action are stored separately in
//! [`InputBindings`](crate::resources::input_bindings::InputBindings).
use bevy_ecs::prelude::*;

use crate::events::input::InputAction;

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

/// Resource capturing the per-tick input state relevant to gameplay.
///
/// Bound actions are read by [`InputAction`] via [`action`](Self::action) /
/// [`action_mut`](Self::action_mut); the raw mouse button, mouse coordinates,
/// scroll and gamepad axes are plain fields.
#[derive(Resource, Debug, Clone, Default)]
pub struct InputState {
    /// One slot per bound action, indexed by [`InputAction::index`].
    actions: [BoolState; InputAction::COUNT],
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
    /// Raw left mouse button state. Unlike the bound actions, this is
    /// NOT routed through InputBindings/InputAction rebinding — GUI hit
    /// testing always reacts to the literal left mouse button, same tier as
    /// mouse_x/mouse_y.
    pub mouse_left_button: BoolState,
    /// Whether pad 0 is connected this tick. Pad-0-only for now; see
    /// `gamepad_axes`.
    pub gamepad_connected: bool,
    /// Pad 0's raw analog axis values (`[LX, LY, RX, RY, LT, RT]`, matching
    /// raylib's `GamepadAxis` ordinal order), last-sample-wins across a
    /// tick's backlog. Deliberately NOT deadzoned -- this is the raw value
    /// surfaced to gameplay/Lua; deadzone only affects
    /// `InputBinding::GamepadAxis`'s digital-threshold resolution.
    pub gamepad_axes: [f32; 6],
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
    /// The state of `action`. `mouse_left_button` is deliberately not an
    /// action (it is never rebindable), so it is a plain field instead.
    pub fn action(&self, action: InputAction) -> &BoolState {
        &self.actions[action.index()]
    }

    /// Mutable counterpart of [`action`](Self::action).
    pub fn action_mut(&mut self, action: InputAction) -> &mut BoolState {
        &mut self.actions[action.index()]
    }

    /// Every digital `BoolState` (all action slots, then
    /// `mouse_left_button`) as mutable references, so `clear_edges`
    /// enumerates them exactly once.
    ///
    /// An exhaustive destructure (no `..`): adding any field to `InputState`
    /// fails to compile here until it is classified as digital (yielded) or
    /// not (`_`).
    fn bool_fields_mut(&mut self) -> impl Iterator<Item = &mut BoolState> {
        let InputState {
            actions,
            scroll_y: _,
            mouse_x: _,
            mouse_y: _,
            mouse_world_x: _,
            mouse_world_y: _,
            mouse_left_button,
            gamepad_connected: _,
            gamepad_axes: _,
        } = self;
        actions.iter_mut().chain(std::iter::once(mouse_left_button))
    }

    /// Clear `just_pressed`/`just_released` on every digital field (including
    /// `mouse_left_button`), leaving `active` untouched. Called once per sim
    /// tick, right after `sim.run`, so an edge is delivered to exactly one
    /// tick.
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
    fn clear_edges_zeroes_every_field_but_keeps_active() {
        // Set and check every digital field independently of
        // `bool_fields_mut` (the list `clear_edges` itself walks), so a field
        // missing from that list fails here instead of passing vacuously.
        let mut input = InputState::default();
        let all_on = BoolState {
            active: true,
            just_pressed: true,
            just_released: true,
        };
        for action in InputAction::ALL {
            *input.action_mut(action) = all_on;
        }
        input.mouse_left_button = all_on;

        input.clear_edges();

        for action in InputAction::ALL {
            let bs = input.action(action);
            assert!(
                bs.active,
                "{action:?}: active must be untouched by clear_edges"
            );
            assert!(!bs.just_pressed, "{action:?}: just_pressed must be cleared");
            assert!(
                !bs.just_released,
                "{action:?}: just_released must be cleared"
            );
        }
        let mouse = input.mouse_left_button;
        assert!(mouse.active, "mouse_left_button: active must be untouched");
        assert!(
            !mouse.just_pressed,
            "mouse_left_button: just_pressed must be cleared"
        );
        assert!(
            !mouse.just_released,
            "mouse_left_button: just_released must be cleared"
        );
    }

    #[test]
    fn action_slots_are_independent() {
        // Uses only the public lookup, so it is independent of how
        // `InputState` stores its actions.
        for action in InputAction::ALL {
            let mut input = InputState::default();
            input.action_mut(action).active = true;

            assert!(input.action(action).active, "{action:?} must read back");
            let active = InputAction::ALL
                .into_iter()
                .filter(|&a| input.action(a).active)
                .count();
            assert_eq!(active, 1, "{action:?} must not share a slot");
            assert!(
                !input.mouse_left_button.active,
                "{action:?} must not alias mouse_left_button"
            );
        }
    }
}
