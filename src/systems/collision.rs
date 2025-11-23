//! Collision detection system.
//!
//! Performs pairwise AABB overlap checks between entities that carry a
//! [`BoxCollider`](crate::components::boxcollider::BoxCollider) and a
//! [`MapPosition`](crate::components::mapposition::MapPosition). For every
//! overlapping pair it triggers a [`CollisionEvent`](crate::events::collision::CollisionEvent).

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::boxcollider::BoxCollider;
use crate::components::collision::{CollisionContext, CollisionRule};
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
#[derive(SystemParam)]
pub struct CollisionObserverParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub groups: Query<'w, 's, &'static Group>,
    pub rules: Query<'w, 's, &'static CollisionRule>,
    pub positions: Query<'w, 's, &'static mut MapPosition>,
    pub rigidbodies: Query<'w, 's, &'static mut RigidBody>,
    // pub signals: Query<'w, 's, &'static mut Signal>,
}

pub fn collision_observer(trigger: On<CollisionEvent>, mut params: CollisionObserverParams) {
    let a = trigger.event().a;
    let b = trigger.event().b;

    //eprintln!("Collision detected: {:?} and {:?}", a, b);
    let ga = if let Ok(group) = params.groups.get(a) {
        group.name()
    } else {
        return;
    };
    let gb = if let Ok(group) = params.groups.get(b) {
        group.name()
    } else {
        return;
    };

    for rule in params.rules.iter() {
        if rule.matches(ga, gb) {
            //eprintln!(
            //    "Collision rule matched for groups '{}' and '{}'",
            //    ga, gb
            //);
            {
                let callback = rule.callback;
                let mut ctx = CollisionContext {
                    commands: &mut params.commands,
                    groups: &params.groups,
                    positions: &mut params.positions,
                    rigidbodies: &mut params.rigidbodies,
                    // signals: &mut params.signals,
                };
                callback(a, b, &mut ctx);
            }
            break; // ensure borrows end before next iteration
        }
    }
}
