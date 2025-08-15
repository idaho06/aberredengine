use bevy_ecs::prelude::Component;

/// Sprite is identified by a texture key, its size in world units and a offset if the texture is a spritesheet.
/// The offset is used to select the correct frame from the spritesheet.
#[derive(Component, Clone, Debug)]
pub struct Sprite {
    pub tex_key: &'static str,
    pub width: f32,
    pub height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}
