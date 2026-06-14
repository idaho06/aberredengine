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

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::group::Group;
#[cfg(feature = "lua")]
use crate::components::luasetup::LuaSetup;
use crate::components::mapposition::MapPosition;
use crate::components::particleemitter::{EmitterShape, ParticleEmitter, TtlSpec};
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::sprite::Sprite;
use crate::components::tilemap::TileMap;
use crate::components::tint::Tint;
use crate::components::zindex::ZIndex;
use crate::events::spawnmap::SpawnMapRequested;
use crate::resources::animationstore::{AnimationResource, AnimationStore};
use crate::resources::fontstore::FontStore;
#[cfg(feature = "lua")]
use crate::resources::lua_runtime::{LuaRuntime, MapLuaCmd};
#[cfg(feature = "lua")]
use crate::resources::mapdata::load_map;
use crate::resources::mapdata::{
    EntityDef, MapData, ParticleEmitterShapeEntry, ParticleEmitterTtlEntry,
};
use crate::resources::texturefilter::TextureFilter;
use crate::resources::texturestore::TextureStore;
use crate::resources::worldsignals::WorldSignals;
use crate::systems::RaylibAccess;

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
                let filter =
                    TextureFilter::from_opt_str_or_warn(entry.filter.as_deref(), &entry.key);
                texture_store.insert(&entry.key, tex, filter);
            }
            Err(e) => {
                log::warn!("spawn_map: failed to load texture '{}': {e}", entry.path);
            }
        }
    }

    for entry in &map.fonts {
        if font_store.meta.contains_key(&entry.key) {
            continue;
        }
        match load_font_with_mipmaps(rl, th, &entry.path, entry.font_size as i32) {
            Ok(font) => {
                font_store.add_with_meta(&entry.key, font, entry.path.clone(), entry.font_size);
            }
            Err(err) => {
                log::warn!(
                    "spawn_map: failed to load font '{}' (key='{}'): {err}",
                    entry.path,
                    entry.key
                );
            }
        }
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

    // Pass 1: spawn all entities and register WorldSignals keys so that
    // ParticleEmitter template resolution in pass 2 can find all entities.
    let spawned: Vec<(Entity, &EntityDef)> = map
        .entities
        .iter()
        .map(|def| {
            let entity = spawn_entity(commands, def);
            if let Some(ref key) = def.registered_as {
                world_signals.set_entity(key.clone(), entity);
            }
            (entity, def)
        })
        .collect();

    // Pass 2: insert ParticleEmitter components with resolved template keys.
    for (entity, def) in &spawned {
        if let Some(ref entry) = def.particle_emitter {
            insert_particle_emitter(commands.entity(*entity), world_signals, entry);
        }
    }
}

fn spawn_entity(commands: &mut Commands, def: &EntityDef) -> Entity {
    let mut ec = commands.spawn_empty();
    let entity = ec.id();

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
    if let Some(ref collider) = def.collider {
        let offset_x = collider.offset.map(|o| o[0]).unwrap_or(0.0);
        let offset_y = collider.offset.map(|o| o[1]).unwrap_or(0.0);
        let origin_x = collider.origin.map(|o| o[0]).unwrap_or(0.0);
        let origin_y = collider.origin.map(|o| o[1]).unwrap_or(0.0);
        log::debug!(
            "spawn_entity: inserting BoxCollider on entity {} size=({:.3}, {:.3}) offset=({:.3}, {:.3}) origin=({:.3}, {:.3}) group={:?} registered_as={:?}",
            entity.to_bits(),
            collider.size[0],
            collider.size[1],
            offset_x,
            offset_y,
            origin_x,
            origin_y,
            def.group.as_deref(),
            def.registered_as.as_deref(),
        );
        ec.insert(
            BoxCollider::new(collider.size[0], collider.size[1])
                .with_offset(Vector2::new(offset_x, offset_y))
                .with_origin(Vector2::new(origin_x, origin_y)),
        );
        if def.position.is_none() {
            log::warn!(
                "spawn_entity: entity {} has BoxCollider but no position — excluded from collision detection (collision_detector requires MapPosition)",
                entity.to_bits(),
            );
        }
        if def.group.is_none() {
            log::warn!(
                "spawn_entity: entity {} has BoxCollider but no group — collision callbacks will never fire (resolve_groups returns None without a Group component)",
                entity.to_bits(),
            );
        }
    }
    if let Some(ref text) = def.dynamic_text {
        ec.insert(DynamicText::new(
            text.text.as_str(),
            text.font_key.as_str(),
            text.font_size,
            Color::new(text.color[0], text.color[1], text.color[2], text.color[3]),
        ));
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
    #[cfg(feature = "lua")]
    if let Some(ref callback) = def.lua_setup {
        ec.insert(LuaSetup::new(callback.clone()));
    }
    #[cfg(feature = "lua")]
    if let Some(ref callback) = def.on_animation_end {
        use crate::components::lua_on_animation_end::LuaOnAnimationEnd;
        ec.insert(LuaOnAnimationEnd::new(callback.clone()));
    }
    if let Some(ref key) = def.animation_key {
        ec.insert(Animation {
            animation_key: key.clone(),
            frame_index: 0,
            elapsed_time: 0.0,
            finished: false,
        });
    }
    entity
}

/// Insert a [`ParticleEmitter`] component by resolving template keys from
/// `WorldSignals`. Called during pass 2 of [`spawn_map`].
fn insert_particle_emitter(
    mut entity_commands: EntityCommands<'_>,
    world_signals: &WorldSignals,
    entry: &crate::resources::mapdata::ParticleEmitterEntry,
) {
    let templates: Vec<Entity> = entry
        .template_keys
        .iter()
        .filter_map(|k| {
            let e = world_signals.get_entity(k).copied();
            if e.is_none() {
                log::warn!(
                    "insert_particle_emitter: template key '{}' not found in WorldSignals; ignoring",
                    k
                );
            }
            e
        })
        .collect();

    if templates.is_empty() && !entry.template_keys.is_empty() {
        log::warn!("insert_particle_emitter: no templates resolved — emitter will not emit");
    }

    let shape = match &entry.shape {
        ParticleEmitterShapeEntry::Point => EmitterShape::Point,
        ParticleEmitterShapeEntry::Rect { width, height } => EmitterShape::Rect {
            width: *width,
            height: *height,
        },
    };

    let ttl = match &entry.ttl {
        ParticleEmitterTtlEntry::None => TtlSpec::None,
        ParticleEmitterTtlEntry::Fixed { value: v } => TtlSpec::Fixed(*v),
        ParticleEmitterTtlEntry::Range { min, max } => TtlSpec::Range {
            min: *min,
            max: *max,
        },
    };

    let [a, b] = entry.arc_degrees;
    let arc_degrees = (a.min(b), a.max(b));

    let [a, b] = entry.speed_range;
    let speed_range = (a.min(b), a.max(b));

    let [x, y] = entry.offset.unwrap_or([0.0, 0.0]);

    entity_commands.insert(ParticleEmitter {
        templates,
        shape,
        offset: raylib::math::Vector2 { x, y },
        particles_per_emission: entry.particles_per_emission,
        emissions_per_second: entry.emissions_per_second,
        emissions_remaining: entry.emissions_remaining,
        initial_emissions_remaining: entry.emissions_remaining,
        arc_degrees,
        speed_range,
        ttl,
        time_since_emit: 0.0,
    });
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
    mut buf: Local<Vec<MapLuaCmd>>,
) {
    lua.drain_map_commands_into(&mut buf);
    for cmd in buf.drain(..) {
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
/// `pub(crate)` so that `lua_plugin` can reuse this rather than duplicating it.
pub fn load_font_with_mipmaps(
    rl: &mut RaylibHandle,
    th: &RaylibThread,
    path: &str,
    size: i32,
) -> Result<Font, String> {
    let mut font = rl
        .load_font_ex(th, path, size, None)
        .map_err(|err| format!("Failed to load font '{path}': {err}"))?;
    unsafe {
        ffi::GenTextureMipmaps(&mut font.texture);
        ffi::SetTextureFilter(font.texture, TEXTURE_FILTER_ANISOTROPIC_8X as i32);
    }
    Ok(font)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::mapdata::BoxColliderEntry;
    use bevy_ecs::world::CommandQueue;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < f32::EPSILON
    }

    #[test]
    fn spawn_entity_inserts_box_collider_from_mapdata() {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let entity = {
            let mut commands = Commands::new(&mut queue, &world);
            let entity_def = EntityDef {
                position: Some([10.0, 20.0]),
                collider: Some(BoxColliderEntry {
                    size: [32.0, 48.0],
                    offset: Some([3.0, 4.0]),
                    origin: Some([5.0, 6.0]),
                }),
                ..Default::default()
            };
            spawn_entity(&mut commands, &entity_def)
        };
        queue.apply(&mut world);

        let collider = world.get::<BoxCollider>(entity).unwrap();
        assert!(approx_eq(collider.size.x, 32.0));
        assert!(approx_eq(collider.size.y, 48.0));
        assert!(approx_eq(collider.offset.x, 3.0));
        assert!(approx_eq(collider.offset.y, 4.0));
        assert!(approx_eq(collider.origin.x, 5.0));
        assert!(approx_eq(collider.origin.y, 6.0));
    }
}
