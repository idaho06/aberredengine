//! Lua observer for animation-finished events.
//!
//! When an [`AnimationFinishedEvent`] fires and the entity has a
//! [`LuaOnAnimationEnd`] component, this observer calls the named Lua function
//! with `(ctx, input)` — the same signature as timer and phase callbacks.
//!
//! Entities without [`LuaOnAnimationEnd`] are silently skipped.
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_death_done(ctx, input)
//!     engine.despawn(ctx.id)
//! end
//! ```

use bevy_ecs::prelude::*;

use crate::components::lua_on_animation_end::LuaOnAnimationEnd;
use crate::events::animation::AnimationFinishedEvent;
use crate::systems::lua_commands::{LuaDispatch, dispatch_and_drain};

pub fn lua_animation_finished_observer(
    trigger: On<AnimationFinishedEvent>,
    on_end_query: Query<&LuaOnAnimationEnd>,
    mut p: LuaDispatch,
) {
    let entity = trigger.event().entity;

    let Ok(callback) = on_end_query.get(entity) else {
        return;
    };
    dispatch_and_drain(&mut p, entity, &callback.callback, "on_animation_end");
}
