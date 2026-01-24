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
/// When `key` is `Some`, the render system will apply the named shader
/// during the final blit. When `None`, no post-processing is applied.
#[derive(Resource, Default)]
pub struct PostProcessShader {
    /// The key of the currently active shader, or None for no post-processing.
    pub key: Option<Arc<str>>,
    /// User-defined uniforms to pass to the shader.
    pub uniforms: FxHashMap<Arc<str>, UniformValue>,
}

impl PostProcessShader {
    /// Creates a new post-process shader resource with no shader selected.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the active post-process shader by key.
    ///
    /// Pass `None` to disable post-processing.
    pub fn set_shader(&mut self, key: Option<&str>) {
        self.key = key.map(Arc::from);
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
