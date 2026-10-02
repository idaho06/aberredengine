//! 2D sprite rendering component.
//!
//! A [`Sprite`] references a texture by key and describes how to sample and
//! place it in world space. For spritesheets, set an `offset` to select the
//! frame. `origin` defines the pivot (in pixels, from the texture's top-left)
//! used when positioning/rotating/scaling the sprite.

use std::sync::Arc;

use crate::math::Vec2;
use bevy_ecs::prelude::Component;

#[derive(Component, Clone, Debug, PartialEq)]
/// Describes how to render a textured quad for an entity.
pub struct Sprite {
    /// Texture identifier used to look up the GPU resource.
    pub tex_key: Arc<str>,
    /// Width in world units.
    pub width: f32,
    /// Height in world units.
    pub height: f32,
    /// Pixel offset into the texture (e.g. frame origin in a spritesheet).
    pub offset: Vec2,
    /// Pixel pivot relative to the texture's top-left for transforms.
    pub origin: Vec2,
    /// Flip horizontally at render time.
    pub flip_h: bool,
    /// Flip vertically at render time.
    pub flip_v: bool,
}

impl Sprite {
    /// A `width` × `height` sprite of texture `tex_key`, with the origin at the top-left,
    /// no texture offset and no flipping.
    pub fn new(tex_key: impl Into<Arc<str>>, width: f32, height: f32) -> Self {
        Self {
            tex_key: tex_key.into(),
            width,
            height,
            offset: Vec2::ZERO,
            origin: Vec2::ZERO,
            flip_h: false,
            flip_v: false,
        }
    }

    /// Sets the pivot, in pixels from the texture's top-left.
    pub fn with_origin(mut self, origin: Vec2) -> Self {
        self.origin = origin;
        self
    }

    /// Puts the pivot at the sprite's center (half its current width and height).
    pub fn centered(self) -> Self {
        let center = Vec2::new(self.width, self.height) * 0.5;
        self.with_origin(center)
    }

    /// Sets the pixel offset into the texture (e.g. a spritesheet frame).
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// Sets horizontal and vertical flipping.
    pub fn with_flip(mut self, flip_h: bool, flip_v: bool) -> Self {
        self.flip_h = flip_h;
        self.flip_v = flip_v;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_top_left_until_centered() {
        let s = Sprite::new("player", 32.0, 16.0);
        assert_eq!(s.origin, Vec2::ZERO);
        assert_eq!(s.centered().origin, Vec2::new(16.0, 8.0));
    }
}
