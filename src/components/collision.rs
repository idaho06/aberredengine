//! Collision rule component and callback context.
//!
//! This module provides the [`CollisionRule`] component which defines how
//! collisions between entity groups should be handled, and [`CollisionContext`]
//! which provides access to world state within collision callbacks.
//!
//! See [`crate::systems::collision`] for the collision detection system.

use bevy_ecs::prelude::*;

use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::events::collision::CollisionEvent;

/// Context passed into collision callbacks to access world state.
///
/// Extend this struct as new queries/resources are needed by callbacks.
pub struct CollisionContext<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub groups: &'a Query<'w, 's, &'static Group>,
    pub positions: &'a mut Query<'w, 's, &'static mut MapPosition>,
    pub rigid_bodies: &'a mut Query<'w, 's, &'static mut RigidBody>,
    // TODO: Add more parameters as needed. They come from the `collision_observer` function,
    // pub signals: &'w mut Query<'w, 's, &'static mut Signal>,
}

/// Callback signature for collision components using a grouped context.
pub type CollisionCallback =
    for<'a, 'w, 's> fn(a: Entity, b: Entity, ctx: &mut CollisionContext<'a, 'w, 's>);

/// Defines how collisions between two entity groups should be handled.
///
/// When a collision is detected between entities with groups matching
/// `group_a` and `group_b`, the `callback` function is invoked with
/// the entities and a [`CollisionContext`].
#[derive(Component)]
pub struct CollisionRule {
    pub group_a: String,
    pub group_b: String,
    pub callback: CollisionCallback,
}
// TODO: Instead of using a fixed function signature for the callback,
// use a closure that can capture additional context if needed.

impl CollisionRule {
    /// Create a new collision rule for two groups with a callback.
    pub fn new(
        group_a: impl Into<String>,
        group_b: impl Into<String>,
        callback: CollisionCallback,
    ) -> Self {
        Self {
            group_a: group_a.into(),
            group_b: group_b.into(),
            callback,
        }
    }

    /// Check if this rule matches the given groups and return entities in order.
    ///
    /// Returns `Some((entity_a, entity_b))` if the rule matches, with entities
    /// ordered to match `group_a` and `group_b` respectively.
    pub fn match_and_order(
        &self,
        ent_a: Entity,
        ent_b: Entity,
        group_a: &str,
        group_b: &str,
    ) -> Option<(Entity, Entity)> {
        if self.group_a == group_a && self.group_b == group_b {
            Some((ent_a, ent_b))
        } else if self.group_a == group_b && self.group_b == group_a {
            Some((ent_b, ent_a))
        } else {
            None
        }
    }
}
