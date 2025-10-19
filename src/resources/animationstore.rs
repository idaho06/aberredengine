use bevy_ecs::prelude::Resource;
use raylib::prelude::Vector2;
use rustc_hash::FxHashMap;

#[derive(Resource)]
pub struct AnimationStore {
    pub animations: FxHashMap<String, AnimationResource>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationResource {
    pub tex_key: String,
    pub position: Vector2,
    pub displacement: f32,
    pub frame_count: usize,
    pub fps: f32,
    pub looped: bool,
}
