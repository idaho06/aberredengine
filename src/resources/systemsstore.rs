//! Registry for dynamically addressable systems.
//!
//! Allows systems to be registered under string keys and looked up later to
//! run via their [`bevy_ecs::system::SystemId`]. This is useful for invoking
//! state-specific setup/teardown hooks without tight coupling.

use bevy_ecs::prelude::{Entity, In, Resource};
use bevy_ecs::system::SystemId;
use rustc_hash::FxHashMap;

/// Map of string names to system IDs.
#[derive(Resource)]
pub struct SystemsStore {
    /// Systems that take no input.
    pub map: FxHashMap<String, SystemId>,
    /// Systems that take an Entity as input (via `In<Entity>`).
    pub entity_map: FxHashMap<String, SystemId<In<Entity>>>,
}

impl Default for SystemsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemsStore {
    /// Create an empty store.
    pub fn new() -> Self {
        SystemsStore {
            map: FxHashMap::default(),
            entity_map: FxHashMap::default(),
        }
    }

    /// Insert a system ID under a human-readable name.
    pub fn insert(&mut self, name: impl Into<String>, id: SystemId) {
        self.map.insert(name.into(), id);
    }

    /// Retrieve a system ID by name, if present.
    pub fn get(&self, name: impl AsRef<str>) -> Option<&SystemId> {
        self.map.get(name.as_ref())
    }

    /// Insert a system ID that takes an Entity as input.
    pub fn insert_entity_system(&mut self, name: impl Into<String>, id: SystemId<In<Entity>>) {
        self.entity_map.insert(name.into(), id);
    }

    /// Retrieve a system ID that takes an Entity as input.
    pub fn get_entity_system(&self, name: impl AsRef<str>) -> Option<&SystemId<In<Entity>>> {
        self.entity_map.get(name.as_ref())
    }
}
