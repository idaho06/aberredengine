use bevy_ecs::prelude::Resource;
use raylib::prelude::Texture2D;
use rustc_hash::FxHashMap;
// use std::collections::HashMap;

#[derive(Resource)]
pub struct TextureStore {
    pub map: FxHashMap<&'static str, Texture2D>,
}
