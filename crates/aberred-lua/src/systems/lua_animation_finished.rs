//! Lua observer for animation-finished events.
//!
//! When an [`AnimationFinished`] fires and the entity has a
//! [`LuaOnAnimationFinished`] component, this observer calls the named Lua
//! function with `(ctx, input)` — the same signature as timer and phase
//! callbacks.
//!
//! Entities without [`LuaOnAnimationFinished`] are silently skipped.
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_death_done(ctx, input)
//!     engine.despawn(ctx.id)
//! end
//! ```

use bevy_ecs::prelude::*;

use crate::components::lua_on_animation_finished::LuaOnAnimationFinished;
use crate::systems::lua_commands::{LuaDispatch, dispatch_and_drain};
use aberred_core::events::animation::AnimationFinished;

pub fn lua_animation_finished_observer(
    trigger: On<AnimationFinished>,
    callbacks: Query<&LuaOnAnimationFinished>,
    mut p: LuaDispatch,
) {
    let entity = trigger.event().entity;

    let Ok(callback) = callbacks.get(entity) else {
        return;
    };
    dispatch_and_drain(&mut p, entity, &callback.callback, "on_animation_finished");
}
