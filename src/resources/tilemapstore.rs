//! Tilemap storage and data types.
//!
//! Provides simple serializable structs for tile map data and a store for
//! loaded maps. Systems can load and render maps by key.

use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// Single tile placement within a layer.
#[derive(Debug, Deserialize, Serialize)]
pub struct Tileposition {
    /// X coordinate in tiles.
    pub x: u32,
    /// Y coordinate in tiles.
    pub y: u32,
    /// Tile identifier (tileset-local).
    pub id: u32,
}

/// A named tile layer containing positions.
#[derive(Debug, Deserialize, Serialize)]
pub struct Tilelayer {
    pub name: String,
    pub positions: Vec<Tileposition>,
}

/// Tilemap metadata and layers.
#[derive(Debug, Deserialize, Serialize)]
pub struct Tilemap {
    /// Size of a tile in pixels.
    pub tile_size: u32,
    /// Map width in tiles.
    pub map_width: u32,
    /// Map height in tiles.
    pub map_height: u32,
    pub layers: Vec<Tilelayer>,
}

/// Registry of loaded tilemaps by key.
#[derive(Resource, Debug, Default)]
pub struct TilemapStore {
    pub map: FxHashMap<String, Tilemap>,
}

impl TilemapStore {
    /// Create an empty store.
    pub fn new() -> Self {
        TilemapStore {
            map: FxHashMap::default(),
        }
    }
    /// Get a tilemap by its key.
    pub fn get(&self, key: impl AsRef<str>) -> Option<&Tilemap> {
        self.map.get(key.as_ref())
    }
    /// Insert a tilemap with a specific key.
    pub fn insert(&mut self, key: impl Into<String>, tilemap: Tilemap) {
        self.map.insert(key.into(), tilemap);
    }

    /// Clear all loaded tilemaps.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}
