//! Map spawning: load assets and instantiate entities from [`MapData`].
//!
//! The entry point for runtime map loading is [`spawn_map_observer`], a
//! persistent Bevy observer registered automatically by the engine. It fires
//! whenever a [`crate::events::spawnmap::SpawnMapRequested`] event is
//! triggered and delegates to [`spawn_map`].
//!
//! [`spawn_map`] is also available as a free function for use cases that need
//! fine-grained control (e.g. editor preview systems that already hold the
//! required system params).

use std::sync::Arc;

use bevy_ecs::prelude::*;
use raylib::ffi;
use raylib::ffi::TextureFilter::TEXTURE_FILTER_ANISOTROPIC_8X;
use raylib::prelude::*;

use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::sprite::Sprite;
use crate::components::tint::Tint;
use crate::components::zindex::ZIndex;
use crate::events::spawnmap::SpawnMapRequested;
use crate::resources::animationstore::{AnimationResource, AnimationStore};
use crate::resources::fontstore::FontStore;
use crate::resources::mapdata::{EntityDef, MapData, load_map};
use crate::resources::texturestore::TextureStore;
use crate::resources::worldsignals::WorldSignals;
use crate::components::tilemap::TileMap;
use crate::systems::RaylibAccess;
#[cfg(feature = "lua")]
use crate::resources::lua_runtime::{LuaRuntime, MapLuaCmd};

/// Load all assets referenced by `map` into the engine stores, then spawn
/// entities. Called by [`spawn_map_observer`]; can also be called directly.
pub fn spawn_map(
    commands: &mut Commands,
    raylib: &mut RaylibAccess,
    texture_store: &mut TextureStore,
    font_store: &mut FontStore,
    animation_store: &mut AnimationStore,
    map: &MapData,
    world_signals: &mut WorldSignals,
) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);

    for entry in &map.textures {
        match rl.load_texture(th, &entry.path) {
            Ok(tex) => {
                texture_store.insert(&entry.key, tex);
            }
            Err(e) => {
                log::warn!("spawn_map: failed to load texture '{}': {e}", entry.path);
            }
        }
    }

    for entry in &map.fonts {
        let font = load_font_with_mipmaps(rl, th, &entry.path, entry.font_size as i32);
        font_store.add(&entry.key, font);
    }

    for entry in &map.animations {
        let anim = AnimationResource {
            tex_key: Arc::from(entry.texture_key.as_str()),
            position: Vector2 {
                x: entry.position[0],
                y: entry.position[1],
            },
            horizontal_displacement: entry.horizontal_displacement,
            vertical_displacement: entry.vertical_displacement,
            frame_count: entry.frame_count as usize,
            fps: entry.fps,
            looped: entry.looping,
        };
        animation_store.insert(&entry.key, anim);
    }

    for def in &map.entities {
        let entity = spawn_entity(commands, def);
        if let Some(ref key) = def.registered_as {
            world_signals.set_entity(key.clone(), entity);
        }
    }
}

fn spawn_entity(commands: &mut Commands, def: &EntityDef) -> Entity {
    let mut ec = commands.spawn_empty();

    if let Some([x, y]) = def.position {
        ec.insert(MapPosition::new(x, y));
    }
    if let Some(ref s) = def.sprite {
        ec.insert(Sprite {
            tex_key: Arc::from(s.texture_key.as_str()),
            width: s.width,
            height: s.height,
            offset: Vector2 {
                x: s.offset.map(|o| o[0]).unwrap_or(0.0),
                y: s.offset.map(|o| o[1]).unwrap_or(0.0),
            },
            origin: Vector2 {
                x: s.origin.map(|o| o[0]).unwrap_or(0.0),
                y: s.origin.map(|o| o[1]).unwrap_or(0.0),
            },
            flip_h: s.flip_h,
            flip_v: s.flip_v,
        });
    }
    if let Some(ref g) = def.group {
        ec.insert(Group::new(g));
    }
    if let Some(z) = def.z_index {
        ec.insert(ZIndex(z));
    }
    if let Some(deg) = def.rotation_deg {
        ec.insert(Rotation { degrees: deg });
    }
    if let Some([sx, sy]) = def.scale {
        ec.insert(Scale::new(sx, sy));
    }
    if let Some(ref p) = def.tilemap_path {
        ec.insert(TileMap::new(p));
    }
    if let Some([r, g, b, a]) = def.tint {
        ec.insert(Tint::new(r, g, b, a));
    }
    ec.id()
}

/// Bevy observer registered by the engine. Fires on
/// [`SpawnMapRequested`] and delegates to [`spawn_map`].
pub fn spawn_map_observer(
    trigger: On<SpawnMapRequested>,
    mut commands: Commands,
    mut raylib: RaylibAccess,
    mut texture_store: ResMut<TextureStore>,
    mut font_store: NonSendMut<FontStore>,
    mut animation_store: ResMut<AnimationStore>,
    mut world_signals: ResMut<WorldSignals>,
) {
    spawn_map(
        &mut commands,
        &mut raylib,
        &mut texture_store,
        &mut font_store,
        &mut animation_store,
        &trigger.event().map,
        &mut world_signals,
    );
}

/// Drains `engine.load_map()` commands queued by Lua and fires
/// [`SpawnMapRequested`] for each, letting [`spawn_map_observer`] handle the
/// Raylib-dependent asset loading and entity spawning.
///
/// Registered by [`crate::engine_app::EngineBuilder::with_lua`] and runs
/// every frame during the Playing state, after `lua_plugin::update`.
#[cfg(feature = "lua")]
pub fn process_lua_map_commands(
    mut commands: Commands,
    lua: NonSend<LuaRuntime>,
) {
    for cmd in lua.drain_map_commands() {
        match cmd {
            MapLuaCmd::LoadMap { path } => match load_map(&path) {
                Ok(map) => commands.trigger(SpawnMapRequested { map }),
                Err(e) => log::error!("engine.load_map: failed to read '{path}': {e}"),
            },
        }
    }
}

/// Load a font with mipmaps and anisotropic filtering.
///
/// Panics if the font file cannot be opened (consistent with the engine's
/// behaviour for missing assets in setup / load phases).
///
/// `pub(crate)` so that `lua_plugin` can reuse this rather than duplicating it.
pub(crate) fn load_font_with_mipmaps(rl: &mut RaylibHandle, th: &RaylibThread, path: &str, size: i32) -> Font {
    let mut font = rl
        .load_font_ex(th, path, size, None)
        .unwrap_or_else(|_| panic!("spawn_map: failed to load font '{path}'"));
    unsafe {
        ffi::GenTextureMipmaps(&mut font.texture);
        ffi::SetTextureFilter(font.texture, TEXTURE_FILTER_ANISOTROPIC_8X as i32);
    }
    font
}
