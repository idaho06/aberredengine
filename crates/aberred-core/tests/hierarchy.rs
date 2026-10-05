//! Integration tests for the transform hierarchy across systems.
//!
//! When a root entity loses its last child, `propagate_transforms` stops
//! updating its `GlobalTransform2D`; `cleanup_orphaned_global_transforms`
//! removes the stale component so world-position lookups fall back to the
//! live `MapPosition`. Collision detection reads those world positions, so
//! a child collides where its parent places it, not at its local offset.

use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;

use aberred_core::components::boxcollider::BoxCollider;
use aberred_core::components::globaltransform2d::GlobalTransform2D;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::events::collision::Overlapping;
use aberred_core::math::Vec2;
use aberred_core::systems::collision_detector::collision_detector;
use aberred_core::systems::propagate_transforms::{
    cleanup_orphaned_global_transforms, propagate_transforms,
};
use aberred_core::testing::approx_eq;

fn tick_propagate_and_cleanup(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(propagate_transforms);
    schedule.add_systems(cleanup_orphaned_global_transforms.after(propagate_transforms));
    schedule.run(world);
}

/// Regression test for the stale-GT movement freeze bug.
///
/// Scenario:
/// 1. Player spawned at (100, 0).
/// 2. A hitbox child is attached → player becomes a hierarchy root →
///    propagate_transforms inserts GlobalTransform2D on player.
/// 3. Player moves to (200, 0).
/// 4. Hitbox is despawned → Bevy removes Children from player.
/// 5. Player moves to (300, 0).
/// 6. propagate_transforms runs — skips player (no Children).
///    GT is now stale at (200, 0).
/// 7. cleanup_orphaned_global_transforms runs — removes stale GT.
/// 8. resolve_world_pos must now return MapPosition (300, 0),
///    not the stale GT (200, 0).
#[test]
fn stale_gt_removed_after_child_despawn() {
    use aberred_core::systems::collision::resolve_world_pos;
    use bevy_ecs::system::SystemState;

    let mut world = World::new();

    // 1. Spawn player at (100, 0)
    let player = world.spawn(MapPosition::new(100.0, 0.0)).id();

    // 2. Attach a hitbox child — world.flush() makes Bevy add Children to player
    let hitbox = world
        .spawn((MapPosition::new(0.0, 0.0), ChildOf(player)))
        .id();
    world.flush();

    assert!(
        world.get::<Children>(player).is_some(),
        "Player should have Children after hitbox attachment"
    );

    // propagate_transforms inserts GT on player (deferred — needs two ticks)
    tick_propagate_and_cleanup(&mut world); // tick 1: inserts GT via commands
    tick_propagate_and_cleanup(&mut world); // tick 2: GT is now visible + updated

    assert!(
        world.get::<GlobalTransform2D>(player).is_some(),
        "Player should have GT after becoming a hierarchy root"
    );

    // 3. Move player to (200, 0) and run propagation to keep GT in sync
    world.get_mut::<MapPosition>(player).unwrap().pos.x = 200.0;
    tick_propagate_and_cleanup(&mut world);

    {
        let gt = world.get::<GlobalTransform2D>(player).unwrap();
        assert!(
            approx_eq(gt.position.x, 200.0),
            "GT should be at 200 after move, got {}",
            gt.position.x
        );
    }

    // 4. Despawn the hitbox → Bevy removes Children from player
    world.despawn(hitbox);
    // world.flush() is implicit after despawn in direct world access

    assert!(
        world.get::<Children>(player).is_none(),
        "Player should have no Children after hitbox despawn"
    );

    // 5. Move player to (300, 0) — GT is now stale at (200, 0)
    world.get_mut::<MapPosition>(player).unwrap().pos.x = 300.0;

    // 6. Run propagate without cleanup to confirm the bug exists:
    //    GT is NOT updated (player not in RootsQuery)
    world
        .run_system_once(propagate_transforms)
        .expect("propagate_transforms should run");

    {
        let gt = world.get::<GlobalTransform2D>(player).unwrap();
        assert!(
            approx_eq(gt.position.x, 200.0),
            "GT should still be stale at 200 (bug condition), got {}",
            gt.position.x
        );
    }

    // 7. Now run the cleanup — it must remove the stale GT
    world
        .run_system_once(cleanup_orphaned_global_transforms)
        .expect("cleanup_orphaned_global_transforms should run");

    assert!(
        world.get::<GlobalTransform2D>(player).is_none(),
        "Stale GT should be removed by cleanup after child despawn"
    );

    // 8. resolve_world_pos must now return MapPosition (300, 0)
    let mut state =
        SystemState::<(Query<&MapPosition>, Query<&GlobalTransform2D>)>::new(&mut world);
    let (positions, global_transforms) = state.get(&world).expect("Hierarchy queries should fetch");
    let resolved = resolve_world_pos(&positions, &global_transforms, player).unwrap();

    assert!(
        approx_eq(resolved.x, 300.0),
        "resolve_world_pos should return live MapPosition 300 after GT removal, got {}",
        resolved.x
    );
    assert!(
        approx_eq(resolved.y, 0.0),
        "resolve_world_pos y should be 0, got {}",
        resolved.y
    );
}

// --- collision detection uses hierarchy world positions ---

fn tick_propagate(world: &mut World) {
    world
        .run_system_once(propagate_transforms)
        .expect("propagate_transforms should run");
}

/// Resource to collect collision events via observer.
#[derive(Resource, Default)]
struct CollisionLog {
    pairs: Vec<(Entity, Entity)>,
}

fn setup_collision_world(world: &mut World) {
    world.insert_resource(CollisionLog::default());
    world.add_observer(|trigger: On<Overlapping>, mut log: ResMut<CollisionLog>| {
        log.pairs.push((trigger.event().a, trigger.event().b));
    });
}

fn tick_collision(world: &mut World) {
    world
        .run_system_once(collision_detector)
        .expect("collision_detector should run");
}

#[test]
fn collision_uses_world_position_for_child_entities() {
    let mut world = World::new();
    setup_collision_world(&mut world);

    // Parent at (200, 200)
    let parent = world
        .spawn((MapPosition::new(200.0, 200.0), GlobalTransform2D::default()))
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
        .spawn((MapPosition::new(205.0, 205.0), BoxCollider::new(20.0, 20.0)))
        .id();

    // Run collision detection
    tick_collision(&mut world);

    let log = world.resource::<CollisionLog>();
    assert!(
        !log.pairs.is_empty(),
        "Collision should be detected between child (world pos 200,200) and other (205,205)"
    );
    // Verify the collision involves the right entities
    let has_pair = log
        .pairs
        .iter()
        .any(|&(a, b)| (a == child && b == other) || (a == other && b == child));
    assert!(
        has_pair,
        "Collision should be between child and other entity"
    );
}

#[test]
fn collision_no_false_positive_from_local_position() {
    let mut world = World::new();
    setup_collision_world(&mut world);

    // Parent at (500, 500)
    let parent = world
        .spawn((MapPosition::new(500.0, 500.0), GlobalTransform2D::default()))
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
    world.spawn((MapPosition::new(10.0, 10.0), BoxCollider::new(10.0, 10.0)));

    // Run collision detection
    tick_collision(&mut world);

    let log = world.resource::<CollisionLog>();
    assert!(
        log.pairs.is_empty(),
        "No collision should be detected — child world pos (505,505) is far from other (10,10)"
    );
}

/// Verifies the full frame pipeline: propagate → cleanup → collision.
///
/// After a child is despawned, the parent's stale GT must be cleaned up
/// before collision detection so that collider rects use the correct,
/// live MapPosition rather than the frozen world position from when the
/// entity was last a hierarchy root.
#[test]
fn collision_uses_live_map_position_after_child_despawn() {
    let mut world = World::new();
    setup_collision_world(&mut world);

    // Spawn player at (0, 0) — will become a hierarchy root
    let player = world
        .spawn((MapPosition::new(0.0, 0.0), BoxCollider::new(20.0, 20.0)))
        .id();

    // Attach a hitbox child → player gains Children
    let hitbox = world
        .spawn((MapPosition::new(0.0, 0.0), ChildOf(player)))
        .id();
    world.flush();

    // Two propagation ticks so GT is inserted and synced
    tick_propagate_and_cleanup(&mut world);
    tick_propagate_and_cleanup(&mut world);

    // Move player far away to (500, 500) — no overlap with origin
    world.get_mut::<MapPosition>(player).unwrap().pos = Vec2 { x: 500.0, y: 500.0 };
    tick_propagate_and_cleanup(&mut world); // GT updated to (500, 500)

    // Despawn hitbox → Children removed from player; GT is now stale at (500, 500)
    world.despawn(hitbox);

    // Move player back to origin (0, 0)
    world.get_mut::<MapPosition>(player).unwrap().pos = Vec2 { x: 0.0, y: 0.0 };

    // Spawn a sensor at origin — should collide with player if position is correct
    let sensor = world
        .spawn((MapPosition::new(5.0, 5.0), BoxCollider::new(20.0, 20.0)))
        .id();

    // Run the full pipeline: propagate → cleanup → collision
    let mut schedule = Schedule::default();
    schedule.add_systems(propagate_transforms);
    schedule.add_systems(cleanup_orphaned_global_transforms.after(propagate_transforms));
    schedule.add_systems(collision_detector.after(cleanup_orphaned_global_transforms));
    schedule.run(&mut world);

    let log = world.resource::<CollisionLog>();
    let has_collision = log
        .pairs
        .iter()
        .any(|&(a, b)| (a == player && b == sensor) || (a == sensor && b == player));

    assert!(
        has_collision,
        "Player at live MapPosition (0,0) should collide with sensor at (5,5) — \
         stale GT at (500,500) must not prevent detection"
    );
}
