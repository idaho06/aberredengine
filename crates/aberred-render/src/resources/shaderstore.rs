//! Shader storage resource.
//!
//! Stores loaded shaders keyed by string IDs for use in post-processing effects.
//! Each shader entry caches uniform locations for efficient uniform setting.

use aberred_core::components::entity_shader::EntityShader;
use log::warn;
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
    pub fn get(&self, id: &str) -> Option<&ShaderEntry> {
        self.shaders.get(id)
    }

    /// Gets a mutable reference to a shader entry by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ShaderEntry> {
        self.shaders.get_mut(id)
    }

    /// The entry an entity's shader draws through, or `None` when it must draw
    /// without one. A shader that is missing from the store or fails
    /// validation logs a warning and yields `None`.
    pub fn entity_shader_mut(&mut self, entity_shader: &EntityShader) -> Option<&mut ShaderEntry> {
        let Some(entry) = self.shaders.get_mut(&*entity_shader.shader_key) else {
            warn!(
                "Entity shader '{}' not found, rendering without shader",
                entity_shader.shader_key
            );
            return None;
        };
        if !entry.shader.is_shader_valid() {
            warn!(
                "Entity shader '{}' is invalid, rendering without shader",
                entity_shader.shader_key
            );
            return None;
        }
        Some(entry)
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
    use aberred_core::components::entity_shader::EntityShader;

    /// A shader key that was never loaded yields no entry, so the caller draws
    /// without a shader.
    #[test]
    fn an_entity_shader_naming_an_unloaded_shader_has_no_active_entry() {
        let mut store = ShaderStore::new();
        assert!(
            store
                .entity_shader_mut(&EntityShader::new("never-loaded"))
                .is_none()
        );
    }
}
