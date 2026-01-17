//! Axis-aligned box collider component.
//!
//! Provides a simple AABB (Axis-Aligned Bounding Box) collider that can be
//! attached to entities for collision detection. The collider is defined by a
//! size, an offset from the entity's pivot, and an origin point.
//!
//! Use in combination with [`MapPosition`](super::mapposition::MapPosition)
//! to compute world-space AABBs for overlap testing.
//!
//! # Coordinate System
//!
//! - `size` – width and height of the collider box
//! - `offset` – displacement from the entity's pivot (positive moves down-right)
//! - `origin` – pivot point relative to the box's top-left (usually matches [`Sprite`](super::sprite::Sprite) origin)
//!
//! The AABB is computed as: `(position - origin + offset)` to `(position - origin + offset + size)`
//!
//! # Related
//!
//! - [`crate::systems::collision`] – collision detection systems
//! - [`crate::components::collision::CollisionRule`] – defines collision handlers
//! - [`crate::events::collision::CollisionEvent`] – emitted on collisions

use bevy_ecs::prelude::Component;
use raylib::prelude::{Rectangle, Vector2};

/// Axis-aligned rectangular collider in local space.
///
/// The collider is defined by a `size` (width, height), an `offset` from the
/// entity's pivot, and an `origin` representing that pivot relative to the
/// collider's local top-left. World AABBs can be computed using
/// [`MapPosition`](super::mapposition::MapPosition) as the pivot position.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct BoxCollider {
    /// Size of the box in world units.
    pub size: Vector2,
    /// Offset from the entity's pivot (positive moves the box down-right).
    pub offset: Vector2,
    /// Pivot point relative to the collider's local top-left (usually the same as Sprite.origin).
    /// MapPosition represents this pivot; AABB is computed from (position - origin + offset).
    pub origin: Vector2,
    // pub is_trigger: bool, // maybe we will use this
}

impl BoxCollider {
    /// Create a BoxCollider with given size
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vector2::new(width, height),
            offset: Vector2::zero(),
            origin: Vector2::zero(),
        }
    }

    /// Modify BoxCollider with given size and offset
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_offset(mut self, offset: Vector2) -> Self {
        self.offset = offset;
        self
    }

    /// Modify BoxCollider with an explicit origin.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_origin(mut self, origin: Vector2) -> Self {
        self.origin = origin;
        self
    }

    /// Returns (min, max) of the collider AABB for a given entity position.
    /// Handles negative size by normalizing to proper min/max.
    pub fn aabb(&self, position: Vector2) -> (Vector2, Vector2) {
        // World-space min corner from MapPosition (pivot) minus origin, plus collider offset
        let p0 = position - self.origin + self.offset;
        let p1 = p0 + self.size;
        let min = Vector2::new(p0.x.min(p1.x), p0.y.min(p1.y));
        let max = Vector2::new(p0.x.max(p1.x), p0.y.max(p1.y));
        (min, max)
    }

    pub fn get_aabb(&self, position: Vector2) -> (f32, f32, f32, f32) {
        let (min, max) = self.aabb(position);
        (min.x, min.y, max.x - min.x, max.y - min.y)
    }

    /// AABB vs AABB overlap test against another BoxCollider at a different entity position.
    #[allow(dead_code)]
    pub fn overlaps(&self, position: Vector2, other: &Self, other_position: Vector2) -> bool {
        let (min_a, max_a) = self.aabb(position);
        let (min_b, max_b) = other.aabb(other_position);
        min_a.x < max_b.x && max_a.x > min_b.x && min_a.y < max_b.y && max_a.y > min_b.y
    }

    /// Point containment in world space.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn contains_point(&self, position: Vector2, point: Vector2) -> bool {
        let (min, max) = self.aabb(position);
        point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
    }

    /// Get the collider as a Raylib Rectangle given the entity position.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn as_rectangle(&self, position: Vector2) -> Rectangle {
        let (x, y, w, h) = self.get_aabb(position);
        Rectangle::new(x, y, w, h)
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

    // ==================== CONSTRUCTOR TESTS ====================

    #[test]
    fn test_new() {
        let col = BoxCollider::new(10.0, 20.0);
        assert!(vec_approx_eq(col.size, Vector2::new(10.0, 20.0)));
        assert!(vec_approx_eq(col.offset, Vector2::zero()));
        assert!(vec_approx_eq(col.origin, Vector2::zero()));
    }

    #[test]
    fn test_new_with_zero_size() {
        let col = BoxCollider::new(0.0, 0.0);
        assert!(vec_approx_eq(col.size, Vector2::zero()));
    }

    #[test]
    fn test_with_offset() {
        let col = BoxCollider::new(10.0, 10.0).with_offset(Vector2::new(5.0, 5.0));
        assert!(vec_approx_eq(col.offset, Vector2::new(5.0, 5.0)));
        assert!(vec_approx_eq(col.size, Vector2::new(10.0, 10.0))); // size unchanged
    }

    #[test]
    fn test_with_origin() {
        let col = BoxCollider::new(10.0, 10.0).with_origin(Vector2::new(5.0, 5.0));
        assert!(vec_approx_eq(col.origin, Vector2::new(5.0, 5.0)));
    }

    #[test]
    fn test_builder_chaining() {
        let col = BoxCollider::new(20.0, 30.0)
            .with_offset(Vector2::new(2.0, 3.0))
            .with_origin(Vector2::new(10.0, 15.0));

        assert!(vec_approx_eq(col.size, Vector2::new(20.0, 30.0)));
        assert!(vec_approx_eq(col.offset, Vector2::new(2.0, 3.0)));
        assert!(vec_approx_eq(col.origin, Vector2::new(10.0, 15.0)));
    }

    // ==================== AABB TESTS ====================

    #[test]
    fn test_aabb_simple() {
        let col = BoxCollider::new(10.0, 10.0);
        let pos = Vector2::new(0.0, 0.0);
        let (min, max) = col.aabb(pos);
        assert!(vec_approx_eq(min, Vector2::new(0.0, 0.0)));
        assert!(vec_approx_eq(max, Vector2::new(10.0, 10.0)));
    }

    #[test]
    fn test_aabb_with_position() {
        let col = BoxCollider::new(10.0, 10.0);
        let pos = Vector2::new(100.0, 50.0);
        let (min, max) = col.aabb(pos);
        assert!(vec_approx_eq(min, Vector2::new(100.0, 50.0)));
        assert!(vec_approx_eq(max, Vector2::new(110.0, 60.0)));
    }

    #[test]
    fn test_aabb_with_offset() {
        let col = BoxCollider::new(10.0, 10.0).with_offset(Vector2::new(5.0, 5.0));
        let pos = Vector2::new(0.0, 0.0);
        let (min, max) = col.aabb(pos);
        assert!(vec_approx_eq(min, Vector2::new(5.0, 5.0)));
        assert!(vec_approx_eq(max, Vector2::new(15.0, 15.0)));
    }

    #[test]
    fn test_aabb_with_origin() {
        // Origin shifts the box in the opposite direction
        let col = BoxCollider::new(10.0, 10.0).with_origin(Vector2::new(5.0, 5.0));
        let pos = Vector2::new(0.0, 0.0);
        let (min, max) = col.aabb(pos);
        // position - origin = (0,0) - (5,5) = (-5,-5)
        assert!(vec_approx_eq(min, Vector2::new(-5.0, -5.0)));
        assert!(vec_approx_eq(max, Vector2::new(5.0, 5.0)));
    }

    #[test]
    fn test_aabb_with_origin_and_offset() {
        let col = BoxCollider::new(10.0, 10.0)
            .with_origin(Vector2::new(5.0, 5.0))
            .with_offset(Vector2::new(3.0, 3.0));
        let pos = Vector2::new(0.0, 0.0);
        let (min, max) = col.aabb(pos);
        // position - origin + offset = (0,0) - (5,5) + (3,3) = (-2,-2)
        assert!(vec_approx_eq(min, Vector2::new(-2.0, -2.0)));
        assert!(vec_approx_eq(max, Vector2::new(8.0, 8.0)));
    }

    #[test]
    fn test_aabb_negative_size_normalizes() {
        // Negative size should be handled correctly
        let mut col = BoxCollider::new(10.0, 10.0);
        col.size = Vector2::new(-10.0, -10.0);
        let pos = Vector2::new(10.0, 10.0);
        let (min, max) = col.aabb(pos);
        // p0 = (10, 10), p1 = (0, 0), normalized: min=(0,0), max=(10,10)
        assert!(vec_approx_eq(min, Vector2::new(0.0, 0.0)));
        assert!(vec_approx_eq(max, Vector2::new(10.0, 10.0)));
    }

    // ==================== GET_AABB TESTS ====================

    #[test]
    fn test_get_aabb() {
        let col = BoxCollider::new(20.0, 30.0);
        let pos = Vector2::new(10.0, 10.0);
        let (x, y, w, h) = col.get_aabb(pos);
        assert!(approx_eq(x, 10.0));
        assert!(approx_eq(y, 10.0));
        assert!(approx_eq(w, 20.0));
        assert!(approx_eq(h, 30.0));
    }

    // ==================== OVERLAPS TESTS ====================

    #[test]
    fn test_overlaps_true() {
        let col_a = BoxCollider::new(10.0, 10.0);
        let col_b = BoxCollider::new(10.0, 10.0);
        let pos_a = Vector2::new(0.0, 0.0);
        let pos_b = Vector2::new(5.0, 5.0); // overlapping
        assert!(col_a.overlaps(pos_a, &col_b, pos_b));
    }

    #[test]
    fn test_overlaps_false() {
        let col_a = BoxCollider::new(10.0, 10.0);
        let col_b = BoxCollider::new(10.0, 10.0);
        let pos_a = Vector2::new(0.0, 0.0);
        let pos_b = Vector2::new(20.0, 0.0); // no overlap
        assert!(!col_a.overlaps(pos_a, &col_b, pos_b));
    }

    #[test]
    fn test_overlaps_edge_touching() {
        // Edge-to-edge touching is NOT an overlap (strict inequality)
        let col_a = BoxCollider::new(10.0, 10.0);
        let col_b = BoxCollider::new(10.0, 10.0);
        let pos_a = Vector2::new(0.0, 0.0);
        let pos_b = Vector2::new(10.0, 0.0); // exactly touching
        assert!(!col_a.overlaps(pos_a, &col_b, pos_b));
    }

    #[test]
    fn test_overlaps_contained() {
        let col_a = BoxCollider::new(20.0, 20.0);
        let col_b = BoxCollider::new(5.0, 5.0);
        let pos_a = Vector2::new(0.0, 0.0);
        let pos_b = Vector2::new(5.0, 5.0); // b inside a
        assert!(col_a.overlaps(pos_a, &col_b, pos_b));
    }

    #[test]
    fn test_overlaps_symmetric() {
        let col_a = BoxCollider::new(10.0, 10.0);
        let col_b = BoxCollider::new(10.0, 10.0);
        let pos_a = Vector2::new(0.0, 0.0);
        let pos_b = Vector2::new(5.0, 5.0);
        // a overlaps b == b overlaps a
        assert_eq!(
            col_a.overlaps(pos_a, &col_b, pos_b),
            col_b.overlaps(pos_b, &col_a, pos_a)
        );
    }

    // ==================== CONTAINS_POINT TESTS ====================

    #[test]
    fn test_contains_point_inside() {
        let col = BoxCollider::new(10.0, 10.0);
        let pos = Vector2::new(0.0, 0.0);
        let point = Vector2::new(5.0, 5.0);
        assert!(col.contains_point(pos, point));
    }

    #[test]
    fn test_contains_point_outside() {
        let col = BoxCollider::new(10.0, 10.0);
        let pos = Vector2::new(0.0, 0.0);
        let point = Vector2::new(15.0, 5.0);
        assert!(!col.contains_point(pos, point));
    }

    #[test]
    fn test_contains_point_on_edge() {
        let col = BoxCollider::new(10.0, 10.0);
        let pos = Vector2::new(0.0, 0.0);
        // Points on edge are included (>=, <=)
        assert!(col.contains_point(pos, Vector2::new(0.0, 5.0)));
        assert!(col.contains_point(pos, Vector2::new(10.0, 5.0)));
        assert!(col.contains_point(pos, Vector2::new(5.0, 0.0)));
        assert!(col.contains_point(pos, Vector2::new(5.0, 10.0)));
    }

    #[test]
    fn test_contains_point_corner() {
        let col = BoxCollider::new(10.0, 10.0);
        let pos = Vector2::new(0.0, 0.0);
        assert!(col.contains_point(pos, Vector2::new(0.0, 0.0)));
        assert!(col.contains_point(pos, Vector2::new(10.0, 10.0)));
    }

    // ==================== AS_RECTANGLE TESTS ====================

    #[test]
    fn test_as_rectangle() {
        let col = BoxCollider::new(15.0, 25.0);
        let pos = Vector2::new(10.0, 20.0);
        let rect = col.as_rectangle(pos);
        assert!(approx_eq(rect.x, 10.0));
        assert!(approx_eq(rect.y, 20.0));
        assert!(approx_eq(rect.width, 15.0));
        assert!(approx_eq(rect.height, 25.0));
    }

    #[test]
    fn test_as_rectangle_with_offset() {
        let col = BoxCollider::new(10.0, 10.0).with_offset(Vector2::new(5.0, 5.0));
        let pos = Vector2::new(0.0, 0.0);
        let rect = col.as_rectangle(pos);
        assert!(approx_eq(rect.x, 5.0));
        assert!(approx_eq(rect.y, 5.0));
    }
}
