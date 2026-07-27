//! Lua observer for tween-finished events.
//!
//! When a [`TweenFinishedEvent<T>`] fires and the entity has a matching
//! [`LuaOnTweenFinished<T>`] component, this observer calls the named Lua
//! function with `(ctx, input)` — the same signature as timer, phase, and
//! `on_animation_end` callbacks.
//!
//! Entities without `LuaOnTweenFinished<T>` are silently skipped. Register
//! one monomorphized instance of this observer per tweened type
//! (`MapPosition`, `Rotation`, `Scale`, `ScreenPosition`).
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_window_hidden(ctx, input)
//!     engine.entity_remove_screen_position(ctx.id)
//! end
//! ```

use bevy_ecs::prelude::*;

use crate::components::lua_on_tween_finished::LuaOnTweenFinished;
use crate::components::tween::TweenValue;
use crate::events::tween::TweenFinishedEvent;
use crate::systems::lua_commands::{LuaDispatch, dispatch_and_drain};

pub fn lua_tween_finished_observer<T: TweenValue>(
    trigger: On<TweenFinishedEvent<T>>,
    on_finished_query: Query<&LuaOnTweenFinished<T>>,
    mut p: LuaDispatch,
) {
    let entity = trigger.event().entity;

    let Ok(callback) = on_finished_query.get(entity) else {
        return;
    };
    let callback_name = callback.callback.clone();

    dispatch_and_drain(&mut p, entity, &callback_name, "on_tween_finished");
}
