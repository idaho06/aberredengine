//! Timer expiration events.
//!
//! When a [`Timer`](crate::components::timer::Timer) component reaches its
//! duration, a [`TimerEvent`] is triggered on the entity. Observers can
//! subscribe to this event to perform actions like removing components or
//! changing entity state.
//!
//! # Example
//!
//! ```ignore
//! commands.add_observer(|trigger: On<TimerEvent>, mut commands: Commands| {
//!     match trigger.signal.as_str() {
//!         "remove_sticky" => {
//!             // Clear flag, remove timer, etc.
//!         }
//!         _ => {}
//!     }
//! });
//! ```
//!
//! # Related
//!
//! - [`crate::components::timer::Timer`] – the timer component
//! - [`crate::systems::time::update_timers`] – the system that emits these events

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
