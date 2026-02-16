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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_from_str() {
        let shader = EntityShader::new("glow");
        assert_eq!(&*shader.shader_key, "glow");
        assert!(shader.uniforms.is_empty());
    }

    #[test]
    fn test_new_from_arc_str() {
        let key: Arc<str> = Arc::from("bloom");
        let shader = EntityShader::new(key);
        assert_eq!(&*shader.shader_key, "bloom");
    }

    #[test]
    fn test_uniform_insert_float() {
        let mut shader = EntityShader::new("test");
        shader
            .uniforms
            .insert(Arc::from("uIntensity"), UniformValue::Float(0.5));
        assert_eq!(shader.uniforms.len(), 1);
        assert!(matches!(
            shader.uniforms.get(&Arc::from("uIntensity") as &Arc<str>),
            Some(UniformValue::Float(v)) if (*v - 0.5).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn test_uniform_insert_vec2() {
        let mut shader = EntityShader::new("test");
        shader
            .uniforms
            .insert(Arc::from("uOffset"), UniformValue::Vec2 { x: 1.0, y: 2.0 });
        assert!(matches!(
            shader.uniforms.get(&Arc::from("uOffset") as &Arc<str>),
            Some(UniformValue::Vec2 { x, y }) if (*x - 1.0).abs() < f32::EPSILON && (*y - 2.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn test_uniform_insert_vec4() {
        let mut shader = EntityShader::new("test");
        shader.uniforms.insert(
            Arc::from("uColor"),
            UniformValue::Vec4 {
                x: 1.0,
                y: 0.5,
                z: 0.0,
                w: 1.0,
            },
        );
        assert_eq!(shader.uniforms.len(), 1);
    }

    #[test]
    fn test_uniform_insert_int() {
        let mut shader = EntityShader::new("test");
        shader
            .uniforms
            .insert(Arc::from("uMode"), UniformValue::Int(3));
        assert!(matches!(
            shader.uniforms.get(&Arc::from("uMode") as &Arc<str>),
            Some(UniformValue::Int(3))
        ));
    }

    #[test]
    fn test_multiple_uniforms() {
        let mut shader = EntityShader::new("complex");
        shader
            .uniforms
            .insert(Arc::from("a"), UniformValue::Float(1.0));
        shader
            .uniforms
            .insert(Arc::from("b"), UniformValue::Int(2));
        shader
            .uniforms
            .insert(Arc::from("c"), UniformValue::Vec2 { x: 0.0, y: 0.0 });
        assert_eq!(shader.uniforms.len(), 3);
    }
}
