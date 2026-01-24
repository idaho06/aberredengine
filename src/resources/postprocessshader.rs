//! Post-process shader selection resource.
//!
//! Controls which shader (if any) is applied during the final blit from
//! render target to window. User uniforms can be set from Lua and will be
//! applied alongside the standard uniforms.

use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;
use std::sync::Arc;

use crate::resources::lua_runtime::UniformValue;

/// Reserved uniform names that are set automatically by the render system.
/// Attempting to set these from Lua will log a warning.
pub const RESERVED_UNIFORMS: &[&str] = &[
    "uTime",
    "uDeltaTime",
    "uResolution",
    "uFrame",
    "uWindowResolution",
    "uLetterbox",
];

/// Resource controlling post-process shader selection and uniforms.
///
/// When `keys` is non-empty, the render system will apply the named shaders
/// in sequence during the final blit. When empty, no post-processing is applied.
#[derive(Resource, Default)]
pub struct PostProcessShader {
    /// Ordered list of shader keys to apply (empty = no post-processing).
    pub keys: Vec<Arc<str>>,
    /// User-defined uniforms to pass to all shaders in the chain.
    pub uniforms: FxHashMap<Arc<str>, UniformValue>,
}

impl PostProcessShader {
    /// Creates a new post-process shader resource with no shader selected.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the active post-process shader chain.
    ///
    /// Pass `None` or empty vec to disable post-processing.
    pub fn set_shader_chain(&mut self, keys: Option<Vec<String>>) {
        self.keys = keys
            .unwrap_or_default()
            .into_iter()
            .map(Arc::from)
            .collect();
    }

    /// Returns true if post-processing is enabled (at least one shader in chain).
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        !self.keys.is_empty()
    }

    /// Sets a user uniform value.
    ///
    /// Returns `true` if the name is reserved (value is still stored but will
    /// be overwritten by the render system).
    pub fn set_uniform(&mut self, name: &str, value: UniformValue) -> bool {
        let is_reserved = RESERVED_UNIFORMS.contains(&name);
        self.uniforms.insert(Arc::from(name), value);
        is_reserved
    }

    /// Clears a single user uniform by name.
    pub fn clear_uniform(&mut self, name: &str) {
        self.uniforms.remove(name);
    }

    /// Clears all user uniforms.
    pub fn clear_uniforms(&mut self) {
        self.uniforms.clear();
    }
}
