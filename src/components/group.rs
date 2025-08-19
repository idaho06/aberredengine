use core::str;

use bevy_ecs::prelude::Component;

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
