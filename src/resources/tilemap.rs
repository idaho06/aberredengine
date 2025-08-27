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
