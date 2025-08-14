use bevy_ecs::prelude::Component;

#[derive(Component, Clone, Copy, Debug)]
pub struct MapPosition {
    pub x: f32,
    pub y: f32,
}
