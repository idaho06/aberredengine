use bevy_ecs::prelude::Resource;
use raylib::prelude::Texture2D;
use rustc_hash::FxHashMap;
// use std::collections::HashMap;

#[derive(Resource)]
pub struct TextureStore {
    pub map: FxHashMap<String, Texture2D>,
}

impl TextureStore {
    /// Get a texture by its key.
    pub fn get(&self, key: impl Into<String>) -> Option<&Texture2D> {
        self.map.get(&key.into())
    }
    /// Insert a texture with a specific key.
    pub fn insert(&mut self, key: impl Into<String>, texture: Texture2D) {
        self.map.insert(key.into(), texture);
    }
}
