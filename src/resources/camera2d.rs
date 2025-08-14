use bevy_ecs::prelude::Resource;
use raylib::prelude::Camera2D;

#[derive(Resource)]
pub struct Camera2DRes(pub Camera2D);
