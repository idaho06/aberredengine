use bevy_ecs::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::mapposition::MapPosition;
use crate::events::collision::CollisionEvent;
// use crate::resources::worldtime::WorldTime; // Collisions are independent of time

pub fn collision(
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
