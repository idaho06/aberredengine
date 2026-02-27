//! Integration tests for the parent-child entity transform hierarchy.
//!
//! Tests are organized by implementation phase.
//!
//! # Usage
//!
//! ```sh
//! cargo test --test hierarchy_integration
//! ```

use std::sync::Arc;

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
#[cfg(feature = "lua")]
use bevy_ecs::system::SystemState;
use raylib::math::Vector2;

use aberredengine::components::globaltransform2d::GlobalTransform2D;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::rotation::Rotation;
use aberredengine::components::scale::Scale;
use aberredengine::components::stuckto::StuckTo;
#[cfg(feature = "lua")]
use aberredengine::resources::lua_runtime::{EntityCmd, SpawnCmd};
#[cfg(feature = "lua")]
use aberredengine::resources::systemsstore::SystemsStore;
#[cfg(feature = "lua")]
use aberredengine::resources::worldsignals::WorldSignals;
#[cfg(feature = "lua")]
use aberredengine::systems::lua_commands::EntityCmdQueries;
#[cfg(feature = "lua")]
use aberredengine::systems::lua_commands::{process_entity_commands, process_spawn_command};
use aberredengine::systems::propagate_transforms::propagate_transforms;
use aberredengine::systems::stuckto::stuck_to_entity_system;

const EPSILON: f32 = 1e-4;

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < EPSILON
}

fn tick_propagate(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(propagate_transforms);
    schedule.run(world);
}

// =============================================================================
// PHASE 1: GlobalTransform2D component + propagate_transforms system
// =============================================================================

#[test]
fn globaltransform2d_default_values() {
    let gt = GlobalTransform2D::default();
    assert!(approx_eq(gt.position.x, 0.0));
    assert!(approx_eq(gt.position.y, 0.0));
    assert!(approx_eq(gt.rotation_degrees, 0.0));
    assert!(approx_eq(gt.scale.x, 1.0));
    assert!(approx_eq(gt.scale.y, 1.0));
}

#[test]
fn propagate_root_entity_without_children() {
    let mut world = World::new();

    let root = world
        .spawn((
            MapPosition::new(100.0, 50.0),
            Rotation { degrees: 45.0 },
            Scale::new(2.0, 2.0),
            GlobalTransform2D::default(),
        ))
        .id();

    // Root has no Children — propagate_transforms only processes roots WITH Children.
    // A standalone entity with GlobalTransform2D but no Children is NOT processed
    // (it's not participating in a hierarchy). This is by design.
    tick_propagate(&mut world);

    // GlobalTransform2D stays at default because the root query requires &Children.
    let gt = world.get::<GlobalTransform2D>(root).unwrap();
    assert!(
        approx_eq(gt.position.x, 0.0),
        "Standalone entity should not be processed by propagate_transforms"
    );
}

#[test]
fn propagate_single_child_position_only() {
    let mut world = World::new();

    let parent = world
        .spawn((
            MapPosition::new(100.0, 100.0),
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

    // Flush so Bevy populates Children on parent
    world.flush();

    tick_propagate(&mut world);

    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    assert!(
        approx_eq(gt.position.x, 140.0),
        "Child world X: expected 140, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 100.0),
        "Child world Y: expected 100, got {}",
        gt.position.y
    );
    assert!(approx_eq(gt.rotation_degrees, 0.0));
    assert!(approx_eq(gt.scale.x, 1.0));
    assert!(approx_eq(gt.scale.y, 1.0));
}

#[test]
fn propagate_child_inherits_parent_rotation() {
    let mut world = World::new();

    let parent = world
        .spawn((
            MapPosition::new(100.0, 100.0),
            Rotation { degrees: 90.0 },
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
    tick_propagate(&mut world);

    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    // Local offset (40, 0) rotated 90deg CW => (0, 40)
    assert!(
        approx_eq(gt.position.x, 100.0),
        "Child world X: expected 100, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 140.0),
        "Child world Y: expected 140, got {}",
        gt.position.y
    );
    assert!(
        approx_eq(gt.rotation_degrees, 90.0),
        "Child world rotation: expected 90, got {}",
        gt.rotation_degrees
    );
}

#[test]
fn propagate_child_inherits_parent_scale() {
    let mut world = World::new();

    let parent = world
        .spawn((
            MapPosition::new(100.0, 100.0),
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
    tick_propagate(&mut world);

    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    // Offset (40, 0) scaled by (2, 2) => (80, 0)
    assert!(
        approx_eq(gt.position.x, 180.0),
        "Child world X: expected 180, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 100.0),
        "Child world Y: expected 100, got {}",
        gt.position.y
    );
    assert!(approx_eq(gt.scale.x, 2.0));
    assert!(approx_eq(gt.scale.y, 2.0));
}

#[test]
fn propagate_child_inherits_rotation_and_scale() {
    let mut world = World::new();

    let parent = world
        .spawn((
            MapPosition::new(0.0, 0.0),
            Rotation { degrees: 90.0 },
            Scale::new(2.0, 1.0),
            GlobalTransform2D::default(),
        ))
        .id();

    let child = world
        .spawn((
            MapPosition::new(10.0, 0.0),
            ChildOf(parent),
            GlobalTransform2D::default(),
        ))
        .id();

    world.flush();
    tick_propagate(&mut world);

    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    // Offset (10, 0) scaled by (2, 1) => (20, 0), rotated 90deg => (0, 20)
    assert!(
        approx_eq(gt.position.x, 0.0),
        "Child world X: expected 0, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 20.0),
        "Child world Y: expected 20, got {}",
        gt.position.y
    );
    assert!(approx_eq(gt.rotation_degrees, 90.0));
    assert!(approx_eq(gt.scale.x, 2.0));
    assert!(approx_eq(gt.scale.y, 1.0));
}

#[test]
fn propagate_chain_grandchild() {
    let mut world = World::new();

    let root = world
        .spawn((
            MapPosition::new(100.0, 0.0),
            GlobalTransform2D::default(),
        ))
        .id();

    let child = world
        .spawn((
            MapPosition::new(50.0, 0.0),
            ChildOf(root),
            GlobalTransform2D::default(),
        ))
        .id();

    let grandchild = world
        .spawn((
            MapPosition::new(10.0, 0.0),
            ChildOf(child),
            GlobalTransform2D::default(),
        ))
        .id();

    world.flush();
    tick_propagate(&mut world);

    // Root: world pos = (100, 0)
    let root_gt = world.get::<GlobalTransform2D>(root).unwrap();
    assert!(
        approx_eq(root_gt.position.x, 100.0),
        "Root X: expected 100, got {}",
        root_gt.position.x
    );

    // Child: world pos = (100 + 50, 0) = (150, 0)
    let child_gt = world.get::<GlobalTransform2D>(child).unwrap();
    assert!(
        approx_eq(child_gt.position.x, 150.0),
        "Child X: expected 150, got {}",
        child_gt.position.x
    );

    // Grandchild: world pos = (150 + 10, 0) = (160, 0)
    let gc_gt = world.get::<GlobalTransform2D>(grandchild).unwrap();
    assert!(
        approx_eq(gc_gt.position.x, 160.0),
        "Grandchild X: expected 160, got {}",
        gc_gt.position.x
    );
    assert!(approx_eq(gc_gt.position.y, 0.0));
}

#[test]
fn propagate_no_crash_on_empty_world() {
    let mut world = World::new();
    // No entities at all — should not panic
    tick_propagate(&mut world);
}

#[test]
fn propagate_entities_without_hierarchy_unchanged() {
    let mut world = World::new();

    // Entity with GlobalTransform2D but no ChildOf and no Children
    // It's not part of any hierarchy, so propagation ignores it.
    let entity = world
        .spawn((
            MapPosition::new(50.0, 25.0),
            Rotation { degrees: 30.0 },
            Scale::new(3.0, 3.0),
            GlobalTransform2D::default(),
        ))
        .id();

    tick_propagate(&mut world);

    // GlobalTransform2D should remain at default (system doesn't process standalone entities)
    let gt = world.get::<GlobalTransform2D>(entity).unwrap();
    assert!(
        approx_eq(gt.position.x, 0.0),
        "Standalone entity GT should not be updated"
    );
}

#[test]
fn propagate_root_with_children_gets_correct_gt() {
    let mut world = World::new();

    let parent = world
        .spawn((
            MapPosition::new(50.0, 25.0),
            Rotation { degrees: 30.0 },
            Scale::new(3.0, 2.0),
            GlobalTransform2D::default(),
        ))
        .id();

    // Need at least one child for parent to have Children component
    world.spawn((
        MapPosition::new(0.0, 0.0),
        ChildOf(parent),
        GlobalTransform2D::default(),
    ));

    world.flush();
    tick_propagate(&mut world);

    // Root's GlobalTransform2D should reflect its own local values
    let gt = world.get::<GlobalTransform2D>(parent).unwrap();
    assert!(
        approx_eq(gt.position.x, 50.0),
        "Root GT X: expected 50, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 25.0),
        "Root GT Y: expected 25, got {}",
        gt.position.y
    );
    assert!(approx_eq(gt.rotation_degrees, 30.0));
    assert!(approx_eq(gt.scale.x, 3.0));
    assert!(approx_eq(gt.scale.y, 2.0));
}

#[test]
fn propagate_child_with_own_rotation_and_scale() {
    let mut world = World::new();

    let parent = world
        .spawn((
            MapPosition::new(0.0, 0.0),
            Rotation { degrees: 0.0 },
            Scale::new(2.0, 2.0),
            GlobalTransform2D::default(),
        ))
        .id();

    let child = world
        .spawn((
            MapPosition::new(10.0, 0.0),
            Rotation { degrees: 45.0 },
            Scale::new(0.5, 0.5),
            ChildOf(parent),
            GlobalTransform2D::default(),
        ))
        .id();

    world.flush();
    tick_propagate(&mut world);

    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    // Offset (10, 0) scaled by parent (2, 2) => (20, 0), parent rot=0 => (20, 0)
    assert!(
        approx_eq(gt.position.x, 20.0),
        "Child X: expected 20, got {}",
        gt.position.x
    );
    assert!(approx_eq(gt.position.y, 0.0));
    // World rotation = parent 0 + child 45 = 45
    assert!(approx_eq(gt.rotation_degrees, 45.0));
    // World scale = parent (2,2) * child (0.5, 0.5) = (1.0, 1.0)
    assert!(approx_eq(gt.scale.x, 1.0));
    assert!(approx_eq(gt.scale.y, 1.0));
}

#[test]
fn propagate_inserts_missing_globaltransform2d_via_commands() {
    let mut world = World::new();

    let parent = world
        .spawn((MapPosition::new(100.0, 0.0),))
        .id();

    let child = world
        .spawn((
            MapPosition::new(10.0, 0.0),
            ChildOf(parent),
        ))
        .id();

    world.flush();

    // Neither entity has GlobalTransform2D yet.
    // First tick: system inserts via Commands (deferred).
    tick_propagate(&mut world);

    // After the schedule runs, commands should have been applied.
    assert!(
        world.get::<GlobalTransform2D>(parent).is_some(),
        "Parent should have GlobalTransform2D inserted by commands"
    );
    assert!(
        world.get::<GlobalTransform2D>(child).is_some(),
        "Child should have GlobalTransform2D inserted by commands"
    );

    // The values should be correct after the commands are applied.
    // Run a second tick so the system can read the now-present components.
    tick_propagate(&mut world);

    let child_gt = world.get::<GlobalTransform2D>(child).unwrap();
    assert!(
        approx_eq(child_gt.position.x, 110.0),
        "Child world X after second tick: expected 110, got {}",
        child_gt.position.x
    );
}

// =============================================================================
// PHASE 2: EntityCmd SetParent/RemoveParent + Lua API
// =============================================================================

/// Helper to run process_entity_commands using SystemState.
#[cfg(feature = "lua")]
fn run_entity_cmds(world: &mut World, cmds: Vec<EntityCmd>) {
    world.insert_resource(SystemsStore::new());

    let mut state = SystemState::<(Commands, EntityCmdQueries, Res<SystemsStore>)>::new(world);
    let (mut commands, mut queries, systems_store) = state.get_mut(world);

    process_entity_commands(
        &mut commands,
        cmds,
        &queries.stuckto,
        &mut queries.signals,
        &mut queries.animation,
        &mut queries.rigid_bodies,
        &mut queries.positions,
        &mut queries.shaders,
        &queries.global_transforms,
        &systems_store,
    );

    state.apply(world);
}

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
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
    assert!(
        children.is_some(),
        "Parent should have Children component"
    );
    assert_eq!(
        children.unwrap().len(),
        3,
        "Parent should have 3 children"
    );
}

// =============================================================================
// PHASE 3: Builder with_parent (SpawnCmd.parent field)
// =============================================================================

#[cfg(feature = "lua")]
#[test]
fn spawn_cmd_with_parent_applies_childof() {
    let mut world = World::new();
    world.insert_resource(WorldSignals::default());

    // Spawn parent entity first
    let parent = world
        .spawn((MapPosition::new(100.0, 50.0),))
        .id();

    // Build a SpawnCmd with parent set
    let cmd = SpawnCmd {
        position: Some((10.0, 0.0)),
        parent: Some(parent.to_bits()),
        ..SpawnCmd::default()
    };

    // Process the spawn command via SystemState
    let mut state = SystemState::<(Commands, ResMut<WorldSignals>)>::new(&mut world);
    let (mut commands, mut world_signals) = state.get_mut(&mut world);
    process_spawn_command(&mut commands, cmd, &mut world_signals);
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

    // Child should have GlobalTransform2D
    assert!(
        world.get::<GlobalTransform2D>(child).is_some(),
        "Spawned child should have GlobalTransform2D"
    );

    // Child should have MapPosition at the local offset
    let pos = world.get::<MapPosition>(child).unwrap();
    assert!(
        approx_eq(pos.pos.x, 10.0),
        "Child local pos X: expected 10, got {}",
        pos.pos.x
    );

    // Run propagation and verify world transform
    tick_propagate(&mut world);

    // After a second tick (first tick inserts GT on parent via commands, second computes)
    tick_propagate(&mut world);

    let gt = world.get::<GlobalTransform2D>(child).unwrap();
    assert!(
        approx_eq(gt.position.x, 110.0),
        "Child world X: expected 110, got {}",
        gt.position.x
    );
    assert!(
        approx_eq(gt.position.y, 50.0),
        "Child world Y: expected 50, got {}",
        gt.position.y
    );
}

// =============================================================================
// PHASE 4: StuckTo filter — skip entities with ChildOf
// =============================================================================

fn tick_stuckto(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(stuck_to_entity_system);
    schedule.run(world);
}

#[test]
fn stuckto_skips_entities_with_childof() {
    let mut world = World::new();

    // Target entity
    let target = world
        .spawn((MapPosition::new(200.0, 200.0),))
        .id();

    // Follower that has both StuckTo AND ChildOf — should be skipped by StuckTo system
    let parent = world
        .spawn((MapPosition::new(0.0, 0.0),))
        .id();

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
    let target = world
        .spawn((MapPosition::new(200.0, 200.0),))
        .id();

    // Follower with StuckTo only (no ChildOf) — should follow target normally
    let follower = world
        .spawn((
            MapPosition::new(10.0, 10.0),
            StuckTo::new(target),
        ))
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

use aberredengine::components::sprite::Sprite;
use aberredengine::components::zindex::ZIndex;

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
                offset: Vector2 { x: 0.0, y: 0.0 },
                origin: Vector2 { x: 0.0, y: 0.0 },
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
                offset: Vector2 { x: 0.0, y: 0.0 },
                origin: Vector2 { x: 0.0, y: 0.0 },
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
// PHASE 6: Collision system integration
// =============================================================================

use aberredengine::components::boxcollider::BoxCollider;
use aberredengine::events::collision::CollisionEvent;
use aberredengine::systems::collision_detector::collision_detector;

/// Resource to collect collision events via observer.
#[derive(Resource, Default)]
struct CollisionLog {
    pairs: Vec<(Entity, Entity)>,
}

fn setup_collision_world(world: &mut World) {
    world.insert_resource(CollisionLog::default());
    world.add_observer(
        |trigger: On<CollisionEvent>, mut log: ResMut<CollisionLog>| {
            log.pairs.push((trigger.event().a, trigger.event().b));
        },
    );
}

fn tick_collision(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(collision_detector);
    schedule.run(world);
}

#[test]
fn collision_uses_world_position_for_child_entities() {
    let mut world = World::new();
    setup_collision_world(&mut world);

    // Parent at (200, 200)
    let parent = world
        .spawn((
            MapPosition::new(200.0, 200.0),
            GlobalTransform2D::default(),
        ))
        .id();

    // Child with local position (0, 0), but parent is at (200, 200)
    // After propagation, child world position = (200, 200)
    let child = world
        .spawn((
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(20.0, 20.0),
            ChildOf(parent),
            GlobalTransform2D::default(),
        ))
        .id();

    world.flush();

    // Run propagation so child gets world position (200, 200)
    tick_propagate(&mut world);

    // Independent entity at (205, 205) — overlaps with child's world position
    let other = world
        .spawn((
            MapPosition::new(205.0, 205.0),
            BoxCollider::new(20.0, 20.0),
        ))
        .id();

    // Run collision detection
    tick_collision(&mut world);

    let log = world.resource::<CollisionLog>();
    assert!(
        !log.pairs.is_empty(),
        "Collision should be detected between child (world pos 200,200) and other (205,205)"
    );
    // Verify the collision involves the right entities
    let has_pair = log.pairs.iter().any(|&(a, b)| {
        (a == child && b == other) || (a == other && b == child)
    });
    assert!(has_pair, "Collision should be between child and other entity");
}

#[test]
fn collision_no_false_positive_from_local_position() {
    let mut world = World::new();
    setup_collision_world(&mut world);

    // Parent at (500, 500)
    let parent = world
        .spawn((
            MapPosition::new(500.0, 500.0),
            GlobalTransform2D::default(),
        ))
        .id();

    // Child with local position (5, 5) — world position = (505, 505)
    world.spawn((
        MapPosition::new(5.0, 5.0),
        BoxCollider::new(10.0, 10.0),
        ChildOf(parent),
        GlobalTransform2D::default(),
    ));

    world.flush();

    // Run propagation so child gets world position (505, 505)
    tick_propagate(&mut world);

    // Independent entity at (10, 10) — near child's LOCAL position but far from WORLD position
    world.spawn((
        MapPosition::new(10.0, 10.0),
        BoxCollider::new(10.0, 10.0),
    ));

    // Run collision detection
    tick_collision(&mut world);

    let log = world.resource::<CollisionLog>();
    assert!(
        log.pairs.is_empty(),
        "No collision should be detected — child world pos (505,505) is far from other (10,10)"
    );
}

// =============================================================================
// PHASE 7: Entity context + Particle emitter
// =============================================================================

#[cfg(feature = "lua")]
use aberredengine::resources::lua_runtime::{
    LuaRuntime, build_entity_context_pooled,
};

#[cfg(feature = "lua")]
#[test]
fn entity_context_includes_world_transform_fields() {
    let runtime = LuaRuntime::new().expect("LuaRuntime init");
    let tables = runtime.get_entity_ctx_pool().expect("ctx pool");
    let lua = runtime.lua();

    let ctx = build_entity_context_pooled(
        lua, &tables, 42_u64,
        None,                       // group
        Some((10.0, 20.0)),         // map_pos (local)
        None,                       // screen_pos
        None,                       // rigid_body
        Some(45.0),                 // rotation (local)
        Some((1.0, 1.0)),           // scale (local)
        None, None, None, None, None, None, None,
        Some((110.0, 120.0)),       // world_pos
        Some(90.0),                 // world_rotation
        Some((2.0, 3.0)),           // world_scale
        Some(99),                   // parent_id
    ).expect("build_entity_context_pooled");

    lua.load(r#"
        local ctx = ...
        assert(ctx.world_pos ~= nil,        "world_pos should not be nil")
        assert(ctx.world_pos.x == 110.0,    "world_pos.x: " .. tostring(ctx.world_pos.x))
        assert(ctx.world_pos.y == 120.0,    "world_pos.y: " .. tostring(ctx.world_pos.y))
        assert(ctx.world_rotation == 90.0,  "world_rotation: " .. tostring(ctx.world_rotation))
        assert(ctx.world_scale ~= nil,      "world_scale should not be nil")
        assert(ctx.world_scale.x == 2.0,    "world_scale.x: " .. tostring(ctx.world_scale.x))
        assert(ctx.world_scale.y == 3.0,    "world_scale.y: " .. tostring(ctx.world_scale.y))
        assert(ctx.parent_id == 99,         "parent_id: " .. tostring(ctx.parent_id))
    "#).call::<()>(ctx).expect("Lua world transform assertions");
}

#[cfg(feature = "lua")]
#[test]
fn entity_context_nil_world_fields_without_hierarchy() {
    let runtime = LuaRuntime::new().expect("LuaRuntime init");
    let tables = runtime.get_entity_ctx_pool().expect("ctx pool");
    let lua = runtime.lua();

    let ctx = build_entity_context_pooled(
        lua, &tables, 1_u64,
        None, None, None, None, None, None, None,
        None, None, None, None, None, None,
        None, None, None, None,  // no world transform, no parent
    ).expect("build_entity_context_pooled");

    lua.load(r#"
        local ctx = ...
        assert(ctx.world_pos      == nil, "world_pos should be nil")
        assert(ctx.world_rotation == nil, "world_rotation should be nil")
        assert(ctx.world_scale    == nil, "world_scale should be nil")
        assert(ctx.parent_id      == nil, "parent_id should be nil")
    "#).call::<()>(ctx).expect("Lua nil world transform assertions");
}

#[test]
fn cascade_despawn_removes_children() {
    let mut world = World::new();

    let parent = world
        .spawn((MapPosition::new(0.0, 0.0),))
        .id();

    let child = world
        .spawn((
            MapPosition::new(10.0, 0.0),
            ChildOf(parent),
        ))
        .id();

    let grandchild = world
        .spawn((
            MapPosition::new(20.0, 0.0),
            ChildOf(child),
        ))
        .id();

    world.flush();

    // All three should exist
    assert!(world.get_entity(parent).is_ok());
    assert!(world.get_entity(child).is_ok());
    assert!(world.get_entity(grandchild).is_ok());

    // Despawn parent — Bevy cascade despawn should remove child and grandchild
    world.despawn(parent);

    assert!(
        world.get_entity(parent).is_err(),
        "Parent should be despawned"
    );
    assert!(
        world.get_entity(child).is_err(),
        "Child should be cascade-despawned"
    );
    assert!(
        world.get_entity(grandchild).is_err(),
        "Grandchild should be cascade-despawned"
    );
}

// =============================================================================
// PHASE 8: Metadata, stubs, and documentation
// =============================================================================

#[cfg(feature = "lua")]
#[test]
fn meta_entity_cmds_include_parent_commands() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
        local fns = engine.__meta.functions
        assert(fns.entity_set_parent, "entity_set_parent missing from __meta.functions")
        assert(fns.entity_set_parent.description, "entity_set_parent missing description")
        assert(fns.entity_remove_parent, "entity_remove_parent missing from __meta.functions")
        assert(fns.entity_remove_parent.description, "entity_remove_parent missing description")
        -- Also check collision_ variants (auto-generated by define_entity_cmds!)
        assert(fns.collision_entity_set_parent, "collision_entity_set_parent missing")
        assert(fns.collision_entity_remove_parent, "collision_entity_remove_parent missing")
    "#).exec().expect("Lua meta parent commands assertions");
}

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
#[test]
fn meta_entity_context_includes_world_fields() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
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
    "#).exec().expect("Lua meta EntityContext world fields assertions");
}
