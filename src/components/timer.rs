//! Rust-based timer component for delayed/periodic callbacks.
//!
//! The [`Timer`] component counts elapsed time each frame. When the
//! accumulated time exceeds `duration`, a [`TimerEvent`](crate::events::timer::TimerEvent)
//! is triggered on the entity, and the timer resets by subtracting the duration.
//!
//! # How It Works
//!
//! 1. Entity is spawned with a `Timer` containing duration and a Rust callback
//! 2. The `update_timers` system runs each frame:
//!    - Accumulates delta time into `elapsed`
//!    - When `elapsed >= duration`, emits `TimerEvent` and resets
//! 3. The `timer_observer` receives the event:
//!    - Calls the Rust callback with `(entity, &mut GameCtx, &InputState)`
//!    - The callback has full ECS access through [`GameCtx`](crate::systems::GameCtx)
//!
//! # Callback Signature
//!
//! ```ignore
//! fn my_timer_callback(entity: Entity, ctx: &mut GameCtx, input: &InputState) {
//!     // Full access to ECS queries and resources via ctx
//!     ctx.audio.write(AudioCmd::PlayFx { id: "beep".into() });
//!     if let Ok(mut rb) = ctx.rigid_bodies.get_mut(entity) {
//!         rb.velocity = Vector2::zero();
//!     }
//! }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! commands.entity(my_entity).insert(Timer::new(2.5, my_timer_callback));
//! ```
//!
//! # Related
//!
//! - [`crate::systems::timer::update_timers`] – system that updates and triggers timers
//! - [`crate::systems::timer::timer_observer`] – observer that executes Rust callbacks
//! - [`crate::events::timer::TimerEvent`] – event emitted when timer expires
//! - [`crate::components::luatimer::LuaTimer`] – Lua equivalent

use bevy_ecs::prelude::{Component, Entity};

use crate::resources::input::InputState;
use crate::systems::GameCtx;

/// Callback type for Rust timers.
///
/// Receives the entity that owns the timer, a mutable reference to [`GameCtx`](crate::systems::GameCtx)
/// providing full ECS query/resource access, and the current input state.
pub type TimerCallback = for<'w, 's> fn(Entity, &mut GameCtx<'w, 's>, &InputState);

/// Generic repeating countdown timer.
///
/// The default `Timer` type stores a Rust function pointer via [`TimerCallback`]
/// and is processed by [`update_timers`](crate::systems::timer::update_timers).
/// The Lua-facing [`LuaTimer`](crate::components::luatimer::LuaTimer) alias
/// reuses this same storage with a [`LuaTimerCallback`](crate::components::luatimer::LuaTimerCallback)
/// payload.
///
/// `elapsed` is reset by subtracting `duration` (not zeroed) for timing accuracy.
#[derive(Component, Clone, Copy)]
pub struct Timer<C = TimerCallback> {
    /// Total duration in seconds before the timer fires.
    pub duration: f32,
    /// Elapsed time since last reset.
    pub elapsed: f32,
    /// Callback payload — a Rust fn pointer for `Timer`, or a
    /// [`LuaTimerCallback`](crate::components::luatimer::LuaTimerCallback) for `LuaTimer`.
    pub callback: C,
}

impl<C> Timer<C> {
    /// Create a new Timer with the given duration and callback.
    ///
    /// # Arguments
    ///
    /// * `duration` - Time in seconds before firing (repeats every `duration` seconds)
    /// * `callback` - Callback payload to store
    pub fn new(duration: f32, callback: C) -> Self {
        Timer {
            duration,
            elapsed: 0.0,
            callback,
        }
    }

    /// Reset the timer by subtracting the duration from elapsed time.
    ///
    /// This maintains timing accuracy even if processing is delayed,
    /// allowing for consistent periodic callbacks.
    pub fn reset(&mut self) {
        self.elapsed -= self.duration;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_callback(_entity: Entity, _ctx: &mut GameCtx, _input: &InputState) {}

    #[test]
    fn test_new_sets_duration_and_zero_elapsed() {
        let timer = Timer::new(2.5, dummy_callback);
        assert_eq!(timer.duration, 2.5);
        assert_eq!(timer.elapsed, 0.0);
    }

    #[test]
    fn test_reset_subtracts_duration() {
        let mut timer = Timer::new(1.0, dummy_callback);
        timer.elapsed = 1.3;
        timer.reset();
        assert!((timer.elapsed - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_timer_is_copy() {
        let timer = Timer::new(1.0, dummy_callback);
        let timer2 = timer; // Copy
        assert_eq!(timer.duration, timer2.duration);
    }
}
