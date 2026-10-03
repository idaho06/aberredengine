//! Public tilemap loading and tile-spawning utilities.
//!
//! These functions are always compiled (no feature gates) so Rust-only downstream
//! crates can use them without enabling the `lua` feature.

use std::io::Read;
use std::sync::Arc;

use crate::math::Vec2;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use log::warn;
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

/// The one spelling of a tilemap directory path: no trailing slashes, no
/// leading `./`. Both the file paths and the atlas key derive from it.
fn normalize_tilemap_dir(path: &str) -> &str {
    let path = path.trim_end_matches('/');
    path.strip_prefix("./").unwrap_or(path)
}

/// Returns the last `/`-separated segment of a normalized `path` (the
/// directory stem).
fn path_stem(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `TextureStore` key of the atlas for the tilemap directory at `path`:
/// `"tilemap:"` plus the path without trailing slashes or a leading `./`.
/// The prefix keeps it apart from textures a game loads itself, and the
/// whole path keeps same-named directories apart.
pub fn tilemap_texture_key(path: &str) -> String {
    format!("tilemap:{}", normalize_tilemap_dir(path))
}

/// Path of the atlas PNG for the tilemap directory at `path`:
/// `<dir>/<stem>.png`, with the directory spelled as in
/// [`tilemap_texture_key`].
pub fn tilemap_png_path(path: &str) -> String {
    let path = normalize_tilemap_dir(path);
    format!("{}/{}.png", path, path_stem(path))
}

/// The command that loads the atlas of the tilemap directory at `path`,
/// shared by `tilemap_spawn_system` and `AssetLoader::load_tilemap`.
pub fn tilemap_texture_cmd(path: &str) -> RenderAssetCmd {
    RenderAssetCmd::TilemapTexture {
        key: tilemap_texture_key(path),
        png_path: tilemap_png_path(path),
    }
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
/// data). Returns the parsed [`Tilemap`] and the atlas's pixel dimensions.
pub fn load_tilemap_data(path: &str) -> Result<(Tilemap, i32, i32), String> {
    let path = normalize_tilemap_dir(path);
    let dirname = path_stem(path);
    let json_path = format!("{}/{}.txt", path, dirname);
    let png_path = tilemap_png_path(path);

    let file = std::fs::File::open(&png_path)
        .map_err(|err| format!("Failed to read tilemap texture '{}': {err}", png_path))?;
    let (tex_w, tex_h) = read_png_dimensions(file, &png_path)?;

    let json_string = std::fs::read_to_string(&json_path)
        .map_err(|err| format!("Failed to load tilemap JSON '{}': {err}", json_path))?;
    let tilemap: Tilemap = serde_json::from_str(&json_string)
        .map_err(|err| format!("Failed to parse tilemap JSON '{}': {err}", json_path))?;

    Ok((tilemap, tex_w, tex_h))
}

/// Spawn tile entities from a loaded tilemap.
///
/// Phase 1 — create one template entity per atlas cell (`Group("tiles-templates")` + `Sprite`).
///
/// Phase 2 — clone the matching template for each tile placement and insert
/// `Group("tiles")`, `MapPosition`, and `ZIndex`. When `parent` is `Some`,
/// each tile clone also gets `ChildOf(parent)` and `ComputeInitialGlobalTransform`
/// is queued so children render at the correct world position on the first frame.
///
/// Phase 3 — despawn the templates; only the tiles remain.
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

    // Phase 3: templates exist only to be cloned. The despawns are queued after every
    // clone above, so they apply once all tiles exist -- and nothing outlives the tiles
    // (templates are not children of `parent`, so a root despawn would not reach them).
    for template in templates {
        commands.entity(template).despawn();
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
        let (tilemap_data, tex_w, tex_h) = match load_tilemap_data(path) {
            Ok(loaded) => loaded,
            Err(err) => {
                warn!(
                    "tilemap_spawn_system: failed to load tilemap for entity {:?} from '{}': {}",
                    entity, path, err
                );
                continue;
            }
        };

        render_asset_cmd_writer.write(tilemap_texture_cmd(path));
        let key = tilemap_texture_key(path);

        if !has_map_pos {
            commands.entity(entity).insert(MapPosition::new(0.0, 0.0));
        }

        spawn_tiles(
            &mut commands,
            key,
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
        let (tilemap, tex_w, tex_h) = load_tilemap_data(FIXTURE_DIR).expect("fixture should load");

        assert_eq!(tex_w, 192);
        assert_eq!(tex_h, 120);
        assert_eq!(
            tilemap_png_path(FIXTURE_DIR),
            "assets/tilemaps/sidescroller_test01/sidescroller_test01.png"
        );
        assert!(tilemap.tile_size > 0);
        assert!(!tilemap.layers.is_empty());
    }

    fn tilemap(tile_size: u32, layers: &[&[(u32, u32, u32)]]) -> Tilemap {
        Tilemap {
            tile_size,
            map_width: 0,
            map_height: 0,
            layers: layers
                .iter()
                .enumerate()
                .map(|(i, positions)| TileLayer {
                    name: format!("layer{i}"),
                    positions: positions
                        .iter()
                        .map(|&(x, y, id)| TilePosition { x, y, id })
                        .collect(),
                })
                .collect(),
        }
    }

    fn run_spawn_tiles(world: &mut World, tex: (i32, i32), map: &Tilemap, parent: Option<Entity>) {
        let mut state = bevy_ecs::system::SystemState::<Commands>::new(world);
        {
            let mut commands = state.get_mut(world).unwrap();
            spawn_tiles(&mut commands, "atlas", tex.0, tex.1, map, parent);
        }
        state.apply(world);
    }

    fn in_group(world: &mut World, group: &str) -> Vec<(Entity, Sprite)> {
        world
            .query::<(Entity, &Group, &Sprite)>()
            .iter(world)
            .filter(|(_, g, _)| g.0 == group)
            .map(|(e, _, s)| (e, s.clone()))
            .collect()
    }

    #[test]
    fn each_tile_uses_the_atlas_cell_of_its_id() {
        let mut world = World::new();
        // 64x32 atlas of 16px tiles -> 4 columns x 2 rows; place one tile per id.
        let positions: Vec<(u32, u32, u32)> = (0..8).map(|id| (id, 0, id)).collect();
        run_spawn_tiles(&mut world, (64, 32), &tilemap(16, &[&positions]), None);

        let mut by_x: Vec<(f32, Vec2)> = world
            .query::<(&MapPosition, &Sprite)>()
            .iter(&world)
            .map(|(p, s)| (p.pos.x, s.offset))
            .collect();
        by_x.sort_by(|a, b| a.0.total_cmp(&b.0));
        let offsets: Vec<Vec2> = by_x.into_iter().map(|(_, o)| o).collect();
        assert_eq!(offsets.len(), 8);
        assert_eq!(offsets[3], Vec2::new(48.0, 0.0), "id 3 = row 0, col 3");
        assert_eq!(offsets[5], Vec2::new(16.0, 16.0), "id 5 = row 1, col 1");
    }

    #[test]
    fn spawn_tiles_places_tiles_by_coordinate_with_layer_zindex_and_parent() {
        let mut world = World::new();
        let root = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let map = tilemap(16, &[&[(0, 0, 0), (2, 1, 5)], &[(1, 0, 3)]]);
        run_spawn_tiles(&mut world, (64, 32), &map, Some(root));

        let mut tiles: Vec<(Vec2, f32, Vec2, Entity)> = world
            .query::<(&Group, &MapPosition, &ZIndex, &Sprite, &ChildOf)>()
            .iter(&world)
            .filter(|(g, ..)| g.0 == TILES_GROUP)
            .map(|(_, p, z, s, c)| (p.pos, z.0, s.offset, c.parent()))
            .collect();
        tiles.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)));
        assert_eq!(
            tiles,
            vec![
                // (world pos = tile coords * size, z: first layer lowest, atlas offset of id, parent)
                (Vec2::new(0.0, 0.0), -2.0, Vec2::new(0.0, 0.0), root),
                (Vec2::new(16.0, 0.0), -1.0, Vec2::new(48.0, 0.0), root),
                (Vec2::new(32.0, 16.0), -2.0, Vec2::new(16.0, 16.0), root),
            ]
        );
    }

    #[test]
    fn spawn_tiles_without_parent_adds_no_childof() {
        let mut world = World::new();
        run_spawn_tiles(&mut world, (32, 16), &tilemap(16, &[&[(0, 0, 1)]]), None);
        let tiles = in_group(&mut world, TILES_GROUP);
        assert_eq!(tiles.len(), 1);
        assert!(world.get::<ChildOf>(tiles[0].0).is_none());
    }

    #[test]
    fn spawn_tiles_skips_ids_outside_the_atlas() {
        let mut world = World::new();
        // 32x16 atlas -> 2 cells (ids 0..=1); id 2 is out of range.
        run_spawn_tiles(
            &mut world,
            (32, 16),
            &tilemap(16, &[&[(0, 0, 1), (1, 0, 2)]]),
            None,
        );
        assert_eq!(in_group(&mut world, TILES_GROUP).len(), 1);
    }

    #[test]
    fn atlas_smaller_than_a_tile_still_yields_one_usable_cell() {
        let mut world = World::new();
        run_spawn_tiles(
            &mut world,
            (8, 8),
            &tilemap(16, &[&[(0, 0, 0), (1, 0, 1)]]),
            None,
        );
        // Only id 0 exists in a sub-tile atlas; id 1 is out of range.
        assert_eq!(in_group(&mut world, TILES_GROUP).len(), 1);
    }

    #[test]
    fn spawn_tiles_leaves_no_template_entities_behind() {
        let mut world = World::new();
        let root = world.spawn(MapPosition::new(0.0, 0.0)).id();
        run_spawn_tiles(
            &mut world,
            (64, 32),
            &tilemap(16, &[&[(0, 0, 0), (1, 0, 5)]]),
            Some(root),
        );
        assert!(
            in_group(&mut world, TILES_TEMPLATES_GROUP).is_empty(),
            "templates despawned"
        );
        assert_eq!(
            in_group(&mut world, TILES_GROUP).len(),
            2,
            "tiles survive their templates"
        );

        world.entity_mut(root).despawn();
        assert!(
            world.query::<&Sprite>().iter(&world).next().is_none(),
            "despawning the root leaves no tile or template sprite behind"
        );
    }

    fn run_system(world: &mut World) -> Vec<RenderAssetCmd> {
        use bevy_ecs::message::Messages;
        world.init_resource::<Messages<RenderAssetCmd>>();
        let mut schedule = Schedule::default();
        schedule.add_systems(tilemap_spawn_system);
        schedule.run(world);
        world
            .resource_mut::<Messages<RenderAssetCmd>>()
            .drain()
            .collect()
    }

    #[test]
    fn system_requests_the_atlas_texture_and_parents_tiles_to_the_root() {
        let mut world = World::new();
        let root = world.spawn(TileMap::new(FIXTURE_DIR)).id();
        let cmds = run_system(&mut world);

        assert!(matches!(
            cmds.as_slice(),
            [RenderAssetCmd::TilemapTexture { key, png_path }]
                if key == &tilemap_texture_key(FIXTURE_DIR) && png_path.ends_with("sidescroller_test01.png")
        ));
        assert_eq!(
            world.get::<MapPosition>(root).unwrap().pos,
            Vec2::ZERO,
            "a root without MapPosition gets one at the origin"
        );
        let tiles = in_group(&mut world, TILES_GROUP);
        assert!(!tiles.is_empty());
        assert!(
            tiles
                .iter()
                .all(|(e, _)| world.get::<ChildOf>(*e).map(|c| c.parent()) == Some(root))
        );
    }

    #[test]
    fn system_keeps_an_existing_root_position() {
        let mut world = World::new();
        let root = world
            .spawn((TileMap::new(FIXTURE_DIR), MapPosition::new(100.0, 50.0)))
            .id();
        run_system(&mut world);
        assert_eq!(
            world.get::<MapPosition>(root).unwrap().pos,
            Vec2::new(100.0, 50.0)
        );
    }

    #[test]
    fn system_with_missing_tilemap_spawns_and_requests_nothing() {
        let mut world = World::new();
        let root = world
            .spawn(TileMap::new("assets/tilemaps/does_not_exist"))
            .id();
        assert!(run_system(&mut world).is_empty());
        // Entity counts are unreliable here (bevy registers resources/systems as entities).
        assert!(in_group(&mut world, TILES_GROUP).is_empty());
        assert!(in_group(&mut world, TILES_TEMPLATES_GROUP).is_empty());
        assert!(
            world.get::<MapPosition>(root).is_none(),
            "root left untouched"
        );
    }

    #[test]
    fn trailing_slash_in_tilemap_path_is_ignored() {
        let with_slash = format!("{FIXTURE_DIR}/");
        let (_, w, h) = load_tilemap_data(&with_slash).expect("trailing slash loads");
        assert_eq!((w, h), (192, 120));
        assert_eq!(
            tilemap_png_path(&with_slash),
            format!("{FIXTURE_DIR}/sidescroller_test01.png")
        );

        let mut world = World::new();
        world.spawn(TileMap::new(format!("{FIXTURE_DIR}//")));
        let cmds = run_system(&mut world);
        assert!(matches!(
            cmds.as_slice(),
            [RenderAssetCmd::TilemapTexture { key, .. }] if key == &tilemap_texture_key(FIXTURE_DIR)
        ));
        assert!(!in_group(&mut world, TILES_GROUP).is_empty());
    }

    /// Copies the fixture tilemap into `<parent>/<fixture stem>` and returns
    /// that directory's path.
    fn copy_fixture_into(parent: &std::path::Path) -> String {
        let stem = path_stem(FIXTURE_DIR);
        let dir = parent.join(stem);
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["png", "txt"] {
            let file = format!("{stem}.{ext}");
            std::fs::copy(format!("{FIXTURE_DIR}/{file}"), dir.join(&file)).unwrap();
        }
        dir.to_str().unwrap().to_owned()
    }

    #[test]
    fn same_named_tilemaps_in_different_directories_get_distinct_atlas_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let path_a = copy_fixture_into(&tmp.path().join("a"));
        let path_b = copy_fixture_into(&tmp.path().join("b"));
        let (key_a, key_b) = (tilemap_texture_key(&path_a), tilemap_texture_key(&path_b));
        assert_ne!(key_a, key_b, "each directory gets its own atlas");

        let mut world = World::new();
        let root_a = world.spawn(TileMap::new(path_a)).id();
        let root_b = world.spawn(TileMap::new(path_b)).id();
        assert_eq!(run_system(&mut world).len(), 2, "one atlas upload per map");

        let tiles = in_group(&mut world, TILES_GROUP);
        for (root, key) in [(root_a, &key_a), (root_b, &key_b)] {
            let own_keys: Vec<_> = tiles
                .iter()
                .filter(|(e, _)| world.get::<ChildOf>(*e).map(|c| c.parent()) == Some(root))
                .map(|(_, sprite)| sprite.tex_key.clone())
                .collect();
            assert!(
                !own_keys.is_empty() && own_keys.iter().all(|k| &**k == key.as_str()),
                "tiles draw with their own map's atlas"
            );
        }
    }

    #[test]
    fn spellings_of_one_tilemap_directory_share_an_atlas_key() {
        assert_eq!(tilemap_texture_key("maps/level01"), "tilemap:maps/level01");
        assert_eq!(
            tilemap_texture_key("./maps/level01/"),
            "tilemap:maps/level01"
        );
    }

    #[test]
    fn load_tilemap_data_reports_error_for_missing_directory() {
        let result = load_tilemap_data("assets/tilemaps/does_not_exist");
        assert!(result.is_err());
    }

    #[test]
    fn read_png_dimensions_reads_valid_header() {
        let file =
            std::fs::File::open("assets/tilemaps/sidescroller_test01/sidescroller_test01.png")
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
