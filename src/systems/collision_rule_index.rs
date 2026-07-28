//! Rebuilds [`CollisionRuleIndex`] whenever `CollisionRule`/`LuaCollisionRule`
//! entities are added, changed, or removed.
//!
//! Runs in `SimSet::Collision`, `.before(collision_detector)`, so the index
//! reflects this tick's rules (including ones spawned this same tick via
//! `SimSet::Spawn`) before any `CollisionEvent` fires.
//!
//! Full rebuild on any change -- rules are few and changes are rare (scene
//! switch spawn/despawn), so incremental bucket maintenance isn't worth the
//! complexity. Steady-state cost is one or two empty change-detection
//! queries.
//!
//! Two separate fn definitions under `#[cfg(feature = "lua")]`/
//! `#[cfg(not(feature = "lua"))]` (bevy `SystemParam`s can't be
//! conditionally compiled inline) sharing [`RuleBuckets::rebuild`](crate::resources::collision_rule_index).
//!
//! `Changed<T>` alone (no `Added<T>`) is enough to catch newly-added rules:
//! bevy's `ComponentTicks::new` sets both `added` and `changed` to the
//! insertion tick, so `Changed<T>` already matches a component on the tick
//! it was added.

use bevy_ecs::prelude::*;

use crate::components::collision::CollisionRule;
#[cfg(feature = "lua")]
use crate::components::luacollision::LuaCollisionRule;
use crate::resources::collision_rule_index::CollisionRuleIndex;

#[cfg(feature = "lua")]
pub fn rebuild_collision_rule_index(
    mut index: ResMut<CollisionRuleIndex>,
    changed_lua: Query<(), Changed<LuaCollisionRule>>,
    changed_rust: Query<(), Changed<CollisionRule>>,
    mut removed_lua: RemovedComponents<LuaCollisionRule>,
    mut removed_rust: RemovedComponents<CollisionRule>,
    all_lua: Query<(Entity, &LuaCollisionRule)>,
    all_rust: Query<(Entity, &CollisionRule)>,
) {
    // RemovedComponents readers must be drained every call regardless of the
    // dirty check outcome, or the reader cursor falls behind.
    let lua_dirty = !changed_lua.is_empty() || removed_lua.read().count() > 0;
    let rust_dirty = !changed_rust.is_empty() || removed_rust.read().count() > 0;

    if lua_dirty {
        index
            .lua
            .rebuild(all_lua.iter().map(|(e, r)| (e, &r.group_a, &r.group_b)));
    }
    if rust_dirty {
        index
            .rust
            .rebuild(all_rust.iter().map(|(e, r)| (e, &r.group_a, &r.group_b)));
    }
}

#[cfg(not(feature = "lua"))]
pub fn rebuild_collision_rule_index(
    mut index: ResMut<CollisionRuleIndex>,
    changed_rust: Query<(), Changed<CollisionRule>>,
    mut removed_rust: RemovedComponents<CollisionRule>,
    all_rust: Query<(Entity, &CollisionRule)>,
) {
    let rust_dirty = !changed_rust.is_empty() || removed_rust.read().count() > 0;

    if rust_dirty {
        index
            .rust
            .rebuild(all_rust.iter().map(|(e, r)| (e, &r.group_a, &r.group_b)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::collision::CollisionCallback;

    fn dummy_callback(
        _a: Entity,
        _b: Entity,
        _sa: &crate::components::collision::BoxSides,
        _sb: &crate::components::collision::BoxSides,
        _ctx: &mut crate::systems::GameCtx,
    ) {
    }

    fn build_world_with_rules() -> World {
        let mut world = World::new();
        world.insert_resource(CollisionRuleIndex::default());
        world
    }

    fn run_rebuild(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(rebuild_collision_rule_index);
        schedule.run(world);
    }

    #[test]
    fn rebuild_populates_bucket_for_matching_pair_both_orders() {
        let mut world = build_world_with_rules();
        let e = world
            .spawn(CollisionRule::rust("ball", "brick", dummy_callback as CollisionCallback))
            .id();
        world.flush();
        run_rebuild(&mut world);

        let index = world.resource::<CollisionRuleIndex>();
        assert_eq!(index.rust_bucket("ball", "brick").unwrap().as_slice(), &[e]);
        assert_eq!(index.rust_bucket("brick", "ball").unwrap().as_slice(), &[e]);
    }

    #[test]
    fn rebuild_after_removal_empties_bucket() {
        let mut world = build_world_with_rules();
        let e = world
            .spawn(CollisionRule::rust("ball", "brick", dummy_callback as CollisionCallback))
            .id();
        world.flush();
        run_rebuild(&mut world);
        assert!(
            world
                .resource::<CollisionRuleIndex>()
                .rust_bucket("ball", "brick")
                .is_some()
        );

        world.despawn(e);
        run_rebuild(&mut world);

        assert!(
            world
                .resource::<CollisionRuleIndex>()
                .rust_bucket("ball", "brick")
                .is_none()
        );
    }

    #[test]
    fn rebuild_sorts_bucket_by_entity() {
        let mut world = build_world_with_rules();
        // Spawn in descending id order so an unsorted bucket would fail.
        let e2 = world
            .spawn(CollisionRule::rust("a", "b", dummy_callback as CollisionCallback))
            .id();
        let e1 = world
            .spawn(CollisionRule::rust("a", "b", dummy_callback as CollisionCallback))
            .id();
        world.flush();
        run_rebuild(&mut world);

        let index = world.resource::<CollisionRuleIndex>();
        let bucket = index.rust_bucket("a", "b").unwrap();
        let mut sorted = bucket.to_vec();
        sorted.sort_unstable();
        assert_eq!(bucket.as_slice(), sorted.as_slice());
        assert!(bucket.contains(&e1) && bucket.contains(&e2));
    }
}
