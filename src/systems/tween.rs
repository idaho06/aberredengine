//! Tween animation systems.
//!
//! These systems update entity properties over time based on tween components:
//! - [`tween_mapposition_system`] – animates [`MapPosition`](crate::components::mapposition::MapPosition)
//! - [`tween_rotation_system`] – animates [`Rotation`](crate::components::rotation::Rotation)
//! - [`tween_scale_system`] – animates [`Scale`](crate::components::scale::Scale)
//!
//! Each tween component specifies start/end values, duration, easing function,
//! and loop mode. The systems read delta time from [`WorldTime`](crate::resources::worldtime::WorldTime)
//! and interpolate the property accordingly.

use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::tween::{Easing, LoopMode, TweenPosition, TweenRotation, TweenScale};
use crate::resources::worldtime::WorldTime;
use bevy_ecs::prelude::*;
use raylib::math::Vector2;

/// Apply an easing function to a normalized time value.
///
/// The input `t` is clamped to [0.0, 1.0] and transformed according to the
/// easing curve.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn ease(e: Easing, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match e {
        Easing::Linear => t,
        Easing::QuadIn => t * t,
        Easing::QuadOut => t * (2.0 - t),
        Easing::QuadInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                -1.0 + (4.0 - 2.0 * t) * t
            }
        }
        Easing::CubicIn => t * t * t,
        Easing::CubicOut => {
            let p = t - 1.0;
            p * p * p + 1.0
        }
        Easing::CubicInOut => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                let p = 2.0 * t - 2.0;
                0.5 * p * p * p + 1.0
            }
        } // TODO: sine, elastic, bounce, etc.
    }
}

/// Linearly interpolate between two 2D vectors.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn lerp_v2(a: Vector2, b: Vector2, t: f32) -> Vector2 {
    Vector2 {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

/// Linearly interpolate between two floats.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Advance tween time and handle looping/completion.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn advance(
    time: &mut f32,
    duration: f32,
    forward: &mut bool,
    playing: &mut bool,
    mode: LoopMode,
    dt: f32,
) {
    let dir = if *forward { 1.0 } else { -1.0 };
    *time += dt * dir;

    let finished_forward = *forward && *time >= duration;
    let finished_backward = !*forward && *time <= 0.0;

    if finished_forward || finished_backward {
        match mode {
            LoopMode::Once => {
                *playing = false;
                *time = time.clamp(0.0, duration);
                // TODO: trigger "finished" event?
            }
            LoopMode::Loop => {
                *time = if finished_forward { 0.0 } else { duration };
            }
            LoopMode::PingPong => {
                *forward = !*forward;
                *time = time.clamp(0.0, duration);
            }
        }
    }
}

/// Animate entity positions based on [`TweenPosition`] components.
pub fn tween_mapposition_system(
    world_time: Res<WorldTime>,
    mut query: Query<(&mut MapPosition, &mut TweenPosition)>,
) {
    let dt = world_time.delta.max(0.0);
    for (mut mp, mut tw) in query.iter_mut() {
        if !tw.playing {
            continue;
        }
        let duration = tw.duration;
        let loop_mode = tw.loop_mode;
        let mut t = tw.time;
        let mut forward = tw.forward;
        let mut playing = tw.playing;
        advance(&mut t, duration, &mut forward, &mut playing, loop_mode, dt);
        tw.time = t;
        tw.forward = forward;
        tw.playing = playing;
        let t = ease(tw.easing, tw.time / duration);
        let new_pos = lerp_v2(tw.from, tw.to, t);
        mp.pos = new_pos;
    }
}

/// Animate entity rotations based on [`TweenRotation`] components.
pub fn tween_rotation_system(
    world_time: Res<WorldTime>,
    mut query: Query<(&mut Rotation, &mut TweenRotation)>,
) {
    let dt = world_time.delta.max(0.0);
    for (mut rot, mut tw) in query.iter_mut() {
        if !tw.playing {
            continue;
        }
        let duration = tw.duration;
        let loop_mode = tw.loop_mode;
        let mut t = tw.time;
        let mut forward = tw.forward;
        let mut playing = tw.playing;
        advance(&mut t, duration, &mut forward, &mut playing, loop_mode, dt);
        tw.time = t;
        tw.forward = forward;
        tw.playing = playing;
        let t = ease(tw.easing, tw.time / duration);
        let new_rot = lerp_f32(tw.from, tw.to, t);
        rot.degrees = new_rot;
    }
}

/// Animate entity scales based on [`TweenScale`] components.
pub fn tween_scale_system(
    world_time: Res<WorldTime>,
    mut query: Query<(&mut Scale, &mut TweenScale)>,
) {
    let dt = world_time.delta.max(0.0);
    for (mut scale, mut tw) in query.iter_mut() {
        if !tw.playing {
            continue;
        }
        let duration = tw.duration;
        let loop_mode = tw.loop_mode;
        let mut t = tw.time;
        let mut forward = tw.forward;
        let mut playing = tw.playing;
        advance(&mut t, duration, &mut forward, &mut playing, loop_mode, dt);
        tw.time = t;
        tw.forward = forward;
        tw.playing = playing;
        let t = ease(tw.easing, tw.time / duration);
        let new_scale = lerp_v2(tw.from, tw.to, t);
        scale.scale = new_scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // ==================== EASING FUNCTION TESTS ====================

    #[test]
    fn test_ease_all_types_at_zero() {
        let types = [
            Easing::Linear,
            Easing::QuadIn,
            Easing::QuadOut,
            Easing::QuadInOut,
            Easing::CubicIn,
            Easing::CubicOut,
            Easing::CubicInOut,
        ];
        for easing in types {
            assert!(
                approx_eq(ease(easing, 0.0), 0.0),
                "{:?} at t=0.0 should be 0.0",
                easing
            );
        }
    }

    #[test]
    fn test_ease_all_types_at_one() {
        let types = [
            Easing::Linear,
            Easing::QuadIn,
            Easing::QuadOut,
            Easing::QuadInOut,
            Easing::CubicIn,
            Easing::CubicOut,
            Easing::CubicInOut,
        ];
        for easing in types {
            assert!(
                approx_eq(ease(easing, 1.0), 1.0),
                "{:?} at t=1.0 should be 1.0",
                easing
            );
        }
    }

    #[test]
    fn test_ease_clamps_negative_input() {
        let types = [
            Easing::Linear,
            Easing::QuadIn,
            Easing::QuadOut,
            Easing::QuadInOut,
            Easing::CubicIn,
            Easing::CubicOut,
            Easing::CubicInOut,
        ];
        for easing in types {
            assert!(
                approx_eq(ease(easing, -0.5), 0.0),
                "{:?} at t=-0.5 should clamp to 0.0",
                easing
            );
        }
    }

    #[test]
    fn test_ease_clamps_above_one() {
        let types = [
            Easing::Linear,
            Easing::QuadIn,
            Easing::QuadOut,
            Easing::QuadInOut,
            Easing::CubicIn,
            Easing::CubicOut,
            Easing::CubicInOut,
        ];
        for easing in types {
            assert!(
                approx_eq(ease(easing, 1.5), 1.0),
                "{:?} at t=1.5 should clamp to 1.0",
                easing
            );
        }
    }

    #[test]
    fn test_ease_linear_midpoint() {
        assert!(approx_eq(ease(Easing::Linear, 0.5), 0.5));
        assert!(approx_eq(ease(Easing::Linear, 0.25), 0.25));
        assert!(approx_eq(ease(Easing::Linear, 0.75), 0.75));
    }

    #[test]
    fn test_ease_quad_in() {
        // QuadIn: t^2
        assert!(approx_eq(ease(Easing::QuadIn, 0.5), 0.25)); // 0.5^2 = 0.25
        assert!(approx_eq(ease(Easing::QuadIn, 0.25), 0.0625)); // 0.25^2 = 0.0625
    }

    #[test]
    fn test_ease_quad_out() {
        // QuadOut: t * (2 - t)
        assert!(approx_eq(ease(Easing::QuadOut, 0.5), 0.75)); // 0.5 * 1.5 = 0.75
        assert!(approx_eq(ease(Easing::QuadOut, 0.25), 0.4375)); // 0.25 * 1.75 = 0.4375
    }

    #[test]
    fn test_ease_quad_inout_first_half() {
        // QuadInOut first half: 2 * t^2
        assert!(approx_eq(ease(Easing::QuadInOut, 0.25), 0.125)); // 2 * 0.25^2 = 0.125
    }

    #[test]
    fn test_ease_quad_inout_second_half() {
        // QuadInOut second half: -1 + (4 - 2t) * t
        assert!(approx_eq(ease(Easing::QuadInOut, 0.75), 0.875)); // -1 + (4 - 1.5) * 0.75 = 0.875
    }

    #[test]
    fn test_ease_quad_inout_midpoint() {
        // At midpoint, both formulas should give 0.5
        assert!(approx_eq(ease(Easing::QuadInOut, 0.5), 0.5));
    }

    #[test]
    fn test_ease_cubic_in() {
        // CubicIn: t^3
        assert!(approx_eq(ease(Easing::CubicIn, 0.5), 0.125)); // 0.5^3 = 0.125
        assert!(approx_eq(ease(Easing::CubicIn, 0.25), 0.015625)); // 0.25^3 = 0.015625
    }

    #[test]
    fn test_ease_cubic_out() {
        // CubicOut: (t-1)^3 + 1
        assert!(approx_eq(ease(Easing::CubicOut, 0.5), 0.875)); // (-0.5)^3 + 1 = 0.875
    }

    #[test]
    fn test_ease_cubic_inout_first_half() {
        // CubicInOut first half: 4 * t^3
        assert!(approx_eq(ease(Easing::CubicInOut, 0.25), 0.0625)); // 4 * 0.25^3 = 0.0625
    }

    #[test]
    fn test_ease_cubic_inout_second_half() {
        // CubicInOut second half: 0.5 * (2t - 2)^3 + 1
        assert!(approx_eq(ease(Easing::CubicInOut, 0.75), 0.9375)); // 0.5 * (-0.5)^3 + 1 = 0.9375
    }

    #[test]
    fn test_ease_cubic_inout_midpoint() {
        assert!(approx_eq(ease(Easing::CubicInOut, 0.5), 0.5));
    }

    #[test]
    fn test_ease_monotonicity() {
        // All easing functions should be monotonically increasing
        let types = [
            Easing::Linear,
            Easing::QuadIn,
            Easing::QuadOut,
            Easing::QuadInOut,
            Easing::CubicIn,
            Easing::CubicOut,
            Easing::CubicInOut,
        ];
        for easing in types {
            let mut prev = ease(easing, 0.0);
            for i in 1..=100 {
                let t = i as f32 / 100.0;
                let curr = ease(easing, t);
                assert!(
                    curr >= prev - EPSILON,
                    "{:?} should be monotonic: ease({}) = {} < ease({}) = {}",
                    easing,
                    (i - 1) as f32 / 100.0,
                    prev,
                    t,
                    curr
                );
                prev = curr;
            }
        }
    }

    // ==================== INTERPOLATION FUNCTION TESTS ====================

    #[test]
    fn test_lerp_f32_basic() {
        assert!(approx_eq(lerp_f32(0.0, 10.0, 0.5), 5.0));
        assert!(approx_eq(lerp_f32(0.0, 10.0, 0.0), 0.0));
        assert!(approx_eq(lerp_f32(0.0, 10.0, 1.0), 10.0));
    }

    #[test]
    fn test_lerp_f32_quarter_points() {
        assert!(approx_eq(lerp_f32(0.0, 100.0, 0.25), 25.0));
        assert!(approx_eq(lerp_f32(0.0, 100.0, 0.75), 75.0));
    }

    #[test]
    fn test_lerp_f32_negative_values() {
        assert!(approx_eq(lerp_f32(-10.0, 10.0, 0.5), 0.0));
        assert!(approx_eq(lerp_f32(-10.0, 10.0, 0.25), -5.0));
        assert!(approx_eq(lerp_f32(-10.0, 10.0, 0.75), 5.0));
    }

    #[test]
    fn test_lerp_f32_identical_values() {
        assert!(approx_eq(lerp_f32(5.0, 5.0, 0.0), 5.0));
        assert!(approx_eq(lerp_f32(5.0, 5.0, 0.5), 5.0));
        assert!(approx_eq(lerp_f32(5.0, 5.0, 1.0), 5.0));
    }

    #[test]
    fn test_lerp_f32_extrapolation() {
        // lerp doesn't clamp, so it extrapolates beyond [0, 1]
        assert!(approx_eq(lerp_f32(0.0, 10.0, -0.5), -5.0));
        assert!(approx_eq(lerp_f32(0.0, 10.0, 1.5), 15.0));
    }

    #[test]
    fn test_lerp_v2_basic() {
        let a = Vector2 { x: 0.0, y: 0.0 };
        let b = Vector2 { x: 10.0, y: 20.0 };
        let result = lerp_v2(a, b, 0.5);
        assert!(approx_eq(result.x, 5.0));
        assert!(approx_eq(result.y, 10.0));
    }

    #[test]
    fn test_lerp_v2_at_boundaries() {
        let a = Vector2 { x: 1.0, y: 2.0 };
        let b = Vector2 { x: 11.0, y: 22.0 };

        let at_zero = lerp_v2(a, b, 0.0);
        assert!(approx_eq(at_zero.x, 1.0));
        assert!(approx_eq(at_zero.y, 2.0));

        let at_one = lerp_v2(a, b, 1.0);
        assert!(approx_eq(at_one.x, 11.0));
        assert!(approx_eq(at_one.y, 22.0));
    }

    #[test]
    fn test_lerp_v2_component_independence() {
        // X and Y interpolate independently
        let a = Vector2 { x: 0.0, y: 100.0 };
        let b = Vector2 { x: 100.0, y: 0.0 };
        let result = lerp_v2(a, b, 0.25);
        assert!(approx_eq(result.x, 25.0));
        assert!(approx_eq(result.y, 75.0));
    }

    #[test]
    fn test_lerp_v2_zero_vector() {
        let zero = Vector2 { x: 0.0, y: 0.0 };
        let target = Vector2 { x: 10.0, y: 20.0 };
        let result = lerp_v2(zero, target, 0.5);
        assert!(approx_eq(result.x, 5.0));
        assert!(approx_eq(result.y, 10.0));
    }

    #[test]
    fn test_lerp_v2_same_vectors() {
        let v = Vector2 { x: 5.0, y: 10.0 };
        let result = lerp_v2(v, v, 0.5);
        assert!(approx_eq(result.x, 5.0));
        assert!(approx_eq(result.y, 10.0));
    }

    // ==================== ADVANCE FUNCTION TESTS ====================

    #[test]
    fn test_advance_forward_normal() {
        let mut time = 0.0;
        let mut forward = true;
        let mut playing = true;
        advance(&mut time, 1.0, &mut forward, &mut playing, LoopMode::Once, 0.1);
        assert!(approx_eq(time, 0.1));
        assert!(forward);
        assert!(playing);
    }

    #[test]
    fn test_advance_backward_normal() {
        let mut time = 1.0;
        let mut forward = false;
        let mut playing = true;
        advance(&mut time, 1.0, &mut forward, &mut playing, LoopMode::Once, 0.1);
        assert!(approx_eq(time, 0.9));
        assert!(!forward);
        assert!(playing);
    }

    #[test]
    fn test_advance_once_stops_at_end() {
        let mut time = 0.9;
        let mut forward = true;
        let mut playing = true;
        advance(&mut time, 1.0, &mut forward, &mut playing, LoopMode::Once, 0.2);
        assert!(approx_eq(time, 1.0)); // clamped
        assert!(!playing); // stopped
    }

    #[test]
    fn test_advance_once_stops_at_start() {
        let mut time = 0.1;
        let mut forward = false;
        let mut playing = true;
        advance(&mut time, 1.0, &mut forward, &mut playing, LoopMode::Once, 0.2);
        assert!(approx_eq(time, 0.0)); // clamped
        assert!(!playing); // stopped
    }

    #[test]
    fn test_advance_loop_wraps_forward() {
        let mut time = 0.9;
        let mut forward = true;
        let mut playing = true;
        advance(&mut time, 1.0, &mut forward, &mut playing, LoopMode::Loop, 0.2);
        assert!(approx_eq(time, 0.0)); // wrapped
        assert!(playing);
    }

    #[test]
    fn test_advance_loop_wraps_backward() {
        let mut time = 0.1;
        let mut forward = false;
        let mut playing = true;
        advance(&mut time, 1.0, &mut forward, &mut playing, LoopMode::Loop, 0.2);
        assert!(approx_eq(time, 1.0)); // wrapped to end
        assert!(playing);
    }

    #[test]
    fn test_advance_pingpong_reverses_at_end() {
        let mut time = 0.9;
        let mut forward = true;
        let mut playing = true;
        advance(
            &mut time,
            1.0,
            &mut forward,
            &mut playing,
            LoopMode::PingPong,
            0.2,
        );
        assert!(approx_eq(time, 1.0)); // clamped to end
        assert!(!forward); // direction reversed
        assert!(playing);
    }

    #[test]
    fn test_advance_pingpong_reverses_at_start() {
        let mut time = 0.1;
        let mut forward = false;
        let mut playing = true;
        advance(
            &mut time,
            1.0,
            &mut forward,
            &mut playing,
            LoopMode::PingPong,
            0.2,
        );
        assert!(approx_eq(time, 0.0)); // clamped to start
        assert!(forward); // direction reversed
        assert!(playing);
    }
}
