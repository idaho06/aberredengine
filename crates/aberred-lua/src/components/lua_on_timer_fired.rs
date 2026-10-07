//! Lua callback component fired by the entity's core `Timer`.
//!
//! The entity holds a core `Timer` (ticked by `update_timers`, same firing rules
//! and `TimerMode` as any Rust timer) plus a [`LuaOnTimerFired`] naming the Lua
//! function. When the timer triggers `TimerFired`,
//! [`lua_timer_fired_observer`](crate::systems::lua_timer_fired::lua_timer_fired_observer)
//! calls the function with `(ctx, input)` and processes the commands it queued.
//! Removing the `Timer` (a fired one-shot, or `engine.entity_remove_lua_timer`)
//! also removes the [`LuaOnTimerFired`]
//! ([`lua_timer_removed_observer`](crate::systems::lua_timer_fired::lua_timer_removed_observer)).
//!
//! An entity has one `Timer`: a Lua timer replaces a Rust `Timer` on the same
//! entity. Inserting a plain `Timer` over a Lua timer keeps its
//! [`LuaOnTimerFired`], so the new timer calls the same Lua function.
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
//! ```

use std::sync::Arc;

use aberred_core::components::timer::{Timer, TimerMode};
use bevy_ecs::prelude::Component;

/// Names the Lua function to call when the entity's core `Timer` fires.
#[derive(Component, Clone, Debug)]
pub struct LuaOnTimerFired {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnTimerFired {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }

    /// A Lua timer: a core [`Timer`] in `mode` plus the Lua function it calls.
    pub fn timer(
        duration: f32,
        callback: impl Into<Arc<str>>,
        mode: TimerMode,
    ) -> (Timer, LuaOnTimerFired) {
        (Timer::with_mode(duration, mode), Self::new(callback))
    }
}
