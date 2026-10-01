//! Input action events.
//!
//! This module defines [`InputEvent`] which is triggered when gameplay-relevant
//! input actions occur (press or release). The [`InputAction`] enum lists all
//! recognized actions.
//!
//! Systems can subscribe to these events to react to input without directly
//! reading the [`InputState`](crate::resources::input::InputState) resource.

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
    /// Primary action button (default: Space).
    Action1 = 9,
    /// Secondary action button (default: Enter).
    Action2 = 10,
    /// Tertiary action button (default: mouse middle button).
    Action3 = 11,
    /// Special function (default: F12).
    Special = 12,
    /// Toggle debug overlays (default: F11). Still triggers [`SwitchDebugEvent`](crate::events::switchdebug::SwitchDebugEvent) internally.
    ToggleDebug = 13,
    /// Toggle fullscreen mode (default: F10). Still triggers `SwitchFullScreenEvent` (render world, `aberred-render`) internally.
    ToggleFullscreen = 14,
}

impl InputAction {
    /// Total number of variants -- the size of the flat array
    /// [`InputBindings`](crate::resources::input_bindings::InputBindings) indexes
    /// with [`index`](Self::index) instead of hashing a `HashMap` key on
    /// every lookup (this enum's bindings are read up to 15 times per
    /// backlogged input sample, every sim tick -- default 240Hz).
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
    /// an [`InputEvent`] (debug triggers its own `SwitchDebugEvent`;
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

/// Event emitted when an input action is pressed or released.
///
/// The `action` field identifies which logical action occurred, and `pressed`
/// indicates whether it was a press (true) or release (false).
#[derive(Event, Debug, Clone, Copy)]
pub struct InputEvent {
    /// The input action that triggered this event.
    pub action: InputAction,
    /// Whether the action was pressed (true) or released (false).
    pub pressed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

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
