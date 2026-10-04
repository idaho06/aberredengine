//! Lua-based timer component for delayed callbacks.
//!
//! The [`LuaTimer`] component wraps a core `Timer` (same firing rules and
//! `TimerMode`) plus the name of a Lua function. When the timer fires, a
//! [`LuaTimerEvent`](crate::events::luatimer::LuaTimerEvent) is triggered on the entity.
//!
//! # How It Works
//!
//! 1. Entity is spawned with a `LuaTimer` containing duration and callback name
//! 2. The `update_lua_timers` system advances its `Timer` each sim tick and emits
//!    `LuaTimerEvent` when it fires (repeating timers reset, one-shots remove the `LuaTimer`)
//! 3. The `lua_timer_observer` receives the event:
//!    - Looks up the Lua function by name
//!    - Calls the function as `(ctx, input)`
//!    - Processes any commands queued by Lua (spawns, audio, signals, etc.)
//!
//! # Lua Callback Signature
//!
//! ```lua
//! function my_timer_callback(ctx, input)
//!     -- ctx contains entity state (id, pos, vel, signals, phase, timer, etc.)
//!     -- input contains digital/analog input state
//!     -- Full access to engine API
//!     engine.play_sound("beep")
//!     engine.spawn():with_position(100, 100):build()
//! end
//! ```
//!
//! # Usage from Lua
//!
//! ```lua
//! -- Add timer to existing entity
//! engine.entity_insert_lua_timer(entity_id, 2.5, "delayed_explosion")
//!
//! -- Add timer during spawn
//! engine.spawn()
//!     :with_position(100, 100)
//!     :with_lua_timer(3.0, "auto_despawn")
//!     :build()
//!
//! -- One-shot timer: fires once, then removes itself (the entity stays)
//! engine.entity_insert_lua_timer_once(entity_id, 1.5, "end_invulnerability")
//!
//! -- Timer callback
//! function delayed_explosion(ctx, input)
//!     engine.play_sound("boom")
//!     -- ctx.id is the entity ID, ctx.pos.x/y for position, etc.
//! end
//! ```
//!
//! # Related
//!
//! - [`crate::systems::luatimer::update_lua_timers`] – system that updates and triggers timers
//! - [`crate::systems::luatimer::lua_timer_observer`] – observer that executes Lua callbacks
//! - [`crate::events::luatimer::LuaTimerEvent`] – event emitted when timer expires

use std::sync::Arc;

use aberred_core::components::timer::{Timer, TimerMode};
use bevy_ecs::prelude::Component;

/// Countdown timer that calls a Lua function when it expires; ticked by
/// [`update_lua_timers`](crate::systems::luatimer::update_lua_timers).
///
/// Wraps a core [`Timer`], so it follows the same firing rules and [`TimerMode`],
/// and adds the name of the Lua function to call.
#[derive(Component, Clone, Debug)]
pub struct LuaTimer {
    /// Duration, elapsed time and mode.
    pub timer: Timer,
    /// Lua function to call when the timer fires.
    pub callback: Arc<str>,
}

impl LuaTimer {
    /// Create a timer that fires every `duration` seconds.
    pub fn new(duration: f32, callback: impl Into<Arc<str>>) -> Self {
        Self::with_mode(duration, callback, TimerMode::Repeat)
    }

    /// Create a timer that fires once after `duration` seconds, then removes itself.
    pub fn once(duration: f32, callback: impl Into<Arc<str>>) -> Self {
        Self::with_mode(duration, callback, TimerMode::Once)
    }

    /// Create a timer with the given [`TimerMode`].
    pub fn with_mode(duration: f32, callback: impl Into<Arc<str>>, mode: TimerMode) -> Self {
        Self {
            timer: Timer::with_mode(duration, mode),
            callback: callback.into(),
        }
    }
}
