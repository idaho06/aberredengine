//! 2D sprite rendering component.
//!
//! A [`Sprite`] references a texture by key and describes how to sample and
//! place it in world space. For spritesheets, set an `offset` to select the
//! frame. `origin` defines the pivot (in pixels, from the texture's top-left)
//! used when positioning/rotating/scaling the sprite.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

#[derive(Component, Clone, Debug)]
/// Describes how to render a textured quad for an entity.
pub struct Sprite {
    /// Texture identifier used to look up the GPU resource.
    pub tex_key: String,
    /// Width in world units.
    pub width: f32,
    /// Height in world units.
    pub height: f32,
    /// Pixel offset into the texture (e.g. frame origin in a spritesheet).
    pub offset: Vector2,
    /// Pixel pivot relative to the texture's top-left for transforms.
    pub origin: Vector2,
    /// Flip horizontally at render time.
    pub flip_h: bool,
    /// Flip vertically at render time.
    pub flip_v: bool,
}
