//! Per-tick resolved input resource.
//!
//! [`InputState`] holds the input the game reads each sim tick: bound actions,
//! the raw left mouse button, mouse position and scroll, and pad 0's analog
//! axes. The hardware inputs that trigger each action are stored separately in
//! [`InputBindings`](crate::resources::input_bindings::InputBindings).
//! [`InputAction`] lists the bindable actions both of them index.
use bevy_ecs::prelude::*;

/// Enumeration of logical input actions.
///
/// These abstract the physical keys into gameplay-meaningful actions.
///
/// Each discriminant is the action's slot index ([`index`](Self::index)).
/// They are explicit so reordering variants never changes another
/// variant's slot, and a duplicate value is a compile error (E0081).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InputAction {
    /// Primary direction: up (default: W key).
    MainDirectionUp = 0,
    /// Primary direction: down (default: S key).
    MainDirectionDown = 1,
    /// Primary direction: left (default: A key).
    MainDirectionLeft = 2,
    /// Primary direction: right (default: D key).
    MainDirectionRight = 3,
    /// Secondary direction: up (default: Up arrow).
    SecondaryDirectionUp = 4,
    /// Secondary direction: down (default: Down arrow).
    SecondaryDirectionDown = 5,
    /// Secondary direction: left (default: Left arrow).
    SecondaryDirectionLeft = 6,
    /// Secondary direction: right (default: Right arrow).
    SecondaryDirectionRight = 7,
    /// Back/cancel action (default: Escape).
    Back = 8,
    /// Primary action button (default: Space, left mouse button).
    Action1 = 9,
    /// Secondary action button (default: Enter, right mouse button).
    Action2 = 10,
    /// Tertiary action button (default: mouse middle button).
    Action3 = 11,
    /// Special function (default: F12).
    Special = 12,
    /// Toggle debug overlays (default: F11). Triggers
    /// [`DebugToggled`](crate::events::switchdebug::DebugToggled)
    /// instead of an [`InputActionChanged`](crate::events::input::InputActionChanged).
    ToggleDebug = 13,
    /// Toggle fullscreen mode (default: F10). Emits no [`InputActionChanged`](crate::events::input::InputActionChanged): the
    /// logic loop sends `RenderMsg::ToggleFullscreen`, which triggers
    /// `SwitchFullScreenEvent` in the render world (`aberred-render`).
    ToggleFullscreen = 14,
}

impl InputAction {
    /// Total number of variants -- the size of the per-action arrays that
    /// [`InputBindings`](crate::resources::input_bindings::InputBindings) and
    /// [`InputState`] index with
    /// [`index`](Self::index), rather than hashing a `HashMap` key on every
    /// lookup (every action's bindings are looked up twice per backlogged
    /// input sample, every sim tick -- default 240Hz).
    pub const COUNT: usize = 15;

    /// Every variant, in the same order as [`Self::index`] assigns slots.
    pub const ALL: [InputAction; Self::COUNT] = [
        InputAction::MainDirectionUp,
        InputAction::MainDirectionDown,
        InputAction::MainDirectionLeft,
        InputAction::MainDirectionRight,
        InputAction::SecondaryDirectionUp,
        InputAction::SecondaryDirectionDown,
        InputAction::SecondaryDirectionLeft,
        InputAction::SecondaryDirectionRight,
        InputAction::Back,
        InputAction::Action1,
        InputAction::Action2,
        InputAction::Action3,
        InputAction::Special,
        InputAction::ToggleDebug,
        InputAction::ToggleFullscreen,
    ];

    /// Slot index of this action, in `0..COUNT`: the explicit discriminant.
    /// `InputBindings` and `InputState` store one slot per action at this
    /// index.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Whether this is an engine-level toggle (F11 debug overlay, F10
    /// fullscreen) rather than a gameplay action. Engine toggles never emit
    /// an [`InputActionChanged`](crate::events::input::InputActionChanged) (debug triggers its own `DebugToggled`;
    /// fullscreen is read straight off the resolved `InputState` by the logic
    /// loop) and are never masked by imgui keyboard capture, so F11 can
    /// always close the overlay that holds focus.
    ///
    /// An exhaustive match (no `_` arm), so a new variant must be classified.
    pub const fn is_engine_toggle(self) -> bool {
        match self {
            InputAction::ToggleDebug | InputAction::ToggleFullscreen => true,
            InputAction::MainDirectionUp
            | InputAction::MainDirectionDown
            | InputAction::MainDirectionLeft
            | InputAction::MainDirectionRight
            | InputAction::SecondaryDirectionUp
            | InputAction::SecondaryDirectionDown
            | InputAction::SecondaryDirectionLeft
            | InputAction::SecondaryDirectionRight
            | InputAction::Back
            | InputAction::Action1
            | InputAction::Action2
            | InputAction::Action3
            | InputAction::Special => false,
        }
    }

    /// Canonical name of this action: the string Lua passes to
    /// `engine.rebind_action()` / `engine.get_binding()`, and the label the
    /// F11 debug overlay's input panel shows.
    pub const fn name(self) -> &'static str {
        match self {
            InputAction::MainDirectionUp => "main_up",
            InputAction::MainDirectionDown => "main_down",
            InputAction::MainDirectionLeft => "main_left",
            InputAction::MainDirectionRight => "main_right",
            InputAction::SecondaryDirectionUp => "secondary_up",
            InputAction::SecondaryDirectionDown => "secondary_down",
            InputAction::SecondaryDirectionLeft => "secondary_left",
            InputAction::SecondaryDirectionRight => "secondary_right",
            InputAction::Back => "back",
            InputAction::Action1 => "action_1",
            InputAction::Action2 => "action_2",
            InputAction::Action3 => "action_3",
            InputAction::Special => "special",
            InputAction::ToggleDebug => "toggle_debug",
            InputAction::ToggleFullscreen => "toggle_fullscreen",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
/// Held/edge state of one digital input (a bound action or the raw left
/// mouse button).
///
/// Tracks whether the input is held this tick, was just pressed, or was just
/// released. Hardware assignments live in `InputBindings`, not here.
pub struct BoolState {
    /// Whether the input is held this tick.
    pub active: bool,
    /// Whether the input was just pressed this tick.
    pub just_pressed: bool,
    /// Whether the input was just released this tick.
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
    /// Mouse wheel scroll delta this tick. Positive = up, negative = down.
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
    /// Whether pad 0 is connected this tick. Only pad 0 is surfaced; see
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

    #[test]
    fn all_lists_variants_in_index_order() {
        // `ALL`'s doc promises `index()` order; `InputState` and
        // `InputBindings` both store per-action slots by `index()`, and
        // `ALL`-ordered loops (event emission, the F11 panel) rely on it.
        for (i, action) in InputAction::ALL.into_iter().enumerate() {
            assert_eq!(action.index(), i, "{action:?} is out of order in ALL");
        }
    }

    #[test]
    fn name_is_the_canonical_lua_action_name() {
        let expected: [(InputAction, &str); InputAction::COUNT] = [
            (InputAction::MainDirectionUp, "main_up"),
            (InputAction::MainDirectionDown, "main_down"),
            (InputAction::MainDirectionLeft, "main_left"),
            (InputAction::MainDirectionRight, "main_right"),
            (InputAction::SecondaryDirectionUp, "secondary_up"),
            (InputAction::SecondaryDirectionDown, "secondary_down"),
            (InputAction::SecondaryDirectionLeft, "secondary_left"),
            (InputAction::SecondaryDirectionRight, "secondary_right"),
            (InputAction::Back, "back"),
            (InputAction::Action1, "action_1"),
            (InputAction::Action2, "action_2"),
            (InputAction::Action3, "action_3"),
            (InputAction::Special, "special"),
            (InputAction::ToggleDebug, "toggle_debug"),
            (InputAction::ToggleFullscreen, "toggle_fullscreen"),
        ];
        for (action, name) in expected {
            assert_eq!(action.name(), name, "{action:?}");
        }
    }

    #[test]
    fn engine_toggles_are_exactly_toggle_debug_and_toggle_fullscreen() {
        let toggles: Vec<InputAction> = InputAction::ALL
            .into_iter()
            .filter(|a| a.is_engine_toggle())
            .collect();
        assert_eq!(
            toggles,
            vec![InputAction::ToggleDebug, InputAction::ToggleFullscreen]
        );
    }
}
