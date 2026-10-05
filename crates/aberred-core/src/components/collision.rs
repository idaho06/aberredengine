//! Collision rule component and contact sides.
//!
//! [`CollisionRule`] names two entity groups. When an entity of one group
//! overlaps an entity of the other, the engine triggers
//! [`Collided`](crate::events::collision::Collided) on the rule entity, so
//! gameplay reacts with an observer on that rule (or a global observer).
//!
//! # Group-Based Collision
//!
//! Collision rules match entities by their [`Group`](super::group::Group)
//! component, in either order. When several rules cover the same pair, the
//! rule with the lowest `Entity` wins.
//!
//! # Example
//!
//! ```
//! # use bevy_ecs::prelude::*;
//! use aberred_core::components::collision::CollisionRule;
//! use aberred_core::events::collision::Collided;
//!
//! fn ball_brick(hit: On<Collided>, mut commands: Commands) {
//!     // `hit.a` is the ball, `hit.b` the brick (rule order).
//!     commands.entity(hit.b).despawn();
//! }
//!
//! # let mut world = World::new();
//! world.spawn(CollisionRule::new("ball", "brick")).observe(ball_brick);
//! ```
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – collision detection system
//! - [`crate::systems::collision_rule`] – triggers `Collided` for matched rules
//! - `aberred_lua::systems::lua_collision` – Lua collision observer
//! - [`crate::events::collision::CollisionEvent`] – raw overlap event
//! - [`super::group::Group`] – group tag used for rule matching

use bevy_ecs::prelude::*;
use smallvec::SmallVec;

use crate::math::Rect;

/// A rule component matched by a pair of group names.
///
/// Implemented by [`CollisionRule`] and by `aberred-lua`'s
/// `LuaCollisionRule`, so both share
/// [`RuleIndex`](crate::resources::collision_rule_index::RuleIndex) and its
/// matching.
pub trait RuleGroups: Component {
    /// The rule's `(group_a, group_b)`.
    fn groups(&self) -> (&str, &str);
}

/// Matches collisions between two entity groups.
///
/// When entities with groups matching `group_a` and `group_b` overlap,
/// [`collision_rule_observer`](crate::systems::collision_rule::collision_rule_observer)
/// triggers [`Collided`](crate::events::collision::Collided) on this rule's
/// entity, every tick while they overlap.
#[derive(Component, Clone, Debug)]
pub struct CollisionRule {
    /// First group name to match.
    pub group_a: String,
    /// Second group name to match.
    pub group_b: String,
}

impl CollisionRule {
    /// Create a rule matching collisions between `group_a` and `group_b`.
    pub fn new(group_a: impl Into<String>, group_b: impl Into<String>) -> Self {
        Self {
            group_a: group_a.into(),
            group_b: group_b.into(),
        }
    }
}

impl RuleGroups for CollisionRule {
    fn groups(&self) -> (&str, &str) {
        (&self.group_a, &self.group_b)
    }
}

/// Check if a collision rule's groups match the given group names and return
/// entities ordered to match `rule_a` and `rule_b`.
///
/// This is the core matching logic used by
/// [`RuleIndex::find_match`](crate::resources::collision_rule_index::RuleIndex::find_match).
pub fn match_groups(
    rule_a: &str,
    rule_b: &str,
    ent_a: Entity,
    ent_b: Entity,
    ga: &str,
    gb: &str,
) -> Option<(Entity, Entity)> {
    if rule_a == ga && rule_b == gb {
        Some((ent_a, ent_b))
    } else if rule_a == gb && rule_b == ga {
        Some((ent_b, ent_a))
    } else {
        None
    }
}

/// One side of an axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxSide {
    Left,
    Right,
    Top,
    Bottom,
}

/// Type alias for collision side vectors (0-4 elements, stack-allocated).
pub type BoxSides = SmallVec<[BoxSide; 4]>;

/// Returns two vectors representing the colliding sides of two Rectangles.
/// If no collision, returns None.
///
/// Uses `SmallVec<[BoxSide; 4]>` to avoid heap allocations since each
/// rectangle can have at most 4 colliding sides.
pub fn get_colliding_sides(rect_a: &Rect, rect_b: &Rect) -> Option<(BoxSides, BoxSides)> {
    let overlap_rect = rect_a.intersection(rect_b)?;
    let mut sides_a = SmallVec::new();
    let mut sides_b = SmallVec::new();

    if overlap_rect.x <= rect_a.x {
        sides_a.push(BoxSide::Left);
    }
    if overlap_rect.x + overlap_rect.width >= rect_a.x + rect_a.width {
        sides_a.push(BoxSide::Right);
    }
    if overlap_rect.y <= rect_a.y {
        sides_a.push(BoxSide::Top);
    }
    if overlap_rect.y + overlap_rect.height >= rect_a.y + rect_a.height {
        sides_a.push(BoxSide::Bottom);
    }

    if overlap_rect.x <= rect_b.x {
        sides_b.push(BoxSide::Left);
    }
    if overlap_rect.x + overlap_rect.width >= rect_b.x + rect_b.width {
        sides_b.push(BoxSide::Right);
    }
    if overlap_rect.y <= rect_b.y {
        sides_b.push(BoxSide::Top);
    }
    if overlap_rect.y + overlap_rect.height >= rect_b.y + rect_b.height {
        sides_b.push(BoxSide::Bottom);
    }

    Some((sides_a, sides_b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_collision_returns_none() {
        let rect_a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 20.0,
            y: 20.0,
            width: 10.0,
            height: 10.0,
        };
        assert!(get_colliding_sides(&rect_a, &rect_b).is_none());
    }

    #[test]
    fn test_collision_from_right() {
        let rect_a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 8.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        assert!(result.is_some());
        let (sides_a, sides_b) = result.unwrap();
        assert!(sides_a.iter().any(|s| matches!(s, BoxSide::Right)));
        assert!(sides_b.iter().any(|s| matches!(s, BoxSide::Left)));
    }

    #[test]
    fn test_collision_from_left() {
        let rect_a = Rect {
            x: 10.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 2.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        assert!(result.is_some());
        let (sides_a, sides_b) = result.unwrap();
        assert!(sides_a.iter().any(|s| matches!(s, BoxSide::Left)));
        assert!(sides_b.iter().any(|s| matches!(s, BoxSide::Right)));
    }

    #[test]
    fn test_collision_from_bottom() {
        let rect_a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 0.0,
            y: 8.0,
            width: 10.0,
            height: 10.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        assert!(result.is_some());
        let (sides_a, sides_b) = result.unwrap();
        assert!(sides_a.iter().any(|s| matches!(s, BoxSide::Bottom)));
        assert!(sides_b.iter().any(|s| matches!(s, BoxSide::Top)));
    }

    #[test]
    fn test_collision_from_top() {
        let rect_a = Rect {
            x: 0.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 0.0,
            y: 2.0,
            width: 10.0,
            height: 10.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        assert!(result.is_some());
        let (sides_a, sides_b) = result.unwrap();
        assert!(sides_a.iter().any(|s| matches!(s, BoxSide::Top)));
        assert!(sides_b.iter().any(|s| matches!(s, BoxSide::Bottom)));
    }

    #[test]
    fn test_rect_a_fully_inside_rect_b() {
        let rect_a = Rect {
            x: 5.0,
            y: 5.0,
            width: 5.0,
            height: 5.0,
        };
        let rect_b = Rect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        assert!(result.is_some());
        let (sides_a, sides_b) = result.unwrap();
        // All sides of rect_a should be colliding
        assert_eq!(sides_a.len(), 4);
        // No sides of rect_b should be colliding (overlap doesn't touch edges)
        assert!(sides_b.is_empty());
    }

    #[test]
    fn test_rect_b_fully_inside_rect_a() {
        let rect_a = Rect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        };
        let rect_b = Rect {
            x: 5.0,
            y: 5.0,
            width: 5.0,
            height: 5.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        assert!(result.is_some());
        let (sides_a, sides_b) = result.unwrap();
        // No sides of rect_a should be colliding
        assert!(sides_a.is_empty());
        // All sides of rect_b should be colliding
        assert_eq!(sides_b.len(), 4);
    }

    #[test]
    fn test_identical_rectangles() {
        let rect_a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        assert!(result.is_some());
        let (sides_a, sides_b) = result.unwrap();
        // All sides should be colliding for both
        assert_eq!(sides_a.len(), 4);
        assert_eq!(sides_b.len(), 4);
    }

    #[test]
    fn test_corner_collision() {
        let rect_a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 8.0,
            y: 8.0,
            width: 10.0,
            height: 10.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        assert!(result.is_some());
        let (sides_a, sides_b) = result.unwrap();
        // rect_a should have Right and Bottom
        assert!(sides_a.iter().any(|s| matches!(s, BoxSide::Right)));
        assert!(sides_a.iter().any(|s| matches!(s, BoxSide::Bottom)));
        // rect_b should have Left and Top
        assert!(sides_b.iter().any(|s| matches!(s, BoxSide::Left)));
        assert!(sides_b.iter().any(|s| matches!(s, BoxSide::Top)));
    }

    #[test]
    fn test_edge_touching_horizontal() {
        let rect_a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 10.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        // Rectangles just touching at edge - depending on get_collision_rec behavior
        let result = get_colliding_sides(&rect_a, &rect_b);
        // This may return None if touching edges don't count as collision
        // or Some with appropriate sides if they do
        if let Some((sides_a, sides_b)) = result {
            assert!(sides_a.iter().any(|s| matches!(s, BoxSide::Right)));
            assert!(sides_b.iter().any(|s| matches!(s, BoxSide::Left)));
        }
    }

    #[test]
    fn test_edge_touching_vertical() {
        let rect_a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rect {
            x: 0.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        };
        let result = get_colliding_sides(&rect_a, &rect_b);
        if let Some((sides_a, sides_b)) = result {
            assert!(sides_a.iter().any(|s| matches!(s, BoxSide::Bottom)));
            assert!(sides_b.iter().any(|s| matches!(s, BoxSide::Top)));
        }
    }

    #[test]
    fn test_match_groups_direct() {
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        assert_eq!(
            match_groups("ball", "brick", ent_a, ent_b, "ball", "brick"),
            Some((ent_a, ent_b))
        );
    }

    #[test]
    fn test_match_groups_reversed() {
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        assert_eq!(
            match_groups("ball", "brick", ent_a, ent_b, "brick", "ball"),
            Some((ent_b, ent_a))
        );
    }

    #[test]
    fn test_match_groups_no_match() {
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        assert_eq!(
            match_groups("ball", "brick", ent_a, ent_b, "player", "enemy"),
            None
        );
    }

    #[test]
    fn test_match_groups_partial_match() {
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        // Only one group matches
        assert_eq!(
            match_groups("ball", "brick", ent_a, ent_b, "ball", "enemy"),
            None
        );
    }

    #[test]
    fn test_match_groups_same_group() {
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        // Rule and entities have the same group on both sides
        assert_eq!(
            match_groups("ball", "ball", ent_a, ent_b, "ball", "ball"),
            Some((ent_a, ent_b))
        );
    }
}
