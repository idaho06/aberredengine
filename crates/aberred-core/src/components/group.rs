//! Group tag component for entity categorization.
//!
//! The [`Group`] component allows labeling entities with a string name.
//! This is useful for filtering queries, collision rules, and broadcasting
//! actions to a set of entities that share a common semantic group.
//!
//! # Use Cases
//!
//! - **Collision rules**: Match collisions between groups (e.g., "ball" vs "brick")
//! - **Entity counting**: Track group populations via [`TrackedGroups`](crate::resources::group::TrackedGroups)
//! - **Bulk operations**: Despawn all entities in a group, apply effects, etc.
//!
//! # Example
//!
//! ```ignore
//! commands.spawn((
//!     Group::new("player"),
//!     MapPosition::new(400.0, 700.0),
//!     Sprite { /* ... */ },
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::components::collision::CollisionRule`] – uses group names for matching
//! - [`crate::resources::group::TrackedGroups`] – tracks group entity counts
//! - [`crate::systems::group::update_group_counts_system`] – publishes counts to WorldSignals

use core::str;

use bevy_ecs::prelude::Component;

/// Tag component used to group entities under a named label.
///
/// Useful for filtering queries or broadcasting actions to a set of entities
/// that share a common semantic group.
#[derive(Component, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Group(pub String);

impl Group {
    /// Create a new group with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Group(name.into())
    }

    /// Get the name of the group.
    pub fn name(&self) -> &str {
        &self.0
    }
}
