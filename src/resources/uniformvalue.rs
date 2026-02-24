//! Shader uniform value types.
//!
//! [`UniformValue`] represents typed values that can be sent to GPU shaders.
//! Used by entity shaders, post-process shaders, and the render system.

/// Value types for shader uniforms.
#[derive(Debug, Clone)]
pub enum UniformValue {
    Float(f32),
    Int(i32),
    Vec2 { x: f32, y: f32 },
    Vec4 { x: f32, y: f32, z: f32, w: f32 },
}
