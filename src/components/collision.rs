use bevy_ecs::prelude::*;

use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::events::collision::CollisionEvent;

/// Callback signature for collision components.
pub type CollisionCallback = fn(
    a: Entity,
    b: Entity,
    commands: &mut Commands,
    groups: &Query<&Group>,
    positions: &mut Query<&mut MapPosition>,
    rigidbodies: &mut Query<&mut RigidBody>,
);

#[derive(Component)]
pub struct CollisionRule {
    pub group_a: String,
    pub group_b: String,
    pub callback: CollisionCallback,
}
// TODO: Instead of using a fixed function signature for the callback,
// use a closure that can capture additional context if needed.

impl CollisionRule {
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
    pub fn matches(&self, group_a: &str, group_b: &str) -> bool {
        (self.group_a == group_a && self.group_b == group_b)
            || (self.group_a == group_b && self.group_b == group_a)
    }
}
