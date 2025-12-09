//! Collision event types.
//!
//! The collision system emits [`CollisionEvent`] whenever two entities with
//! compatible colliders overlap. This event is primarily consumed by the
//! [`collision_observer`](crate::systems::collision::collision_observer), which
//! looks up matching [`CollisionRule`](crate::components::collision::CollisionRule)
//! components and invokes their callbacks.
//!
//! # Flow
//!
//! 1. [`collision_detector`](crate::systems::collision::collision_detector) detects overlaps
//! 2. Emits `CollisionEvent` for each collision
//! 3. [`collision_observer`](crate::systems::collision::collision_observer) receives the event
//! 4. Finds matching `CollisionRule` by group names
//! 5. Invokes the rule's callback with both entities
//!
//! # Related
//!
//! - [`crate::systems::collision`] – detection and observer systems
//! - [`crate::components::collision::CollisionRule`] – defines collision handlers
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
