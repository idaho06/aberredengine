//! Timer expiration event.
//!
//! When a [`Timer`](crate::components::timer::Timer) component reaches its
//! duration, [`update_timers`](crate::systems::timer::update_timers) triggers a
//! [`TimerFired`] targeted at the timer's entity. Observe it per entity
//! (`commands.spawn(..).observe(handler)`) or globally (`add_observer(handler)`).
//!
//! # Related
//!
//! - [`crate::components::timer::Timer`] – the timer component
//! - `aberred_lua::events::luatimer::LuaTimerEvent` – Lua equivalent

use bevy_ecs::prelude::*;

/// Triggered on an entity each time its [`Timer`](crate::components::timer::Timer) fires.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct TimerFired {
    /// The entity whose timer fired.
    #[event_target]
    pub entity: Entity,
}
