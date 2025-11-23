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
    pub rigidbodies: &'a mut Query<'w, 's, &'static mut RigidBody>,
    // TODO: Add more parameters as needed. They come from the `collision_observer` function,
    // pub signals: &'w mut Query<'w, 's, &'static mut Signal>,
}

/// Callback signature for collision components using a grouped context.
pub type CollisionCallback =
    for<'a, 'w, 's> fn(a: Entity, b: Entity, ctx: &mut CollisionContext<'a, 'w, 's>);

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
