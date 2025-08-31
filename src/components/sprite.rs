use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Sprite is identified by a texture key, its size in world units and a offset if the texture is a spritesheet.
/// The offset is used to select the correct frame from the spritesheet.
/// The origin selects the pivot point (in pixels) relative to the texture's top-left
/// used for placement/rotation/scaling when rendering.
#[derive(Component, Clone, Debug)]
pub struct Sprite {
    pub tex_key: String,
    pub width: f32,
    pub height: f32,
    pub offset: Vector2,
    pub origin: Vector2,
    pub flip_h: bool,
    pub flip_v: bool,
}
