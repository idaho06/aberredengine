//! Generic tween animation systems.
//!
//! These systems update entity properties over time based on [`Tween<T>`]
//! components. Register one concrete system per tweened component type, such as
//! `tween_system::<MapPosition>`, `tween_system::<Rotation>`, and
//! `tween_system::<Scale>`.

use crate::components::tween::{Easing, LoopMode, Tween, TweenValue};
use crate::resources::worldtime::WorldTime;
use bevy_ecs::prelude::*;

/// Apply an easing function to a normalized time value.
///
/// The input `t` is clamped to [0.0, 1.0] and transformed according to the
/// easing curve.
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
        }
    }
}

/// Advance tween time and handle looping/completion.
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

/// Animate components based on their matching [`Tween<T>`] component.
pub fn tween_system<T: TweenValue>(
    world_time: Res<WorldTime>,
    mut query: Query<(&mut T, &mut Tween<T>)>,
) {
    let dt = world_time.delta.max(0.0);
    for (mut value, mut tw) in query.iter_mut() {
        if !tw.playing {
            continue;
        }

        let duration = tw.duration;
        let loop_mode = tw.loop_mode;
        let mut time = tw.time;
        let mut forward = tw.forward;
        let mut playing = tw.playing;
        advance(
            &mut time,
            duration,
            &mut forward,
            &mut playing,
            loop_mode,
            dt,
        );
        tw.time = time;
        tw.forward = forward;
        tw.playing = playing;

        let t = ease(tw.easing, tw.time / duration);
        *value = T::interpolate(&tw.from, &tw.to, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::mapposition::MapPosition;
    use crate::components::rotation::Rotation;
    use crate::components::scale::Scale;
    use raylib::prelude::Vector2;

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
        assert!(approx_eq(ease(Easing::QuadIn, 0.5), 0.25));
        assert!(approx_eq(ease(Easing::QuadIn, 0.25), 0.0625));
    }

    #[test]
    fn test_ease_quad_out() {
        assert!(approx_eq(ease(Easing::QuadOut, 0.5), 0.75));
        assert!(approx_eq(ease(Easing::QuadOut, 0.25), 0.4375));
    }

    #[test]
    fn test_ease_quad_inout_first_half() {
        assert!(approx_eq(ease(Easing::QuadInOut, 0.25), 0.125));
    }

    #[test]
    fn test_ease_quad_inout_second_half() {
        assert!(approx_eq(ease(Easing::QuadInOut, 0.75), 0.875));
    }

    #[test]
    fn test_ease_quad_inout_midpoint() {
        assert!(approx_eq(ease(Easing::QuadInOut, 0.5), 0.5));
    }

    #[test]
    fn test_ease_cubic_in() {
        assert!(approx_eq(ease(Easing::CubicIn, 0.5), 0.125));
        assert!(approx_eq(ease(Easing::CubicIn, 0.25), 0.015625));
    }

    #[test]
    fn test_ease_cubic_out() {
        assert!(approx_eq(ease(Easing::CubicOut, 0.5), 0.875));
    }

    #[test]
    fn test_ease_cubic_inout_first_half() {
        assert!(approx_eq(ease(Easing::CubicInOut, 0.25), 0.0625));
    }

    #[test]
    fn test_ease_cubic_inout_second_half() {
        assert!(approx_eq(ease(Easing::CubicInOut, 0.75), 0.9375));
    }

    #[test]
    fn test_ease_cubic_inout_midpoint() {
        assert!(approx_eq(ease(Easing::CubicInOut, 0.5), 0.5));
    }

    #[test]
    fn test_ease_monotonicity() {
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

    // ==================== ADVANCE FUNCTION TESTS ====================

    #[test]
    fn test_advance_forward_normal() {
        let mut time = 0.0;
        let mut forward = true;
        let mut playing = true;
        advance(
            &mut time,
            1.0,
            &mut forward,
            &mut playing,
            LoopMode::Once,
            0.1,
        );
        assert!(approx_eq(time, 0.1));
        assert!(forward);
        assert!(playing);
    }

    #[test]
    fn test_advance_backward_normal() {
        let mut time = 1.0;
        let mut forward = false;
        let mut playing = true;
        advance(
            &mut time,
            1.0,
            &mut forward,
            &mut playing,
            LoopMode::Once,
            0.1,
        );
        assert!(approx_eq(time, 0.9));
        assert!(!forward);
        assert!(playing);
    }

    #[test]
    fn test_advance_once_stops_at_end() {
        let mut time = 0.9;
        let mut forward = true;
        let mut playing = true;
        advance(
            &mut time,
            1.0,
            &mut forward,
            &mut playing,
            LoopMode::Once,
            0.2,
        );
        assert!(approx_eq(time, 1.0));
        assert!(!playing);
    }

    #[test]
    fn test_advance_once_stops_at_start() {
        let mut time = 0.1;
        let mut forward = false;
        let mut playing = true;
        advance(
            &mut time,
            1.0,
            &mut forward,
            &mut playing,
            LoopMode::Once,
            0.2,
        );
        assert!(approx_eq(time, 0.0));
        assert!(!playing);
    }

    #[test]
    fn test_advance_loop_wraps_forward() {
        let mut time = 0.9;
        let mut forward = true;
        let mut playing = true;
        advance(
            &mut time,
            1.0,
            &mut forward,
            &mut playing,
            LoopMode::Loop,
            0.2,
        );
        assert!(approx_eq(time, 0.0));
        assert!(playing);
    }

    #[test]
    fn test_advance_loop_wraps_backward() {
        let mut time = 0.1;
        let mut forward = false;
        let mut playing = true;
        advance(
            &mut time,
            1.0,
            &mut forward,
            &mut playing,
            LoopMode::Loop,
            0.2,
        );
        assert!(approx_eq(time, 1.0));
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
        assert!(approx_eq(time, 1.0));
        assert!(!forward);
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
        assert!(approx_eq(time, 0.0));
        assert!(forward);
        assert!(playing);
    }

    // ==================== GENERIC SYSTEM TESTS ====================

    fn run_tween_once<T: TweenValue>(target: T, tween: Tween<T>, delta: f32) -> (T, Tween<T>) {
        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        let entity = world.spawn((target, tween)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(tween_system::<T>);
        schedule.run(&mut world);

        let updated_target = world.entity(entity).get::<T>().unwrap().clone();
        let updated_tween = world.entity(entity).get::<Tween<T>>().unwrap().clone();
        (updated_target, updated_tween)
    }

    #[test]
    fn test_tween_system_updates_map_position() {
        let (target, tween) = run_tween_once(
            MapPosition::from_vec(Vector2 { x: 0.0, y: 0.0 }),
            Tween::new(
                MapPosition::from_vec(Vector2 { x: 0.0, y: 0.0 }),
                MapPosition::from_vec(Vector2 { x: 10.0, y: 20.0 }),
                1.0,
            ),
            0.5,
        );

        assert!(approx_eq(target.pos.x, 5.0));
        assert!(approx_eq(target.pos.y, 10.0));
        assert!(approx_eq(tween.time, 0.5));
        assert!(tween.playing);
    }

    #[test]
    fn test_tween_system_updates_rotation() {
        let (target, tween) = run_tween_once(
            Rotation { degrees: 0.0 },
            Tween::new(Rotation { degrees: 0.0 }, Rotation { degrees: 180.0 }, 1.0),
            0.25,
        );

        assert!(approx_eq(target.degrees, 45.0));
        assert!(approx_eq(tween.time, 0.25));
        assert!(tween.playing);
    }

    #[test]
    fn test_tween_system_updates_scale_with_easing() {
        let (target, tween) = run_tween_once(
            Scale::new(1.0, 1.0),
            Tween::new(Scale::new(1.0, 1.0), Scale::new(3.0, 5.0), 1.0).with_easing(Easing::QuadIn),
            0.5,
        );

        assert!(approx_eq(target.scale.x, 1.5));
        assert!(approx_eq(target.scale.y, 2.0));
        assert!(approx_eq(tween.time, 0.5));
        assert!(tween.playing);
    }

    #[test]
    fn test_tween_system_applies_backwards_start_state() {
        let (target, tween) = run_tween_once(
            Rotation { degrees: 10.0 },
            Tween::new(Rotation { degrees: 0.0 }, Rotation { degrees: 180.0 }, 1.0).with_backwards(),
            0.0,
        );

        assert!(approx_eq(target.degrees, 180.0));
        assert!(approx_eq(tween.time, 1.0));
        assert!(!tween.forward);
    }
}
