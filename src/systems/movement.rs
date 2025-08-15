//use std::ops::Add;

use bevy_ecs::prelude::*;
//use raylib::prelude::*;

use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::resources::worldtime::WorldTime;

pub fn movement_system(
    mut query: Query<(Entity, &mut MapPosition, &RigidBody)>,
    time: Res<WorldTime>,
) {
    for (_entity, mut position, rigidbody) in query.iter_mut() {
        //position.x += rigidbody.velocity.x * time.delta_seconds();
        //position.y += rigidbody.velocity.y * time.delta_seconds();
        //position += rigidbody.velocity() * time.delta_seconds();
        //let delta = rigidbody.velocity.scale_by(time.delta);
        //position.pos = position.pos.add(delta);
        //position.pos = position.pos + delta;
        let delta = rigidbody.velocity * time.delta;
        position.pos += delta;

        // get entity index
        /*
        let entity_index = entity.index();
        println!(
            "Entity {:?} moved to position {:?}",
            entity_index, position.pos
        );
        */
    }
}
