//! System for handling entities stuck to other entities.
//!
//! This system updates the position of entities with the [`StuckTo`] component
//! to follow their target entity's position.
//!
//! # Use Cases
//!
//! - Ball stuck to paddle at game start (follows X only)
//! - Objects attached to moving platforms
//! - Temporary "sticky" effects with auto-release via [`Timer`](crate::components::timer::Timer)
//!
//! # Related
//!
//! - [`StuckTo`](crate::components::stuckto::StuckTo) – the attachment component
//! - [`Timer`](crate::components::timer::Timer) – can auto-remove `StuckTo` after a delay

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;

use crate::components::mapposition::MapPosition;
use crate::components::stuckto::StuckTo;

/// Updates positions of entities with `StuckTo` to follow their targets.
///
/// For each entity with a `StuckTo` component:
/// - Gets the target entity's `MapPosition`
/// - Updates this entity's position based on `follow_x` and `follow_y` flags
/// - Applies the offset
pub fn stuck_to_entity_system(
    mut followers: Query<(&StuckTo, &mut MapPosition), Without<ChildOf>>,
    targets: Query<&MapPosition, Without<StuckTo>>,
) {
    for (stuck_to, mut follower_pos) in followers.iter_mut() {
        // Try to get the target's position
        if let Ok(target_pos) = targets.get(stuck_to.target) {
            if stuck_to.follow_x {
                follower_pos.pos.x = target_pos.pos.x + stuck_to.offset.x;
            }
            if stuck_to.follow_y {
                follower_pos.pos.y = target_pos.pos.y + stuck_to.offset.y;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec2;
    use crate::testing::approx_eq;
    use bevy_ecs::system::RunSystemOnce;

    fn tick_stuckto(world: &mut World) {
        world
            .run_system_once(stuck_to_entity_system)
            .expect("stuck_to_entity_system should run");
    }

    #[test]
    fn stuckto_follows_target_both_axes() {
        let mut world = World::new();

        let target = world.spawn((MapPosition::new(100.0, 50.0),)).id();
        let follower = world
            .spawn((MapPosition::new(0.0, 0.0), StuckTo::new(target)))
            .id();

        tick_stuckto(&mut world);

        let pos = world.get::<MapPosition>(follower).unwrap();
        assert!(approx_eq(pos.pos.x, 100.0));
        assert!(approx_eq(pos.pos.y, 50.0));
    }

    #[test]
    fn stuckto_follows_target_x_only() {
        let mut world = World::new();

        let target = world.spawn((MapPosition::new(100.0, 50.0),)).id();
        let follower = world
            .spawn((MapPosition::new(0.0, 25.0), StuckTo::follow_x_only(target)))
            .id();

        tick_stuckto(&mut world);

        let pos = world.get::<MapPosition>(follower).unwrap();
        assert!(approx_eq(pos.pos.x, 100.0));
        assert!(approx_eq(pos.pos.y, 25.0)); // Y unchanged
    }

    #[test]
    fn stuckto_follows_target_y_only() {
        let mut world = World::new();

        let target = world.spawn((MapPosition::new(100.0, 50.0),)).id();
        let follower = world
            .spawn((MapPosition::new(30.0, 0.0), StuckTo::follow_y_only(target)))
            .id();

        tick_stuckto(&mut world);

        let pos = world.get::<MapPosition>(follower).unwrap();
        assert!(approx_eq(pos.pos.x, 30.0)); // X unchanged
        assert!(approx_eq(pos.pos.y, 50.0));
    }

    #[test]
    fn stuckto_applies_offset() {
        let mut world = World::new();

        let target = world.spawn((MapPosition::new(100.0, 100.0),)).id();
        let follower = world
            .spawn((
                MapPosition::new(0.0, 0.0),
                StuckTo::new(target).with_offset(Vec2 { x: 10.0, y: -20.0 }),
            ))
            .id();

        tick_stuckto(&mut world);

        let pos = world.get::<MapPosition>(follower).unwrap();
        assert!(approx_eq(pos.pos.x, 110.0));
        assert!(approx_eq(pos.pos.y, 80.0));
    }

    #[test]
    fn stuckto_does_not_move_if_target_missing() {
        let mut world = World::new();

        // Create a fake entity ID that doesn't exist
        let fake_target = Entity::from_bits(99999);
        let follower = world
            .spawn((MapPosition::new(50.0, 50.0), StuckTo::new(fake_target)))
            .id();

        tick_stuckto(&mut world);

        let pos = world.get::<MapPosition>(follower).unwrap();
        assert!(approx_eq(pos.pos.x, 50.0)); // Unchanged
        assert!(approx_eq(pos.pos.y, 50.0));
    }

    /// Hierarchy takes precedence: a `ChildOf` entity is positioned by
    /// `propagate_transforms`, so `StuckTo` must leave it alone.
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
}
