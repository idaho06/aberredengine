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
    /// Special function (default: F12).
    Special,
    // ToggleDebug, // Debug toggle has its own event
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
