//! Collision event types and a simple observer.
//!
//! The collision system emits [`CollisionEvent`] whenever two entities with
//! compatible colliders overlap. Observers can subscribe to this event to
//! react in a decoupled manner (damage, sound, despawn, etc.).

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
