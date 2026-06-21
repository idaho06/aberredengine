//! Lua callback component fired when a `Tween<T>` finishes.
//!
//! Attach this component (one per tweened type `T`) to any entity whose
//! `Tween<T>` should invoke a Lua function when it stops playing after
//! reaching its end — see [`TweenFinishedEvent`](crate::events::tween::TweenFinishedEvent).
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_window_hidden(ctx, input)
//!     engine.entity_remove_screen_position(ctx.id)
//! end
//! ```
//!
//! The callback fires **exactly once** per tween completion — never for
//! `LoopMode::Loop`/`LoopMode::PingPong` tweens, which never stop playing.
//!
//! # Usage from Lua
//!
//! ```lua
//! engine.spawn()
//!     :with_tween_screen_position(0, 400, 0, 100, 1.0)
//!     :with_tween_screen_position_on_finished("on_window_shown")
//!     :build()
//! ```

use std::marker::PhantomData;

use bevy_ecs::prelude::Component;

use crate::components::tween::TweenValue;

/// Attaches a Lua callback to be called when the entity's `Tween<T>` finishes.
#[derive(Component, Clone, Debug)]
pub struct LuaOnTweenFinished<T: TweenValue> {
    /// Name of the Lua function to call.
    pub callback: String,
    _marker: PhantomData<T>,
}

impl<T: TweenValue> LuaOnTweenFinished<T> {
    pub fn new(callback: impl Into<String>) -> Self {
        Self {
            callback: callback.into(),
            _marker: PhantomData,
        }
    }
}
