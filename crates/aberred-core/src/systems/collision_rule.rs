//! Rule-matched collision events.
//!
//! [`collision_rule_observer`] receives each [`CollisionEvent`], finds the
//! [`CollisionRule`] covering the pair's groups, and triggers [`Collided`] on
//! that rule's entity.
//!
//! # Collision Flow
//!
//! 1. [`collision_detector`](crate::systems::collision_detector::collision_detector) detects overlaps
//!    and triggers `CollisionEvent`s
//! 2. `collision_rule_observer` looks up the matching rule by
//!    [`Group`] names
//! 3. It computes the contact sides and triggers `Collided` on the rule entity
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – collision detection
//! - [`crate::components::collision::CollisionRule`] – group-pair rule
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision`] – `CollisionEvent` and `Collided`

use bevy_ecs::prelude::*;

use crate::components::collision::CollisionRule;
use crate::components::group::Group;
use crate::events::collision::{Collided, CollisionEvent};
use crate::resources::collision_rule_index::CollisionRuleIndex;
use crate::systems::collision::{ColliderRects, compute_sides, find_matching_rule, resolve_groups};

/// Observer that turns a [`CollisionEvent`] into a [`Collided`] on the
/// matching [`CollisionRule`] entity.
///
/// 1. Looks up [`Group`] names for both entities (returns early if missing)
/// 2. Scans the [`CollisionRuleIndex`] bucket for the pair's groups for a
///    matching rule (deterministic lowest-`Entity`-first if more than one
///    covers the pair)
/// 3. Computes contact sides via [`compute_sides`]
/// 4. Triggers `Collided` with `a`/`b` in the rule's group order
pub fn collision_rule_observer(
    trigger: On<CollisionEvent>,
    rules: Query<&CollisionRule>,
    index: Res<CollisionRuleIndex>,
    groups: Query<&Group>,
    rects: ColliderRects,
    mut commands: Commands,
) {
    if index.is_empty() {
        return;
    }

    let CollisionEvent { a, b } = *trigger.event();

    let Some((ga, gb)) = resolve_groups(&groups, a, b) else {
        return;
    };

    let Some(bucket) = index.bucket(ga, gb) else {
        return;
    };

    let lookup = |e| rules.get(e).ok().map(|r| (&*r.group_a, &*r.group_b, e));
    let Some((rule, ent_a, ent_b)) = find_matching_rule(bucket, lookup, a, b, ga, gb) else {
        return;
    };

    let (sides_a, sides_b) = compute_sides(rects.rect(ent_a), rects.rect(ent_b));

    commands.trigger(Collided {
        rule,
        a: ent_a,
        b: ent_b,
        sides_a,
        sides_b,
    });
}
