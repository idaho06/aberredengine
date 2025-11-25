//! Timer component for delayed events.
//!
//! The [`Timer`] component counts elapsed time each frame. When the
//! accumulated time exceeds `duration`, a [`TimerEvent`](crate::events::timer::TimerEvent)
//! is emitted carrying the specified signal name, and the timer resets.
//!
//! See [`crate::systems::time::update_timers`] for the update logic.

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
