//! Integration tests for the parent-child entity transform hierarchy.
//!
//! Tests are organized by implementation phase.
//!
//! # Usage
//!
//! ```sh
//! cargo test --test hierarchy_integration
//! ```

use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;

use aberredengine::components::globaltransform2d::GlobalTransform2D;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::rotation::Rotation;
use aberredengine::components::scale::Scale;
use aberredengine::resources::lua_runtime::EntityCmd;
use aberredengine::resources::systemsstore::SystemsStore;
use aberredengine::systems::lua_commands::EntityCmdQueries;
use aberredengine::systems::lua_commands::process_entity_commands;
use aberredengine::systems::propagate_transforms::propagate_transforms;

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
