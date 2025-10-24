use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::animation::{Animation, AnimationController, CmpOp, Condition};
use crate::components::signals::Signals;
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
                        // TODO: Trigger animation end event or put signal
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

// Evaluate a condition against the current signals
fn evaluate_condition(signals: &Signals, condition: &Condition) -> bool {
    match condition {
        Condition::ScalarCmp { key, op, value } => {
            if let Some(signal_value) = signals.get_scalar(key) {
                match op {
                    CmpOp::Lt => signal_value < *value,
                    CmpOp::Le => signal_value <= *value,
                    CmpOp::Gt => signal_value > *value,
                    CmpOp::Ge => signal_value >= *value,
                    CmpOp::Eq => (signal_value - *value).abs() < f32::EPSILON,
                    CmpOp::Ne => (signal_value - *value).abs() >= f32::EPSILON,
                }
            } else {
                false
            }
        }
        Condition::ScalarRange {
            key,
            min,
            max,
            inclusive,
        } => {
            if let Some(signal_value) = signals.get_scalar(key) {
                if *inclusive {
                    signal_value >= *min && signal_value <= *max
                } else {
                    signal_value > *min && signal_value < *max
                }
            } else {
                false
            }
        }
        Condition::IntegerCmp { key, op, value } => {
            if let Some(signal_value) = signals.get_integer(key) {
                match op {
                    CmpOp::Lt => signal_value < *value,
                    CmpOp::Le => signal_value <= *value,
                    CmpOp::Gt => signal_value > *value,
                    CmpOp::Ge => signal_value >= *value,
                    CmpOp::Eq => signal_value == *value,
                    CmpOp::Ne => signal_value != *value,
                }
            } else {
                false
            }
        }
        Condition::IntegerRange {
            key,
            min,
            max,
            inclusive,
        } => {
            if let Some(signal_value) = signals.get_integer(key) {
                if *inclusive {
                    signal_value >= *min && signal_value <= *max
                } else {
                    signal_value > *min && signal_value < *max
                }
            } else {
                false
            }
        }
        Condition::HasFlag { key } => signals.has_flag(key),
        Condition::LacksFlag { key } => !signals.has_flag(key),
        Condition::All(conditions) => conditions
            .iter()
            .all(|cond| evaluate_condition(signals, cond)),
        Condition::Any(conditions) => conditions
            .iter()
            .any(|cond| evaluate_condition(signals, cond)),
        Condition::Not(cond) => !evaluate_condition(signals, cond),
    }
}

pub fn animation_controller(
    mut query: Query<(&mut AnimationController, &mut Animation, &Signals)>,
) {
    for (mut controller, mut animation, signals) in query.iter_mut() {
        let mut selected: Option<&str> = None;
        for rule in &controller.rules {
            if evaluate_condition(signals, &rule.when) {
                selected = Some(rule.set_key.as_str());
                break;
            }
        }
        let target_key = match selected {
            Some(s) => s.to_string(),
            None => controller.fallback_key.clone(),
        };
        if animation.animation_key != target_key {
            animation.animation_key = target_key.clone();
            animation.frame_index = 0;
            animation.elapsed_time = 0.0;
            controller.current_key = target_key;
        }
    }
}
