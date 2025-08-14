use bevy_ecs::prelude::Component;

/// Sprite is identified by a texture key and its size in world units.
#[derive(Component, Clone, Debug)]
pub struct Sprite {
    pub tex_key: &'static str,
    pub width: f32,
    pub height: f32,
}
