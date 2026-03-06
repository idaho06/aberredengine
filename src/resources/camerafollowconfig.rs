//! Camera-follow configuration resource.
//!
//! [`CameraFollowConfig`] controls how the camera tracks entities marked with
//! [`CameraTarget`](crate::components::cameratarget::CameraTarget). It is
//! inserted by the engine with `enabled: false` and can be activated and tuned
//! at any time.

use bevy_ecs::prelude::Resource;
use raylib::prelude::{Rectangle, Vector2};

/// How the camera approaches its target position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FollowMode {
    /// Snap to the target every frame with no smoothing.
    Instant,
    /// Smoothed approach governed by [`EasingCurve`] and
    /// [`lerp_speed`](CameraFollowConfig::lerp_speed).
    Lerp,
    /// Spring-damper simulation. Produces natural-feeling motion with optional
    /// overshoot. Tuned via
    /// [`spring_stiffness`](CameraFollowConfig::spring_stiffness) and
    /// [`spring_damping`](CameraFollowConfig::spring_damping).
    SmoothDamp,
    /// Camera holds still while the target stays inside a rectangle centred on
    /// the camera. Once the target exits the deadzone the camera catches up
    /// with a plain linear lerp at
    /// [`lerp_speed`](CameraFollowConfig::lerp_speed).
    Deadzone {
        /// Half-width of the deadzone rectangle in world units.
        half_w: f32,
        /// Half-height of the deadzone rectangle in world units.
        half_h: f32,
    },
}

/// Easing curve applied when [`FollowMode::Lerp`] is active.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EasingCurve {
    /// Constant-speed interpolation.
    Linear,
    /// Exponential decay — fast start, slow finish. Frame-rate independent.
    /// This is the most common camera easing and the default.
    #[default]
    EaseOut,
    /// Slow start, fast finish.
    EaseIn,
    /// Smooth both ends.
    EaseInOut,
}

/// Configuration and internal state for the camera-follow system.
///
/// Inserted by the engine with [`Default`] values (`enabled: false`).
/// Activate it and adjust settings from your setup/scene-enter code.
#[derive(Resource, Clone, Debug)]
pub struct CameraFollowConfig {
    /// Master switch. When `false` the camera-follow system is a no-op.
    pub enabled: bool,
    /// Following behaviour. See [`FollowMode`].
    pub mode: FollowMode,
    /// Easing curve used by [`FollowMode::Lerp`].
    pub easing: EasingCurve,
    /// Speed factor for [`FollowMode::Lerp`] and catch-up speed for
    /// [`FollowMode::Deadzone`]. Higher values = faster approach.
    pub lerp_speed: f32,
    /// Spring stiffness for [`FollowMode::SmoothDamp`]. Higher = stiffer.
    pub spring_stiffness: f32,
    /// Damping factor for [`FollowMode::SmoothDamp`]. Higher = less bounce.
    pub spring_damping: f32,
    /// Fixed offset added to the target position (in world units).
    pub offset: Vector2,
    /// Optional world-space bounding rectangle. When set, the camera position
    /// is clamped so that the viewport stays inside these bounds.
    pub bounds: Option<Rectangle>,

    // -- internal state (not intended for direct user modification) ----------
    /// Spring velocity for [`FollowMode::SmoothDamp`]. Reset to zero when
    /// switching targets or modes.
    pub(crate) velocity: Vector2,
}

impl Default for CameraFollowConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: FollowMode::Lerp,
            easing: EasingCurve::default(), // EaseOut
            lerp_speed: 5.0,
            spring_stiffness: 10.0,
            spring_damping: 5.0,
            offset: Vector2 { x: 0.0, y: 0.0 },
            bounds: None,
            velocity: Vector2 { x: 0.0, y: 0.0 },
        }
    }
}

impl CameraFollowConfig {
    /// Reset internal spring velocity. Call this when switching targets or
    /// modes to avoid a sudden jump.
    pub fn reset_velocity(&mut self) {
        self.velocity = Vector2 { x: 0.0, y: 0.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_disabled() {
        let cfg = CameraFollowConfig::default();
        assert!(!cfg.enabled);
    }

    #[test]
    fn default_mode_is_lerp() {
        let cfg = CameraFollowConfig::default();
        assert_eq!(cfg.mode, FollowMode::Lerp);
    }

    #[test]
    fn default_easing_is_ease_out() {
        let cfg = CameraFollowConfig::default();
        assert_eq!(cfg.easing, EasingCurve::EaseOut);
    }

    #[test]
    fn reset_velocity_zeroes() {
        let mut cfg = CameraFollowConfig::default();
        cfg.velocity = Vector2 { x: 99.0, y: -42.0 };
        cfg.reset_velocity();
        assert_eq!(cfg.velocity.x, 0.0);
        assert_eq!(cfg.velocity.y, 0.0);
    }
}
