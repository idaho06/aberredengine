//! Rust timer expiration events.
//!
//! When a [`Timer`](crate::components::timer::Timer) component reaches its
//! duration, a [`TimerEvent`] is triggered. The observer system calls the
//! Rust callback with the entity and a `TimerCtx` providing full ECS access.
//!
//! # Event Flow
//!
//! 1. `update_timers` system detects timer expiration
//! 2. Emits `TimerEvent` with entity and callback function pointer
//! 3. `timer_observer` receives the event
//! 4. Calls the Rust callback with `(entity, &mut TimerCtx, &InputState)`
//!
//! # Related
//!
//! - [`crate::components::timer::Timer`] – the timer component
//! - [`crate::systems::timer::update_timers`] – system that emits these events
//! - [`crate::systems::timer::timer_observer`] – observer that handles these events
//! - [`crate::events::luatimer::LuaTimerEvent`] – Lua equivalent

use bevy_ecs::prelude::*;

use crate::components::timer::TimerCallback;

/// Event emitted when a Rust timer expires.
///
/// The `entity` field identifies the entity with the timer, and `callback`
/// contains the Rust function pointer to invoke. The function will be called
/// with `(entity, &mut TimerCtx, &InputState)`.
#[derive(Event, Clone, Copy)]
pub struct TimerEvent {
    /// The entity whose timer expired.
    pub entity: Entity,
    /// The Rust function to call.
    pub callback: TimerCallback,
}
