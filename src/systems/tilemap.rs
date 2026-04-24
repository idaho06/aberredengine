//! Public tilemap loading and tile-spawning utilities.
//!
//! These functions are always compiled (no feature gates) so Rust-only downstream
//! crates can use them without enabling the `lua` feature.

use std::sync::Arc;

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use log::warn;
use raylib::prelude::{Texture2D, Vector2};
use serde::{Deserialize, Serialize};

use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::sprite::Sprite;
use crate::components::tilemap::TileMap;
use crate::components::zindex::ZIndex;
use crate::resources::texturestore::TextureStore;
use crate::systems::propagate_transforms::ComputeInitialGlobalTransform;
use crate::systems::RaylibAccess;

/// Single tile placement within a layer.
#[derive(Debug, Deserialize, Serialize)]
pub struct Tileposition {
    pub x: u32,
    pub y: u32,
    pub id: u32,
}

/// A named tile layer containing tile placements.
#[derive(Debug, Deserialize, Serialize)]
pub struct Tilelayer {
    pub name: String,
    pub positions: Vec<Tileposition>,
}

/// Tilemap metadata and layer data, as parsed from Tilesetter 2.1.0 JSON.
#[derive(Debug, Deserialize, Serialize)]
pub struct Tilemap {
    pub tile_size: u32,
    pub map_width: u32,
    pub map_height: u32,
    pub layers: Vec<Tilelayer>,
}

/// Returns the last `/`-separated segment of `path` (the directory stem).
fn path_stem(path: &str) -> &str {
    path.split('/').next_back().unwrap_or(path)
}

/// Load a tilemap from a directory produced by Tilesetter 2.1.0.
///
/// `path` is a directory; the last path segment is used as the stem for
/// `<stem>.png` (texture) and `<stem>.txt` (JSON data).
pub fn load_tilemap(
    rl: &mut raylib::RaylibHandle,
    thread: &raylib::RaylibThread,
    path: &str,
) -> (Texture2D, Tilemap) {
    let dirname = path_stem(path);
    let json_path = format!("{}/{}.txt", path, dirname);
    let png_path = format!("{}/{}.png", path, dirname);
    let texture = rl
        .load_texture(thread, &png_path)
        .expect("Failed to load tilemap texture");
    let json_string =
        std::fs::read_to_string(json_path).expect("Failed to load tilemap JSON");
    let tilemap: Tilemap =
        serde_json::from_str(&json_string).expect("Failed to parse tilemap JSON");
    (texture, tilemap)
}

/// Spawn tile entities from a loaded tilemap.
///
/// Phase 1 — create one template entity per atlas cell (`Group("tiles-templates")` + `Sprite`).
/// Templates are kept alive in the world (no `MapPosition`, so they are not rendered).
///
/// Phase 2 — clone the matching template for each tile placement and insert
/// `Group("tiles")`, `MapPosition`, and `ZIndex`. When `parent` is `Some`,
/// each tile clone also gets `ChildOf(parent)` and `ComputeInitialGlobalTransform`
/// is queued so children render at the correct world position on the first frame.
pub fn spawn_tiles(
    commands: &mut Commands,
    tilemap_tex_key: impl Into<String>,
    tex_width: i32,
    tex_height: i32,
    tilemap: &Tilemap,
    parent: Option<Entity>,
) {
    let tilemap_tex_key: Arc<str> = Arc::from(tilemap_tex_key.into());
    let tile_size = tilemap.tile_size as f32;
    let tiles_per_row = ((tex_width as f32 / tile_size).floor() as u32).max(1);
    let tiles_per_col = ((tex_height as f32 / tile_size).floor() as u32).max(1);
    let total_tiles = tiles_per_row * tiles_per_col;

    // Phase 1: one template entity per atlas cell — Sprite only, no position/layer.
    let templates: Vec<Entity> = (0..total_tiles)
        .map(|id| {
            let col = id % tiles_per_row;
            let row = id / tiles_per_row;
            commands
                .spawn((
                    Group::new("tiles-templates"),
                    Sprite {
                        tex_key: tilemap_tex_key.clone(),
                        width: tile_size,
                        height: tile_size,
                        offset: Vector2 {
                            x: col as f32 * tile_size,
                            y: row as f32 * tile_size,
                        },
                        origin: Vector2::zero(),
                        flip_h: false,
                        flip_v: false,
                    },
                ))
                .id()
        })
        .collect();

    // Phase 2: clone the matching template for each tile placement.
    let layer_count = tilemap.layers.len() as f32;
    for (layer_index, layer) in tilemap.layers.iter().enumerate() {
        let z = -(layer_count - layer_index as f32);
        for pos in &layer.positions {
            let id = pos.id as usize;
            if id >= templates.len() {
                warn!(
                    "Tile id {} out of range (atlas has {} tiles), skipping",
                    id,
                    templates.len()
                );
                continue;
            }
            let wx = pos.x as f32 * tile_size;
            let wy = pos.y as f32 * tile_size;
            let clone_id = commands
                .entity(templates[id])
                .clone_and_spawn()
                .insert(Group::new("tiles"))
                .insert(MapPosition::new(wx, wy))
                .insert(ZIndex(z))
                .id();
            if let Some(p) = parent {
                commands
                    .entity(clone_id)
                    .insert(ChildOf(p))
                    .queue(ComputeInitialGlobalTransform);
            }
        }
    }
}

/// Watches for newly added [`TileMap`] components, loads the tilemap from disk,
/// stores the texture in [`TextureStore`], and spawns tile entities as `ChildOf`
/// children of the root entity.
///
/// If the root entity has no [`MapPosition`], a default `(0, 0)` one is inserted
/// so that [`crate::systems::propagate_transforms`] can compute child transforms.
pub fn tilemap_spawn_system(
    mut commands: Commands,
    query: Query<(Entity, &TileMap, Has<MapPosition>), Added<TileMap>>,
    mut raylib: RaylibAccess,
    mut texture_store: ResMut<TextureStore>,
) {
    for (entity, tilemap_comp, has_map_pos) in query.iter() {
        let path = &tilemap_comp.path;
        let key: String = path_stem(path).to_owned();

        let (texture, tilemap_data) = load_tilemap(&mut raylib.rl, &raylib.th, path);
        let tex_w = texture.width;
        let tex_h = texture.height;
        if texture_store.get(&key).is_none() {
            texture_store.insert(&key, texture);
        }

        if !has_map_pos {
            commands.entity(entity).insert(MapPosition::new(0.0, 0.0));
        }

        spawn_tiles(&mut commands, &key, tex_w, tex_h, &tilemap_data, Some(entity));
    }
}
