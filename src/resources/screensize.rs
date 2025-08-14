use bevy_ecs::prelude::Resource;

#[derive(Resource, Clone, Copy)]
pub struct ScreenSize {
    pub w: i32,
    pub h: i32,
}
