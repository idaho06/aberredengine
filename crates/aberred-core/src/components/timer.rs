//! Countdown timer component (repeating or one-shot).
//!
//! [`update_timers`](crate::systems::timer::update_timers) accumulates elapsed
//! time on every [`Timer`] each sim tick. When `elapsed >= duration`, it triggers
//! a [`TimerFired`](crate::events::timer::TimerFired) event targeted at the
//! entity, then applies the timer's [`TimerMode`].
//!
//! # Usage
//!
//! ```
//! # use bevy_ecs::prelude::*;
//! # use aberred_core::components::timer::Timer;
//! # use aberred_core::events::timer::TimerFired;
//! # use aberred_core::protocol::audio::AudioCmd;
//! # let mut world = World::new();
//! # let mut commands = world.commands();
//! commands
//!     .spawn(Timer::new(2.5))
//!     .observe(|_: On<TimerFired>, mut audio: MessageWriter<AudioCmd>| {
//!         audio.write(AudioCmd::PlayFx { id: "beep".into() });
//!     });
//! ```
//!
//! # Related
//!
//! - [`crate::systems::timer::update_timers`] – system that updates and triggers timers
//! - [`crate::events::timer::TimerFired`] – event triggered when a timer expires
//! - `aberred_lua::components::luatimer::LuaTimer` – Lua equivalent

use bevy_ecs::prelude::Component;

/// What a [`Timer`] does after it fires.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimerMode {
    /// Reset by subtracting `duration` (not zeroing, for timing accuracy) and
    /// fire again every `duration` seconds.
    #[default]
    Repeat,
    /// Fire once, then remove the `Timer` component (the entity is kept).
    /// `TimerFired` observers still see the `Timer`. A `Timer` inserted or
    /// changed after it fires (by an observer, or by a later system before the
    /// commands apply) is kept. A zero-duration timer fires on the next tick.
    Once,
}

/// Countdown timer.
///
/// Fires a [`TimerFired`](crate::events::timer::TimerFired) event when
/// `elapsed >= duration`; see [`TimerMode`] for what happens next.
#[derive(Component, Clone, Copy, Debug)]
pub struct Timer {
    /// Total duration in seconds before the timer fires.
    pub duration: f32,
    /// Elapsed time since last reset.
    pub elapsed: f32,
    /// Whether the timer repeats or fires once.
    pub mode: TimerMode,
}

impl Timer {
    /// Create a timer that fires every `duration` seconds.
    pub fn new(duration: f32) -> Self {
        Timer::with_mode(duration, TimerMode::Repeat)
    }

    /// Create a timer that fires once after `duration` seconds, then removes itself.
    pub fn once(duration: f32) -> Self {
        Timer::with_mode(duration, TimerMode::Once)
    }

    /// Create a timer with the given [`TimerMode`].
    pub fn with_mode(duration: f32, mode: TimerMode) -> Self {
        Timer {
            duration,
            elapsed: 0.0,
            mode,
        }
    }

    /// Add `delta` to `elapsed`; returns `true` when the timer is due to fire
    /// (`elapsed >= duration`).
    pub fn advance(&mut self, delta: f32) -> bool {
        self.elapsed += delta;
        self.elapsed >= self.duration
    }

    /// Reset the timer by subtracting the duration from elapsed time.
    ///
    /// This maintains timing accuracy even if processing is delayed,
    /// allowing for consistent periodic firing.
    pub fn reset(&mut self) {
        self.elapsed -= self.duration;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::approx_eq;

    #[test]
    fn advance_accumulates_and_reports_when_due() {
        let mut timer = Timer::new(1.0);
        assert!(!timer.advance(0.6));
        assert!(timer.advance(0.6), "elapsed 1.2 >= duration 1.0");
        assert!(approx_eq(timer.elapsed, 1.2));
    }

    #[test]
    fn zero_duration_timer_is_due_on_its_first_advance() {
        assert!(Timer::once(0.0).advance(0.0));
    }

    #[test]
    fn test_reset_subtracts_duration() {
        let mut timer = Timer::new(1.0);
        timer.elapsed = 1.3;
        timer.reset();
        assert!(approx_eq(timer.elapsed, 0.3));
    }
}
