//! Shader storage resource.
//!
//! Stores loaded shaders keyed by string IDs for use in post-processing effects.
//! Each shader entry caches uniform locations for efficient uniform setting.

use raylib::prelude::Shader;
use rustc_hash::FxHashMap;

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
}

impl ShaderStore {
    /// Creates a new empty shader store.
    pub fn new() -> Self {
        Self {
            shaders: FxHashMap::default(),
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
    #[allow(dead_code)]
    pub fn get(&self, id: &str) -> Option<&ShaderEntry> {
        self.shaders.get(id)
    }

    /// Gets a mutable reference to a shader entry by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ShaderEntry> {
        self.shaders.get_mut(id)
    }

    /// Checks if a shader with the given ID exists.
    #[allow(dead_code)]
    pub fn contains(&self, id: &str) -> bool {
        self.shaders.contains_key(id)
    }
}

impl Default for ShaderStore {
    fn default() -> Self {
        Self::new()
    }
}
