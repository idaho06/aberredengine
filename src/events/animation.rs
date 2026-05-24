//! Animation lifecycle events.
//!
//! [`AnimationFinishedEvent`] is triggered once by the [`animation`](crate::systems::animation)
//! system on the frame a non-looped animation first reaches its last frame.
//!
//! Rust consumers can observe it via [`EngineBuilder::add_observer`].
//! Lua consumers attach a [`LuaOnAnimationEnd`](crate::components::lua_on_animation_end::LuaOnAnimationEnd)
//! component to the entity (feature = "lua").

use bevy_ecs::prelude::*;

/// Triggered once when a non-looped animation first reaches its final frame.
///
/// The event is **not** re-triggered on subsequent frames even though the entity
/// stays on the last frame. Looped animations never trigger this event.
#[derive(Event, Debug, Clone, Copy)]
pub struct AnimationFinishedEvent {
    /// The entity whose animation finished.
    pub entity: Entity,
}
