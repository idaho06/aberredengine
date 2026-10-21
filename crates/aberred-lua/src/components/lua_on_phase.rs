//! Lua callback components for an entity's core `Phase`.
//!
//! The entity holds a core `Phase` (driven by `phase_system`, same rules as any
//! Rust phase entity) plus up to one callback per kind, each naming a Lua
//! function that branches on `ctx.phase`:
//!
//! - [`LuaOnPhaseEntered`]: `fn(ctx, input)`, for each `PhaseEntered`.
//!   `ctx.previous_phase` is the phase left (`nil` for the initial phase).
//! - [`LuaOnPhaseExited`]: `fn(ctx)`, for each `PhaseExited`. `ctx.phase` is
//!   already the new phase; `ctx.previous_phase` is the exited one.
//! - [`LuaOnPhaseUpdate`]: `fn(ctx, input, dt)`, once per sim tick.
//!
//! [`lua_phase_callbacks_system`](crate::systems::lua_phase::lua_phase_callbacks_system)
//! calls all three after the tick's collision, while the game is playing.
//!
//! A string returned by an enter or update callback requests that phase (sets
//! `Phase.next`) and beats an `engine.phase_transition` call made in the same
//! tick. Returning the current phase name is a no-op.
//!
//! # Lua Callback Signature
//!
//! ```lua
//! function player_phase_entered(ctx, input)
//!     if ctx.phase == "hurt" then
//!         engine.play_sound("ouch")
//!         return nil
//!     end
//! end
//! ```

use std::sync::Arc;

use bevy_ecs::prelude::Component;

/// Names the Lua function to call when the entity's core `Phase` enters a phase.
#[derive(Component, Clone, Debug)]
pub struct LuaOnPhaseEntered {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnPhaseEntered {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}

/// Names the Lua function to call when the entity's core `Phase` exits a phase.
#[derive(Component, Clone, Debug)]
pub struct LuaOnPhaseExited {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnPhaseExited {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}

/// Names the Lua function to call once per sim tick for the entity's core `Phase`.
#[derive(Component, Clone, Debug)]
pub struct LuaOnPhaseUpdate {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnPhaseUpdate {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}
