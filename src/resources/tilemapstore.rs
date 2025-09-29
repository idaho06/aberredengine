use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Tileposition {
    pub x: u32,
    pub y: u32,
    pub id: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tilelayer {
    pub name: String,
    pub positions: Vec<Tileposition>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tilemap {
    pub tile_size: u32,
    pub map_width: u32,
    pub map_height: u32,
    pub layers: Vec<Tilelayer>,
}

#[derive(Resource, Debug, Default)]
pub struct TilemapStore {
    pub map: FxHashMap<String, Tilemap>,
}

impl TilemapStore {
    pub fn new() -> Self {
        TilemapStore {
            map: FxHashMap::default(),
        }
    }
    pub fn get(&self, key: impl Into<String>) -> Option<&Tilemap> {
        self.map.get(&key.into())
    }
    pub fn insert(&mut self, key: impl Into<String>, tilemap: Tilemap) {
        self.map.insert(key.into(), tilemap);
    }
}
