//! Integration tests for the parent-child entity transform hierarchy.
//!
//! Tests are organized by implementation phase.
//!
//! # Usage
//!
//! ```sh
//! cargo test --test hierarchy_integration
//! ```

use bevy_ecs::system::RunSystemOnce;
use std::sync::Arc;

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;
use aberred_core::math::Vec2;

use aberred_core::components::globaltransform2d::GlobalTransform2D;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::components::stuckto::StuckTo;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_lua::resources::lua_runtime::{EntityCmd, SpawnCmd};
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_lua::systems::lua_commands::EntityCmdQueries;
use aberred_lua::systems::lua_commands::{process_entity_commands, process_spawn_command};
use aberred_core::systems::propagate_transforms::propagate_transforms;
use aberred_core::systems::stuckto::stuck_to_entity_system;
use aberred_core::testing::approx_eq;

fn tick_propagate(world: &mut World) {
    world
        .run_system_once(propagate_transforms)
        .expect("propagate_transforms should run");
}

// =============================================================================
// PHASE 2: EntityCmd SetParent/RemoveParent + Lua API
// =============================================================================

/// Helper to run process_entity_commands using SystemState.
fn run_entity_cmds(world: &mut World, cmds: Vec<EntityCmd>) {
    world.insert_resource(SystemsStore::new());
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });
    if !world.contains_resource::<WorldSignals>() {
        world.insert_resource(WorldSignals::default());
    }

    let mut state = SystemState::<(
        Commands,
        EntityCmdQueries,
        ResMut<WorldSignals>,
        Res<SystemsStore>,
        Res<AnimationStore>,
    )>::new(world);
    let (mut commands, mut queries, mut world_signals, systems_store, anim_store) = state
        .get_mut(world)
        .expect("Hierarchy test params should fetch");

    process_entity_commands(
        &mut commands,
        cmds,
        &mut world_signals,
        &mut queries,
        &systems_store,
        &anim_store,
    );

    state.apply(world);
}

#[test]
fn entity_cmd_set_parent_inserts_childof() {
    let mut world = World::new();

    let parent = world.spawn((MapPosition::new(100.0, 100.0),)).id();
    let child = world.spawn((MapPosition::new(0.0, 0.0),)).id();

    run_entity_cmds(
        &mut world,
        vec![EntityCmd::SetParent {
            entity_id: child.to_bits(),
            parent_id: parent.to_bits(),
        }],
    );

    // Child should have ChildOf pointing to parent
    assert!(
        world.get::<ChildOf>(child).is_some(),
        "Child should have ChildOf after SetParent"
    );

    // Child should have GlobalTransform2D
    assert!(
        world.get::<GlobalTransform2D>(child).is_some(),
        "Child should have GlobalTransform2D after SetParent"
    );

    // Parent should also have GlobalTransform2D (auto-inserted)
    assert!(
        world.get::<GlobalTransform2D>(parent).is_some(),
        "Parent should have GlobalTransform2D after SetParent"
    );
}

#[test]
fn entity_cmd_remove_parent_snaps_to_world_position() {
    let mut world = World::new();

    let parent = world
        .spawn((
            MapPosition::new(100.0, 100.0),
            Rotation { degrees: 90.0 },
            Scale::new(2.0, 2.0),
            GlobalTransform2D::default(),
        ))
        .id();

    let child = world
        .spawn((
            MapPosition::new(40.0, 0.0),
            ChildOf(parent),
            GlobalTransform2D::default(),
        ))
        .id();

    world.flush();

    // Run propagation to compute child's world transform
    tick_propagate(&mut world);

    // Verify child has a computed GlobalTransform2D
    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    // Offset (40, 0) scaled by (2, 2) => (80, 0), rotated 90deg => (0, 80)
    // World pos = (100 + 0, 100 + 80) = (100, 180)
    assert!(
        approx_eq(gt.position.x, 100.0),
        "Before RemoveParent, child world X: expected 100, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 180.0),
        "Before RemoveParent, child world Y: expected 180, got {}",
        gt.position.y
    );

    // Now remove the parent
    run_entity_cmds(
        &mut world,
        vec![EntityCmd::RemoveParent {
            entity_id: child.to_bits(),
        }],
    );

    // Child should no longer have ChildOf
    assert!(
        world.get::<ChildOf>(child).is_none(),
        "Child should not have ChildOf after RemoveParent"
    );

    // Child should no longer have GlobalTransform2D
    assert!(
        world.get::<GlobalTransform2D>(child).is_none(),
        "Child should not have GlobalTransform2D after RemoveParent"
    );

    // MapPosition should be snapped to the world position
    let pos = world.get::<MapPosition>(child).unwrap();
    assert!(
        approx_eq(pos.pos.x, 100.0),
        "After RemoveParent, child pos X: expected 100, got {}",
        pos.pos.x
    );
    assert!(
        approx_eq(pos.pos.y, 180.0),
        "After RemoveParent, child pos Y: expected 180, got {}",
        pos.pos.y
    );

    // Rotation should be snapped to world rotation
    let rot = world.get::<Rotation>(child).unwrap();
    assert!(
        approx_eq(rot.degrees, 90.0),
        "After RemoveParent, child rotation: expected 90, got {}",
        rot.degrees
    );

    // Scale should be snapped to world scale
    let scale = world.get::<Scale>(child).unwrap();
    assert!(
        approx_eq(scale.scale.x, 2.0),
        "After RemoveParent, child scale X: expected 2, got {}",
        scale.scale.x
    );
}

/// P0-1: an EntityCmd with invalid entity bits (the EntityIndex niche value)
/// must be skipped with a warn, not panic during processing.
#[test]
fn entity_cmd_invalid_entity_id_does_not_panic() {
    let mut world = World::new();

    // The low 32 bits of Entity::to_bits() encode EntityIndex via NonMaxU32's
    // niche transmute, where a raw value of 0 corresponds to the excluded
    // index -> Entity::try_from_bits(0) returns None. A bare 0 is also a
    // plausible malformed id from Lua (e.g. an uninitialized/nil entity ref).
    let invalid_id = 0u64;
    assert_eq!(Entity::try_from_bits(invalid_id), None);

    run_entity_cmds(
        &mut world,
        vec![EntityCmd::SetRotation {
            entity_id: invalid_id,
            degrees: 45.0,
        }],
    );
}

/// P0-2: Despawn followed by an insert-based command on the same entity in
/// the same drained batch must not panic at command-apply time.
#[test]
fn entity_cmd_despawn_then_set_rotation_same_batch_does_not_panic() {
    let mut world = World::new();

    let e = world
        .spawn((MapPosition::new(0.0, 0.0), Rotation { degrees: 0.0 }))
        .id();

    run_entity_cmds(
        &mut world,
        vec![
            EntityCmd::Despawn {
                entity_id: e.to_bits(),
            },
            EntityCmd::SetRotation {
                entity_id: e.to_bits(),
                degrees: 90.0,
            },
        ],
    );

    assert!(world.get_entity(e).is_err(), "entity should be despawned");
    assert!(world.get::<Rotation>(e).is_none());
}

#[test]
fn entity_cmd_set_parent_multiple_children() {
    let mut world = World::new();

    let parent = world.spawn((MapPosition::new(0.0, 0.0),)).id();
    let child1 = world.spawn((MapPosition::new(10.0, 0.0),)).id();
    let child2 = world.spawn((MapPosition::new(20.0, 0.0),)).id();
    let child3 = world.spawn((MapPosition::new(30.0, 0.0),)).id();

    run_entity_cmds(
        &mut world,
        vec![
            EntityCmd::SetParent {
                entity_id: child1.to_bits(),
                parent_id: parent.to_bits(),
            },
            EntityCmd::SetParent {
                entity_id: child2.to_bits(),
                parent_id: parent.to_bits(),
            },
            EntityCmd::SetParent {
                entity_id: child3.to_bits(),
                parent_id: parent.to_bits(),
            },
        ],
    );

    // All children should have ChildOf
    assert!(world.get::<ChildOf>(child1).is_some());
    assert!(world.get::<ChildOf>(child2).is_some());
    assert!(world.get::<ChildOf>(child3).is_some());

    // Parent should have Children with 3 entries (auto-populated by Bevy)
    let children = world.get::<Children>(parent);
    assert!(children.is_some(), "Parent should have Children component");
    assert_eq!(children.unwrap().len(), 3, "Parent should have 3 children");
}

// =============================================================================
// PHASE 3: Builder with_parent (SpawnCmd.parent field)
// =============================================================================

/// When the parent already has a GlobalTransform2D (the normal case — parent
/// has existed for at least one frame), the child should receive the correct
/// world-space GlobalTransform2D immediately on the same frame it is spawned,
/// with no propagation ticks required.
#[test]
fn spawn_cmd_with_parent_applies_childof() {
    let mut world = World::new();
    world.insert_resource(WorldSignals::default());

    // Spawn parent with an already-computed GlobalTransform2D, as would be
    // the case in real gameplay (parent existed for at least one frame).
    let parent = world
        .spawn((
            MapPosition::new(100.0, 50.0),
            GlobalTransform2D {
                position: Vec2 { x: 100.0, y: 50.0 },
                rotation_degrees: 0.0,
                scale: Vec2 { x: 1.0, y: 1.0 },
            },
        ))
        .id();

    // Build a SpawnCmd with parent set
    let cmd = SpawnCmd {
        position: Some((10.0, 0.0)),
        parent: Some(parent.to_bits()),
        ..SpawnCmd::default()
    };

    // Process the spawn command via SystemState
    let mut state = SystemState::<(Commands, ResMut<WorldSignals>)>::new(&mut world);
    let (mut commands, mut world_signals) = state
        .get_mut(&mut world)
        .expect("Hierarchy test params should fetch");
    process_spawn_command(&mut commands, Box::new(cmd), &mut world_signals);
    state.apply(&mut world);

    // Find the spawned child (entity that has ChildOf)
    let mut child_entity = None;
    let mut query = world.query::<(Entity, &ChildOf)>();
    for (entity, child_of) in query.iter(&world) {
        if child_of.0 == parent {
            child_entity = Some(entity);
        }
    }

    let child = child_entity.expect("Spawned entity should have ChildOf pointing to parent");

    // Child should have the correct world-space GlobalTransform2D immediately
    // after spawn — ComputeInitialGlobalTransform ran during state.apply().
    let gt = world
        .get::<GlobalTransform2D>(child)
        .expect("Spawned child should have GlobalTransform2D");
    assert!(
        approx_eq(gt.position.x, 110.0),
        "Child world X immediately after spawn: expected 110, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 50.0),
        "Child world Y immediately after spawn: expected 50, got {}",
        gt.position.y
    );

    // Child's local MapPosition should be unchanged
    let pos = world.get::<MapPosition>(child).unwrap();
    assert!(
        approx_eq(pos.pos.x, 10.0),
        "Child local pos X: expected 10, got {}",
        pos.pos.x
    );

    // After a propagation tick the world transform should still be correct.
    tick_propagate(&mut world);

    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    assert!(
        approx_eq(gt.position.x, 110.0),
        "Child world X after propagation: expected 110, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 50.0),
        "Child world Y after propagation: expected 50, got {}",
        gt.position.y
    );
}

/// When a standalone parent has no GlobalTransform2D yet, the initial child
/// world transform can still be synthesized from the parent's local transform.
#[test]
fn spawn_cmd_child_without_parent_gt_uses_parent_local_transform_immediately() {
    let mut world = World::new();
    world.insert_resource(WorldSignals::default());

    // Spawn a standalone parent without GlobalTransform2D.
    let parent = world.spawn((MapPosition::new(100.0, 50.0),)).id();

    let cmd = SpawnCmd {
        position: Some((10.0, 0.0)),
        parent: Some(parent.to_bits()),
        ..SpawnCmd::default()
    };

    let mut state = SystemState::<(Commands, ResMut<WorldSignals>)>::new(&mut world);
    let (mut commands, mut world_signals) = state
        .get_mut(&mut world)
        .expect("Hierarchy test params should fetch");
    process_spawn_command(&mut commands, Box::new(cmd), &mut world_signals);
    state.apply(&mut world);

    let mut child_entity = None;
    let mut query = world.query::<(Entity, &ChildOf)>();
    for (entity, child_of) in query.iter(&world) {
        if child_of.0 == parent {
            child_entity = Some(entity);
        }
    }
    let child = child_entity.expect("Spawned entity should have ChildOf pointing to parent");

    // Child should get the correct world-space transform immediately.
    let gt = world
        .get::<GlobalTransform2D>(child)
        .expect("Child should have GlobalTransform2D synthesized from parent local transform");
    assert!(
        approx_eq(gt.position.x, 110.0),
        "Child world X immediately after spawn: expected 110, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 50.0),
        "Child world Y immediately after spawn: expected 50, got {}",
        gt.position.y
    );

    // After one propagation tick the result should remain correct.
    tick_propagate(&mut world);

    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    assert!(
        approx_eq(gt.position.x, 110.0),
        "Child world X after propagation: expected 110, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 50.0),
        "Child world Y after propagation: expected 50, got {}",
        gt.position.y
    );
}

/// When the parent is itself a child and lacks GlobalTransform2D, the initial
/// child world transform remains unresolved until propagation runs.
#[test]
fn spawn_cmd_child_without_parent_gt_defers_when_parent_is_nested() {
    let mut world = World::new();
    world.insert_resource(WorldSignals::default());

    let grandparent = world
        .spawn((
            MapPosition::new(100.0, 50.0),
            GlobalTransform2D {
                position: Vec2 { x: 100.0, y: 50.0 },
                rotation_degrees: 0.0,
                scale: Vec2 { x: 1.0, y: 1.0 },
            },
        ))
        .id();
    let parent = world
        .spawn((MapPosition::new(10.0, 0.0), ChildOf(grandparent)))
        .id();
    world.flush();

    let cmd = SpawnCmd {
        position: Some((5.0, 0.0)),
        parent: Some(parent.to_bits()),
        ..SpawnCmd::default()
    };

    let mut state = SystemState::<(Commands, ResMut<WorldSignals>)>::new(&mut world);
    let (mut commands, mut world_signals) = state
        .get_mut(&mut world)
        .expect("Hierarchy test params should fetch");
    process_spawn_command(&mut commands, Box::new(cmd), &mut world_signals);
    state.apply(&mut world);

    let mut child_entity = None;
    let mut query = world.query::<(Entity, &ChildOf)>();
    for (entity, child_of) in query.iter(&world) {
        if child_of.0 == parent {
            child_entity = Some(entity);
        }
    }
    let child = child_entity.expect("Spawned entity should have ChildOf pointing to parent");

    assert!(
        world.get::<GlobalTransform2D>(child).is_none(),
        "Nested parent without GT should still defer child GT until propagation"
    );

    tick_propagate(&mut world);

    let parent_gt = world.get::<GlobalTransform2D>(parent).unwrap();
    assert!(
        approx_eq(parent_gt.position.x, 110.0),
        "Parent world X after propagation: expected 110, got {}",
        parent_gt.position.x
    );
    assert!(
        approx_eq(parent_gt.position.y, 50.0),
        "Parent world Y after propagation: expected 50, got {}",
        parent_gt.position.y
    );

    let child_gt = world.get::<GlobalTransform2D>(child).unwrap();
    assert!(
        approx_eq(child_gt.position.x, 115.0),
        "Child world X after propagation: expected 115, got {}",
        child_gt.position.x
    );
    assert!(
        approx_eq(child_gt.position.y, 50.0),
        "Child world Y after propagation: expected 50, got {}",
        child_gt.position.y
    );
}

// =============================================================================
// PHASE 4: StuckTo filter — skip entities with ChildOf
// =============================================================================

fn tick_stuckto(world: &mut World) {
    world
        .run_system_once(stuck_to_entity_system)
        .expect("stuck_to_entity_system should run");
}

#[test]
fn stuckto_skips_entities_with_childof() {
    let mut world = World::new();

    // Target entity
    let target = world.spawn((MapPosition::new(200.0, 200.0),)).id();

    // Follower that has both StuckTo AND ChildOf — should be skipped by StuckTo system
    let parent = world.spawn((MapPosition::new(0.0, 0.0),)).id();

    let follower = world
        .spawn((
            MapPosition::new(10.0, 10.0),
            StuckTo::new(target),
            ChildOf(parent),
        ))
        .id();

    world.flush();
    tick_stuckto(&mut world);

    // Position should NOT have been updated to target's position
    let pos = world.get::<MapPosition>(follower).unwrap();
    assert!(
        approx_eq(pos.pos.x, 10.0),
        "Follower with ChildOf should not be moved by StuckTo, got x={}",
        pos.pos.x
    );
    assert!(
        approx_eq(pos.pos.y, 10.0),
        "Follower with ChildOf should not be moved by StuckTo, got y={}",
        pos.pos.y
    );
}

#[test]
fn stuckto_still_works_without_childof() {
    let mut world = World::new();

    // Target entity
    let target = world.spawn((MapPosition::new(200.0, 200.0),)).id();

    // Follower with StuckTo only (no ChildOf) — should follow target normally
    let follower = world
        .spawn((MapPosition::new(10.0, 10.0), StuckTo::new(target)))
        .id();

    tick_stuckto(&mut world);

    // Position should have been updated to target's position (follow_x and follow_y default to true)
    let pos = world.get::<MapPosition>(follower).unwrap();
    assert!(
        approx_eq(pos.pos.x, 200.0),
        "Follower without ChildOf should follow target, got x={}",
        pos.pos.x
    );
    assert!(
        approx_eq(pos.pos.y, 200.0),
        "Follower without ChildOf should follow target, got y={}",
        pos.pos.y
    );
}

// =============================================================================
// PHASE 5: Render system integration (query smoke tests)
// =============================================================================

use aberred_core::components::sprite::Sprite;
use aberred_core::components::zindex::ZIndex;

#[test]
fn render_query_includes_global_transform() {
    let mut world = World::new();

    // Entity with GlobalTransform2D (hierarchy participant)
    let entity = world
        .spawn((
            Sprite {
                tex_key: Arc::from("test"),
                width: 32.0,
                height: 32.0,
                offset: Vec2 { x: 0.0, y: 0.0 },
                origin: Vec2 { x: 0.0, y: 0.0 },
                flip_h: false,
                flip_v: false,
            },
            MapPosition::new(0.0, 0.0),
            ZIndex(0.0),
            GlobalTransform2D::default(),
        ))
        .id();

    // The MapSpriteQueryData type includes Option<&GlobalTransform2D>
    // Verify the query matches and GlobalTransform2D is Some
    let mut query = world.query::<(
        Entity,
        &Sprite,
        &MapPosition,
        &ZIndex,
        Option<&Scale>,
        Option<&Rotation>,
        Option<&GlobalTransform2D>,
    )>();

    let mut found = false;
    for (e, _s, _p, _z, _scale, _rot, maybe_gt) in query.iter(&world) {
        if e == entity {
            assert!(
                maybe_gt.is_some(),
                "Entity with GlobalTransform2D should have Some in query"
            );
            found = true;
        }
    }
    assert!(found, "Entity should be matched by the query");
}

#[test]
fn render_query_works_without_global_transform() {
    let mut world = World::new();

    // Entity without GlobalTransform2D (standalone, no hierarchy)
    let entity = world
        .spawn((
            Sprite {
                tex_key: Arc::from("test"),
                width: 32.0,
                height: 32.0,
                offset: Vec2 { x: 0.0, y: 0.0 },
                origin: Vec2 { x: 0.0, y: 0.0 },
                flip_h: false,
                flip_v: false,
            },
            MapPosition::new(50.0, 50.0),
            ZIndex(1.0),
        ))
        .id();

    let mut query = world.query::<(
        Entity,
        &Sprite,
        &MapPosition,
        &ZIndex,
        Option<&Scale>,
        Option<&Rotation>,
        Option<&GlobalTransform2D>,
    )>();

    let mut found = false;
    for (e, _s, _p, _z, _scale, _rot, maybe_gt) in query.iter(&world) {
        if e == entity {
            assert!(
                maybe_gt.is_none(),
                "Entity without GlobalTransform2D should have None in query"
            );
            found = true;
        }
    }
    assert!(found, "Entity should still be matched by the query");
}

// =============================================================================
// PHASE 7: Entity context + Particle emitter
// =============================================================================

use aberred_lua::resources::lua_runtime::{
    EntitySnapshot, LuaRuntime, build_entity_context_pooled,
};

#[test]
fn entity_context_includes_world_transform_fields() {
    let runtime = LuaRuntime::new().expect("LuaRuntime init");
    let tables = runtime.get_entity_ctx_pool();
    let lua = runtime.lua();

    let snapshot = EntitySnapshot {
        entity_id: 42_u64,
        group: None,
        map_pos: Some((10.0, 20.0)),
        screen_pos: None,
        rigid_body: None,
        rotation: Some(45.0),
        scale: Some((1.0, 1.0)),
        rect: None,
        sprite: None,
        animation: None,
        signals: None,
        lua_phase: None,
        lua_timer: None,
        previous_phase: None,
        world_pos: Some((110.0, 120.0)),
        world_rotation: Some(90.0),
        world_scale: Some((2.0, 3.0)),
        parent_id: Some(99),
    };
    let ctx =
        build_entity_context_pooled(lua, &tables, &snapshot).expect("build_entity_context_pooled");

    lua.load(
        r#"
        local ctx = ...
        assert(ctx.world_pos ~= nil,        "world_pos should not be nil")
        assert(ctx.world_pos.x == 110.0,    "world_pos.x: " .. tostring(ctx.world_pos.x))
        assert(ctx.world_pos.y == 120.0,    "world_pos.y: " .. tostring(ctx.world_pos.y))
        assert(ctx.world_rotation == 90.0,  "world_rotation: " .. tostring(ctx.world_rotation))
        assert(ctx.world_scale ~= nil,      "world_scale should not be nil")
        assert(ctx.world_scale.x == 2.0,    "world_scale.x: " .. tostring(ctx.world_scale.x))
        assert(ctx.world_scale.y == 3.0,    "world_scale.y: " .. tostring(ctx.world_scale.y))
        assert(ctx.parent_id == 99,         "parent_id: " .. tostring(ctx.parent_id))
    "#,
    )
    .call::<()>(ctx)
    .expect("Lua world transform assertions");
}

#[test]
fn entity_context_nil_world_fields_without_hierarchy() {
    let runtime = LuaRuntime::new().expect("LuaRuntime init");
    let tables = runtime.get_entity_ctx_pool();
    let lua = runtime.lua();

    let snapshot = EntitySnapshot {
        entity_id: 1_u64,
        group: None,
        map_pos: None,
        screen_pos: None,
        rigid_body: None,
        rotation: None,
        scale: None,
        rect: None,
        sprite: None,
        animation: None,
        signals: None,
        lua_phase: None,
        lua_timer: None,
        previous_phase: None,
        world_pos: None,
        world_rotation: None,
        world_scale: None,
        parent_id: None,
    };
    let ctx =
        build_entity_context_pooled(lua, &tables, &snapshot).expect("build_entity_context_pooled");

    lua.load(
        r#"
        local ctx = ...
        assert(ctx.world_pos      == nil, "world_pos should be nil")
        assert(ctx.world_rotation == nil, "world_rotation should be nil")
        assert(ctx.world_scale    == nil, "world_scale should be nil")
        assert(ctx.parent_id      == nil, "parent_id should be nil")
    "#,
    )
    .call::<()>(ctx)
    .expect("Lua nil world transform assertions");
}

// =============================================================================
// PHASE 8: Metadata, stubs, and documentation
// =============================================================================

#[test]
fn meta_entity_cmds_include_parent_commands() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(
        r#"
        local fns = engine.__meta.functions
        assert(fns.entity_set_parent, "entity_set_parent missing from __meta.functions")
        assert(fns.entity_set_parent.description, "entity_set_parent missing description")
        assert(fns.entity_remove_parent, "entity_remove_parent missing from __meta.functions")
        assert(fns.entity_remove_parent.description, "entity_remove_parent missing description")
        -- Also check collision_ variants (auto-generated by define_entity_cmds!)
        assert(fns.collision_entity_set_parent, "collision_entity_set_parent missing")
        assert(fns.collision_entity_remove_parent, "collision_entity_remove_parent missing")
    "#,
    )
    .exec()
    .expect("Lua meta parent commands assertions");
}

#[test]
fn meta_builder_includes_with_parent() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
        local builder = engine.__meta.classes.EntityBuilder
        assert(builder, "EntityBuilder class missing from __meta.classes")
        local method = builder.methods.with_parent
        assert(method, "with_parent method missing from EntityBuilder")
        assert(method.description, "with_parent missing description")
        assert(method.params, "with_parent missing params")
        -- Verify param
        local p1 = method.params[1]
        assert(p1.name == "parent_id", "first param should be parent_id, got: " .. tostring(p1.name))
        assert(p1.type == "integer", "parent_id type should be integer, got: " .. tostring(p1.type))
        -- Also check CollisionEntityBuilder
        local collision_builder = engine.__meta.classes.CollisionEntityBuilder
        assert(collision_builder.methods.with_parent, "with_parent missing from CollisionEntityBuilder")
    "#).exec().expect("Lua meta builder with_parent assertions");
}

#[test]
fn meta_entity_context_includes_world_fields() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(
        r#"
        local types = engine.__meta.types
        local ctx_type = types.EntityContext
        assert(ctx_type, "EntityContext type missing from __meta.types")
        -- Find world_pos, world_rotation, world_scale, parent_id fields
        local found_world_pos = false
        local found_world_rotation = false
        local found_world_scale = false
        local found_parent_id = false
        for _, field in ipairs(ctx_type.fields) do
            if field.name == "world_pos" then found_world_pos = true end
            if field.name == "world_rotation" then found_world_rotation = true end
            if field.name == "world_scale" then found_world_scale = true end
            if field.name == "parent_id" then found_parent_id = true end
        end
        assert(found_world_pos, "world_pos field missing from EntityContext type")
        assert(found_world_rotation, "world_rotation field missing from EntityContext type")
        assert(found_world_scale, "world_scale field missing from EntityContext type")
        assert(found_parent_id, "parent_id field missing from EntityContext type")
    "#,
    )
    .exec()
    .expect("Lua meta EntityContext world fields assertions");
}

#[test]
fn entity_cmd_set_screen_position_updates_screen_position() {
    use aberred_core::components::screenposition::ScreenPosition;

    let mut world = World::new();
    let entity = world.spawn(ScreenPosition::new(0.0, 0.0)).id();

    run_entity_cmds(
        &mut world,
        vec![EntityCmd::SetScreenPosition {
            entity_id: entity.to_bits(),
            x: 42.0,
            y: 77.0,
        }],
    );

    let pos = world.get::<ScreenPosition>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 42.0));
    assert!(approx_eq(pos.pos.y, 77.0));
}

#[test]
fn entity_cmd_set_screen_position_no_op_on_map_entity() {
    // Entity has MapPosition only — SetScreenPosition should silently do nothing.
    let mut world = World::new();
    let entity = world.spawn(MapPosition::new(10.0, 20.0)).id();

    run_entity_cmds(
        &mut world,
        vec![EntityCmd::SetScreenPosition {
            entity_id: entity.to_bits(),
            x: 99.0,
            y: 99.0,
        }],
    );

    // MapPosition unchanged
    let pos = world.get::<MapPosition>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 10.0));
    assert!(approx_eq(pos.pos.y, 20.0));
}
