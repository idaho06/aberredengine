//! Map / level data structures with JSON serialization.
//!
//! # Loading and saving
//!
//! ```rust,no_run
//! use aberredengine::resources::mapdata::{MapData, load_map, save_map};
//!
//! let map = load_map("assets/levels/level01.json").expect("failed to load level");
//! save_map("/tmp/level01.json", &map).expect("failed to save level");
//! ```
//!
//! # Runtime spawning
//!
//! After loading, trigger [`crate::events::spawnmap::SpawnMapRequested`] to
//! have the engine load all referenced assets and spawn entities:
//!
//! ```rust,no_run
//! # use aberredengine::resources::mapdata::load_map;
//! # use aberredengine::events::spawnmap::SpawnMapRequested;
//! # use bevy_ecs::prelude::Commands;
//! # fn example(mut commands: Commands) {
//! let map = load_map("assets/levels/level01.json").unwrap();
//! commands.trigger(SpawnMapRequested { map });
//! # }
//! ```

use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Top-level map / level descriptor. Serializes to/from JSON.
///
/// Insert as a Bevy resource via `commands.insert_resource(map)` if you want
/// to make the loaded data accessible to other systems, or trigger
/// [`crate::events::spawnmap::SpawnMapRequested`] to spawn all assets and
/// entities in one step.
#[derive(Serialize, Deserialize, Default, Resource, Clone, Debug, PartialEq)]
pub struct MapData {
    /// Human-readable display name (not used as a file path).
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// RGB clear color override. `None` means use the engine default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<[u8; 3]>,
    pub textures: Vec<TextureEntry>,
    pub fonts: Vec<FontEntry>,
    pub animations: Vec<AnimationEntry>,
    pub entities: Vec<EntityDef>,
}

/// A texture asset to load into [`crate::resources::texturestore::TextureStore`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TextureEntry {
    /// Key used to look up the texture in `TextureStore`.
    pub key: String,
    /// Relative path to the image file.
    pub path: String,
    /// Texture sampling filter: "nearest" (default), "bilinear", "trilinear",
    /// "anisotropic_4x", "anisotropic_8x", or "anisotropic_16x". Absent or
    /// unrecognized values fall back to "nearest".
    #[serde(default)]
    pub filter: Option<String>,
}

/// A font asset to load into [`crate::resources::fontstore::FontStore`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FontEntry {
    pub key: String,
    pub path: String,
    pub font_size: f32,
}

/// An animation clip defined as a strip of frames in a sprite sheet.
///
/// Fields map 1:1 to [`crate::resources::animationstore::AnimationResource`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnimationEntry {
    /// Key used to look up the animation in `AnimationStore`.
    pub key: String,
    /// Key of the texture in `TextureStore` that holds the sprite sheet.
    pub texture_key: String,
    /// Pixel offset `[x, y]` of the first frame within the sprite sheet.
    pub position: [f32; 2],
    /// Horizontal distance between consecutive frames (usually = frame width).
    pub horizontal_displacement: f32,
    /// Vertical distance between rows. Use `0.0` for single-row animations.
    pub vertical_displacement: f32,
    /// Total number of frames in the clip.
    pub frame_count: u32,
    /// Playback speed in frames per second.
    pub fps: f32,
    /// Whether the animation loops after the last frame.
    pub looping: bool,
}

/// A single entity placement in the map.
///
/// All fields are optional — only add the components relevant to your entity.
/// Extend with new optional fields as new component types are introduced.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct EntityDef {
    /// World-space position `[x, y]` (maps to [`crate::components::mapposition::MapPosition`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SpriteEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collider: Option<BoxColliderEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Render order (maps to [`crate::components::zindex::ZIndex`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_index: Option<f32>,
    /// Rotation in degrees (maps to [`crate::components::rotation::Rotation`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_deg: Option<f32>,
    /// Scale `[x, y]` (maps to [`crate::components::scale::Scale`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f32; 2]>,
    /// If set, spawns a [`crate::components::tilemap::TileMap`] component with this directory path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilemap_path: Option<String>,
    /// If set, registers the spawned entity in `WorldSignals.entities` under this key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registered_as: Option<String>,
    /// Color tint `[r, g, b, a]` in 0–255 (maps to [`crate::components::tint::Tint`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<[u8; 4]>,
    /// *(feature = "lua")* Lua function to call once when this entity is first seen by the engine
    /// (maps to [`crate::components::luasetup::LuaSetup`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lua_setup: Option<String>,
    /// *(feature = "lua")* Lua function to call once when the entity's non-looped animation first finishes
    /// (maps to [`crate::components::lua_on_animation_end::LuaOnAnimationEnd`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_animation_end: Option<String>,
    /// Text rendering data (maps to [`crate::components::dynamictext::DynamicText`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_text: Option<DynamicTextEntry>,
    /// Animation key in [`crate::resources::animationstore::AnimationStore`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation_key: Option<String>,
    /// Particle emitter component data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub particle_emitter: Option<ParticleEmitterEntry>,
}

/// Dynamic text rendering data for an entity placement.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DynamicTextEntry {
    /// Initial string content.
    pub text: String,
    /// Key into `FontStore`.
    pub font_key: String,
    /// Font size in pixels.
    pub font_size: f32,
    /// Base color `[r, g, b, a]` in 0-255.
    pub color: [u8; 4],
}

/// Sprite rendering data for an entity placement.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SpriteEntry {
    /// Key into `TextureStore`.
    pub texture_key: String,
    /// Width of the sprite in world units.
    pub width: f32,
    /// Height of the sprite in world units.
    pub height: f32,
    /// Pixel offset `[x, y]` into the texture (top-left corner of the frame).
    /// `None` means `(0.0, 0.0)`.
    pub offset: Option<[f32; 2]>,
    /// Pivot point `[x, y]` relative to the texture's top-left corner.
    /// `None` means `(0.0, 0.0)`.
    pub origin: Option<[f32; 2]>,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
}

/// Box collider data for an entity placement.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BoxColliderEntry {
    /// Size `[width, height]` in world units.
    pub size: [f32; 2],
    /// Offset `[x, y]` from the entity pivot.
    /// `None` means `(0.0, 0.0)`.
    pub offset: Option<[f32; 2]>,
    /// Pivot origin `[x, y]` relative to the collider top-left.
    /// `None` means `(0.0, 0.0)`.
    pub origin: Option<[f32; 2]>,
}

/// Emitter shape for a [`ParticleEmitterEntry`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParticleEmitterShapeEntry {
    #[default]
    Point,
    Rect {
        width: f32,
        height: f32,
    },
}

/// TTL spec for a [`ParticleEmitterEntry`].
///
/// Uses `tag + content` (adjacently-tagged) serde so that `Fixed { value }` round-trips
/// correctly. Internally-tagged enums cannot serialize newtype variants over primitives.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ParticleEmitterTtlEntry {
    #[default]
    None,
    Fixed {
        value: f32,
    },
    Range {
        min: f32,
        max: f32,
    },
}

/// Particle emitter data for an entity placement.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ParticleEmitterEntry {
    pub template_keys: Vec<String>,
    pub shape: ParticleEmitterShapeEntry,
    /// Offset from entity pivot. `None` means `[0, 0]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 2]>,
    pub particles_per_emission: u32,
    pub emissions_per_second: f32,
    /// `u32::MAX` means unlimited.
    pub emissions_remaining: u32,
    /// `[min_deg, max_deg]` arc for particle launch direction.
    pub arc_degrees: [f32; 2],
    /// `[min, max]` speed range.
    pub speed_range: [f32; 2],
    pub ttl: ParticleEmitterTtlEntry,
}

/// Load a [`MapData`] from a JSON file at `path`.
pub fn load_map(path: impl AsRef<Path>) -> Result<MapData, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let map = serde_json::from_str(&text)?;
    Ok(map)
}

/// Serialize a [`MapData`] to pretty-printed JSON and write it to `path`.
pub fn save_map(path: impl AsRef<Path>, map: &MapData) -> Result<(), Box<dyn std::error::Error>> {
    let text = serde_json::to_string_pretty(map)?;
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_map() -> MapData {
        MapData {
            name: "Test Level".into(),
            textures: vec![TextureEntry {
                key: "player".into(),
                path: "assets/textures/player.png".into(),
                filter: None,
            }],
            fonts: vec![FontEntry {
                key: "arcade".into(),
                path: "assets/fonts/Arcade.ttf".into(),
                font_size: 32.0,
            }],
            animations: vec![AnimationEntry {
                key: "walk".into(),
                texture_key: "player".into(),
                position: [0.0, 0.0],
                horizontal_displacement: 16.0,
                vertical_displacement: 0.0,
                frame_count: 4,
                fps: 12.0,
                looping: true,
            }],
            entities: vec![
                EntityDef {
                    position: Some([100.0, 200.0]),
                    sprite: Some(SpriteEntry {
                        texture_key: "player".into(),
                        width: 16.0,
                        height: 16.0,
                        offset: None,
                        origin: None,
                        flip_h: false,
                        flip_v: false,
                    }),
                    group: Some("player".into()),
                    z_index: Some(1.0),
                    ..Default::default()
                },
                EntityDef {
                    position: Some([140.0, 220.0]),
                    dynamic_text: Some(DynamicTextEntry {
                        text: "Hello".into(),
                        font_key: "arcade".into(),
                        font_size: 24.0,
                        color: [255, 255, 255, 255],
                    }),
                    ..Default::default()
                },
                EntityDef {
                    position: Some([180.0, 260.0]),
                    collider: Some(BoxColliderEntry {
                        size: [32.0, 48.0],
                        offset: Some([3.0, 4.0]),
                        origin: Some([5.0, 6.0]),
                    }),
                    group: Some("colliders".into()),
                    registered_as: Some("test_collider".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn round_trip_file() {
        let original = sample_map();
        let path = std::env::temp_dir().join("mapdata_round_trip_test.json");
        save_map(&path, &original).expect("save_map failed");
        let loaded = load_map(&path).expect("load_map failed");
        assert_eq!(original, loaded);
    }

    #[test]
    fn default_flip_omitted_in_json() {
        // When flip_h/flip_v are false (default), they may be omitted from JSON.
        // Deserialization must still produce false for the missing fields.
        let json = r#"{
            "texture_key": "player",
            "width": 16.0,
            "height": 16.0,
            "offset": null,
            "origin": null
        }"#;
        let entry: SpriteEntry = serde_json::from_str(json).unwrap();
        assert!(!entry.flip_h);
        assert!(!entry.flip_v);
    }

    #[test]
    fn empty_map_round_trips() {
        let original = MapData::default();
        let json = serde_json::to_string_pretty(&original).unwrap();
        let loaded: MapData = serde_json::from_str(&json).unwrap();
        assert_eq!(original, loaded);
    }

    #[test]
    fn collider_field_is_optional_in_json() {
        let json = r#"{
            "position": [10.0, 20.0],
            "group": "colliders"
        }"#;
        let entity: EntityDef = serde_json::from_str(json).unwrap();
        assert_eq!(entity.position, Some([10.0, 20.0]));
        assert!(entity.collider.is_none());
    }
}
