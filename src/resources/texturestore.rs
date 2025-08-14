use bevy_ecs::prelude::Resource;
use raylib::prelude::Texture2D;
use std::collections::HashMap;

#[derive(Resource)]
pub struct TextureStore {
    pub map: HashMap<&'static str, Texture2D>,
}
