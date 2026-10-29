//! Shader storage resource.
//!
//! Stores loaded shaders keyed by string IDs for use in post-processing effects.
//! Each shader entry caches uniform locations for efficient uniform setting.

use std::sync::Arc;

use aberred_core::resources::warn_once::first_seen;
use raylib::prelude::Shader;
use rustc_hash::{FxHashMap, FxHashSet};

/// Entry containing a shader and its cached uniform locations.
pub struct ShaderEntry {
    pub shader: Shader,
    /// Cached uniform locations by name. Location -1 means uniform not found.
    pub locations: FxHashMap<String, i32>,
}

/// Non-Send resource storing loaded shaders.
///
/// Shaders are loaded during setup and used during rendering.
/// This is a `NonSend` resource because shaders are tied to the OpenGL context.
pub struct ShaderStore {
    shaders: FxHashMap<String, ShaderEntry>,
    /// Shader keys already reported as unusable by [`ShaderStore::warn_once`].
    warned: FxHashSet<Arc<str>>,
}

impl ShaderStore {
    /// Creates a new empty shader store.
    pub fn new() -> Self {
        Self {
            shaders: FxHashMap::default(),
            warned: FxHashSet::default(),
        }
    }

    /// Adds a shader to the store with the given ID.
    ///
    /// If a shader with the same ID already exists, it will be replaced.
    pub fn add(&mut self, id: &str, shader: Shader) {
        self.shaders.insert(
            id.to_string(),
            ShaderEntry {
                shader,
                locations: FxHashMap::default(),
            },
        );
    }

    /// Gets an immutable reference to a shader entry by ID.
    pub fn get(&self, id: &str) -> Option<&ShaderEntry> {
        self.shaders.get(id)
    }

    /// Gets a mutable reference to a shader entry by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ShaderEntry> {
        self.shaders.get_mut(id)
    }

    /// Returns `true` the first time `id` is reported unusable (missing or
    /// invalid), `false` on every later call, so a bad shader used by many
    /// entities logs once rather than per entity per frame.
    pub fn warn_once(&mut self, id: &str) -> bool {
        first_seen(&mut self.warned, id)
    }

    /// Checks if a shader with the given ID exists.
    pub fn contains(&self, id: &str) -> bool {
        self.shaders.contains_key(id)
    }

    /// Returns the number of loaded shaders.
    pub fn len(&self) -> usize {
        self.shaders.len()
    }

    /// Returns `true` if no shaders are loaded.
    pub fn is_empty(&self) -> bool {
        self.shaders.is_empty()
    }
}

impl Default for ShaderStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warn_once_is_true_only_the_first_time_for_a_shader_key() {
        let mut store = ShaderStore::new();
        assert!(store.warn_once("glow"));
        assert!(!store.warn_once("glow"));
        assert!(!store.warn_once("glow"));
    }

    #[test]
    fn warn_once_tracks_each_shader_key_separately() {
        let mut store = ShaderStore::new();
        assert!(store.warn_once("glow"));
        assert!(store.warn_once("outline"));
        assert!(!store.warn_once("glow"));
        assert!(!store.warn_once("outline"));
    }
}
