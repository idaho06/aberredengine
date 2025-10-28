use bevy_ecs::prelude::Resource;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone, Resource)]
pub struct WorldSignals {
    /// Floating-point numeric signals addressed by string keys.
    pub scalars: FxHashMap<String, f32>,
    /// Integer numeric signals addressed by string keys.
    pub integers: FxHashMap<String, i32>,
    /// Presence-only boolean flags; a key being present means "true".
    pub flags: FxHashSet<String>,
}
impl Default for WorldSignals {
    fn default() -> Self {
        Self {
            scalars: FxHashMap::default(),
            integers: FxHashMap::default(),
            flags: FxHashSet::default(),
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
}
