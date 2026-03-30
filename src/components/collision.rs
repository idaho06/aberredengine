//! Collision rule component and callback type.
//!
//! This module provides the [`CollisionRule`] component which defines how
//! collisions between entity groups should be handled, and the
//! [`CollisionCallback`] type alias for the Rust callback signature.
//!
//! # Group-Based Collision
//!
//! Collision rules match entities by their [`Group`](super::group::Group)
//! component. When two entities collide, the observer looks up rules that match
//! both groups and invokes the corresponding callback.
//!
//! # Example
//!
//! ```ignore
//! fn ball_brick_callback(
//!     ball: Entity,
//!     brick: Entity,
//!     sides_a: &BoxSides,
//!     sides_b: &BoxSides,
//!     ctx: &mut CollisionCtx,
//! ) {
//!     // Reflect ball, damage brick, play sound, etc.
//! }
//!
//! commands.spawn((
//!     CollisionRule::new("ball", "brick", ball_brick_callback),
//!     Group::new("collision_rules"),
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – collision detection system
//! - [`crate::systems::rust_collision`] – Rust collision observer
//! - [`crate::systems::lua_collision`] – Lua collision observer
//! - [`crate::events::collision::CollisionEvent`] – event emitted on collisions
//! - [`super::group::Group`] – group tag used for rule matching

use bevy_ecs::prelude::*;
use raylib::prelude::Rectangle;
use smallvec::SmallVec;

use crate::systems::GameCtx;

/// Callback type for Rust collision rules.
///
/// Receives the two matched entities (ordered to match `group_a` and `group_b`),
/// the colliding sides for each entity, and a mutable reference to
/// [`GameCtx`](crate::systems::GameCtx) providing full ECS query/resource access.
pub type CollisionCallback =
    for<'w, 's> fn(Entity, Entity, &BoxSides, &BoxSides, &mut GameCtx<'w, 's>);

/// Defines how collisions between two entity groups should be handled.
///
/// The default `CollisionRule` stores a Rust function pointer via
/// [`CollisionCallback`] and is processed by
/// [`rust_collision_observer`](crate::systems::rust_collision::rust_collision_observer).
/// The Lua-facing [`LuaCollisionRule`](crate::components::luacollision::LuaCollisionRule)
/// alias reuses this same storage with a [`LuaCollisionCallback`](crate::components::luacollision::LuaCollisionCallback)
/// payload.
///
/// When a collision is detected between entities with groups matching
/// `group_a` and `group_b`, the `callback` is invoked with the entities and
/// collision context.
#[derive(Component, Clone, Debug)]
pub struct CollisionRule<C = CollisionCallback> {
    /// First group name to match.
    pub group_a: String,
    /// Second group name to match.
    pub group_b: String,
    /// Callback payload — a Rust fn pointer for `CollisionRule`, or a
    /// [`LuaCollisionCallback`](crate::components::luacollision::LuaCollisionCallback)
    /// for `LuaCollisionRule`.
    pub callback: C,
}

impl<C> CollisionRule<C> {
    /// Create a new collision rule for two groups with a callback payload.
    pub fn new(group_a: impl Into<String>, group_b: impl Into<String>, callback: C) -> Self {
        Self {
            group_a: group_a.into(),
            group_b: group_b.into(),
            callback,
        }
    }

    /// Check if this rule matches the given groups and return entities in order.
    ///
    /// Returns `Some((entity_a, entity_b))` if the rule matches, with entities
    /// ordered to match `group_a` and `group_b` respectively.
    pub fn match_and_order(
        &self,
        ent_a: Entity,
        ent_b: Entity,
        group_a: &str,
        group_b: &str,
    ) -> Option<(Entity, Entity)> {
        match_groups(&self.group_a, &self.group_b, ent_a, ent_b, group_a, group_b)
    }
}

/// Check if a collision rule's groups match the given group names and return
/// entities ordered to match `rule_a` and `rule_b`.
///
/// This is the core matching logic used by [`CollisionRule::match_and_order`].
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
pub fn get_colliding_sides(rect_a: &Rectangle, rect_b: &Rectangle) -> Option<(BoxSides, BoxSides)> {
    let overlap_rect = rect_a.get_collision_rec(rect_b)?;
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
        let rect_a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
            x: 20.0,
            y: 20.0,
            width: 10.0,
            height: 10.0,
        };
        assert!(get_colliding_sides(&rect_a, &rect_b).is_none());
    }

    #[test]
    fn test_collision_from_right() {
        let rect_a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 10.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 0.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 5.0,
            y: 5.0,
            width: 5.0,
            height: 5.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
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
        let rect_a = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let rect_b = Rectangle {
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

    // CollisionRule::match_and_order tests

    fn dummy_collision_callback(
        _a: Entity,
        _b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        _ctx: &mut GameCtx,
    ) {
    }

    #[test]
    fn test_match_and_order_direct() {
        let rule = CollisionRule::new("ball", "brick", dummy_collision_callback);
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        let result = rule.match_and_order(ent_a, ent_b, "ball", "brick");
        assert_eq!(result, Some((ent_a, ent_b)));
    }

    #[test]
    fn test_match_and_order_reversed() {
        let rule = CollisionRule::new("ball", "brick", dummy_collision_callback);
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        // Groups come in swapped relative to the rule
        let result = rule.match_and_order(ent_a, ent_b, "brick", "ball");
        // Entities should be reordered so ball maps to group_a
        assert_eq!(result, Some((ent_b, ent_a)));
    }

    #[test]
    fn test_match_and_order_no_match() {
        let rule = CollisionRule::new("ball", "brick", dummy_collision_callback);
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        let result = rule.match_and_order(ent_a, ent_b, "player", "enemy");
        assert_eq!(result, None);
    }

    // match_groups free function tests

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
