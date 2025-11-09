use bevy_ecs::prelude::Component;

#[derive(Component, Clone, Debug, Copy, Default)]
pub struct Rotation {
    pub degrees: f32,
}
