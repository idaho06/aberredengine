//! Integration tests for the Lua-facing side of the entity hierarchy: parent
//! commands, spawning with a parent, and the world-transform fields exposed in
//! the Lua entity context.
//!
//! Core transform propagation and orphan cleanup are tested in `aberred-core`
//! (`systems/propagate_transforms.rs`, `tests/hierarchy.rs`).

use bevy_ecs::system::RunSystemOnce;

use aberred_core::math::Vec2;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;

use aberred_core::components::globaltransform2d::GlobalTransform2D;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_core::systems::propagate_transforms::propagate_transforms;
use aberred_core::testing::approx_eq;
use aberred_lua::resources::lua_runtime::{
    EntityCmd, EntitySnapshot, LuaRuntime, SpawnCmd, build_entity_context_pooled,
};
use aberred_lua::systems::lua_commands::EntityCmdQueries;
use aberred_lua::systems::lua_commands::{process_entity_commands, process_spawn_command};

fn tick_propagate(world: &mut World) {
    world
        .run_system_once(propagate_transforms)
        .expect("propagate_transforms should run");
}

// =============================================================================
// Parent commands (EntityCmd::SetParent / RemoveParent)
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

/// An EntityCmd with invalid entity bits (the EntityIndex niche value)
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

/// Despawn followed by an insert-based command on the same entity in
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
// Spawning with a parent (SpawnCmd.parent, `:with_parent()`)
//
// `ComputeInitialGlobalTransform` gives the child its world transform on the
// spawn frame when the parent has one, or when the parent is a standalone
// root (synthesized from its local transform). Only a nested parent without
// a GlobalTransform2D defers to the next propagation.
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
// Entity context: world-transform fields
// =============================================================================

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
// Screen-position command (EntityCmd::SetScreenPosition)
// =============================================================================

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
