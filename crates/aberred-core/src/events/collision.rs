//! Collision event types.
//!
//! [`BoxOverlap`] is the raw overlap event:
//! [`collision_detector`](crate::systems::collision_detector::collision_detector)
//! triggers it for every overlapping pair, every tick. Observe it for
//! "any overlap" logic.
//!
//! [`CollisionOngoing`] is the rule-matched, every-tick event:
//! [`collision_rule_observer`](crate::systems::collision_rule::collision_rule_observer)
//! triggers it on the [`CollisionRule`](crate::components::collision::CollisionRule)
//! entity whose groups match the pair. Observe it per rule
//! (`commands.spawn(rule).observe(handler)`) or globally (`add_observer(handler)`).
//!
//! [`CollisionStarted`] and [`CollisionEnded`] are the rule-matched contact
//! events: a pair's first touching tick triggers `CollisionStarted` (before
//! that tick's `CollisionOngoing`), and its first tick apart triggers
//! `CollisionEnded`.
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – collision detection system
//! - `aberred_lua::systems::lua_collision` – Lua collision observer
//! - [`crate::components::box_collider::BoxCollider`] – the collider component

use bevy_ecs::prelude::*;

use crate::components::collision::BoxSides;

/// The raw, unruled overlap: triggered for every pair of overlapping
/// `BoxCollider` entities, every tick they overlap. [`CollisionOngoing`] is the
/// rule-matched event.
///
/// The two fields, [`BoxOverlap::a`] and [`BoxOverlap::b`], are the
/// entity IDs of the participants. No ordering guarantees are provided.
/// Additional collision details (normals, penetration, etc.) can be added by
/// extending this type when needed.
#[derive(Event, Debug, Clone, Copy)]
pub struct BoxOverlap {
    pub a: Entity,
    pub b: Entity,
}

/// Triggered on a [`CollisionRule`](crate::components::collision::CollisionRule)
/// entity every tick while entities of its two groups overlap. For one-shot
/// reactions, observe [`CollisionStarted`] (just before it, on a pair's first
/// touching tick).
///
/// `a`/`b` and `sides_a`/`sides_b` follow the rule's `group_a`/`group_b`
/// order, whatever order the pair was detected in.
#[derive(EntityEvent, Clone, Debug)]
pub struct CollisionOngoing {
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

/// Triggered on a [`CollisionRule`](crate::components::collision::CollisionRule)
/// entity when entities of its two groups start touching: on the first tick
/// they overlap, before that tick's [`CollisionOngoing`].
///
/// Fields match [`CollisionOngoing`]. A pair that stops touching and touches
/// again starts again. A scene switch forgets every contact, so a
/// `Persistent` pair still touching after a switch starts again too.
#[derive(EntityEvent, Clone, Debug)]
pub struct CollisionStarted {
    /// The matched rule entity, the event's target.
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

/// Triggered on a [`CollisionRule`](crate::components::collision::CollisionRule)
/// entity on the first tick a started contact no longer overlaps or no
/// longer matches the rule.
///
/// Any of `rule`, `a` and `b` may already be despawned (an entity despawned
/// while touching ends its contact on the next tick), so look them up with
/// `get`/`try_*` and never assume they are alive. A despawned rule's own
/// observers are gone; global observers still see the event. A scene switch
/// forgets contacts without ending them.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct CollisionEnded {
    /// The matched rule entity, the event's target.
    #[event_target]
    pub rule: Entity,
    /// The entity in the rule's `group_a`.
    pub a: Entity,
    /// The entity in the rule's `group_b`.
    pub b: Entity,
}
