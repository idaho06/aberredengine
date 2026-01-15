//! Tween components for animated interpolation.
//!
//! This module provides components for smoothly animating entity properties
//! over time:
//! - [`TweenPosition`] – animate [`MapPosition`](super::mapposition::MapPosition)
//! - [`TweenRotation`] – animate [`Rotation`](super::rotation::Rotation)
//! - [`TweenScale`] – animate [`Scale`](super::scale::Scale)
//!
//! Each tween supports multiple [`Easing`] functions and [`LoopMode`] settings.
//! See [`crate::systems::tween`] for the update systems.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

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

/// Animates an entity's [`MapPosition`](super::mapposition::MapPosition) between two points.
///
/// The tween interpolates `from` to `to` over `duration` seconds using the
/// specified `easing` function and `loop_mode`.
#[derive(Component, Clone, Debug)]
pub struct TweenPosition {
    /// Starting position.
    pub from: Vector2,
    /// Ending position.
    pub to: Vector2,
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

impl TweenPosition {
    pub fn new(from: Vector2, to: Vector2, duration: f32) -> Self {
        TweenPosition {
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

/// Animates an entity's [`Rotation`](super::rotation::Rotation) between two angles.
///
/// The tween interpolates `from` to `to` (in degrees) over `duration` seconds.
#[derive(Component, Clone, Debug)]
pub struct TweenRotation {
    /// Starting angle in degrees.
    pub from: f32,
    /// Ending angle in degrees.
    pub to: f32,
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
impl TweenRotation {
    pub fn new(from: f32, to: f32, duration: f32) -> Self {
        TweenRotation {
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

/// Animates an entity's [`Scale`](super::scale::Scale) between two values.
///
/// The tween interpolates `from` to `to` over `duration` seconds.
#[derive(Component, Clone, Debug)]
pub struct TweenScale {
    /// Starting scale.
    pub from: Vector2,
    /// Ending scale.
    pub to: Vector2,
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

impl TweenScale {
    pub fn new(from: Vector2, to: Vector2, duration: f32) -> Self {
        TweenScale {
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

    // ==================== TWEEN POSITION TESTS ====================

    #[test]
    fn test_tween_position_new() {
        let from = Vector2 { x: 0.0, y: 0.0 };
        let to = Vector2 { x: 100.0, y: 200.0 };
        let tw = TweenPosition::new(from, to, 2.0);

        assert!(vec_approx_eq(tw.from, from));
        assert!(vec_approx_eq(tw.to, to));
        assert!(approx_eq(tw.duration, 2.0));
        assert!(matches!(tw.easing, Easing::Linear));
        assert!(matches!(tw.loop_mode, LoopMode::Once));
        assert!(tw.playing);
        assert!(approx_eq(tw.time, 0.0));
        assert!(tw.forward);
    }

    #[test]
    fn test_tween_position_with_easing() {
        let tw = TweenPosition::new(
            Vector2 { x: 0.0, y: 0.0 },
            Vector2 { x: 10.0, y: 10.0 },
            1.0,
        )
        .with_easing(Easing::QuadIn);

        assert!(matches!(tw.easing, Easing::QuadIn));
    }

    #[test]
    fn test_tween_position_with_loop_mode() {
        let tw = TweenPosition::new(
            Vector2 { x: 0.0, y: 0.0 },
            Vector2 { x: 10.0, y: 10.0 },
            1.0,
        )
        .with_loop_mode(LoopMode::PingPong);

        assert!(matches!(tw.loop_mode, LoopMode::PingPong));
    }

    #[test]
    fn test_tween_position_with_backwards() {
        let tw = TweenPosition::new(
            Vector2 { x: 0.0, y: 0.0 },
            Vector2 { x: 10.0, y: 10.0 },
            2.0,
        )
        .with_backwards();

        assert!(approx_eq(tw.time, 2.0)); // time set to duration
        assert!(!tw.forward);
    }

    #[test]
    fn test_tween_position_builder_chaining() {
        let tw = TweenPosition::new(
            Vector2 { x: 0.0, y: 0.0 },
            Vector2 { x: 10.0, y: 10.0 },
            1.0,
        )
        .with_easing(Easing::CubicOut)
        .with_loop_mode(LoopMode::Loop)
        .with_backwards();

        assert!(matches!(tw.easing, Easing::CubicOut));
        assert!(matches!(tw.loop_mode, LoopMode::Loop));
        assert!(!tw.forward);
    }

    // ==================== TWEEN ROTATION TESTS ====================

    #[test]
    fn test_tween_rotation_new() {
        let tw = TweenRotation::new(0.0, 360.0, 1.5);

        assert!(approx_eq(tw.from, 0.0));
        assert!(approx_eq(tw.to, 360.0));
        assert!(approx_eq(tw.duration, 1.5));
        assert!(matches!(tw.easing, Easing::Linear));
        assert!(matches!(tw.loop_mode, LoopMode::Once));
        assert!(tw.playing);
        assert!(approx_eq(tw.time, 0.0));
        assert!(tw.forward);
    }

    #[test]
    fn test_tween_rotation_with_easing() {
        let tw = TweenRotation::new(0.0, 180.0, 1.0).with_easing(Easing::QuadOut);
        assert!(matches!(tw.easing, Easing::QuadOut));
    }

    #[test]
    fn test_tween_rotation_with_loop_mode() {
        let tw = TweenRotation::new(0.0, 180.0, 1.0).with_loop_mode(LoopMode::Loop);
        assert!(matches!(tw.loop_mode, LoopMode::Loop));
    }

    #[test]
    fn test_tween_rotation_with_backwards() {
        let tw = TweenRotation::new(0.0, 180.0, 3.0).with_backwards();
        assert!(approx_eq(tw.time, 3.0));
        assert!(!tw.forward);
    }

    #[test]
    fn test_tween_rotation_negative_angles() {
        let tw = TweenRotation::new(-90.0, 90.0, 1.0);
        assert!(approx_eq(tw.from, -90.0));
        assert!(approx_eq(tw.to, 90.0));
    }

    // ==================== TWEEN SCALE TESTS ====================

    #[test]
    fn test_tween_scale_new() {
        let from = Vector2 { x: 1.0, y: 1.0 };
        let to = Vector2 { x: 2.0, y: 2.0 };
        let tw = TweenScale::new(from, to, 0.5);

        assert!(vec_approx_eq(tw.from, from));
        assert!(vec_approx_eq(tw.to, to));
        assert!(approx_eq(tw.duration, 0.5));
        assert!(matches!(tw.easing, Easing::Linear));
        assert!(matches!(tw.loop_mode, LoopMode::Once));
        assert!(tw.playing);
        assert!(approx_eq(tw.time, 0.0));
        assert!(tw.forward);
    }

    #[test]
    fn test_tween_scale_with_easing() {
        let tw = TweenScale::new(
            Vector2 { x: 1.0, y: 1.0 },
            Vector2 { x: 2.0, y: 2.0 },
            1.0,
        )
        .with_easing(Easing::CubicInOut);

        assert!(matches!(tw.easing, Easing::CubicInOut));
    }

    #[test]
    fn test_tween_scale_with_loop_mode() {
        let tw = TweenScale::new(
            Vector2 { x: 1.0, y: 1.0 },
            Vector2 { x: 2.0, y: 2.0 },
            1.0,
        )
        .with_loop_mode(LoopMode::PingPong);

        assert!(matches!(tw.loop_mode, LoopMode::PingPong));
    }

    #[test]
    fn test_tween_scale_with_backwards() {
        let tw = TweenScale::new(
            Vector2 { x: 1.0, y: 1.0 },
            Vector2 { x: 2.0, y: 2.0 },
            4.0,
        )
        .with_backwards();

        assert!(approx_eq(tw.time, 4.0));
        assert!(!tw.forward);
    }

    #[test]
    fn test_tween_scale_non_uniform() {
        let from = Vector2 { x: 1.0, y: 2.0 };
        let to = Vector2 { x: 3.0, y: 0.5 };
        let tw = TweenScale::new(from, to, 1.0);

        assert!(vec_approx_eq(tw.from, from));
        assert!(vec_approx_eq(tw.to, to));
    }

    // ==================== EASING ENUM TESTS ====================

    #[test]
    fn test_easing_variants_exist() {
        // Ensure all variants can be created
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
        let e2 = e1; // copy
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
        let l2 = l1; // copy
        assert!(matches!(l1, LoopMode::PingPong));
        assert!(matches!(l2, LoopMode::PingPong));
    }
}
