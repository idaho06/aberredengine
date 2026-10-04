//! Lua timer expiration events.
//!
//! When a [`LuaTimer`](crate::components::luatimer::LuaTimer) component reaches its
//! duration, a [`LuaTimerEvent`] is triggered on the entity. The observer system
//! calls the named Lua function as `(ctx, input)`.
//!
//! # Event Flow
//!
//! 1. `update_lua_timers` system detects timer expiration
//! 2. Emits `LuaTimerEvent` with entity and callback name
//! 3. `lua_timer_observer` receives the event
//! 4. Looks up and calls the Lua function
//! 5. Processes any commands queued by the Lua callback
//!
//! # Example Lua Callback
//!
//! ```lua
//! function my_timer_callback(ctx, input)
//!     engine.log("Timer fired for entity: " .. tostring(ctx.id))
//!     engine.play_sound("timer_beep")
//!     -- Can spawn entities, modify components, etc.
//! end
//! ```
//!
//! # Related
//!
//! - [`crate::components::luatimer::LuaTimer`] – the timer component
//! - [`crate::systems::luatimer::update_lua_timers`] – system that emits these events
//! - [`crate::systems::luatimer::lua_timer_observer`] – observer that handles these events

use bevy_ecs::prelude::*;

/// Event emitted when a Lua timer expires.
///
/// The `entity` field identifies the entity with the timer, and `callback`
/// contains the Lua function name to invoke. The Lua function is called as
/// `(ctx, input)`.
#[derive(Event, Debug, Clone)]
pub struct LuaTimerEvent {
    /// The entity whose timer expired.
    pub entity: Entity,
    /// The Lua function name to call.
    pub callback: std::sync::Arc<str>,
}
