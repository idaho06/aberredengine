//! Z-index component for render ordering.
//!
//! The [`ZIndex`] component provides a simple way to control the drawing
//! order of entities. Entities with higher z-index values are drawn on top
//! of those with lower values.

use bevy_ecs::prelude::Component;

/// Rendering order hint for 2D drawing.
///
/// Higher values are drawn later (on top). The renderer sorts by
/// `ZIndex` to achieve a painter's algorithm.
#[derive(Component, Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct ZIndex(pub f32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zindex_construction() {
        let z = ZIndex(5.0);
        assert!((z.0 - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zindex_negative() {
        let z = ZIndex(-10.0);
        assert!((z.0 - (-10.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zindex_zero() {
        let z = ZIndex(0.0);
        assert!((z.0 - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zindex_fractional() {
        // Test that fractional values between integers work correctly
        let z1 = ZIndex(1.0);
        let z2 = ZIndex(1.5);
        let z3 = ZIndex(2.0);

        assert!(z1 < z2);
        assert!(z2 < z3);
        assert!(z1 < z3);
    }

    #[test]
    fn test_zindex_comparison_positive() {
        let z1 = ZIndex(1.0);
        let z2 = ZIndex(2.0);

        assert!(z1 < z2);
        assert!(z2 > z1);
        assert!(z1 != z2);
    }

    #[test]
    fn test_zindex_comparison_negative() {
        let z1 = ZIndex(-5.0);
        let z2 = ZIndex(-1.0);
        let z3 = ZIndex(0.0);

        assert!(z1 < z2);
        assert!(z2 < z3);
        assert!(z1 < z3);
    }

    #[test]
    fn test_zindex_equality() {
        let z1 = ZIndex(3.0);
        let z2 = ZIndex(3.0);

        assert_eq!(z1, z2);
    }

    #[test]
    fn test_zindex_sorting() {
        let mut zindices = vec![
            ZIndex(10.0),
            ZIndex(-5.0),
            ZIndex(0.0),
            ZIndex(2.5),
            ZIndex(-0.5),
        ];

        zindices.sort_by(|a, b| a.partial_cmp(b).unwrap());

        assert!((zindices[0].0 - (-5.0)).abs() < f32::EPSILON);
        assert!((zindices[1].0 - (-0.5)).abs() < f32::EPSILON);
        assert!((zindices[2].0 - 0.0).abs() < f32::EPSILON);
        assert!((zindices[3].0 - 2.5).abs() < f32::EPSILON);
        assert!((zindices[4].0 - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zindex_infinity() {
        let neg_inf = ZIndex(f32::NEG_INFINITY);
        let pos_inf = ZIndex(f32::INFINITY);
        let zero = ZIndex(0.0);

        assert!(neg_inf < zero);
        assert!(zero < pos_inf);
        assert!(neg_inf < pos_inf);
    }

    #[test]
    fn test_zindex_nan_comparison_is_none() {
        let nan = ZIndex(f32::NAN);
        let normal = ZIndex(1.0);

        // NaN comparisons with partial_cmp should return None
        assert!(nan.partial_cmp(&normal).is_none());
        assert!(normal.partial_cmp(&nan).is_none());
        assert!(nan.partial_cmp(&nan).is_none());
    }

    #[test]
    fn test_zindex_nan_not_equal_to_itself() {
        let nan = ZIndex(f32::NAN);

        // NaN should not equal itself (IEEE 754 behavior)
        assert!(nan != nan);
    }

    #[test]
    fn test_zindex_very_close_values() {
        // Test that very close but different values are still distinguishable
        let z1 = ZIndex(1.0);
        let z2 = ZIndex(1.0 + f32::EPSILON);

        assert!(z1 < z2);
        assert!(z1 != z2);
    }

    #[test]
    fn test_zindex_clone_and_copy() {
        let z1 = ZIndex(42.5);
        let z2 = z1; // Copy
        let z3 = z1.clone(); // Clone

        assert_eq!(z1, z2);
        assert_eq!(z1, z3);
    }

    #[test]
    fn test_zindex_large_values() {
        let large_pos = ZIndex(1e30);
        let large_neg = ZIndex(-1e30);
        let zero = ZIndex(0.0);

        assert!(large_neg < zero);
        assert!(zero < large_pos);
        assert!(large_neg < large_pos);
    }
}
