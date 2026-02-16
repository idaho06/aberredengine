//! Loaded texture store.
//!
//! A thin wrapper around a hash map that stores `raylib::prelude::Texture2D`
//! objects keyed by string IDs. Insert textures during setup and read them in
//! render systems.
use bevy_ecs::prelude::Resource;
use raylib::prelude::Texture2D;
use rustc_hash::FxHashMap;
// use std::collections::HashMap;

#[derive(Resource)]
/// Map of texture keys to loaded textures.
pub struct TextureStore {
    pub map: FxHashMap<String, Texture2D>,
}

impl Default for TextureStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureStore {
    pub fn new() -> Self {
        TextureStore {
            map: FxHashMap::default(),
        }
    }
    /// Get a texture by its key.
    pub fn get(&self, key: impl AsRef<str>) -> Option<&Texture2D> {
        self.map.get(key.as_ref())
    }
    /// Insert or replace a texture with a specific key.
    pub fn insert(&mut self, key: impl Into<String>, texture: Texture2D) {
        self.map.insert(key.into(), texture);
    }
    /// Remove a texture by its key, returning it if it existed.
    pub fn remove(&mut self, key: impl AsRef<str>) -> Option<Texture2D> {
        self.map.remove(key.as_ref())
    }
}
