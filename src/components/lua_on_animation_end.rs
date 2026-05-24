//! Lua callback component fired when a non-looped animation finishes.
//!
//! Attach this component to any entity whose non-looped [`Animation`](crate::components::animation::Animation)
//! should invoke a Lua function when it first reaches its last frame.
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_death_done(ctx, input)
//!     engine.despawn(ctx.id)
//! end
//! ```
//!
//! The callback fires **exactly once** per animation completion — not on every
//! frame the entity stays on the last frame.
//!
//! # Usage from Lua
//!
//! ```lua
//! engine.spawn()
//!     :with_animation("death")
//!     :with_on_animation_end("on_death_done")
//!     :build()
//! ```
//!
//! # Usage from map files
//!
//! ```json
//! { "animation_key": "death", "on_animation_end": "on_death_done" }
//! ```

use bevy_ecs::prelude::Component;

/// Attaches a Lua callback to be called when the entity's non-looped animation finishes.
#[derive(Component, Clone, Debug)]
pub struct LuaOnAnimationEnd {
    /// Name of the Lua function to call.
    pub callback: String,
}

impl LuaOnAnimationEnd {
    pub fn new(callback: impl Into<String>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}
