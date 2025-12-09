//! Timer component for delayed events.
//!
//! The [`Timer`] component counts elapsed time each frame. When the
//! accumulated time exceeds `duration`, a [`TimerEvent`](crate::events::timer::TimerEvent)
//! is triggered on the entity, and the timer resets.
//!
//! # Usage with Observers
//!
//! Subscribe to [`TimerEvent`](crate::events::timer::TimerEvent) with an observer
//! to react when timers expire. The `signal` field helps identify which timer
//! fired when an entity has multiple timer uses.
//!
//! # Common Patterns
//!
//! - Remove [`StuckTo`](super::stuckto::StuckTo) after a delay (signal: `"remove_stuck_to"`)
//! - Clear a flag from [`Signals`](super::signals::Signals) (signal: `"remove_sticky"`)
//! - Trigger phase transitions or spawn effects
//!
//! # Example
//!
//! ```ignore
//! // Set up a timer
//! commands.entity(ball).insert(Timer::new(2.0, "remove_stuck_to"));
//!
//! // Add an observer to handle the event
//! commands.add_observer(|trigger: On<TimerEvent>, mut commands: Commands| {
//!     if trigger.signal == "remove_stuck_to" {
//!         commands.entity(trigger.entity).remove::<StuckTo>();
//!         commands.entity(trigger.entity).remove::<Timer>();
//!     }
//! });
//! ```
//!
//! # Related
//!
//! - [`crate::systems::time::update_timers`] – the system that updates and triggers timers
//! - [`crate::events::timer::TimerEvent`] – the event emitted when a timer expires

use bevy_ecs::prelude::Component;

/// Countdown timer that emits an event when finished.
///
/// The timer accumulates time from [`WorldTime`](crate::resources::worldtime::WorldTime)
/// and emits a [`TimerEvent`](crate::events::timer::TimerEvent) when `elapsed >= duration`.
#[derive(Component)]
pub struct Timer {
    /// Total duration in seconds before the timer fires.
    pub duration: f32,
    /// Elapsed time since last reset.
    pub elapsed: f32,
    /// Signal name included in the emitted [`TimerEvent`](crate::events::timer::TimerEvent).
    pub signal: String,
}
impl Timer {
    pub fn new(duration: f32, signal: impl Into<String>) -> Self {
        Timer {
            duration,
            elapsed: 0.0,
            signal: signal.into(),
        }
    }
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
    }
}
