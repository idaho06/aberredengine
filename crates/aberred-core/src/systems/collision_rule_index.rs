//! Rebuilds the [`CollisionRuleIndex`] whenever [`CollisionRule`] entities
//! are added, changed, or removed.
//!
//! `rebuild_rule_index` always runs in `SimSet::Collision`,
//! `.before(collision_detector)`, so the index reflects this tick's rules
//! (including ones spawned this same tick via `SimSet::Spawn`) before any
//! `Overlapping` fires.
//!
//! Full rebuild on any change -- rules are few and changes are rare (scene
//! switch spawn/despawn), so incremental bucket maintenance isn't worth the
//! complexity. Steady-state cost is one or two empty change-detection
//! queries.
//!
//! `Changed<CollisionRule>` alone (no `Added`) is enough to catch newly-added
//! rules: bevy's `ComponentTicks::new` sets both `added` and `changed` to the
//! insertion tick, so `Changed` already matches a component on the tick
//! it was added.

use bevy_ecs::prelude::*;

use crate::components::collision::CollisionRule;
use crate::resources::collision_rule_index::CollisionRuleIndex;

/// Rebuilds [`CollisionRuleIndex`] from every [`CollisionRule`] entity when
/// one changed or was removed since the last run.
pub fn rebuild_rule_index(
    mut index: ResMut<CollisionRuleIndex>,
    changed: Query<(), Changed<CollisionRule>>,
    mut removed: RemovedComponents<CollisionRule>,
    rules: Query<(Entity, &CollisionRule)>,
) {
    // Drain the removal reader on every run, dirty or not, or its cursor
    // falls behind and replays removals next tick. Read it first: `||`
    // would skip the drain whenever `changed` is non-empty.
    let any_removed = removed.read().count() > 0;
    if any_removed || !changed.is_empty() {
        index.rebuild(rules.iter());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_world_with_rules() -> World {
        let mut world = World::new();
        world.insert_resource(CollisionRuleIndex::default());
        world
    }

    fn run_rebuild(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(rebuild_rule_index);
        schedule.run(world);
    }

    #[test]
    fn rebuild_populates_bucket_for_matching_pair_both_orders() {
        let mut world = build_world_with_rules();
        let e = world.spawn(CollisionRule::new("ball", "brick")).id();
        world.flush();
        run_rebuild(&mut world);

        let index = world.resource::<CollisionRuleIndex>();
        assert_eq!(index.bucket("ball", "brick").unwrap(), &[e]);
        assert_eq!(index.bucket("brick", "ball").unwrap(), &[e]);
    }

    #[test]
    fn rebuild_after_removal_empties_bucket() {
        let mut world = build_world_with_rules();
        let e = world.spawn(CollisionRule::new("ball", "brick")).id();
        world.flush();
        run_rebuild(&mut world);
        assert!(
            world
                .resource::<CollisionRuleIndex>()
                .bucket("ball", "brick")
                .is_some()
        );

        world.despawn(e);
        run_rebuild(&mut world);

        assert!(
            world
                .resource::<CollisionRuleIndex>()
                .bucket("ball", "brick")
                .is_none()
        );
    }

    #[test]
    fn rebuild_sorts_bucket_by_entity() {
        let mut world = build_world_with_rules();
        // Spawn in descending id order so an unsorted bucket would fail.
        let e2 = world.spawn(CollisionRule::new("a", "b")).id();
        let e1 = world.spawn(CollisionRule::new("a", "b")).id();
        world.flush();
        run_rebuild(&mut world);

        let index = world.resource::<CollisionRuleIndex>();
        let bucket = index.bucket("a", "b").unwrap();
        assert!(bucket.is_sorted());
        assert!(bucket.contains(&e1) && bucket.contains(&e2));
    }
}
