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
