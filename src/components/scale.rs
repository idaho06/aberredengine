//! Scale component for 2D entities.
//!
//! The [`Scale`] component stores a 2D scale factor applied to sprites during
//! rendering. Values greater than 1.0 enlarge the sprite; values less than 1.0
//! shrink it. Negative values can be used to flip.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// 2D scale factor for sprite rendering.
///
/// The render system multiplies sprite dimensions by these values. Can be
/// animated via [`TweenScale`](super::tween::TweenScale).
#[derive(Component, Clone, Debug, Copy)]
pub struct Scale {
    pub scale: Vector2,
}
impl Scale {
    pub fn new(sx: f32, sy: f32) -> Self {
        Self {
            scale: Vector2 { x: sx, y: sy },
        }
    }
}
impl Default for Scale {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_new() {
        let s = Scale::new(2.0, 3.0);
        assert!(approx_eq(s.scale.x, 2.0));
        assert!(approx_eq(s.scale.y, 3.0));
    }

    #[test]
    fn test_default_is_one_one() {
        let s = Scale::default();
        assert!(approx_eq(s.scale.x, 1.0));
        assert!(approx_eq(s.scale.y, 1.0));
    }

    #[test]
    fn test_non_uniform_scale() {
        let s = Scale::new(0.5, 2.0);
        assert!(approx_eq(s.scale.x, 0.5));
        assert!(approx_eq(s.scale.y, 2.0));
    }

    #[test]
    fn test_zero_scale() {
        let s = Scale::new(0.0, 0.0);
        assert!(approx_eq(s.scale.x, 0.0));
        assert!(approx_eq(s.scale.y, 0.0));
    }

    #[test]
    fn test_negative_scale() {
        let s = Scale::new(-1.0, -1.0);
        assert!(approx_eq(s.scale.x, -1.0));
        assert!(approx_eq(s.scale.y, -1.0));
    }
}
