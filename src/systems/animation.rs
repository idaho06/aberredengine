use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::animation::Animation;
use crate::components::sprite::Sprite;
use crate::resources::animationstore::AnimationStore;
use crate::resources::worldtime::WorldTime;

pub fn animation(
    mut query: Query<(&mut Animation, &mut Sprite)>,
    animation_store: Res<AnimationStore>,
    time: Res<WorldTime>,
) {
    for (mut anim_comp, mut sprite) in query.iter_mut() {
        if let Some(animation) = animation_store.animations.get(&anim_comp.animation_key) {
            anim_comp.elapsed_time += time.delta;

            let frame_duration = 1.0 / animation.fps;
            if anim_comp.elapsed_time >= frame_duration {
                anim_comp.frame_index += 1;
                anim_comp.elapsed_time -= frame_duration;

                if anim_comp.frame_index >= animation.frame_count {
                    if animation.looped {
                        anim_comp.frame_index = 0;
                    } else {
                        anim_comp.frame_index = animation.frame_count - 1; // stay on last frame
                        // TODO: Trigger animation end event
                    }
                }
            }

            // Update sprite offset based on current frame
            let frame_x =
                animation.position.x + (anim_comp.frame_index as f32 * animation.displacement);
            // Assuming vertical position remains constant for horizontal sprite sheets
            let frame_y = animation.position.y;

            // Update the sprite's offset to display the correct frame
            sprite.offset = Vector2 {
                x: frame_x,
                y: frame_y,
            };
        }
    }
}
