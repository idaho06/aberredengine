//! Rotation component for 2D entities.
//!
//! The [`Rotation`] component stores a rotation angle in degrees. The render
//! system uses this value when drawing sprites to rotate them around their
//! origin.

use bevy_ecs::prelude::Component;

/// Rotation angle in degrees for 2D rendering.
///
/// Positive values rotate clockwise. Used by the render system when drawing
/// sprites and can be animated via [`TweenRotation`](super::tween::TweenRotation).
#[derive(Component, Clone, Debug, Copy, Default)]
pub struct Rotation {
    pub degrees: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_construction() {
        let r = Rotation { degrees: 45.0 };
        assert!(approx_eq(r.degrees, 45.0));
    }

    #[test]
    fn test_default_is_zero() {
        let r = Rotation::default();
        assert!(approx_eq(r.degrees, 0.0));
    }

    #[test]
    fn test_negative_degrees() {
        let r = Rotation { degrees: -90.0 };
        assert!(approx_eq(r.degrees, -90.0));
    }

    #[test]
    fn test_large_values() {
        let r = Rotation { degrees: 720.0 };
        assert!(approx_eq(r.degrees, 720.0));
    }

    #[test]
    fn test_copy_trait() {
        let r = Rotation { degrees: 30.0 };
        let r2 = r;
        assert!(approx_eq(r.degrees, 30.0));
        assert!(approx_eq(r2.degrees, 30.0));
    }
}
