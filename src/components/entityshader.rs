//! Per-entity shader component.
//!
//! Allows individual entities (sprites and text) to render with custom shaders.
//! Reuses the existing [`ShaderStore`](crate::resources::shaderstore::ShaderStore)
//! and uniform infrastructure.

use bevy_ecs::prelude::Component;
use rustc_hash::FxHashMap;
use std::sync::Arc;

use crate::resources::lua_runtime::UniformValue;

/// Component that attaches a shader to an individual entity.
///
/// When present, the render system will apply the referenced shader
/// before drawing the entity's sprite or text.
///
/// # Example
/// ```ignore
/// // Apply an "inverse" shader to an entity
/// let shader = EntityShader::new("inverse");
///
/// // With uniforms
/// let mut shader = EntityShader::new("glow");
/// shader.uniforms.insert(Arc::from("uIntensity"), UniformValue::Float(0.8));
/// ```
#[derive(Component, Clone, Debug)]
pub struct EntityShader {
    /// Key referencing a shader in the ShaderStore.
    pub shader_key: Arc<str>,
    /// Per-entity uniform values. These are set on the shader before drawing.
    pub uniforms: FxHashMap<Arc<str>, UniformValue>,
}

impl EntityShader {
    /// Create a new EntityShader with the given shader key and no uniforms.
    pub fn new(key: impl Into<Arc<str>>) -> Self {
        Self {
            shader_key: key.into(),
            uniforms: FxHashMap::default(),
        }
    }
}
