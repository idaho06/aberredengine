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
use crate::resources::texturestore::TextureStore;
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
pub fn animation(
    mut query: Query<(&mut Animation, &mut Sprite, Option<&mut Signals>), With<MapPosition>>,
    animation_store: Res<AnimationStore>,
    texture_store: Res<TextureStore>,
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
                } else if let Some(signals) = maybe_signals.as_mut() {
                    signals.clear_flag("animation_ended");
                }
            }

            // Compute sprite offset for the current frame.
            let tex_width = if animation.vertical_displacement > 0.0 {
                texture_store
                    .map
                    .get(animation.tex_key.as_ref())
                    .map(|t| t.width as f32)
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
    position: Vector2,
    h_disp: f32,
    v_disp: f32,
    tex_width: Option<f32>,
) -> Vector2 {
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
            return Vector2 {
                x: col as f32 * h_disp,
                y: position.y + row as f32 * v_disp,
            };
        }
    }

    Vector2 {
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
mod tests {
    use super::*;

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

    fn v2(x: f32, y: f32) -> Vector2 {
        Vector2 { x, y }
    }

    fn assert_offset(result: Vector2, expected_x: f32, expected_y: f32) {
        assert!(
            (result.x - expected_x).abs() < f32::EPSILON
                && (result.y - expected_y).abs() < f32::EPSILON,
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
        assert_offset(compute_frame_offset(0, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 0.0);
        assert_offset(compute_frame_offset(1, v2(0.0, 0.0), 64.0, 64.0, tw), 64.0, 0.0);
        assert_offset(compute_frame_offset(2, v2(0.0, 0.0), 64.0, 64.0, tw), 128.0, 0.0);
        assert_offset(compute_frame_offset(3, v2(0.0, 0.0), 64.0, 64.0, tw), 192.0, 0.0);
        // Row 1: frames 4–7
        assert_offset(compute_frame_offset(4, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 64.0);
        assert_offset(compute_frame_offset(5, v2(0.0, 0.0), 64.0, 64.0, tw), 64.0, 64.0);
        assert_offset(compute_frame_offset(6, v2(0.0, 0.0), 64.0, 64.0, tw), 128.0, 64.0);
        assert_offset(compute_frame_offset(7, v2(0.0, 0.0), 64.0, 64.0, tw), 192.0, 64.0);
        // Row 2: frames 8–11
        assert_offset(compute_frame_offset(8, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 128.0);
        assert_offset(compute_frame_offset(11, v2(0.0, 0.0), 64.0, 64.0, tw), 192.0, 128.0);
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
        assert_offset(compute_frame_offset(0, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 0.0);
        assert_offset(compute_frame_offset(1, v2(0.0, 0.0), 64.0, 64.0, tw), 64.0, 0.0);
        assert_offset(compute_frame_offset(2, v2(0.0, 0.0), 64.0, 64.0, tw), 128.0, 0.0);
    }

    #[test]
    fn frame_offset_boundary_last_frame_on_edge() {
        // Frame 3 at x=192, width=64, texture=256: 192+64=256 ≤ 256 → NO wrap
        let tw = Some(256.0);
        assert_offset(compute_frame_offset(3, v2(0.0, 0.0), 64.0, 64.0, tw), 192.0, 0.0);
        // Frame 4: 256+64=320 > 256 → wrap
        assert_offset(compute_frame_offset(4, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 64.0);
    }

    #[test]
    fn frame_offset_single_frame_per_row() {
        // 64px wide texture, 64px frames → 1 frame per row
        let tw = Some(64.0);
        assert_offset(compute_frame_offset(0, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 0.0);
        assert_offset(compute_frame_offset(1, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 64.0);
        assert_offset(compute_frame_offset(2, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 128.0);
        assert_offset(compute_frame_offset(5, v2(0.0, 0.0), 64.0, 64.0, tw), 0.0, 320.0);
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
        let target_key: &str = selected.unwrap_or(controller.fallback_key.as_str());
        if animation.animation_key.as_str() != target_key {
            // Transition: allocate once here, not every frame
            let owned = target_key.to_string();
            animation.animation_key = owned.clone();
            animation.frame_index = 0;
            animation.elapsed_time = 0.0;
            controller.current_key = owned;
        }
    }
}
