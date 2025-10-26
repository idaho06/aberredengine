// Signals for communication between components

use bevy_ecs::prelude::Component;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone, Component)]
pub struct Signals {
    pub scalars: FxHashMap<String, f32>,
    pub integers: FxHashMap<String, i32>,
    pub flags: FxHashSet<String>,
}

impl Default for Signals {
    fn default() -> Self {
        Self {
            scalars: FxHashMap::default(),
            integers: FxHashMap::default(),
            flags: FxHashSet::default(),
        }
    }
}

impl Signals {
    pub fn set_scalar(&mut self, key: impl Into<String>, value: f32) {
        self.scalars.insert(key.into(), value);
    }
    pub fn get_scalar(&self, key: &str) -> Option<f32> {
        self.scalars.get(key).copied()
    }
    pub fn get_scalars(&self) -> &FxHashMap<String, f32> {
        &self.scalars
    }
    pub fn set_integer(&mut self, key: impl Into<String>, value: i32) {
        self.integers.insert(key.into(), value);
    }
    pub fn get_integer(&self, key: &str) -> Option<i32> {
        self.integers.get(key).copied()
    }
    pub fn get_integers(&self) -> &FxHashMap<String, i32> {
        &self.integers
    }
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.flags.insert(key.into());
    }
    pub fn clear_flag(&mut self, key: &str) {
        self.flags.remove(key);
    }
    pub fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
    pub fn get_flags(&self) -> &FxHashSet<String> {
        &self.flags
    }
}
