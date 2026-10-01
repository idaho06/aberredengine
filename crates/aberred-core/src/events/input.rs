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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputAction {
    /// Primary direction: up (default: W key).
    MainDirectionUp,
    /// Primary direction: down (default: S key).
    MainDirectionDown,
    /// Primary direction: left (default: A key).
    MainDirectionLeft,
    /// Primary direction: right (default: D key).
    MainDirectionRight,
    /// Secondary direction: up (default: Up arrow).
    SecondaryDirectionUp,
    /// Secondary direction: down (default: Down arrow).
    SecondaryDirectionDown,
    /// Secondary direction: left (default: Left arrow).
    SecondaryDirectionLeft,
    /// Secondary direction: right (default: Right arrow).
    SecondaryDirectionRight,
    /// Back/cancel action (default: Escape).
    Back,
    /// Primary action button (default: Space).
    Action1,
    /// Secondary action button (default: Enter).
    Action2,
    /// Tertiary action button (default: mouse middle button).
    Action3,
    /// Special function (default: F12).
    Special,
    /// Toggle debug overlays (default: F11). Still triggers [`SwitchDebugEvent`](crate::events::switchdebug::SwitchDebugEvent) internally.
    ToggleDebug,
    /// Toggle fullscreen mode (default: F10). Still triggers `SwitchFullScreenEvent` (render world, `aberred-render`) internally.
    ToggleFullscreen,
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

    /// Slot index into `InputBindings`'s flat `[Vec<InputBinding>; COUNT]`
    /// array. A manual match (not `as usize` on the enum's discriminant) so
    /// adding/reordering a variant can't silently change another variant's
    /// index without a compiler-visible diff.
    pub const fn index(self) -> usize {
        match self {
            InputAction::MainDirectionUp => 0,
            InputAction::MainDirectionDown => 1,
            InputAction::MainDirectionLeft => 2,
            InputAction::MainDirectionRight => 3,
            InputAction::SecondaryDirectionUp => 4,
            InputAction::SecondaryDirectionDown => 5,
            InputAction::SecondaryDirectionLeft => 6,
            InputAction::SecondaryDirectionRight => 7,
            InputAction::Back => 8,
            InputAction::Action1 => 9,
            InputAction::Action2 => 10,
            InputAction::Action3 => 11,
            InputAction::Special => 12,
            InputAction::ToggleDebug => 13,
            InputAction::ToggleFullscreen => 14,
        }
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
