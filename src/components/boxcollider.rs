use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct BoxCollider {
    pub size: Vector2,
    pub offset: Vector2,
    // pub is_trigger: bool, // maybe we will use this
}

impl BoxCollider {
    /// Create a BoxCollider with given size
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vector2::new(width, height),
            offset: Vector2::zero(),
        }
    }

    /// Modify BoxCollider with given size and offset
    pub fn with_offset(mut self, offset: Vector2) -> Self {
        self.offset = offset;
        self
    }

    /// Returns (min, max) of the collider AABB for a given entity position.
    /// Handles negative size by normalizing to proper min/max.
    pub fn aabb(&self, position: Vector2) -> (Vector2, Vector2) {
        // let min = position + self.offset;
        // let max = min + self.size;
        let p0 = position + self.offset;
        let p1 = p0 + self.size;
        let min = Vector2::new(p0.x.min(p1.x), p0.y.min(p1.y));
        let max = Vector2::new(p0.x.max(p1.x), p0.y.max(p1.y));
        (min, max)
    }

    /// AABB vs AABB overlap test against another BoxCollider at a different entity position.
    pub fn overlaps(&self, position: Vector2, other: &Self, other_position: Vector2) -> bool {
        let (min_a, max_a) = self.aabb(position);
        let (min_b, max_b) = other.aabb(other_position);
        min_a.x < max_b.x && max_a.x > min_b.x && min_a.y < max_b.y && max_a.y > min_b.y
    }

    /// Point containment in world space.
    pub fn contains_point(&self, position: Vector2, point: Vector2) -> bool {
        let (min, max) = self.aabb(position);
        point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
    }
}
