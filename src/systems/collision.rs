use bevy_ecs::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::mapposition::MapPosition;
// use crate::resources::worldtime::WorldTime; // Collisions are independent of time

pub fn collision(mut query: Query<(Entity, &mut MapPosition, &BoxCollider)>) {
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

    // TODO: for each pair of entities, we emit a collision event
    for (entity_a, entity_b) in pairs {
        // Here you would typically emit a collision event
        // For now, we just print the entities involved in the collision
        println!(
            "Collision detected between {:?} and {:?}",
            entity_a, entity_b
        );
    }
}
