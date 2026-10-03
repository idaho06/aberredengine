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

use crate::math::Vec2;
use bevy_ecs::prelude::*;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::particleemitter::{EmitterShape, ParticleEmitter, TtlSpec};
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::sprite::Sprite;
use crate::components::tilemap::TileMap;
use crate::components::tint::Tint;
use crate::components::zindex::ZIndex;
use crate::events::spawnmap::SpawnMapRequested;
use crate::math::Color;
use crate::protocol::render_assets::RenderAssetCmd;
use crate::resources::animationstore::{AnimationResource, AnimationStore};
use crate::resources::mapdata::{
    EntityDef, MapData, ParticleEmitterShapeEntry, ParticleEmitterTtlEntry,
};
use crate::resources::texturefilter::TextureFilter;
use crate::resources::worldsignals::WorldSignals;

/// Load all assets referenced by `map` into the engine stores, then spawn
/// entities. Called by [`spawn_map_observer`]; can also be called directly.
///
/// GL asset loads are not performed here — instead the
/// texture/font entries are translated into [`RenderAssetCmd`]s appended to
/// `render_asset_cmds`, applied later by
/// `crate::systems::render_assets::process_render_asset_cmds`. This keeps
/// `spawn_map` callable from contexts with no live GL access (e.g. tests).
///
/// Returns exactly one entity per `map.entities` entry, in the same order
/// (never skipped, never reordered) -- callers zip the result 1:1 against
/// `map.entities` by index (e.g. the facade's `spawn_map_observer`, which
/// attaches the Lua-gated `LuaSetup`/`LuaOnAnimationEnd` components
/// `aberred-core` cannot name). Any future change that makes this function
/// skip a malformed `EntityDef` instead of always spawning one entity for it
/// would silently break that zip -- see this file's own
/// `spawn_map_queues_render_asset_cmds_and_spawns_entities` test, which
/// asserts the length invariant directly.
#[allow(clippy::too_many_arguments)]
pub fn spawn_map(
    commands: &mut Commands,
    animation_store: &mut AnimationStore,
    map: &MapData,
    world_signals: &mut WorldSignals,
    render_asset_cmds: &mut Vec<RenderAssetCmd>,
) -> Vec<Entity> {
    for entry in &map.textures {
        let filter = TextureFilter::from_opt_str_or_warn(entry.filter.as_deref(), &entry.key);
        render_asset_cmds.push(RenderAssetCmd::Texture {
            id: entry.key.clone(),
            path: entry.path.clone(),
            filter,
        });
    }

    for entry in &map.fonts {
        // "Already loaded" dedup happens in `apply_render_asset_cmd`, gated by
        // `skip_if_loaded: true` -- `spawn_map` does not read `FontStore`
        // directly.
        render_asset_cmds.push(RenderAssetCmd::Font {
            id: entry.key.clone(),
            path: entry.path.clone(),
            size: entry.font_size as i32,
            skip_if_loaded: true,
        });
    }

    for entry in &map.animations {
        let anim = AnimationResource {
            tex_key: Arc::from(entry.texture_key.as_str()),
            position: Vec2 {
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

    spawned.into_iter().map(|(entity, _)| entity).collect()
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
            offset: Vec2 {
                x: s.offset.map(|o| o[0]).unwrap_or(0.0),
                y: s.offset.map(|o| o[1]).unwrap_or(0.0),
            },
            origin: Vec2 {
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
                .with_offset(Vec2::new(offset_x, offset_y))
                .with_origin(Vec2::new(origin_x, origin_y)),
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
    // `def.lua_setup`/`def.on_animation_end` are handled by the facade's own
    // `spawn_map_observer` (under `#[cfg(feature = "lua")]`), which zips
    // `spawn_map`'s returned entities against `map.entities` to attach
    // `LuaSetup`/`LuaOnAnimationEnd` -- `aberred-core` cannot name those
    // Lua-only component types.
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
            let e = world_signals.get_entity(k);
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
        offset: Vec2 { x, y },
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
#[allow(clippy::too_many_arguments)]
pub fn spawn_map_observer(
    trigger: On<SpawnMapRequested>,
    mut commands: Commands,
    mut animation_store: ResMut<AnimationStore>,
    mut world_signals: ResMut<WorldSignals>,
    mut render_asset_cmd_writer: MessageWriter<RenderAssetCmd>,
) {
    let mut render_asset_cmds = Vec::new();
    let _entities = spawn_map(
        &mut commands,
        &mut animation_store,
        &trigger.event().map,
        &mut world_signals,
        &mut render_asset_cmds,
    );
    for cmd in render_asset_cmds {
        render_asset_cmd_writer.write(cmd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::mapdata::BoxColliderEntry;
    use crate::testing::approx_eq;
    use bevy_ecs::world::CommandQueue;

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

    #[test]
    fn spawn_map_queues_render_asset_cmds_and_spawns_entities() {
        use crate::resources::mapdata::{FontEntry, TextureEntry};

        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut animation_store = AnimationStore::default();
        let mut world_signals = WorldSignals::default();
        let mut render_asset_cmds = Vec::new();

        let map = MapData {
            textures: vec![TextureEntry {
                key: "tex1".into(),
                path: "assets/tex1.png".into(),
                filter: None,
            }],
            fonts: vec![FontEntry {
                key: "font1".into(),
                path: "assets/font1.ttf".into(),
                font_size: 24.0,
            }],
            entities: vec![EntityDef {
                position: Some([1.0, 2.0]),
                ..Default::default()
            }],
            ..Default::default()
        };

        let entities = {
            let mut commands = Commands::new(&mut queue, &world);
            spawn_map(
                &mut commands,
                &mut animation_store,
                &map,
                &mut world_signals,
                &mut render_asset_cmds,
            )
        };
        queue.apply(&mut world);

        // One entity per `map.entities` entry -- callers zip the two by
        // index (see spawn_map's doc comment), so this length must hold.
        assert_eq!(entities.len(), map.entities.len());
        assert_eq!(render_asset_cmds.len(), 2);
        match &render_asset_cmds[0] {
            RenderAssetCmd::Texture { id, path, .. } => {
                assert_eq!(id, "tex1");
                assert_eq!(path, "assets/tex1.png");
            }
            other => panic!("expected RenderAssetCmd::Texture, got {other:?}"),
        }
        match &render_asset_cmds[1] {
            RenderAssetCmd::Font {
                id,
                path,
                size,
                skip_if_loaded,
            } => {
                assert_eq!(id, "font1");
                assert_eq!(path, "assets/font1.ttf");
                assert_eq!(*size, 24);
                assert!(
                    skip_if_loaded,
                    "spawn_map fonts should skip if already loaded"
                );
            }
            other => panic!("expected RenderAssetCmd::Font, got {other:?}"),
        }

        let mut position_query = world.query::<&MapPosition>();
        assert_eq!(position_query.iter(&world).count(), 1);
    }

    /// Spawns `map` into a fresh world; returns the world, the spawned entities (1:1 with
    /// `map.entities`), the animation store and the WorldSignals it registered into.
    fn run_spawn_map(map: &MapData) -> (World, Vec<Entity>, AnimationStore, WorldSignals) {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut animation_store = AnimationStore::default();
        let mut world_signals = WorldSignals::default();
        let mut cmds = Vec::new();
        let entities = {
            let mut commands = Commands::new(&mut queue, &world);
            spawn_map(
                &mut commands,
                &mut animation_store,
                map,
                &mut world_signals,
                &mut cmds,
            )
        };
        queue.apply(&mut world);
        (world, entities, animation_store, world_signals)
    }

    #[test]
    fn spawn_map_registers_animations_in_the_store() {
        use crate::resources::mapdata::AnimationEntry;
        let map = MapData {
            animations: vec![AnimationEntry {
                key: "walk".into(),
                texture_key: "hero".into(),
                position: [16.0, 32.0],
                horizontal_displacement: 16.0,
                vertical_displacement: 8.0,
                frame_count: 4,
                fps: 12.0,
                looping: true,
            }],
            ..Default::default()
        };
        let (_, _, store, _) = run_spawn_map(&map);
        let anim = store.animations.get("walk").expect("registered");
        assert_eq!(&*anim.tex_key, "hero");
        assert_eq!(anim.position, Vec2::new(16.0, 32.0));
        assert_eq!(
            (anim.horizontal_displacement, anim.vertical_displacement),
            (16.0, 8.0)
        );
        assert_eq!((anim.frame_count, anim.fps, anim.looped), (4, 12.0, true));
    }

    #[test]
    fn entity_def_fields_become_components() {
        use crate::resources::mapdata::{DynamicTextEntry, SpriteEntry};
        let map = MapData {
            entities: vec![
                EntityDef {
                    position: Some([1.0, 2.0]),
                    sprite: Some(SpriteEntry {
                        texture_key: "hero".into(),
                        width: 16.0,
                        height: 24.0,
                        offset: Some([32.0, 0.0]),
                        origin: Some([8.0, 12.0]),
                        flip_h: true,
                        flip_v: false,
                    }),
                    group: Some("player".into()),
                    z_index: Some(3.0),
                    rotation_deg: Some(45.0),
                    scale: Some([2.0, 3.0]),
                    tint: Some([1, 2, 3, 4]),
                    animation_key: Some("walk".into()),
                    registered_as: Some("hero".into()),
                    ..Default::default()
                },
                EntityDef {
                    sprite: Some(SpriteEntry {
                        texture_key: "plain".into(),
                        width: 8.0,
                        height: 8.0,
                        offset: None,
                        origin: None,
                        flip_h: false,
                        flip_v: false,
                    }),
                    dynamic_text: Some(DynamicTextEntry {
                        text: "Score".into(),
                        font_key: "arcade".into(),
                        font_size: 12.0,
                        color: [5, 6, 7, 8],
                    }),
                    tilemap_path: Some("assets/tilemaps/x".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (world, entities, _, signals) = run_spawn_map(&map);
        let (hero, other) = (entities[0], entities[1]);

        assert_eq!(
            world.get::<MapPosition>(hero).unwrap().pos,
            Vec2::new(1.0, 2.0)
        );
        let s = world.get::<Sprite>(hero).unwrap();
        assert_eq!((&*s.tex_key, s.width, s.height), ("hero", 16.0, 24.0));
        assert_eq!(
            (s.offset, s.origin),
            (Vec2::new(32.0, 0.0), Vec2::new(8.0, 12.0))
        );
        assert_eq!((s.flip_h, s.flip_v), (true, false));
        assert_eq!(world.get::<Group>(hero).unwrap().0, "player");
        assert_eq!(world.get::<ZIndex>(hero).unwrap().0, 3.0);
        assert_eq!(world.get::<Rotation>(hero).unwrap().degrees, 45.0);
        assert_eq!(world.get::<Scale>(hero).unwrap().scale, Vec2::new(2.0, 3.0));
        assert_eq!(
            world.get::<Tint>(hero).unwrap().color,
            Color::new(1, 2, 3, 4)
        );
        let anim = world.get::<Animation>(hero).unwrap();
        assert_eq!((anim.animation_key.as_str(), anim.frame_index), ("walk", 0));
        assert_eq!(signals.get_entity("hero"), Some(hero));

        let s = world.get::<Sprite>(other).unwrap();
        assert_eq!(
            (s.offset, s.origin),
            (Vec2::ZERO, Vec2::ZERO),
            "omitted offset/origin default to 0"
        );
        let text = world.get::<DynamicText>(other).unwrap();
        assert_eq!(
            (&*text.text, &*text.font, text.font_size),
            ("Score", "arcade", 12.0)
        );
        assert_eq!(text.color, Color::new(5, 6, 7, 8));
        assert_eq!(
            world.get::<TileMap>(other).unwrap().path,
            "assets/tilemaps/x"
        );
        assert!(world.get::<MapPosition>(other).is_none());
        assert!(world.get::<Group>(other).is_none());
    }

    #[test]
    fn particle_emitter_resolves_templates_registered_later_in_the_map() {
        use crate::resources::mapdata::{
            ParticleEmitterEntry, ParticleEmitterShapeEntry, ParticleEmitterTtlEntry,
        };
        let emitter = |ttl| ParticleEmitterEntry {
            template_keys: vec!["spark".into(), "missing".into()],
            shape: ParticleEmitterShapeEntry::Rect {
                width: 8.0,
                height: 4.0,
            },
            offset: None,
            particles_per_emission: 3,
            emissions_per_second: 20.0,
            emissions_remaining: 5,
            arc_degrees: [90.0, 10.0],
            speed_range: [200.0, 100.0],
            ttl,
        };
        let map = MapData {
            entities: vec![
                // The emitter comes BEFORE its template in the file.
                EntityDef {
                    position: Some([0.0, 0.0]),
                    particle_emitter: Some(emitter(ParticleEmitterTtlEntry::Fixed { value: 1.5 })),
                    ..Default::default()
                },
                EntityDef {
                    particle_emitter: Some(emitter(ParticleEmitterTtlEntry::Range {
                        min: 1.0,
                        max: 2.0,
                    })),
                    ..Default::default()
                },
                EntityDef {
                    registered_as: Some("spark".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (world, entities, _, _) = run_spawn_map(&map);
        let spark = entities[2];

        let e = world.get::<ParticleEmitter>(entities[0]).unwrap();
        assert_eq!(
            e.templates,
            [spark],
            "later template resolved; unknown key dropped"
        );
        assert!(matches!(
            e.shape,
            EmitterShape::Rect {
                width: 8.0,
                height: 4.0
            }
        ));
        assert_eq!(e.offset, Vec2::ZERO);
        assert_eq!(
            (e.particles_per_emission, e.emissions_per_second),
            (3, 20.0)
        );
        assert_eq!(
            (e.emissions_remaining, e.initial_emissions_remaining),
            (5, 5)
        );
        assert_eq!(
            (e.arc_degrees, e.speed_range),
            ((10.0, 90.0), (100.0, 200.0)),
            "ranges normalized"
        );
        assert!(matches!(e.ttl, TtlSpec::Fixed(v) if v == 1.5));
        assert!(matches!(
            world.get::<ParticleEmitter>(entities[1]).unwrap().ttl,
            TtlSpec::Range { min, max } if min == 1.0 && max == 2.0
        ));
    }

    #[test]
    fn spawn_map_observer_spawns_and_forwards_render_asset_cmds() {
        use crate::resources::mapdata::TextureEntry;
        use bevy_ecs::message::Messages;
        let mut world = World::new();
        world.init_resource::<AnimationStore>();
        world.init_resource::<WorldSignals>();
        world.init_resource::<Messages<RenderAssetCmd>>();
        world.add_observer(spawn_map_observer);

        world.trigger(SpawnMapRequested {
            map: MapData {
                textures: vec![TextureEntry {
                    key: "tex".into(),
                    path: "assets/tex.png".into(),
                    filter: Some("bilinear".into()),
                }],
                entities: vec![EntityDef {
                    position: Some([4.0, 5.0]),
                    registered_as: Some("thing".into()),
                    ..Default::default()
                }],
                ..Default::default()
            },
        });
        world.flush();

        let cmds: Vec<RenderAssetCmd> = world
            .resource_mut::<Messages<RenderAssetCmd>>()
            .drain()
            .collect();
        assert!(matches!(
            cmds.as_slice(),
            [RenderAssetCmd::Texture { id, filter: TextureFilter::Bilinear, .. }] if id == "tex"
        ));
        let thing = world
            .resource::<WorldSignals>()
            .get_entity("thing")
            .unwrap();
        assert_eq!(
            world.get::<MapPosition>(thing).unwrap().pos,
            Vec2::new(4.0, 5.0)
        );
    }
}
