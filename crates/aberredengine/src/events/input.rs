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
    /// Toggle debug overlays (default: F11). Still triggers [`SwitchDebugEvent`] internally.
    ToggleDebug,
    /// Toggle fullscreen mode (default: F10). Still triggers [`SwitchFullScreenEvent`] internally.
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
