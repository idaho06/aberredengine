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
    pub fn get_scalar(&self, key: impl Into<String>) -> Option<f32> {
        self.scalars.get(&key.into()).copied()
    }
    pub fn get_scalars(&self) -> &FxHashMap<String, f32> {
        &self.scalars
    }
    pub fn set_integer(&mut self, key: impl Into<String>, value: i32) {
        self.integers.insert(key.into(), value);
    }
    pub fn get_integer(&self, key: impl Into<String>) -> Option<i32> {
        self.integers.get(&key.into()).copied()
    }
    pub fn get_integers(&self) -> &FxHashMap<String, i32> {
        &self.integers
    }
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.flags.insert(key.into());
    }
    pub fn clear_flag(&mut self, key: impl Into<String>) {
        self.flags.remove(&key.into());
    }
    pub fn has_flag(&self, key: impl Into<String>) -> bool {
        self.flags.contains(&key.into())
    }
    pub fn get_flags(&self) -> &FxHashSet<String> {
        &self.flags
    }
}
