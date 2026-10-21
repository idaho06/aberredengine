//! Input action events.
//!
//! This module defines [`InputActionChanged`] which is triggered when gameplay-relevant
//! input actions occur (press or release). The actions themselves are
//! [`InputAction`], defined next to the [`InputState`](crate::resources::input::InputState)
//! it indexes.
//!
//! Systems can subscribe to these events to react to input without directly
//! reading the [`InputState`](crate::resources::input::InputState) resource.

use bevy_ecs::prelude::*;

use crate::resources::input::InputAction;

/// Event emitted when an input action is pressed or released.
///
/// The `action` field identifies which logical action occurred, and `pressed`
/// indicates whether it was a press (true) or release (false).
#[derive(Event, Debug, Clone, Copy)]
pub struct InputActionChanged {
    /// The input action that triggered this event.
    pub action: InputAction,
    /// Whether the action was pressed (true) or released (false).
    pub pressed: bool,
}
