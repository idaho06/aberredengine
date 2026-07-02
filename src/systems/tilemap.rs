//! Public tilemap loading and tile-spawning utilities.
//!
//! These functions are always compiled (no feature gates) so Rust-only downstream
//! crates can use them without enabling the `lua` feature.

use std::sync::Arc;

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use log::warn;
use raylib::prelude::Vector2;
use serde::Deserialize;

use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::sprite::Sprite;
use crate::components::tilemap::TileMap;
use crate::components::zindex::ZIndex;
use crate::events::render_assets::RenderAssetCmd;
use crate::systems::propagate_transforms::ComputeInitialGlobalTransform;

pub const TILES_GROUP: &str = "tiles";
pub const TILES_TEMPLATES_GROUP: &str = "tiles-templates";

/// Single tile placement within a layer.
#[derive(Debug, Deserialize)]
pub struct TilePosition {
    pub x: u32,
    pub y: u32,
    pub id: u32,
}

/// A named tile layer containing tile placements.
#[derive(Debug, Deserialize)]
pub struct TileLayer {
    pub name: String,
    pub positions: Vec<TilePosition>,
}

/// Tilemap metadata and layer data, as parsed from Tilesetter 2.1.0 JSON.
#[derive(Debug, Deserialize)]
pub struct Tilemap {
    pub tile_size: u32,
    pub map_width: u32,
    pub map_height: u32,
    pub layers: Vec<TileLayer>,
}

/// Returns the last `/`-separated segment of `path` (the directory stem).
fn path_stem(path: &str) -> &str {
    path.split('/').next_back().unwrap_or(path)
}

/// Load tilemap JSON and read atlas PNG dimensions, CPU-only (no GL context
/// required — Phase 5c). `path` is a directory; the last path segment is
/// used as the stem for `<stem>.png` (texture) and `<stem>.txt` (JSON
/// data). Returns the parsed [`Tilemap`], the atlas's pixel dimensions, and
/// the computed `png_path` (so callers building a
/// `RenderAssetCmd::TilemapTexture` don't need to recompute it).
pub fn load_tilemap_data(path: &str) -> Result<(Tilemap, i32, i32, String), String> {
    let dirname = path_stem(path);
    let json_path = format!("{}/{}.txt", path, dirname);
    let png_path = format!("{}/{}.png", path, dirname);

    // Full CPU pixel decode just to read width/height — raylib has no
    // header-only image reader. `process_render_asset_cmds`'s
    // `RenderAssetCmd::TilemapTexture` handler decodes the same PNG a
    // second time to actually upload it, so a tilemap spawn now decodes
    // its atlas twice. Accepted: this is one-time, scene-load-time cost
    // (not a hot path), and atlases are small; revisit with a header-only
    // reader only if a large atlas makes this measurable.
    let image = raylib::prelude::Image::load_image(&png_path)
        .map_err(|err| format!("Failed to read tilemap texture '{}': {err}", png_path))?;
    let (tex_w, tex_h) = (image.width(), image.height());

    let json_string = std::fs::read_to_string(&json_path)
        .map_err(|err| format!("Failed to load tilemap JSON '{}': {err}", json_path))?;
    let tilemap: Tilemap = serde_json::from_str(&json_string)
        .map_err(|err| format!("Failed to parse tilemap JSON '{}': {err}", json_path))?;

    Ok((tilemap, tex_w, tex_h, png_path))
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
                    Group::new(TILES_TEMPLATES_GROUP),
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
                .insert(Group::new(TILES_GROUP))
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

/// Watches for newly added [`TileMap`] components, loads the tilemap data
/// (CPU-only), queues the atlas texture upload via
/// [`RenderAssetCmd::TilemapTexture`], and spawns tile entities as
/// `ChildOf` children of the root entity.
///
/// If the root entity has no [`MapPosition`], a default `(0, 0)` one is inserted
/// so that [`crate::systems::propagate_transforms`] can compute child transforms.
pub fn tilemap_spawn_system(
    mut commands: Commands,
    query: Query<(Entity, &TileMap, Has<MapPosition>), Added<TileMap>>,
    mut render_asset_cmd_writer: MessageWriter<RenderAssetCmd>,
) {
    for (entity, tilemap_comp, has_map_pos) in query.iter() {
        let path = &tilemap_comp.path;
        let key: String = path_stem(path).to_owned();

        let (tilemap_data, tex_w, tex_h, png_path) = match load_tilemap_data(path) {
            Ok(loaded) => loaded,
            Err(err) => {
                warn!(
                    "tilemap_spawn_system: failed to load tilemap for entity {:?} from '{}': {}",
                    entity, path, err
                );
                continue;
            }
        };

        render_asset_cmd_writer.write(RenderAssetCmd::TilemapTexture {
            key: key.clone(),
            png_path,
        });

        if !has_map_pos {
            commands.entity(entity).insert(MapPosition::new(0.0, 0.0));
        }

        spawn_tiles(
            &mut commands,
            &key,
            tex_w,
            tex_h,
            &tilemap_data,
            Some(entity),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real tilemap fixture already checked into the repo
    /// (`assets/tilemaps/sidescroller_test01/`), 192x120 PNG atlas.
    const FIXTURE_DIR: &str = "assets/tilemaps/sidescroller_test01";

    #[test]
    fn load_tilemap_data_reads_dimensions_and_parses_json_cpu_only() {
        let (tilemap, tex_w, tex_h, png_path) =
            load_tilemap_data(FIXTURE_DIR).expect("fixture should load");

        assert_eq!(tex_w, 192);
        assert_eq!(tex_h, 120);
        assert_eq!(
            png_path,
            "assets/tilemaps/sidescroller_test01/sidescroller_test01.png"
        );
        assert!(tilemap.tile_size > 0);
        assert!(!tilemap.layers.is_empty());
    }

    #[test]
    fn load_tilemap_data_reports_error_for_missing_directory() {
        let result = load_tilemap_data("assets/tilemaps/does_not_exist");
        assert!(result.is_err());
    }
}
