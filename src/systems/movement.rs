//! Movement system.
//!
//! Integrates entity positions from their current rigid body velocities and
//! the world's unscaled delta time. As a temporary demo behavior, entities
//! bounce off the screen edges by inverting velocity when they leave bounds.
//use std::ops::Add;

use bevy_ecs::prelude::*;
//use raylib::camera::Camera2D;
//use raylib::prelude::*;

use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::events::audio::AudioCmd;
use crate::resources::screensize::ScreenSize;
use crate::resources::worldtime::WorldTime;

/// Apply velocity to `MapPosition` using the frame's delta time.
///
/// Also emits optional audio and signal updates when bouncing or moving.
pub fn movement(
    mut query: Query<(
        Entity,
        &mut MapPosition,
        &mut RigidBody,
        Option<&mut Signals>,
    )>,
    time: Res<WorldTime>,
    screensize: Res<ScreenSize>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    for (_entity, mut position, mut rigidbody, mut maybe_signals) in query.iter_mut() {
        // If the entity is going to be outside the camera bounds, bounce the borders
        // by inverting the velocity
        // This is temporal for the demo purposes

        /* let x_min = 0.0_f32;
        let y_min = 0.0_f32;
        let x_max = screensize.w as f32;
        let y_max = screensize.h as f32;

        let outside_x = position.pos.x < x_min || position.pos.x > x_max;
        let outside_y = position.pos.y < y_min || position.pos.y > y_max;

        if outside_x || outside_y {
            if outside_x {
                rigidbody.velocity.x = -rigidbody.velocity.x;
                // Play a sound effect on bounce
                let _ = audio_cmd_writer.write(AudioCmd::PlayFx { id: "growl".into() });
            }
            if outside_y {
                rigidbody.velocity.y = -rigidbody.velocity.y;
                // Play a sound effect on bounce
                let _ = audio_cmd_writer.write(AudioCmd::PlayFx { id: "growl".into() });
            }
        } */

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
        if let Some(signals) = maybe_signals.as_mut() {
            let speed = rigidbody.velocity.length();
            if speed > 0.0 {
                signals.set_flag("moving");
            } else {
                signals.clear_flag("moving");
            }
            signals.set_scalar("speed", speed);
        }
    }
}
