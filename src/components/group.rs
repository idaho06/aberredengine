//! Group tag component for entity categorization.
//!
//! The [`Group`] component allows labeling entities with a string name.
//! This is useful for filtering queries, collision rules, and broadcasting
//! actions to a set of entities that share a common semantic group.

use core::str;

use bevy_ecs::prelude::Component;

/// Tag component used to group entities under a named label.
///
/// Useful for filtering queries or broadcasting actions to a set of entities
/// that share a common semantic group.
#[derive(Component, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
///
/// Useful for filtering queries or broadcasting actions to a set of entities
/// that share a common semantic group.
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
