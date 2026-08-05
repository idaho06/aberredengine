//! Public tilemap loading and tile-spawning utilities.
//!
//! These functions are always compiled (no feature gates) so Rust-only downstream
//! crates can use them without enabling the `lua` feature.

use std::io::Read;
use std::sync::Arc;

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use log::warn;
use crate::math::Vec2;
use serde::Deserialize;

use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::sprite::Sprite;
use crate::components::tilemap::TileMap;
use crate::components::zindex::ZIndex;
use crate::protocol::render_assets::RenderAssetCmd;
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

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// Reads a PNG's width/height straight out of its `IHDR` chunk, without
/// decoding any pixel data. A PNG always starts with an 8-byte signature,
/// then its first chunk — by spec, always `IHDR` — as a 4-byte length, a
/// 4-byte type tag, then the chunk body: 4-byte big-endian width, 4-byte
/// big-endian height. Only the first 24 bytes are read.
fn read_png_dimensions(mut reader: impl Read, path: &str) -> Result<(i32, i32), String> {
    let mut header = [0u8; 24];
    reader
        .read_exact(&mut header)
        .map_err(|err| format!("Failed to read tilemap texture '{}': {err}", path))?;

    if header[0..8] != PNG_SIGNATURE {
        return Err(format!("'{}' is not a valid PNG file", path));
    }
    if &header[12..16] != b"IHDR" {
        return Err(format!("'{}' has no IHDR chunk", path));
    }

    let read_dim = |range: std::ops::Range<usize>, label: &str| -> Result<i32, String> {
        let raw = u32::from_be_bytes(header[range].try_into().unwrap());
        i32::try_from(raw).map_err(|_| format!("'{}' has an unrepresentable {label}: {raw}", path))
    };
    let width = read_dim(16..20, "width")?;
    let height = read_dim(20..24, "height")?;
    Ok((width, height))
}

/// Load tilemap JSON and read atlas PNG dimensions, CPU-only (no GL context
/// required). `path` is a directory; the last path segment is
/// used as the stem for `<stem>.png` (texture) and `<stem>.txt` (JSON
/// data). Returns the parsed [`Tilemap`], the atlas's pixel dimensions, and
/// the computed `png_path` (so callers building a
/// `RenderAssetCmd::TilemapTexture` don't need to recompute it).
pub fn load_tilemap_data(path: &str) -> Result<(Tilemap, i32, i32, String), String> {
    let dirname = path_stem(path);
    let json_path = format!("{}/{}.txt", path, dirname);
    let png_path = format!("{}/{}.png", path, dirname);

    let file = std::fs::File::open(&png_path)
        .map_err(|err| format!("Failed to read tilemap texture '{}': {err}", png_path))?;
    let (tex_w, tex_h) = read_png_dimensions(file, &png_path)?;

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
                        offset: Vec2 {
                            x: col as f32 * tile_size,
                            y: row as f32 * tile_size,
                        },
                        origin: Vec2::ZERO,
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

    #[test]
    fn read_png_dimensions_reads_valid_header() {
        let file = std::fs::File::open(
            "assets/tilemaps/sidescroller_test01/sidescroller_test01.png",
        )
        .expect("fixture PNG should exist");
        let (w, h) = read_png_dimensions(file, "fixture.png").expect("valid PNG header");
        assert_eq!((w, h), (192, 120));
    }

    #[test]
    fn read_png_dimensions_reports_error_for_truncated_file() {
        let truncated: &[u8] = &PNG_SIGNATURE;
        let result = read_png_dimensions(truncated, "truncated.png");
        assert!(result.is_err());
    }

    #[test]
    fn read_png_dimensions_reports_error_for_non_png_file() {
        let not_png = [0u8; 24];
        let result = read_png_dimensions(&not_png[..], "not_a.png");
        let err = result.expect_err("non-PNG bytes should be rejected");
        assert!(err.contains("not a valid PNG"), "unexpected error: {err}");
    }
}
