//! Animation systems.
//!
//! - [`animation`] advances animations based on elapsed time and updates the
//!   visible sprite frame. It also emits optional signals as frames change.
//! - [`animation_controller`] selects which animation should be active based
//!   on a set of rule conditions evaluated against entity [`Signals`].
//!
//! # Animation Flow
//!
//! 1. Animation data is defined in [`AnimationStore`]
//! 2. Entities have an [`Animation`] component pointing to a key
//! 3. The `animation` system advances frames based on `fps` and updates [`Sprite`] offset
//! 4. The `animation_controller` system evaluates rules against signals to switch animations
//!
//! # Related
//!
//! - [`crate::components::animation::Animation`] – per-entity animation state
//! - [`crate::components::animation::AnimationController`] – rule-based animation selection
//! - [`crate::resources::animationstore::AnimationStore`] – animation definitions

use crate::math::Vec2;
use bevy_ecs::prelude::*;

use crate::components::animation::{Animation, AnimationController, CmpOp, Condition};
use crate::components::mapposition::MapPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::events::animation::AnimationFinishedEvent;
use crate::resources::animationstore::AnimationStore;
use crate::resources::signal_keys as sk;
use crate::resources::texturedims::TextureDimsStore;
use crate::resources::worldtime::WorldTime;

/// Advance animation playback and update the sprite frame.
///
/// Contract
/// - Reads [`WorldTime`] for the unscaled delta.
/// - Looks up animation data from [`AnimationStore`].
/// - Mutates [`Animation`] component state and [`Sprite`] frame index.
/// - Optionally writes signal flags/scalars for transitions.
/// - When `vertical_displacement > 0`, wraps frames to the next row when
///   the computed x offset exceeds the texture width.
/// - Triggers [`AnimationFinishedEvent`]
///   exactly once on the frame a non-looped animation first reaches its last frame.
pub fn animation(
    mut query: Query<
        (Entity, &mut Animation, &mut Sprite, Option<&mut Signals>),
        With<MapPosition>,
    >,
    animation_store: Res<AnimationStore>,
    texture_dims: Res<TextureDimsStore>,
    time: Res<WorldTime>,
    mut commands: Commands,
) {
    crate::tracy::tracy_span!("animation");
    for (entity, mut anim_comp, mut sprite, mut maybe_signals) in query.iter_mut() {
        if let Some(animation) = animation_store.animations.get(&anim_comp.animation_key) {
            if animation.frame_count == 0 {
                continue;
            }
            if !anim_comp.finished
                && anim_comp.frame_index == 0
                && let Some(signals) = maybe_signals.as_mut()
            {
                signals.clear_flag(sk::ANIMATION_ENDED);
            }
            if anim_comp.finished {
                continue;
            }
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
                            signals.set_flag(sk::ANIMATION_ENDED);
                        }
                        if !anim_comp.finished {
                            anim_comp.finished = true;
                            commands.trigger(AnimationFinishedEvent { entity });
                        }
                    }
                } else if let Some(signals) = maybe_signals.as_mut() {
                    signals.clear_flag(sk::ANIMATION_ENDED);
                }
            }

            // Compute sprite offset for the current frame. The atlas width
            // comes from the CPU-side dims mirror — the GPU
            // TextureStore lives in the render world only.
            let tex_width = if animation.vertical_displacement > 0.0 {
                texture_dims
                    .width(animation.tex_key.as_ref())
                    .map(|w| w as f32)
            } else {
                None
            };

            sprite.offset = compute_frame_offset(
                anim_comp.frame_index,
                animation.position,
                animation.horizontal_displacement,
                animation.vertical_displacement,
                tex_width,
            );
        }
    }
}

/// Compute the sprite-sheet offset for a given frame index.
///
/// When `vertical_displacement > 0` and `tex_width` is `Some`, frames that
/// would extend past the texture width wrap to subsequent rows. The first
/// (possibly partial) row starts at `position.x`; subsequent rows start at
/// x = 0.
///
/// When `vertical_displacement == 0` or `tex_width` is `None`, frames advance
/// horizontally without wrapping (original behaviour).
pub(crate) fn compute_frame_offset(
    frame_index: usize,
    position: Vec2,
    h_disp: f32,
    v_disp: f32,
    tex_width: Option<f32>,
) -> Vec2 {
    let raw_x = position.x + (frame_index as f32 * h_disp);

    if v_disp > 0.0
        && let Some(tw) = tex_width
        && raw_x + h_disp > tw
    {
        let frames_in_first_row = ((tw - position.x) / h_disp).floor() as usize;
        if frame_index >= frames_in_first_row {
            let remaining = frame_index - frames_in_first_row;
            let frames_per_full_row = (tw / h_disp).floor() as usize;
            let row = remaining / frames_per_full_row + 1;
            let col = remaining % frames_per_full_row;
            return Vec2 {
                x: col as f32 * h_disp,
                y: position.y + row as f32 * v_disp,
            };
        }
    }

    Vec2 {
        x: raw_x,
        y: position.y,
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::testing::approx_eq;
    use bevy_ecs::system::RunSystemOnce;

    fn empty_signals() -> Signals {
        Signals::default()
    }

    fn signals_with_scalar(key: &str, value: f32) -> Signals {
        let mut s = Signals::default();
        s.set_scalar(key, value);
        s
    }

    fn signals_with_integer(key: &str, value: i32) -> Signals {
        let mut s = Signals::default();
        s.set_integer(key, value);
        s
    }

    fn signals_with_flag(key: &str) -> Signals {
        let mut s = Signals::default();
        s.set_flag(key);
        s
    }

    // --- ScalarCmp ---

    #[test]
    fn test_scalar_cmp_lt_true() {
        let signals = signals_with_scalar("speed", 5.0);
        let cond = Condition::ScalarCmp {
            key: "speed".to_string(),
            op: CmpOp::Lt,
            value: 10.0,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_cmp_lt_false() {
        let signals = signals_with_scalar("speed", 15.0);
        let cond = Condition::ScalarCmp {
            key: "speed".to_string(),
            op: CmpOp::Lt,
            value: 10.0,
        };
        assert!(!evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_cmp_le() {
        let signals = signals_with_scalar("speed", 10.0);
        let cond = Condition::ScalarCmp {
            key: "speed".to_string(),
            op: CmpOp::Le,
            value: 10.0,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_cmp_gt() {
        let signals = signals_with_scalar("speed", 15.0);
        let cond = Condition::ScalarCmp {
            key: "speed".to_string(),
            op: CmpOp::Gt,
            value: 10.0,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_cmp_ge() {
        let signals = signals_with_scalar("speed", 10.0);
        let cond = Condition::ScalarCmp {
            key: "speed".to_string(),
            op: CmpOp::Ge,
            value: 10.0,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_cmp_eq() {
        let signals = signals_with_scalar("speed", 10.0);
        let cond = Condition::ScalarCmp {
            key: "speed".to_string(),
            op: CmpOp::Eq,
            value: 10.0,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_cmp_ne() {
        let signals = signals_with_scalar("speed", 10.0);
        let cond = Condition::ScalarCmp {
            key: "speed".to_string(),
            op: CmpOp::Ne,
            value: 5.0,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_cmp_missing_key() {
        let signals = empty_signals();
        let cond = Condition::ScalarCmp {
            key: "missing".to_string(),
            op: CmpOp::Eq,
            value: 0.0,
        };
        assert!(!evaluate_condition(&signals, &cond));
    }

    // --- ScalarRange ---

    #[test]
    fn test_scalar_range_inclusive_inside() {
        let signals = signals_with_scalar("hp", 50.0);
        let cond = Condition::ScalarRange {
            key: "hp".to_string(),
            min: 0.0,
            max: 100.0,
            inclusive: true,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_range_inclusive_at_boundary() {
        let signals = signals_with_scalar("hp", 0.0);
        let cond = Condition::ScalarRange {
            key: "hp".to_string(),
            min: 0.0,
            max: 100.0,
            inclusive: true,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_range_exclusive_at_boundary() {
        let signals = signals_with_scalar("hp", 0.0);
        let cond = Condition::ScalarRange {
            key: "hp".to_string(),
            min: 0.0,
            max: 100.0,
            inclusive: false,
        };
        assert!(!evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_scalar_range_missing_key() {
        let signals = empty_signals();
        let cond = Condition::ScalarRange {
            key: "missing".to_string(),
            min: 0.0,
            max: 100.0,
            inclusive: true,
        };
        assert!(!evaluate_condition(&signals, &cond));
    }

    // --- IntegerCmp ---

    #[test]
    fn test_integer_cmp_eq() {
        let signals = signals_with_integer("level", 5);
        let cond = Condition::IntegerCmp {
            key: "level".to_string(),
            op: CmpOp::Eq,
            value: 5,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_integer_cmp_ne() {
        let signals = signals_with_integer("level", 5);
        let cond = Condition::IntegerCmp {
            key: "level".to_string(),
            op: CmpOp::Ne,
            value: 3,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_integer_cmp_lt() {
        let signals = signals_with_integer("level", 3);
        let cond = Condition::IntegerCmp {
            key: "level".to_string(),
            op: CmpOp::Lt,
            value: 5,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_integer_cmp_missing_key() {
        let signals = empty_signals();
        let cond = Condition::IntegerCmp {
            key: "missing".to_string(),
            op: CmpOp::Eq,
            value: 0,
        };
        assert!(!evaluate_condition(&signals, &cond));
    }

    // --- IntegerRange ---

    #[test]
    fn test_integer_range_inclusive_inside() {
        let signals = signals_with_integer("score", 50);
        let cond = Condition::IntegerRange {
            key: "score".to_string(),
            min: 0,
            max: 100,
            inclusive: true,
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_integer_range_exclusive_at_boundary() {
        let signals = signals_with_integer("score", 100);
        let cond = Condition::IntegerRange {
            key: "score".to_string(),
            min: 0,
            max: 100,
            inclusive: false,
        };
        assert!(!evaluate_condition(&signals, &cond));
    }

    // --- Flags ---

    #[test]
    fn test_has_flag_true() {
        let signals = signals_with_flag("moving");
        let cond = Condition::HasFlag {
            key: "moving".to_string(),
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_has_flag_false() {
        let signals = empty_signals();
        let cond = Condition::HasFlag {
            key: "moving".to_string(),
        };
        assert!(!evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_lacks_flag_true() {
        let signals = empty_signals();
        let cond = Condition::LacksFlag {
            key: "moving".to_string(),
        };
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_lacks_flag_false() {
        let signals = signals_with_flag("moving");
        let cond = Condition::LacksFlag {
            key: "moving".to_string(),
        };
        assert!(!evaluate_condition(&signals, &cond));
    }

    // --- Combinators ---

    #[test]
    fn test_all_true() {
        let mut signals = Signals::default();
        signals.set_flag("a");
        signals.set_flag("b");
        let cond = Condition::All(vec![
            Condition::HasFlag {
                key: "a".to_string(),
            },
            Condition::HasFlag {
                key: "b".to_string(),
            },
        ]);
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_all_one_false() {
        let signals = signals_with_flag("a");
        let cond = Condition::All(vec![
            Condition::HasFlag {
                key: "a".to_string(),
            },
            Condition::HasFlag {
                key: "b".to_string(),
            },
        ]);
        assert!(!evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_all_empty() {
        let signals = empty_signals();
        let cond = Condition::All(vec![]);
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_any_one_true() {
        let signals = signals_with_flag("a");
        let cond = Condition::Any(vec![
            Condition::HasFlag {
                key: "a".to_string(),
            },
            Condition::HasFlag {
                key: "b".to_string(),
            },
        ]);
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_any_none_true() {
        let signals = empty_signals();
        let cond = Condition::Any(vec![
            Condition::HasFlag {
                key: "a".to_string(),
            },
            Condition::HasFlag {
                key: "b".to_string(),
            },
        ]);
        assert!(!evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_any_empty() {
        let signals = empty_signals();
        let cond = Condition::Any(vec![]);
        assert!(!evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_not_inverts_true() {
        let signals = signals_with_flag("a");
        let cond = Condition::Not(Box::new(Condition::HasFlag {
            key: "a".to_string(),
        }));
        assert!(!evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_not_inverts_false() {
        let signals = empty_signals();
        let cond = Condition::Not(Box::new(Condition::HasFlag {
            key: "a".to_string(),
        }));
        assert!(evaluate_condition(&signals, &cond));
    }

    #[test]
    fn test_nested_combinators() {
        let mut signals = Signals::default();
        signals.set_flag("moving");
        signals.set_scalar("speed", 5.0);
        // All(HasFlag("moving"), Not(ScalarCmp(speed >= 10)))
        let cond = Condition::All(vec![
            Condition::HasFlag {
                key: "moving".to_string(),
            },
            Condition::Not(Box::new(Condition::ScalarCmp {
                key: "speed".to_string(),
                op: CmpOp::Ge,
                value: 10.0,
            })),
        ]);
        assert!(evaluate_condition(&signals, &cond));
    }

    // --- compute_frame_offset ---

    fn v2(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }

    fn assert_offset(result: Vec2, expected_x: f32, expected_y: f32) {
        assert!(
            approx_eq(result.x, expected_x) && approx_eq(result.y, expected_y),
            "expected ({}, {}), got ({}, {})",
            expected_x,
            expected_y,
            result.x,
            result.y,
        );
    }

    #[test]
    fn frame_offset_no_vertical_displacement() {
        // v_disp == 0 → purely horizontal, no wrapping regardless of tex_width
        for i in 0..8 {
            let off = compute_frame_offset(i, v2(0.0, 0.0), 64.0, 0.0, Some(256.0));
            assert_offset(off, i as f32 * 64.0, 0.0);
        }
    }

    #[test]
    fn frame_offset_no_vertical_displacement_with_start_pos() {
        let off = compute_frame_offset(3, v2(10.0, 20.0), 64.0, 0.0, None);
        assert_offset(off, 10.0 + 3.0 * 64.0, 20.0);
    }

    #[test]
    fn frame_offset_vertical_displacement_no_texture() {
        // v_disp > 0 but no texture → fallback to horizontal-only
        let off = compute_frame_offset(5, v2(0.0, 0.0), 64.0, 64.0, None);
        assert_offset(off, 5.0 * 64.0, 0.0);
    }

    #[test]
    fn frame_offset_wrap_from_origin() {
        // 256px wide, 64px frames starting at x=0 → 4 frames per row
        let tw = Some(256.0);
        // Row 0: frames 0–3
        assert_offset(
            compute_frame_offset(0, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            0.0,
        );
        assert_offset(
            compute_frame_offset(1, v2(0.0, 0.0), 64.0, 64.0, tw),
            64.0,
            0.0,
        );
        assert_offset(
            compute_frame_offset(2, v2(0.0, 0.0), 64.0, 64.0, tw),
            128.0,
            0.0,
        );
        assert_offset(
            compute_frame_offset(3, v2(0.0, 0.0), 64.0, 64.0, tw),
            192.0,
            0.0,
        );
        // Row 1: frames 4–7
        assert_offset(
            compute_frame_offset(4, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            64.0,
        );
        assert_offset(
            compute_frame_offset(5, v2(0.0, 0.0), 64.0, 64.0, tw),
            64.0,
            64.0,
        );
        assert_offset(
            compute_frame_offset(6, v2(0.0, 0.0), 64.0, 64.0, tw),
            128.0,
            64.0,
        );
        assert_offset(
            compute_frame_offset(7, v2(0.0, 0.0), 64.0, 64.0, tw),
            192.0,
            64.0,
        );
        // Row 2: frames 8–11
        assert_offset(
            compute_frame_offset(8, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            128.0,
        );
        assert_offset(
            compute_frame_offset(11, v2(0.0, 0.0), 64.0, 64.0, tw),
            192.0,
            128.0,
        );
    }

    #[test]
    fn frame_offset_wrap_partial_first_row() {
        // Start at x=128, texture 256px → first row has 2 frames, subsequent rows have 4
        let tw = Some(256.0);
        let pos = v2(128.0, 0.0);
        // First row: frames 0–1 at x=128, x=192
        assert_offset(compute_frame_offset(0, pos, 64.0, 64.0, tw), 128.0, 0.0);
        assert_offset(compute_frame_offset(1, pos, 64.0, 64.0, tw), 192.0, 0.0);
        // Row 1: frames 2–5 at x=0,64,128,192
        assert_offset(compute_frame_offset(2, pos, 64.0, 64.0, tw), 0.0, 64.0);
        assert_offset(compute_frame_offset(3, pos, 64.0, 64.0, tw), 64.0, 64.0);
        assert_offset(compute_frame_offset(4, pos, 64.0, 64.0, tw), 128.0, 64.0);
        assert_offset(compute_frame_offset(5, pos, 64.0, 64.0, tw), 192.0, 64.0);
        // Row 2: frames 6–9
        assert_offset(compute_frame_offset(6, pos, 64.0, 64.0, tw), 0.0, 128.0);
        assert_offset(compute_frame_offset(9, pos, 64.0, 64.0, tw), 192.0, 128.0);
    }

    #[test]
    fn frame_offset_different_v_disp() {
        // v_disp different from h_disp (e.g. rows taller than frames are wide)
        let tw = Some(256.0);
        let pos = v2(0.0, 10.0);
        // 4 frames per row, v_disp=80
        assert_offset(compute_frame_offset(3, pos, 64.0, 80.0, tw), 192.0, 10.0);
        assert_offset(compute_frame_offset(4, pos, 64.0, 80.0, tw), 0.0, 90.0);
        assert_offset(compute_frame_offset(8, pos, 64.0, 80.0, tw), 0.0, 170.0);
    }

    #[test]
    fn frame_offset_non_aligned_texture_width() {
        // 200px wide, 64px frames → 3 frames per full row
        // Starting at x=10 → first row fits floor((200-10)/64) = 2 frames
        let tw = Some(200.0);
        let pos = v2(10.0, 0.0);
        assert_offset(compute_frame_offset(0, pos, 64.0, 64.0, tw), 10.0, 0.0);
        assert_offset(compute_frame_offset(1, pos, 64.0, 64.0, tw), 74.0, 0.0);
        // frame 2 wraps: remaining=0, row=1, col=0
        assert_offset(compute_frame_offset(2, pos, 64.0, 64.0, tw), 0.0, 64.0);
        assert_offset(compute_frame_offset(3, pos, 64.0, 64.0, tw), 64.0, 64.0);
        assert_offset(compute_frame_offset(4, pos, 64.0, 64.0, tw), 128.0, 64.0);
        // frame 5 wraps to row 2
        assert_offset(compute_frame_offset(5, pos, 64.0, 64.0, tw), 0.0, 128.0);
    }

    #[test]
    fn frame_offset_fits_exactly_no_wrap_needed() {
        // 3 frames exactly filling a 192px wide texture → no wrap triggered
        let tw = Some(192.0);
        assert_offset(
            compute_frame_offset(0, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            0.0,
        );
        assert_offset(
            compute_frame_offset(1, v2(0.0, 0.0), 64.0, 64.0, tw),
            64.0,
            0.0,
        );
        assert_offset(
            compute_frame_offset(2, v2(0.0, 0.0), 64.0, 64.0, tw),
            128.0,
            0.0,
        );
    }

    #[test]
    fn frame_offset_boundary_last_frame_on_edge() {
        // Frame 3 at x=192, width=64, texture=256: 192+64=256 ≤ 256 → NO wrap
        let tw = Some(256.0);
        assert_offset(
            compute_frame_offset(3, v2(0.0, 0.0), 64.0, 64.0, tw),
            192.0,
            0.0,
        );
        // Frame 4: 256+64=320 > 256 → wrap
        assert_offset(
            compute_frame_offset(4, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            64.0,
        );
    }

    #[test]
    fn frame_offset_single_frame_per_row() {
        // 64px wide texture, 64px frames → 1 frame per row
        let tw = Some(64.0);
        assert_offset(
            compute_frame_offset(0, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            0.0,
        );
        assert_offset(
            compute_frame_offset(1, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            64.0,
        );
        assert_offset(
            compute_frame_offset(2, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            128.0,
        );
        assert_offset(
            compute_frame_offset(5, v2(0.0, 0.0), 64.0, 64.0, tw),
            0.0,
            320.0,
        );
    }

    #[test]
    fn frame_offset_with_y_origin() {
        // Starting position has non-zero y → wrapping adds v_disp relative to it
        let tw = Some(128.0);
        let pos = v2(0.0, 100.0);
        assert_offset(compute_frame_offset(0, pos, 64.0, 64.0, tw), 0.0, 100.0);
        assert_offset(compute_frame_offset(1, pos, 64.0, 64.0, tw), 64.0, 100.0);
        assert_offset(compute_frame_offset(2, pos, 64.0, 64.0, tw), 0.0, 164.0);
        assert_offset(compute_frame_offset(3, pos, 64.0, 64.0, tw), 64.0, 164.0);
        assert_offset(compute_frame_offset(4, pos, 64.0, 64.0, tw), 0.0, 228.0);
    }

    // --- animation system: AnimationFinishedEvent fires exactly once ---

    #[test]
    fn animation_finished_event_fires_exactly_once() {
        use crate::events::animation::AnimationFinishedEvent;
        use crate::resources::animationstore::AnimationResource;
        use std::sync::Arc;

        #[derive(Resource, Default)]
        struct EventCount(u32);

        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta: 0.11,
            ..WorldTime::default()
        });
        world.insert_resource(TextureDimsStore::default());
        world.insert_resource(EventCount::default());

        let mut anim_store = AnimationStore::default();
        // Single-frame non-looped animation: the very first tick tries to advance
        // from frame 0 to frame 1, which exceeds frame_count (1), so the finish
        // branch fires immediately and the event is triggered.
        anim_store.animations.insert(
            "die".to_string(),
            AnimationResource {
                tex_key: Arc::from("t"),
                position: Vec2 { x: 0.0, y: 0.0 },
                horizontal_displacement: 32.0,
                vertical_displacement: 0.0,
                frame_count: 1,
                fps: 10.0,
                looped: false,
            },
        );
        world.insert_resource(anim_store);

        // Observer that counts how many times AnimationFinishedEvent fires.
        // world.flush() is required after spawning observers so their hooks register
        // before the schedule runs — same pattern used in engine_app::spawn_observers.
        world.spawn(Observer::new(
            |_trigger: On<AnimationFinishedEvent>, mut count: ResMut<EventCount>| {
                count.0 += 1;
            },
        ));
        world.flush();

        let make_sprite = || Sprite {
            tex_key: Arc::from("t"),
            width: 32.0,
            height: 32.0,
            offset: Vec2 { x: 0.0, y: 0.0 },
            origin: Vec2 { x: 0.0, y: 0.0 },
            flip_h: false,
            flip_v: false,
        };

        // Start at frame 0; delta=0.11 > frame_duration=0.1 so each tick advances a frame.
        // After tick 1: frame 0 → 1 (= frame_count-1, first finish) → event fires.
        // After tick 2: already at last frame → event must NOT fire again.
        let probe_entity = world
            .spawn((
                Animation {
                    animation_key: "die".to_string(),
                    frame_index: 0,
                    elapsed_time: 0.0,
                    finished: false,
                },
                make_sprite(),
                MapPosition::new(0.0, 0.0),
            ))
            .id();

        // Sanity-check: world.trigger fires the observer immediately (proves observer is registered).
        world.trigger(AnimationFinishedEvent {
            entity: probe_entity,
        });
        assert_eq!(
            world.resource::<EventCount>().0,
            1,
            "world.trigger should fire the observer immediately",
        );
        world.resource_mut::<EventCount>().0 = 0; // reset for the real test

        let mut schedule = Schedule::default();
        schedule.add_systems(animation);

        schedule.run(&mut world);
        assert_eq!(
            world.resource::<EventCount>().0,
            1,
            "AnimationFinishedEvent should fire exactly once on the frame the animation finishes",
        );

        schedule.run(&mut world);
        assert_eq!(
            world.resource::<EventCount>().0,
            1,
            "AnimationFinishedEvent must not fire again on subsequent frames at the last frame",
        );
    }

    // --- animation system: break bug (Finding 1) ---

    #[test]
    fn animation_break_starves_entities_after_finished_nonlooped() {
        use crate::resources::animationstore::AnimationResource;
        use std::sync::Arc;

        // delta > frame_duration (0.1s at 10 fps) so every entity advances this tick.
        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta: 0.11,
            ..WorldTime::default()
        });
        world.insert_resource(TextureDimsStore::default());

        let mut anim_store = AnimationStore::default();
        anim_store.animations.insert(
            "death".to_string(),
            AnimationResource {
                tex_key: Arc::from("t"),
                position: Vec2 { x: 0.0, y: 0.0 },
                horizontal_displacement: 32.0,
                vertical_displacement: 0.0,
                frame_count: 4,
                fps: 10.0,
                looped: false,
            },
        );
        anim_store.animations.insert(
            "idle".to_string(),
            AnimationResource {
                tex_key: Arc::from("t"),
                position: Vec2 { x: 0.0, y: 0.0 },
                horizontal_displacement: 32.0,
                vertical_displacement: 0.0,
                frame_count: 4,
                fps: 10.0,
                looped: true,
            },
        );
        world.insert_resource(anim_store);

        let make_sprite = || Sprite {
            tex_key: Arc::from("t"),
            width: 32.0,
            height: 32.0,
            offset: Vec2 { x: 0.0, y: 0.0 },
            origin: Vec2 { x: 0.0, y: 0.0 },
            flip_h: false,
            flip_v: false,
        };
        let make_pos = || MapPosition::new(0.0, 0.0);

        // Entity A: non-looped "death" already at its last valid frame (index 3).
        // The tick will try to advance to 4, clamp back to 3, and — with the bug — break
        // out of the entire query loop before entity B is ever processed.
        let _entity_a = world
            .spawn((
                Animation {
                    animation_key: "death".to_string(),
                    frame_index: 3,
                    elapsed_time: 0.0,
                    finished: false,
                },
                make_sprite(),
                make_pos(),
            ))
            .id();

        // Entity B: looped "idle" at frame 0. Should advance to frame 1 this tick, but
        // won't if the break in entity A's branch exits the loop early.
        let entity_b = world
            .spawn((
                Animation {
                    animation_key: "idle".to_string(),
                    frame_index: 0,
                    elapsed_time: 0.0,
                    finished: false,
                },
                make_sprite(),
                make_pos(),
            ))
            .id();

        world
            .run_system_once(animation)
            .expect("animation should run");

        let b_frame = world
            .entity(entity_b)
            .get::<Animation>()
            .unwrap()
            .frame_index;
        assert_eq!(
            b_frame, 1,
            "entity B should advance from frame 0 to 1, but got {} — break in \
             non-looped branch exits the outer loop early (Finding 1)",
            b_frame,
        );
    }

    // --- stale sk::ANIMATION_ENDED signal cleared on restart ---

    #[test]
    fn animation_stale_signal_cleared_after_restart() {
        use crate::components::signals::Signals;
        use crate::resources::animationstore::AnimationResource;
        use std::sync::Arc;

        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta: 0.11,
            ..WorldTime::default()
        });
        world.insert_resource(TextureDimsStore::default());

        let mut anim_store = AnimationStore::default();
        // 4-frame animation: ticks 1–4 advance frames; tick 4 hits overflow and finishes.
        // After finished=true, tick 5 is skipped entirely (new guard). Restart resets
        // frame_index=0/finished=false; tick 6 clears the stale flag via the frame_index==0
        // branch, then advances normally without re-entering the overflow branch.
        anim_store.animations.insert(
            "die".to_string(),
            AnimationResource {
                tex_key: Arc::from("t"),
                position: Vec2 { x: 0.0, y: 0.0 },
                horizontal_displacement: 32.0,
                vertical_displacement: 0.0,
                frame_count: 4,
                fps: 10.0,
                looped: false,
            },
        );
        world.insert_resource(anim_store);

        let make_sprite = || Sprite {
            tex_key: Arc::from("t"),
            width: 32.0,
            height: 32.0,
            offset: Vec2 { x: 0.0, y: 0.0 },
            origin: Vec2 { x: 0.0, y: 0.0 },
            flip_h: false,
            flip_v: false,
        };

        let entity = world
            .spawn((
                Animation {
                    animation_key: "die".to_string(),
                    frame_index: 0,
                    elapsed_time: 0.0,
                    finished: false,
                },
                make_sprite(),
                MapPosition::new(0.0, 0.0),
                Signals::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(animation);

        // Ticks 1–4: advance through the 4-frame animation to completion.
        for _ in 0..4 {
            schedule.run(&mut world);
        }
        assert!(
            world
                .entity(entity)
                .get::<Signals>()
                .unwrap()
                .has_flag(sk::ANIMATION_ENDED),
            "animation_ended should be set after non-looped animation finishes",
        );
        assert!(world.entity(entity).get::<Animation>().unwrap().finished);

        // Tick 5: finished=true → animation system skips entirely. Flag stays set.
        schedule.run(&mut world);
        assert!(
            world
                .entity(entity)
                .get::<Signals>()
                .unwrap()
                .has_flag(sk::ANIMATION_ENDED),
            "animation_ended should still be set on subsequent ticks (finished guard, no processing)",
        );

        // Simulate RestartAnimation: reset via Animation::reset()
        world
            .entity_mut(entity)
            .get_mut::<Animation>()
            .unwrap()
            .reset();

        // Tick 6: frame_index==0 && !finished → clear stale flag; then advance to frame 1
        // (no overflow), so clear is not re-set.
        schedule.run(&mut world);
        assert!(
            !world
                .entity(entity)
                .get::<Signals>()
                .unwrap()
                .has_flag(sk::ANIMATION_ENDED),
            "animation_ended should be cleared on first tick after restart",
        );
    }

    // --- animation_controller system ---

    fn make_animation_resource(
        tex_key: &str,
        position: (f32, f32),
        displacement: (f32, f32),
        frame_count: usize,
        fps: f32,
        looped: bool,
    ) -> crate::resources::animationstore::AnimationResource {
        crate::resources::animationstore::AnimationResource {
            tex_key: std::sync::Arc::from(tex_key),
            position: Vec2 {
                x: position.0,
                y: position.1,
            },
            horizontal_displacement: displacement.0,
            vertical_displacement: displacement.1,
            frame_count,
            fps,
            looped,
        }
    }

    fn make_sprite(tex_key: &str) -> Sprite {
        Sprite {
            tex_key: std::sync::Arc::from(tex_key),
            width: 64.0,
            height: 64.0,
            offset: Vec2 { x: 0.0, y: 0.0 },
            origin: Vec2 { x: 0.0, y: 0.0 },
            flip_h: false,
            flip_v: false,
        }
    }

    /// Minimal world for `animation_controller`: an empty `AnimationStore`.
    fn world_with_empty_store() -> World {
        let mut world = World::new();
        world.insert_resource(AnimationStore::default());
        world
    }

    fn tick_animation_controller(world: &mut World) {
        world
            .run_system_once(animation_controller)
            .expect("animation_controller should run");
    }

    #[test]
    fn animation_controller_switches_on_flag() {
        let mut world = world_with_empty_store();

        let controller = AnimationController::new("idle").with_rule(
            Condition::HasFlag {
                key: "moving".to_string(),
            },
            "walk",
        );

        let entity = world
            .spawn((
                Animation::new("idle"),
                controller,
                Signals::default().with_flag("moving"),
            ))
            .id();

        tick_animation_controller(&mut world);

        let anim = world.get::<Animation>(entity).unwrap();
        assert_eq!(anim.animation_key, "walk");
    }

    #[test]
    fn animation_controller_uses_fallback_when_no_match() {
        let mut world = world_with_empty_store();

        let controller = AnimationController::new("idle").with_rule(
            Condition::HasFlag {
                key: "running".to_string(),
            },
            "run",
        );

        let entity = world
            .spawn((
                Animation::new("idle"),
                controller,
                Signals::default(), // No "running" flag
            ))
            .id();

        tick_animation_controller(&mut world);

        let anim = world.get::<Animation>(entity).unwrap();
        assert_eq!(anim.animation_key, "idle"); // Fallback
    }

    #[test]
    fn animation_controller_resets_animation_on_switch() {
        let mut world = world_with_empty_store();

        let controller = AnimationController::new("idle").with_rule(
            Condition::HasFlag {
                key: "attack".to_string(),
            },
            "attack",
        );

        // Start with animation already advanced
        let mut anim = Animation::new("idle");
        anim.frame_index = 5;
        anim.elapsed_time = 0.5;

        let entity = world
            .spawn((anim, controller, Signals::default().with_flag("attack")))
            .id();

        tick_animation_controller(&mut world);

        let anim = world.get::<Animation>(entity).unwrap();
        assert_eq!(anim.animation_key, "attack");
        assert_eq!(anim.frame_index, 0); // Reset
        assert!(approx_eq(anim.elapsed_time, 0.0)); // Reset
    }

    #[test]
    fn animation_controller_first_matching_rule_wins() {
        let mut world = world_with_empty_store();

        let controller = AnimationController::new("idle")
            .with_rule(
                Condition::HasFlag {
                    key: "dead".to_string(),
                },
                "death",
            )
            .with_rule(
                Condition::HasFlag {
                    key: "moving".to_string(),
                },
                "walk",
            );

        // Both flags set, but "dead" rule comes first
        let mut signals = Signals::default();
        signals.set_flag("dead");
        signals.set_flag("moving");

        let entity = world
            .spawn((Animation::new("idle"), controller, signals))
            .id();

        tick_animation_controller(&mut world);

        let anim = world.get::<Animation>(entity).unwrap();
        assert_eq!(anim.animation_key, "death"); // First match wins
    }

    #[test]
    fn animation_controller_syncs_sprite_tex_key_on_switch() {
        // Verify that when the controller switches animation, Sprite.tex_key is
        // updated to match the new animation's texture (the bug was that only
        // Animation.animation_key was updated, leaving Sprite.tex_key stale).
        let mut world = world_with_empty_store();

        // Register two animations with distinct textures
        let mut anim_store = AnimationStore {
            animations: Default::default(),
        };
        anim_store.animations.insert(
            "idle".to_string(),
            make_animation_resource("sheet_idle", (0.0, 0.0), (64.0, 0.0), 4, 10.0, true),
        );
        anim_store.animations.insert(
            "walk".to_string(),
            make_animation_resource("sheet_walk", (0.0, 0.0), (64.0, 0.0), 8, 12.0, true),
        );
        world.insert_resource(anim_store);

        let controller = AnimationController::new("idle").with_rule(
            Condition::HasFlag {
                key: "moving".to_string(),
            },
            "walk",
        );

        // Sprite starts on the idle sheet
        let entity = world
            .spawn((
                Animation::new("idle"),
                controller,
                make_sprite("sheet_idle"),
                Signals::default().with_flag("moving"),
            ))
            .id();

        tick_animation_controller(&mut world);

        let anim = world.get::<Animation>(entity).unwrap();
        let sprite = world.get::<Sprite>(entity).unwrap();

        assert_eq!(anim.animation_key, "walk", "animation key should switch");
        assert_eq!(
            sprite.tex_key.as_ref(),
            "sheet_walk",
            "sprite tex_key must update to match the new animation's texture"
        );
    }

    #[test]
    fn animation_controller_does_not_change_sprite_tex_key_when_no_switch() {
        // When the animation does not change, tex_key must remain untouched.
        let mut world = world_with_empty_store();

        let mut anim_store = AnimationStore {
            animations: Default::default(),
        };
        anim_store.animations.insert(
            "idle".to_string(),
            make_animation_resource("sheet_idle", (0.0, 0.0), (64.0, 0.0), 4, 10.0, true),
        );
        world.insert_resource(anim_store);

        let controller = AnimationController::new("idle"); // no rules → always fallback "idle"

        let entity = world
            .spawn((
                Animation::new("idle"),
                controller,
                make_sprite("sheet_idle"),
                Signals::default(), // no flags
            ))
            .id();

        tick_animation_controller(&mut world);

        let sprite = world.get::<Sprite>(entity).unwrap();
        assert_eq!(
            sprite.tex_key.as_ref(),
            "sheet_idle",
            "tex_key must not change when animation stays the same"
        );
    }

    #[test]
    fn animation_controller_skips_tex_key_when_animation_not_in_store() {
        // If the target animation key is not registered in AnimationStore, the
        // controller still switches Animation.animation_key but leaves
        // Sprite.tex_key unchanged — no panic.
        let mut world = world_with_empty_store();
        // AnimationStore is empty (inserted by world_with_empty_store) — "run" is not registered

        let controller = AnimationController::new("idle").with_rule(
            Condition::HasFlag {
                key: "moving".to_string(),
            },
            "run",
        );

        let entity = world
            .spawn((
                Animation::new("idle"),
                controller,
                make_sprite("sheet_idle"),
                Signals::default().with_flag("moving"),
            ))
            .id();

        tick_animation_controller(&mut world);

        let anim = world.get::<Animation>(entity).unwrap();
        let sprite = world.get::<Sprite>(entity).unwrap();

        assert_eq!(
            anim.animation_key, "run",
            "animation key should still switch"
        );
        assert_eq!(
            sprite.tex_key.as_ref(),
            "sheet_idle",
            "tex_key must remain unchanged when animation is not in store"
        );
    }

    // --- animation system: row wrapping via TextureDimsStore ---
    //
    // The offset math itself is covered by the `frame_offset_*` tests above;
    // these check that the system feeds the atlas width from
    // `TextureDimsStore` into it, and falls back when the key is missing.

    /// Minimal world for `animation`: fixed per-tick delta, empty stores.
    fn world_for_animation(delta: f32) -> World {
        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        world.insert_resource(AnimationStore::default());
        world.insert_resource(TextureDimsStore::default());
        world
    }

    fn tick_animation(world: &mut World) {
        world
            .run_system_once(animation)
            .expect("animation should run");
    }

    fn tick_animation_n(world: &mut World, n: usize) {
        for _ in 0..n {
            tick_animation(world);
        }
    }

    #[test]
    fn animation_wraps_rows_with_vertical_displacement() {
        // 256px wide texture, 64px frames, v_disp=64 → 4 frames per row.
        // Animation has 12 frames spanning 3 rows.
        let fps = 10.0;
        let delta = 1.0 / fps;
        let mut world = world_for_animation(delta);

        let mut anim_store = AnimationStore {
            animations: Default::default(),
        };
        anim_store.animations.insert(
            "big".to_string(),
            make_animation_resource("sheet", (0.0, 0.0), (64.0, 64.0), 12, fps, true),
        );
        world.insert_resource(anim_store);

        // Record the atlas dims so the system can look up the width
        // (animation reads TextureDimsStore, not the GPU TextureStore).
        world
            .resource_mut::<TextureDimsStore>()
            .insert("sheet", 256, 256);

        let entity = world
            .spawn((
                Animation {
                    animation_key: "big".to_string(),
                    frame_index: 0,
                    elapsed_time: 0.0,
                    finished: false,
                },
                make_sprite("sheet"),
                MapPosition::new(0.0, 0.0),
            ))
            .id();

        // Advance through all 12 frames, checking offsets at key points.
        // frame 0 already set on first tick (will advance to frame 1)
        // We need frame_index=0 first (initial state, before any tick).
        let sprite = world.get::<Sprite>(entity).unwrap();
        assert!(
            approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 0.0),
            "initial: ({}, {})",
            sprite.offset.x,
            sprite.offset.y
        );

        // Tick to frames 1..4 — frame 3 is last on row 0, frame 4 should wrap
        tick_animation_n(&mut world, 4);
        let anim = world.get::<Animation>(entity).unwrap();
        assert_eq!(anim.frame_index, 4, "should be on frame 4");
        let sprite = world.get::<Sprite>(entity).unwrap();
        assert!(
            approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 64.0),
            "frame 4: expected (0, 64), got ({}, {})",
            sprite.offset.x,
            sprite.offset.y
        );

        // Tick to frame 7 (last on row 1)
        tick_animation_n(&mut world, 3);
        let sprite = world.get::<Sprite>(entity).unwrap();
        assert!(
            approx_eq(sprite.offset.x, 192.0) && approx_eq(sprite.offset.y, 64.0),
            "frame 7: expected (192, 64), got ({}, {})",
            sprite.offset.x,
            sprite.offset.y
        );

        // Tick to frame 8 (first on row 2)
        tick_animation(&mut world);
        let sprite = world.get::<Sprite>(entity).unwrap();
        assert!(
            approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 128.0),
            "frame 8: expected (0, 128), got ({}, {})",
            sprite.offset.x,
            sprite.offset.y
        );

        // Tick to frame 11 (last frame)
        tick_animation_n(&mut world, 3);
        let sprite = world.get::<Sprite>(entity).unwrap();
        assert!(
            approx_eq(sprite.offset.x, 192.0) && approx_eq(sprite.offset.y, 128.0),
            "frame 11: expected (192, 128), got ({}, {})",
            sprite.offset.x,
            sprite.offset.y
        );

        // Tick once more: looped animation wraps to frame 0
        tick_animation(&mut world);
        let anim = world.get::<Animation>(entity).unwrap();
        assert_eq!(anim.frame_index, 0, "should loop back to frame 0");
        let sprite = world.get::<Sprite>(entity).unwrap();
        assert!(
            approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 0.0),
            "loop: expected (0, 0), got ({}, {})",
            sprite.offset.x,
            sprite.offset.y
        );
    }

    #[test]
    fn animation_vdisp_no_texture_falls_back_to_horizontal() {
        // v_disp > 0 but no texture in store → fallback to horizontal-only (no crash).
        let fps = 10.0;
        let delta = 1.0 / fps;
        let mut world = world_for_animation(delta);

        let mut anim_store = AnimationStore {
            animations: Default::default(),
        };
        anim_store.animations.insert(
            "missing_tex".to_string(),
            make_animation_resource("nonexistent", (0.0, 0.0), (64.0, 64.0), 8, fps, true),
        );
        world.insert_resource(anim_store);

        let entity = world
            .spawn((
                Animation {
                    animation_key: "missing_tex".to_string(),
                    frame_index: 0,
                    elapsed_time: 0.0,
                    finished: false,
                },
                make_sprite("nonexistent"),
                MapPosition::new(0.0, 0.0),
            ))
            .id();

        // Advance 5 frames — should still work, just no wrapping
        tick_animation_n(&mut world, 5);
        let sprite = world.get::<Sprite>(entity).unwrap();
        assert!(
            approx_eq(sprite.offset.x, 320.0),
            "frame 5: x={}",
            sprite.offset.x
        );
        assert!(approx_eq(sprite.offset.y, 0.0), "frame 5: y should stay 0");
    }
}

/// Select the active animation track according to controller rules.
///
/// The first matching rule wins. If no rules match, the controller's default
/// target is used. When the selected key differs from the current one, the
/// animation state is reset.
pub fn animation_controller(
    mut query: Query<(Entity, &mut AnimationController, &mut Animation, &Signals)>,
    mut sprite_query: Query<&mut Sprite>,
    animation_store: Res<AnimationStore>,
) {
    crate::tracy::tracy_span!("animation_controller");
    for (entity, mut controller, mut animation, signals) in query.iter_mut() {
        let mut selected: Option<&str> = None;
        for rule in &controller.rules {
            if evaluate_condition(signals, &rule.when) {
                selected = Some(rule.set_key.as_str());
                break;
            }
        }
        let target_key: &str = selected.unwrap_or(controller.fallback_key.as_str());
        if animation.animation_key.as_str() != target_key {
            // Transition: allocate once here, not every frame
            let owned = target_key.to_string();
            animation.animation_key = owned.clone();
            animation.frame_index = 0;
            animation.elapsed_time = 0.0;
            animation.finished = false;
            controller.current_key = owned.clone();
            // Sync Sprite.tex_key to the new animation's texture (mirrors SetAnimation EntityCmd)
            if let Some(anim_res) = animation_store.animations.get(owned.as_str())
                && let Ok(mut sprite) = sprite_query.get_mut(entity)
            {
                sprite.tex_key = anim_res.tex_key.clone();
            }
        }
    }
}
