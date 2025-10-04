//use std::ops::Add;

use bevy_ecs::prelude::*;
//use raylib::camera::Camera2D;
//use raylib::prelude::*;

use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::screensize::ScreenSize;
use crate::resources::worldtime::WorldTime;

pub fn movement(
    mut query: Query<(Entity, &mut MapPosition, &mut RigidBody)>,
    time: Res<WorldTime>,
    screensize: Res<ScreenSize>,
    _camera: Res<Camera2DRes>,
) {
    for (_entity, mut position, mut rigidbody) in query.iter_mut() {
        // If the entity is going to be outside the camera bounds, bounce the borders
        // by inverting the velocity

        let x_min = 0.0_f32;
        let y_min = 0.0_f32;
        let x_max = screensize.w as f32;
        let y_max = screensize.h as f32;

        let outside_x = position.pos.x < x_min || position.pos.x > x_max;
        let outside_y = position.pos.y < y_min || position.pos.y > y_max;

        if outside_x || outside_y {
            if outside_x {
                rigidbody.velocity.x = -rigidbody.velocity.x;
            }
            if outside_y {
                rigidbody.velocity.y = -rigidbody.velocity.y;
            }
        }

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
