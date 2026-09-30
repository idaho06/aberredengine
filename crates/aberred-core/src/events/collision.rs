//! Collision event types.
//!
//! The collision system emits [`CollisionEvent`] whenever two entities with
//! compatible colliders overlap. This event is primarily consumed by the
//! `lua_collision_observer` (`aberred_lua::systems::lua_collision`), which
//! looks up matching `LuaCollisionRule`
//! components and invokes their Lua callbacks.
//!
//! # Flow
//!
//! 1. [`collision_detector`](crate::systems::collision_detector::collision_detector) detects overlaps
//! 2. Emits `CollisionEvent` for each collision
//! 3. `lua_collision_observer` (`aberred_lua::systems::lua_collision`) receives the event
//! 4. Finds matching `LuaCollisionRule` by group names
//! 5. Invokes the rule's Lua callback with both entities
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – collision detection system
//! - `aberred_lua::systems::lua_collision` – Lua collision observer
//! - `aberred_lua::components::luacollision::LuaCollisionRule` – defines Lua collision handlers
//! - [`crate::components::boxcollider::BoxCollider`] – the collider component

use bevy_ecs::prelude::*;

/// Event fired when two entities with BoxCollider overlap.
///
/// The two fields, [`CollisionEvent::a`] and [`CollisionEvent::b`], are the
/// entity IDs of the participants. No ordering guarantees are provided.
/// Additional collision details (normals, penetration, etc.) can be added by
/// extending this type when needed.
#[derive(Event, Debug, Clone, Copy)]
pub struct CollisionEvent {
    pub a: Entity,
    pub b: Entity,
}
