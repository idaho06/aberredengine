//! Global signal storage resource.
//!
//! The [`WorldSignals`] resource provides a world-wide signal map for
//! cross-system communication. Unlike per-entity
//! [`Signals`](crate::components::signals::Signals), these signals are
//! global and accessible from any system.
//!
//! Use cases include:
//! - Storing the current scene name
//! - Global flags like "game_paused" or "player_dead"
//! - Passing data between unrelated systems

use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::{FxHashMap, FxHashSet};

/// Global signal storage for cross-system communication.
///
/// Provides maps for scalars, integers, strings, and flags accessible from
/// any system without entity queries.
#[derive(Debug, Clone, Resource)]
pub struct WorldSignals {
    /// Floating-point numeric signals addressed by string keys.
    pub scalars: FxHashMap<String, f32>,
    /// Integer numeric signals addressed by string keys.
    pub integers: FxHashMap<String, i32>,
    /// String signals addressed by string keys.
    pub strings: FxHashMap<String, String>,
    /// Presence-only boolean flags; a key being present means "true".
    pub flags: FxHashSet<String>,
    /// Map of entities of interest for the current game state.
    pub entities: FxHashMap<String, Entity>,
}
impl Default for WorldSignals {
    fn default() -> Self {
        Self {
            scalars: FxHashMap::default(),
            integers: FxHashMap::default(),
            strings: FxHashMap::default(),
            flags: FxHashSet::default(),
            entities: FxHashMap::default(),
        }
    }
}
impl WorldSignals {
    /// Set a floating-point signal value.
    pub fn set_scalar(&mut self, key: impl Into<String>, value: f32) {
        self.scalars.insert(key.into(), value);
    }
    /// Get a floating-point signal by key.
    pub fn get_scalar(&self, key: &str) -> Option<f32> {
        self.scalars.get(key).copied()
    }
    /// Read-only view of all scalar signals.
    pub fn get_scalars(&self) -> &FxHashMap<String, f32> {
        &self.scalars
    }
    /// Set an integer signal value.
    pub fn set_integer(&mut self, key: impl Into<String>, value: i32) {
        self.integers.insert(key.into(), value);
    }
    /// Get an integer signal by key.
    pub fn get_integer(&self, key: &str) -> Option<i32> {
        self.integers.get(key).copied()
    }
    /// Set a string signal value.
    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.strings.insert(key.into(), value.into());
    }
    /// Get a string signal by key.
    /// It's recommended to clone the String if you need ownership.
    pub fn get_string(&self, key: &str) -> Option<&String> {
        self.strings.get(key)
    }
    /// Remove a string signal by key.
    pub fn remove_string(&mut self, key: &str) -> Option<String> {
        self.strings.remove(key)
    }
    /// Read-only view of all integer signals.
    pub fn get_integers(&self) -> &FxHashMap<String, i32> {
        &self.integers
    }
    /// Mark a flag as present/true.
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.flags.insert(key.into());
    }
    /// Remove a flag (make it false/absent).
    pub fn clear_flag(&mut self, key: &str) {
        self.flags.remove(key);
    }
    /// Check whether a flag is present/true.
    pub fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
    /// Read-only view of all flags.
    pub fn get_flags(&self) -> &FxHashSet<String> {
        &self.flags
    }
    /// Get an entity by key.
    pub fn get_entity(&self, key: &str) -> Option<&Entity> {
        self.entities.get(key)
    }
    /// Set an entity by key.
    pub fn set_entity(&mut self, key: impl Into<String>, entity: Entity) {
        self.entities.insert(key.into(), entity);
    }
    /// Remove an entity by key. Returns the removed entity if it existed.
    pub fn remove_entity(&mut self, key: &str) -> Option<Entity> {
        self.entities.remove(key)
    }
}
