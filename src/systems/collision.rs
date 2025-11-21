//! Collision detection system.
//!
//! Performs pairwise AABB overlap checks between entities that carry a
//! [`BoxCollider`](crate::components::boxcollider::BoxCollider) and a
//! [`MapPosition`](crate::components::mapposition::MapPosition). For every
//! overlapping pair it triggers a [`CollisionEvent`](crate::events::collision::CollisionEvent).

use bevy_ecs::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::collision::CollisionRule;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::events::collision::CollisionEvent;
// use crate::resources::worldtime::WorldTime; // Collisions are independent of time

/// Broad-phase pairwise overlap test with event emission.
///
/// Uses ECS `iter_combinations_mut()` to efficiently iterate unique pairs,
/// checks overlap, and triggers an event for each collision. Observers can
/// react to despawn, apply damage, or play sounds.
pub fn collision_detector(
    mut query: Query<(Entity, &mut MapPosition, &BoxCollider)>,
    mut commands: Commands,
) {
    // first we create a Vector of pairs of entities
    let mut pairs: Vec<(Entity, Entity)> = Vec::new();

    let mut combos = query.iter_combinations_mut();
    while let Some(
        [
            (entity_a, position_a, collider_a),
            (entity_b, position_b, collider_b),
        ],
    ) = combos.fetch_next()
    {
        if collider_a.overlaps(position_a.pos, collider_b, position_b.pos) {
            pairs.push((entity_a, entity_b));
        }
    }

    // Trigger a CollisionEvent for each pair. Observers will run immediately when commands flush.
    for (entity_a, entity_b) in pairs {
        // println!(
        //     "Triggering CollisionEvent between {:?} and {:?}",
        //     entity_a, entity_b
        // );
        commands.trigger(CollisionEvent {
            a: entity_a,
            b: entity_b,
        });
    }
}

/// Global observer when a CollisionEvent is triggered.
///
pub fn collision_observer(
    trigger: On<CollisionEvent>,
    mut commands: Commands,
    groups: Query<&Group>,
    rules: Query<&CollisionRule>,
    mut positions: Query<&mut MapPosition>,
    mut rigidbodies: Query<&mut RigidBody>,
) {
    let a = trigger.event().a;
    let b = trigger.event().b;

    //eprintln!("Collision detected: {:?} and {:?}", a, b);
    let ga = if let Ok(group) = groups.get(a) {
        group.name()
    } else {
        return;
    };
    let gb = if let Ok(group) = groups.get(b) {
        group.name()
    } else {
        return;
    };

    for rule in rules.iter() {
        if rule.matches(ga, gb) {
            //eprintln!(
            //    "Collision rule matched for groups '{}' and '{}'",
            //    ga, gb
            //);
            (rule.callback)(
                a,
                b,
                &mut commands,
                &groups,
                &mut positions,
                &mut rigidbodies,
            );
        }
    }
}
