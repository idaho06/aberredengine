//! Shared collision helpers used by both the Lua and Rust collision observers.
//!
//! This module contains system-level utility functions that de-duplicate logic
//! common to `lua_collision` (`aberred-lua`) and
//! [`rust_collision`](crate::systems::rust_collision).
//!
//! All functions are pure Rust with no Lua dependency and are always compiled
//! regardless of the `lua` feature flag.
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – AABB detection system
//! - [`crate::systems::rust_collision`] – Rust collision observer
//! - `aberred_lua::systems::lua_collision` – Lua collision observer
//! - [`crate::components::collision`] – collision types and side detection

use bevy_ecs::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::collision::{BoxSides, get_colliding_sides, match_groups};
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::math::Rect;

/// Resolve the world position of an entity.
///
/// Uses [`GlobalTransform2D`] when available, otherwise falls back to the
/// local [`MapPosition`]. Returns `None` if the entity has no position.
pub fn resolve_world_pos(
    positions: &Query<&MapPosition>,
    global_transforms: &Query<&GlobalTransform2D>,
    entity: Entity,
) -> Option<crate::math::Vec2> {
    positions.get(entity).ok().map(|p| {
        global_transforms
            .get(entity)
            .ok()
            .map_or(p.pos, |gt| gt.position)
    })
}

/// Compute the collider rectangle for an entity at its world position.
///
/// Combines [`resolve_world_pos`] with [`BoxCollider::as_rectangle`] in one call.
pub fn resolve_collider_rect(
    positions: &Query<&MapPosition>,
    global_transforms: &Query<&GlobalTransform2D>,
    box_colliders: &Query<&BoxCollider>,
    entity: Entity,
) -> Option<Rect> {
    let pos = resolve_world_pos(positions, global_transforms, entity)?;
    box_colliders.get(entity).ok().map(|c| c.as_rectangle(pos))
}

/// Compute colliding sides for two optional rectangles.
///
/// Returns `(BoxSides, BoxSides)` — both empty if either rectangle is `None`
/// or if there is no overlap.
pub fn compute_sides(rect_a: Option<Rect>, rect_b: Option<Rect>) -> (BoxSides, BoxSides) {
    match (rect_a, rect_b) {
        (Some(ra), Some(rb)) => get_colliding_sides(&ra, &rb).unwrap_or_default(),
        _ => Default::default(),
    }
}

/// Resolve group names for two entities.
///
/// Returns `None` if either entity lacks a [`Group`] component.
pub fn resolve_groups<'q>(
    groups: &'q Query<&Group>,
    a: Entity,
    b: Entity,
) -> Option<(&'q str, &'q str)> {
    let ga = groups.get(a).ok()?;
    let gb = groups.get(b).ok()?;
    Some((ga.name(), gb.name()))
}

/// Scan a [`CollisionRuleIndex`](crate::resources::collision_rule_index::CollisionRuleIndex)
/// bucket for the first rule whose groups still match `(ga, gb)`, skipping
/// any entity that despawned between this tick's index rebuild and now.
///
/// `lookup` returns a rule entity's `(group_a, group_b, rule)`, or `None` if
/// it no longer is a rule; `rule` is whatever the caller needs back. Returns
/// `(rule, ent_a, ent_b)`, with `ent_a`/`ent_b` ordered to match the rule's
/// `group_a`/`group_b`.
///
/// Shared by [`rust_collision_observer`](crate::systems::rust_collision::rust_collision_observer)
/// and `lua_collision_observer` (`aberred-lua`), which query different rule
/// components.
pub fn find_matching_rule<'q, R>(
    bucket: &[Entity],
    lookup: impl Fn(Entity) -> Option<(&'q str, &'q str, R)>,
    a: Entity,
    b: Entity,
    ga: &str,
    gb: &str,
) -> Option<(R, Entity, Entity)> {
    bucket.iter().find_map(|&rule_entity| {
        let (rule_a, rule_b, rule) = lookup(rule_entity)?;
        let (ent_a, ent_b) = match_groups(rule_a, rule_b, a, b, ga, gb)?;
        Some((rule, ent_a, ent_b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::approx_eq;
    use bevy_ecs::system::SystemState;
    use bevy_ecs::world::World;

    // --- resolve_groups tests ---

    #[test]
    fn resolve_groups_both_present() {
        let mut world = World::new();
        let a = world.spawn(Group::new("player")).id();
        let b = world.spawn(Group::new("enemy")).id();

        let mut state = SystemState::<Query<&Group>>::new(&mut world);
        let groups = state.get(&world).expect("Group query should fetch");

        let result = resolve_groups(&groups, a, b);
        assert_eq!(result, Some(("player", "enemy")));
    }

    #[test]
    fn resolve_groups_a_missing() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn(Group::new("enemy")).id();

        let mut state = SystemState::<Query<&Group>>::new(&mut world);
        let groups = state.get(&world).expect("Group query should fetch");

        assert_eq!(resolve_groups(&groups, a, b), None);
    }

    #[test]
    fn resolve_groups_b_missing() {
        let mut world = World::new();
        let a = world.spawn(Group::new("player")).id();
        let b = world.spawn_empty().id();

        let mut state = SystemState::<Query<&Group>>::new(&mut world);
        let groups = state.get(&world).expect("Group query should fetch");

        assert_eq!(resolve_groups(&groups, a, b), None);
    }

    #[test]
    fn resolve_groups_both_missing() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();

        let mut state = SystemState::<Query<&Group>>::new(&mut world);
        let groups = state.get(&world).expect("Group query should fetch");

        assert_eq!(resolve_groups(&groups, a, b), None);
    }

    // --- compute_sides tests ---

    #[test]
    fn compute_sides_both_none() {
        let (sa, sb) = compute_sides(None, None);
        assert!(sa.is_empty());
        assert!(sb.is_empty());
    }

    #[test]
    fn compute_sides_one_none() {
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let (sa, sb) = compute_sides(Some(rect), None);
        assert!(sa.is_empty());
        assert!(sb.is_empty());
    }

    #[test]
    fn compute_sides_no_overlap() {
        let ra = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rb = Rect {
            x: 50.0,
            y: 50.0,
            width: 10.0,
            height: 10.0,
        };
        let (sa, sb) = compute_sides(Some(ra), Some(rb));
        assert!(sa.is_empty());
        assert!(sb.is_empty());
    }

    #[test]
    fn compute_sides_overlap() {
        use crate::components::collision::BoxSide;
        let ra = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rb = Rect {
            x: 8.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let (sa, sb) = compute_sides(Some(ra), Some(rb));
        assert!(sa.iter().any(|s| matches!(s, BoxSide::Right)));
        assert!(sb.iter().any(|s| matches!(s, BoxSide::Left)));
    }

    // --- resolve_world_pos tests ---

    #[test]
    fn resolve_world_pos_no_position() {
        let mut world = World::new();
        let e = world.spawn_empty().id();

        let mut state =
            SystemState::<(Query<&MapPosition>, Query<&GlobalTransform2D>)>::new(&mut world);
        let (positions, global_transforms) = state
            .get(&world)
            .expect("Position and transform queries should fetch");

        assert!(resolve_world_pos(&positions, &global_transforms, e).is_none());
    }

    #[test]
    fn resolve_world_pos_local_only() {
        let mut world = World::new();
        let e = world.spawn(MapPosition::new(5.0, 10.0)).id();

        let mut state =
            SystemState::<(Query<&MapPosition>, Query<&GlobalTransform2D>)>::new(&mut world);
        let (positions, global_transforms) = state
            .get(&world)
            .expect("Position and transform queries should fetch");

        let result = resolve_world_pos(&positions, &global_transforms, e);
        assert!(result.is_some());
        let v = result.unwrap();
        assert!(approx_eq(v.x, 5.0));
        assert!(approx_eq(v.y, 10.0));
    }

    #[test]
    fn resolve_world_pos_with_global_transform() {
        let mut world = World::new();
        let e = world
            .spawn((
                MapPosition::new(5.0, 10.0),
                GlobalTransform2D {
                    position: crate::math::Vec2 { x: 100.0, y: 200.0 },
                    rotation_degrees: 0.0,
                    scale: crate::math::Vec2 { x: 1.0, y: 1.0 },
                },
            ))
            .id();

        let mut state =
            SystemState::<(Query<&MapPosition>, Query<&GlobalTransform2D>)>::new(&mut world);
        let (positions, global_transforms) = state
            .get(&world)
            .expect("Position and transform queries should fetch");

        let result = resolve_world_pos(&positions, &global_transforms, e);
        assert!(result.is_some());
        let v = result.unwrap();
        // Should prefer GlobalTransform2D over local MapPosition
        assert!(approx_eq(v.x, 100.0));
        assert!(approx_eq(v.y, 200.0));
    }

    // --- resolve_collider_rect tests ---

    #[test]
    fn resolve_collider_rect_no_components() {
        let mut world = World::new();
        let e = world.spawn_empty().id();

        let mut state = SystemState::<(
            Query<&MapPosition>,
            Query<&GlobalTransform2D>,
            Query<&BoxCollider>,
        )>::new(&mut world);
        let (positions, global_transforms, box_colliders) =
            state.get(&world).expect("Collider queries should fetch");

        assert!(resolve_collider_rect(&positions, &global_transforms, &box_colliders, e).is_none());
    }

    #[test]
    fn resolve_collider_rect_with_components() {
        let mut world = World::new();
        let e = world
            .spawn((MapPosition::new(10.0, 20.0), BoxCollider::new(30.0, 40.0)))
            .id();

        let mut state = SystemState::<(
            Query<&MapPosition>,
            Query<&GlobalTransform2D>,
            Query<&BoxCollider>,
        )>::new(&mut world);
        let (positions, global_transforms, box_colliders) =
            state.get(&world).expect("Collider queries should fetch");

        let rect = resolve_collider_rect(&positions, &global_transforms, &box_colliders, e);
        assert!(rect.is_some());
        let r = rect.unwrap();
        assert!(approx_eq(r.width, 30.0));
        assert!(approx_eq(r.height, 40.0));
    }
}
