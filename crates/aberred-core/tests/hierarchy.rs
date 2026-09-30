//! Integration tests for the transform hierarchy across systems.
//!
//! When a root entity loses its last child, `propagate_transforms` stops
//! updating its `GlobalTransform2D`; `cleanup_orphaned_global_transforms`
//! removes the stale component so world-position lookups fall back to the
//! live `MapPosition`.

use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;

use aberred_core::components::globaltransform2d::GlobalTransform2D;
use aberred_core::components::mapposition::MapPosition;
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
