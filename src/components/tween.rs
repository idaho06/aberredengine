//! Generic tween components for animated interpolation.
//!
//! This module provides a shared [`Tween<T>`] component for smoothly animating
//! entity properties over time:
//! - `Tween<MapPosition>` – animate [`MapPosition`](super::mapposition::MapPosition)
//! - `Tween<Rotation>` – animate [`Rotation`](super::rotation::Rotation)
//! - `Tween<Scale>` – animate [`Scale`](super::scale::Scale)
//!
//! Each tween supports multiple [`Easing`] functions and [`LoopMode`] settings.
//! See [`crate::systems::tween`] for the update systems.

use std::fmt::Debug;

use bevy_ecs::component::Mutable;
use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;

/// Determines how a tween behaves when it reaches the end.
#[derive(Copy, Clone, Debug)]
pub enum LoopMode {
    /// Play once and stop.
    Once,
    /// Restart from the beginning when finished.
    Loop,
    /// Reverse direction when reaching either end.
    PingPong,
}

impl std::str::FromStr for LoopMode {
    type Err = std::convert::Infallible;

    /// Parse a Lua string into a `LoopMode`. Unknown strings default to `Once`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "once" => LoopMode::Once,
            "loop" => LoopMode::Loop,
            "ping_pong" => LoopMode::PingPong,
            _ => LoopMode::Once,
        })
    }
}

/// Easing functions for smooth interpolation.
///
/// These functions transform a linear `t` value (0.0 to 1.0) to create
/// different acceleration/deceleration curves.
#[derive(Copy, Clone, Debug)]
pub enum Easing {
    /// Constant speed (no easing).
    Linear,
    /// Starts slow, accelerates (quadratic).
    QuadIn,
    /// Starts fast, decelerates (quadratic).
    QuadOut,
    /// Slow start and end (quadratic).
    QuadInOut,
    /// Starts slow, accelerates (cubic).
    CubicIn,
    /// Starts fast, decelerates (cubic).
    CubicOut,
    /// Slow start and end (cubic).
    CubicInOut,
}

impl std::str::FromStr for Easing {
    type Err = std::convert::Infallible;

    /// Parse a Lua string into an `Easing`. Unknown strings default to `Linear`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "linear" => Easing::Linear,
            "quad_in" => Easing::QuadIn,
            "quad_out" => Easing::QuadOut,
            "quad_in_out" => Easing::QuadInOut,
            "cubic_in" => Easing::CubicIn,
            "cubic_out" => Easing::CubicOut,
            "cubic_in_out" => Easing::CubicInOut,
            _ => Easing::Linear,
        })
    }
}

fn lerp_v2(a: Vector2, b: Vector2, t: f32) -> Vector2 {
    Vector2 {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Value trait for tweenable ECS components.
///
/// Implementors define how to interpolate between two component values for a
/// normalized time `t` in the range `[0.0, 1.0]`.
pub trait TweenValue: Component<Mutability = Mutable> + Clone + Debug {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self;
}

impl TweenValue for MapPosition {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self::from_vec(lerp_v2(from.pos, to.pos, t))
    }
}

impl TweenValue for Rotation {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            degrees: lerp_f32(from.degrees, to.degrees, t),
        }
    }
}

impl TweenValue for Scale {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            scale: lerp_v2(from.scale, to.scale, t),
        }
    }
}

/// Generic tween component for interpolating between two component values.
#[derive(Component, Clone, Debug)]
pub struct Tween<T: TweenValue> {
    /// Starting value.
    pub from: T,
    /// Ending value.
    pub to: T,
    /// Duration in seconds.
    pub duration: f32,
    /// Easing function to use.
    pub easing: Easing,
    /// Behavior when the tween ends.
    pub loop_mode: LoopMode,
    /// Whether the tween is currently playing.
    pub playing: bool,
    /// Current time within the tween.
    pub time: f32,
    /// Direction of playback (true = forward).
    pub forward: bool,
}

impl<T: TweenValue> Tween<T> {
    pub fn new(from: T, to: T, duration: f32) -> Self {
        Self {
            from,
            to,
            duration,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
            playing: true,
            time: 0.0,
            forward: true,
        }
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn with_loop_mode(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }

    pub fn with_backwards(mut self) -> Self {
        self.time = self.duration;
        self.forward = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn vec_approx_eq(a: Vector2, b: Vector2) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y)
    }

    fn map_position(x: f32, y: f32) -> MapPosition {
        MapPosition::from_vec(Vector2 { x, y })
    }

    fn scale(x: f32, y: f32) -> Scale {
        Scale::new(x, y)
    }

    // ==================== GENERIC TWEEN TESTS ====================

    #[test]
    fn test_tween_map_position_new() {
        let from = map_position(0.0, 0.0);
        let to = map_position(100.0, 200.0);
        let tw: Tween<MapPosition> = Tween::new(from, to, 2.0);

        assert!(vec_approx_eq(tw.from.pos, from.pos));
        assert!(vec_approx_eq(tw.to.pos, to.pos));
        assert!(approx_eq(tw.duration, 2.0));
        assert!(matches!(tw.easing, Easing::Linear));
        assert!(matches!(tw.loop_mode, LoopMode::Once));
        assert!(tw.playing);
        assert!(approx_eq(tw.time, 0.0));
        assert!(tw.forward);
    }

    #[test]
    fn test_tween_map_position_with_easing() {
        let tw: Tween<MapPosition> = Tween::new(map_position(0.0, 0.0), map_position(10.0, 10.0), 1.0)
            .with_easing(Easing::QuadIn);

        assert!(matches!(tw.easing, Easing::QuadIn));
    }

    #[test]
    fn test_tween_map_position_with_loop_mode() {
        let tw: Tween<MapPosition> =
            Tween::new(map_position(0.0, 0.0), map_position(10.0, 10.0), 1.0)
                .with_loop_mode(LoopMode::PingPong);

        assert!(matches!(tw.loop_mode, LoopMode::PingPong));
    }

    #[test]
    fn test_tween_map_position_with_backwards() {
        let tw: Tween<MapPosition> = Tween::new(map_position(0.0, 0.0), map_position(10.0, 10.0), 2.0)
            .with_backwards();

        assert!(approx_eq(tw.time, 2.0));
        assert!(!tw.forward);
    }

    #[test]
    fn test_tween_map_position_builder_chaining() {
        let tw: Tween<MapPosition> = Tween::new(map_position(0.0, 0.0), map_position(10.0, 10.0), 1.0)
            .with_easing(Easing::CubicOut)
            .with_loop_mode(LoopMode::Loop)
            .with_backwards();

        assert!(matches!(tw.easing, Easing::CubicOut));
        assert!(matches!(tw.loop_mode, LoopMode::Loop));
        assert!(!tw.forward);
    }

    #[test]
    fn test_tween_rotation_new() {
        let from = Rotation { degrees: 0.0 };
        let to = Rotation { degrees: 360.0 };
        let tw: Tween<Rotation> = Tween::new(from, to, 1.5);

        assert!(approx_eq(tw.from.degrees, from.degrees));
        assert!(approx_eq(tw.to.degrees, to.degrees));
        assert!(approx_eq(tw.duration, 1.5));
        assert!(matches!(tw.easing, Easing::Linear));
        assert!(matches!(tw.loop_mode, LoopMode::Once));
        assert!(tw.playing);
        assert!(approx_eq(tw.time, 0.0));
        assert!(tw.forward);
    }

    #[test]
    fn test_tween_rotation_with_easing() {
        let tw: Tween<Rotation> =
            Tween::new(Rotation { degrees: 0.0 }, Rotation { degrees: 180.0 }, 1.0)
                .with_easing(Easing::QuadOut);
        assert!(matches!(tw.easing, Easing::QuadOut));
    }

    #[test]
    fn test_tween_rotation_with_loop_mode() {
        let tw: Tween<Rotation> =
            Tween::new(Rotation { degrees: 0.0 }, Rotation { degrees: 180.0 }, 1.0)
                .with_loop_mode(LoopMode::Loop);
        assert!(matches!(tw.loop_mode, LoopMode::Loop));
    }

    #[test]
    fn test_tween_rotation_with_backwards() {
        let tw: Tween<Rotation> =
            Tween::new(Rotation { degrees: 0.0 }, Rotation { degrees: 180.0 }, 3.0)
                .with_backwards();
        assert!(approx_eq(tw.time, 3.0));
        assert!(!tw.forward);
    }

    #[test]
    fn test_tween_rotation_negative_angles() {
        let tw: Tween<Rotation> =
            Tween::new(Rotation { degrees: -90.0 }, Rotation { degrees: 90.0 }, 1.0);
        assert!(approx_eq(tw.from.degrees, -90.0));
        assert!(approx_eq(tw.to.degrees, 90.0));
    }

    #[test]
    fn test_tween_scale_new() {
        let from = scale(1.0, 1.0);
        let to = scale(2.0, 2.0);
        let tw: Tween<Scale> = Tween::new(from, to, 0.5);

        assert!(vec_approx_eq(tw.from.scale, from.scale));
        assert!(vec_approx_eq(tw.to.scale, to.scale));
        assert!(approx_eq(tw.duration, 0.5));
        assert!(matches!(tw.easing, Easing::Linear));
        assert!(matches!(tw.loop_mode, LoopMode::Once));
        assert!(tw.playing);
        assert!(approx_eq(tw.time, 0.0));
        assert!(tw.forward);
    }

    #[test]
    fn test_tween_scale_with_easing() {
        let tw: Tween<Scale> = Tween::new(scale(1.0, 1.0), scale(2.0, 2.0), 1.0)
            .with_easing(Easing::CubicInOut);

        assert!(matches!(tw.easing, Easing::CubicInOut));
    }

    #[test]
    fn test_tween_scale_with_loop_mode() {
        let tw: Tween<Scale> = Tween::new(scale(1.0, 1.0), scale(2.0, 2.0), 1.0)
            .with_loop_mode(LoopMode::PingPong);

        assert!(matches!(tw.loop_mode, LoopMode::PingPong));
    }

    #[test]
    fn test_tween_scale_with_backwards() {
        let tw: Tween<Scale> = Tween::new(scale(1.0, 1.0), scale(2.0, 2.0), 4.0).with_backwards();

        assert!(approx_eq(tw.time, 4.0));
        assert!(!tw.forward);
    }

    #[test]
    fn test_tween_scale_non_uniform() {
        let from = scale(1.0, 2.0);
        let to = scale(3.0, 0.5);
        let tw: Tween<Scale> = Tween::new(from, to, 1.0);

        assert!(vec_approx_eq(tw.from.scale, from.scale));
        assert!(vec_approx_eq(tw.to.scale, to.scale));
    }

    #[test]
    fn test_map_position_interpolation() {
        let mid = MapPosition::interpolate(&map_position(0.0, 0.0), &map_position(10.0, 20.0), 0.5);
        assert!(vec_approx_eq(mid.pos, Vector2 { x: 5.0, y: 10.0 }));
    }

    #[test]
    fn test_rotation_interpolation() {
        let mid = Rotation::interpolate(
            &Rotation { degrees: -90.0 },
            &Rotation { degrees: 90.0 },
            0.75,
        );
        assert!(approx_eq(mid.degrees, 45.0));
    }

    #[test]
    fn test_scale_interpolation() {
        let mid = Scale::interpolate(&scale(1.0, 2.0), &scale(3.0, 6.0), 0.5);
        assert!(vec_approx_eq(mid.scale, Vector2 { x: 2.0, y: 4.0 }));
    }

    // ==================== EASING ENUM TESTS ====================

    #[test]
    fn test_easing_variants_exist() {
        let _linear = Easing::Linear;
        let _quad_in = Easing::QuadIn;
        let _quad_out = Easing::QuadOut;
        let _quad_inout = Easing::QuadInOut;
        let _cubic_in = Easing::CubicIn;
        let _cubic_out = Easing::CubicOut;
        let _cubic_inout = Easing::CubicInOut;
    }

    #[test]
    fn test_easing_is_copy() {
        let e1 = Easing::Linear;
        let e2 = e1;
        assert!(matches!(e1, Easing::Linear));
        assert!(matches!(e2, Easing::Linear));
    }

    // ==================== LOOP MODE ENUM TESTS ====================

    #[test]
    fn test_loop_mode_variants_exist() {
        let _once = LoopMode::Once;
        let _loop_mode = LoopMode::Loop;
        let _pingpong = LoopMode::PingPong;
    }

    #[test]
    fn test_loop_mode_is_copy() {
        let l1 = LoopMode::PingPong;
        let l2 = l1;
        assert!(matches!(l1, LoopMode::PingPong));
        assert!(matches!(l2, LoopMode::PingPong));
    }

    // ==================== EASING FROM_STR TESTS ====================

    #[test]
    fn test_easing_from_str_linear() {
        assert!(matches!("linear".parse::<Easing>().unwrap(), Easing::Linear));
    }

    #[test]
    fn test_easing_from_str_quad_in() {
        assert!(matches!("quad_in".parse::<Easing>().unwrap(), Easing::QuadIn));
    }

    #[test]
    fn test_easing_from_str_quad_out() {
        assert!(matches!("quad_out".parse::<Easing>().unwrap(), Easing::QuadOut));
    }

    #[test]
    fn test_easing_from_str_quad_in_out() {
        assert!(matches!(
            "quad_in_out".parse::<Easing>().unwrap(),
            Easing::QuadInOut
        ));
    }

    #[test]
    fn test_easing_from_str_cubic_in() {
        assert!(matches!("cubic_in".parse::<Easing>().unwrap(), Easing::CubicIn));
    }

    #[test]
    fn test_easing_from_str_cubic_out() {
        assert!(matches!("cubic_out".parse::<Easing>().unwrap(), Easing::CubicOut));
    }

    #[test]
    fn test_easing_from_str_cubic_in_out() {
        assert!(matches!(
            "cubic_in_out".parse::<Easing>().unwrap(),
            Easing::CubicInOut
        ));
    }

    #[test]
    fn test_easing_from_str_unknown_defaults_to_linear() {
        assert!(matches!("unknown".parse::<Easing>().unwrap(), Easing::Linear));
        assert!(matches!("".parse::<Easing>().unwrap(), Easing::Linear));
    }

    // ==================== LOOP MODE FROM_STR TESTS ====================

    #[test]
    fn test_loop_mode_from_str_once() {
        assert!(matches!("once".parse::<LoopMode>().unwrap(), LoopMode::Once));
    }

    #[test]
    fn test_loop_mode_from_str_loop() {
        assert!(matches!("loop".parse::<LoopMode>().unwrap(), LoopMode::Loop));
    }

    #[test]
    fn test_loop_mode_from_str_ping_pong() {
        assert!(matches!(
            "ping_pong".parse::<LoopMode>().unwrap(),
            LoopMode::PingPong
        ));
    }

    #[test]
    fn test_loop_mode_from_str_unknown_defaults_to_once() {
        assert!(matches!("unknown".parse::<LoopMode>().unwrap(), LoopMode::Once));
    }

    #[test]
    fn test_loop_mode_from_str_empty_defaults_to_once() {
        assert!(matches!("".parse::<LoopMode>().unwrap(), LoopMode::Once));
    }
}
