//! Lua-based timer component for delayed callbacks.
//!
//! The [`LuaTimer`] component counts elapsed time each frame. When the
//! accumulated time exceeds `duration`, a [`LuaTimerEvent`](crate::events::luatimer::LuaTimerEvent)
//! is triggered on the entity, and the timer resets by subtracting the duration.
//!
//! # How It Works
//!
//! 1. Entity is spawned with a `LuaTimer` containing duration and callback name
//! 2. The `update_lua_timers` system runs each frame:
//!    - Accumulates delta time into `elapsed`
//!    - When `elapsed >= duration`, emits `LuaTimerEvent` and resets
//! 3. The `lua_timer_observer` receives the event:
//!    - Looks up the Lua function by name
//!    - Calls the function with `entity_id` as parameter
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

use bevy_ecs::prelude::Component;

/// Countdown timer that calls a Lua function when finished.
///
/// The timer accumulates time from [`WorldTime`](crate::resources::worldtime::WorldTime)
/// and emits a [`LuaTimerEvent`](crate::events::luatimer::LuaTimerEvent) when `elapsed >= duration`.
/// The Lua callback receives the entity ID and can perform any engine operations.
#[derive(Component, Clone)]
pub struct LuaTimer {
    /// Total duration in seconds before the timer fires.
    pub duration: f32,
    /// Elapsed time since last reset.
    pub elapsed: f32,
    /// Lua function name to call when timer expires.
    pub callback: String,
}

impl LuaTimer {
    /// Create a new LuaTimer with the given duration and callback function name.
    ///
    /// # Arguments
    ///
    /// * `duration` - Time in seconds before firing
    /// * `callback` - Name of Lua function to call (receives entity_id as parameter)
    pub fn new(duration: f32, callback: impl Into<String>) -> Self {
        LuaTimer {
            duration,
            elapsed: 0.0,
            callback: callback.into(),
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
