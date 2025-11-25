//! Timer expiration events.
//!
//! When a [`Timer`](crate::components::timer::Timer) component reaches its
//! duration, a [`TimerEvent`] is triggered containing the entity and signal
//! name.

use bevy_ecs::prelude::*;

/// Event emitted when a timer expires.
///
/// The `entity` field identifies the entity with the timer, and `signal`
/// contains the user-defined signal name from the timer component.
#[derive(Event, Debug, Clone, PartialEq, Eq)]
pub struct TimerEvent {
    /// The entity whose timer expired.
    pub entity: Entity,
    /// The signal name configured on the timer.
    pub signal: String,
}
