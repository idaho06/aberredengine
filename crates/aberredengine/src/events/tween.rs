//! Tween lifecycle events.
//!
//! [`TweenFinishedEvent`] is triggered once by [`tween_system`](crate::systems::tween::tween_system)
//! on the frame a `Tween<T>` stops playing (a `LoopMode::Once` tween reaching
//! its end, or the zero-duration snap-to-end case). `Loop`/`PingPong` tweens
//! never trigger it, since they never stop playing.
//!
//! Rust consumers can observe it via `EngineBuilder::add_observer`.
//! Lua consumers attach a [`LuaOnTweenFinished`](crate::components::lua_on_tween_finished::LuaOnTweenFinished)
//! component to the entity (feature = "lua").

use std::marker::PhantomData;

use bevy_ecs::prelude::*;

use crate::components::tween::TweenValue;

/// Triggered once when a `Tween<T>` stops playing after reaching its end.
///
/// Not re-triggered on subsequent frames while the tween stays stopped.
/// `LoopMode::Loop`/`LoopMode::PingPong` tweens never trigger this event.
#[derive(Event, Debug, Clone, Copy)]
pub struct TweenFinishedEvent<T: TweenValue> {
    /// The entity whose tween finished.
    pub entity: Entity,
    _marker: PhantomData<T>,
}

impl<T: TweenValue> TweenFinishedEvent<T> {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            _marker: PhantomData,
        }
    }
}
