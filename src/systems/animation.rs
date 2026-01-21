//! Animation systems.
//!
//! - [`animation`] advances animations based on elapsed time and updates the
//!   visible sprite frame. It also emits optional signals as frames change.
//! - [`animation_controller`] selects which animation should be active based
//!   on a set of rule conditions evaluated against entity [`Signals`](crate::components::signals::Signals).
//!
//! # Animation Flow
//!
//! 1. Animation data is defined in [`AnimationStore`](crate::resources::animationstore::AnimationStore)
//! 2. Entities have an [`Animation`](crate::components::animation::Animation) component pointing to a key
//! 3. The `animation` system advances frames based on `fps` and updates [`Sprite`](crate::components::sprite::Sprite) offset
//! 4. The `animation_controller` system evaluates rules against signals to switch animations
//!
//! # Related
//!
//! - [`crate::components::animation::Animation`] – per-entity animation state
//! - [`crate::components::animation::AnimationController`] – rule-based animation selection
//! - [`crate::resources::animationstore::AnimationStore`] – animation definitions

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::animation::{Animation, AnimationController, CmpOp, Condition};
use crate::components::mapposition::MapPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::resources::animationstore::AnimationStore;
use crate::resources::worldtime::WorldTime;

/// Advance animation playback and update the sprite frame.
///
/// Contract
/// - Reads [`WorldTime`] for the unscaled delta.
/// - Looks up animation data from [`AnimationStore`].
/// - Mutates [`Animation`] component state and [`Sprite`] frame index.
/// - Optionally writes signal flags/scalars for transitions.
pub fn animation(
    mut query: Query<(&mut Animation, &mut Sprite, Option<&mut Signals>), With<MapPosition>>,
    animation_store: Res<AnimationStore>,
    time: Res<WorldTime>,
) {
    for (mut anim_comp, mut sprite, mut maybe_signals) in query.iter_mut() {
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
                        if let Some(signals) = maybe_signals.as_mut() {
                            signals.set_flag("animation_ended");
                        }
                        // TODO: Trigger animation end event
                        break;
                    }
                } else {
                    if let Some(signals) = maybe_signals.as_mut() {
                        signals.clear_flag("animation_ended");
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

/// Evaluate a controller condition against an entity's current signals.
///
/// Recursively evaluates conditions including `All`, `Any`, and `Not`
/// combinators. Returns true if the condition is satisfied.
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

/// Select the active animation track according to controller rules.
///
/// The first matching rule wins. If no rules match, the controller's default
/// target is used. When the selected key differs from the current one, the
/// animation state is reset.
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
