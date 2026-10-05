//! Collision event types.
//!
//! [`Overlapping`] is the raw overlap event:
//! [`collision_detector`](crate::systems::collision_detector::collision_detector)
//! triggers it for every overlapping pair, every tick. Observe it for
//! "any overlap" logic.
//!
//! [`Collided`] is the rule-matched event:
//! [`collision_rule_observer`](crate::systems::collision_rule::collision_rule_observer)
//! triggers it on the [`CollisionRule`](crate::components::collision::CollisionRule)
//! entity whose groups match the pair. Observe it per rule
//! (`commands.spawn(rule).observe(handler)`) or globally (`add_observer(handler)`).
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – collision detection system
//! - `aberred_lua::systems::lua_collision` – Lua collision observer
//! - [`crate::components::boxcollider::BoxCollider`] – the collider component

use bevy_ecs::prelude::*;

use crate::components::collision::BoxSides;

/// The raw, unruled overlap: triggered for every pair of overlapping
/// `BoxCollider` entities, every tick they overlap. [`Collided`] is the
/// rule-matched event.
///
/// The two fields, [`Overlapping::a`] and [`Overlapping::b`], are the
/// entity IDs of the participants. No ordering guarantees are provided.
/// Additional collision details (normals, penetration, etc.) can be added by
/// extending this type when needed.
#[derive(Event, Debug, Clone, Copy)]
pub struct Overlapping {
    pub a: Entity,
    pub b: Entity,
}

/// Triggered on a [`CollisionRule`](crate::components::collision::CollisionRule)
/// entity when entities of its two groups overlap, every tick while they do.
///
/// `a`/`b` and `sides_a`/`sides_b` follow the rule's `group_a`/`group_b`
/// order, whatever order the pair was detected in.
#[derive(EntityEvent, Clone, Debug)]
pub struct Collided {
    /// The matched rule entity, the event's target. Named `rule` rather than
    /// `entity` because a collision involves three entities.
    #[event_target]
    pub rule: Entity,
    /// The entity in the rule's `group_a`.
    pub a: Entity,
    /// The entity in the rule's `group_b`.
    pub b: Entity,
    /// Sides of `a`'s collider that touch `b`.
    pub sides_a: BoxSides,
    /// Sides of `b`'s collider that touch `a`.
    pub sides_b: BoxSides,
}
