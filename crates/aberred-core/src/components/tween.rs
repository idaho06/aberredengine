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

use crate::math::Vec2;
use bevy_ecs::component::Mutable;
use bevy_ecs::prelude::Component;

use crate::components::position2d::{Position2D, PositionSpace};
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

impl LoopMode {
    /// Every name [`LoopMode::from_name`] accepts.
    pub const NAMES: [&'static str; 3] = ["once", "loop", "ping_pong"];

    /// Strict parse: `None` for anything not in [`LoopMode::NAMES`].
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "once" => Some(LoopMode::Once),
            "loop" => Some(LoopMode::Loop),
            "ping_pong" => Some(LoopMode::PingPong),
            _ => None,
        }
    }
}

impl std::str::FromStr for LoopMode {
    type Err = std::convert::Infallible;

    /// Lenient parse: unknown strings default to `Once`. Lua entry points validate with
    /// [`LoopMode::from_name`] first, so a typo is an error there, not a silent `Once`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_name(s).unwrap_or(LoopMode::Once))
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

impl Easing {
    /// Every name [`Easing::from_name`] accepts.
    pub const NAMES: [&'static str; 7] = [
        "linear",
        "quad_in",
        "quad_out",
        "quad_in_out",
        "cubic_in",
        "cubic_out",
        "cubic_in_out",
    ];

    /// Strict parse: `None` for anything not in [`Easing::NAMES`].
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "linear" => Some(Easing::Linear),
            "quad_in" => Some(Easing::QuadIn),
            "quad_out" => Some(Easing::QuadOut),
            "quad_in_out" => Some(Easing::QuadInOut),
            "cubic_in" => Some(Easing::CubicIn),
            "cubic_out" => Some(Easing::CubicOut),
            "cubic_in_out" => Some(Easing::CubicInOut),
            _ => None,
        }
    }
}

impl std::str::FromStr for Easing {
    type Err = std::convert::Infallible;

    /// Lenient parse: unknown strings default to `Linear`. Lua entry points validate with
    /// [`Easing::from_name`] first, so a typo is an error there, not a silent `Linear`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_name(s).unwrap_or(Easing::Linear))
    }
}

/// Linear interpolation for tweenable scalar/vector value types.
pub trait Lerp: Copy {
    fn lerp(a: Self, b: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

impl Lerp for Vec2 {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a.lerp(b, t)
    }
}

/// Value trait for tweenable ECS components.
///
/// Implementors define how to interpolate between two component values for a
/// normalized time `t` in the range `[0.0, 1.0]`.
pub trait TweenValue: Component<Mutability = Mutable> + Clone + Debug {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self;
}

impl<S: PositionSpace + Debug> TweenValue for Position2D<S> {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self::from_vec(Lerp::lerp(from.pos, to.pos, t))
    }
}

impl TweenValue for Rotation {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            degrees: f32::lerp(from.degrees, to.degrees, t),
        }
    }
}

impl TweenValue for Scale {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            scale: Lerp::lerp(from.scale, to.scale, t),
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
    use crate::components::mapposition::MapPosition;
    use crate::components::screenposition::ScreenPosition;
    use crate::testing::{approx_eq, vec2_approx_eq};

    fn map_position(x: f32, y: f32) -> MapPosition {
        MapPosition::from_vec(Vec2 { x, y })
    }

    fn screen_position(x: f32, y: f32) -> ScreenPosition {
        ScreenPosition::from_vec(Vec2 { x, y })
    }

    fn scale(x: f32, y: f32) -> Scale {
        Scale::new(x, y)
    }

    #[test]
    fn new_starts_playing_forward_linear_once() {
        let tw: Tween<MapPosition> =
            Tween::new(map_position(0.0, 0.0), map_position(1.0, 1.0), 2.0);
        assert!(tw.playing && tw.forward);
        assert_eq!(tw.time, 0.0);
        assert!(matches!(tw.easing, Easing::Linear));
        assert!(matches!(tw.loop_mode, LoopMode::Once));
    }

    #[test]
    fn with_backwards_starts_at_end_reversed() {
        let tw: Tween<MapPosition> =
            Tween::new(map_position(0.0, 0.0), map_position(10.0, 10.0), 2.0).with_backwards();
        assert!(approx_eq(tw.time, 2.0));
        assert!(!tw.forward);
    }

    #[test]
    fn map_position_interpolation() {
        let mid = MapPosition::interpolate(&map_position(0.0, 0.0), &map_position(10.0, 20.0), 0.5);
        assert!(vec2_approx_eq(mid.pos, Vec2 { x: 5.0, y: 10.0 }));
    }

    #[test]
    fn screen_position_interpolation() {
        let mid = ScreenPosition::interpolate(
            &screen_position(0.0, 0.0),
            &screen_position(10.0, 20.0),
            0.5,
        );
        assert!(vec2_approx_eq(mid.pos, Vec2 { x: 5.0, y: 10.0 }));
    }

    #[test]
    fn rotation_interpolation() {
        let mid = Rotation::interpolate(
            &Rotation { degrees: -90.0 },
            &Rotation { degrees: 90.0 },
            0.75,
        );
        assert!(approx_eq(mid.degrees, 45.0));
    }

    #[test]
    fn scale_interpolation() {
        let mid = Scale::interpolate(&scale(1.0, 2.0), &scale(3.0, 6.0), 0.5);
        assert!(vec2_approx_eq(mid.scale, Vec2 { x: 2.0, y: 4.0 }));
    }

    // Lua passes these strings (`engine.lua` stubs); unknown values fall back
    // instead of erroring.
    #[test]
    fn easing_from_str() {
        for (s, expected) in [
            ("linear", Easing::Linear),
            ("quad_in", Easing::QuadIn),
            ("quad_out", Easing::QuadOut),
            ("quad_in_out", Easing::QuadInOut),
            ("cubic_in", Easing::CubicIn),
            ("cubic_out", Easing::CubicOut),
            ("cubic_in_out", Easing::CubicInOut),
            ("bogus", Easing::Linear),
        ] {
            let parsed: Easing = s.parse().unwrap();
            assert_eq!(format!("{parsed:?}"), format!("{expected:?}"), "{s:?}");
        }
    }

    #[test]
    fn loop_mode_from_str() {
        for (s, expected) in [
            ("once", LoopMode::Once),
            ("loop", LoopMode::Loop),
            ("ping_pong", LoopMode::PingPong),
            ("bogus", LoopMode::Once),
            ("", LoopMode::Once),
        ] {
            let parsed: LoopMode = s.parse().unwrap();
            assert_eq!(format!("{parsed:?}"), format!("{expected:?}"), "{s:?}");
        }
    }

    #[test]
    fn from_name_accepts_exactly_the_listed_names() {
        for name in Easing::NAMES {
            assert!(Easing::from_name(name).is_some(), "{name}");
        }
        for name in LoopMode::NAMES {
            assert!(LoopMode::from_name(name).is_some(), "{name}");
        }
        for bad in ["", "Linear", "quad_inn", "ease_in"] {
            assert!(Easing::from_name(bad).is_none(), "{bad}");
        }
        for bad in ["", "Once", "pingpong", "repeat"] {
            assert!(LoopMode::from_name(bad).is_none(), "{bad}");
        }
    }
}
