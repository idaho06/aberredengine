//! Collision rule component and callback context.
//!
//! This module provides the [`CollisionRule`] component which defines how
//! collisions between entity groups should be handled, and [`CollisionContext`]
//! which provides access to world state within collision callbacks.
//!
//! # Group-Based Collision
//!
//! Collision rules match entities by their [`Group`](super::group::Group)
//! component. When two entities collide, the system looks up rules that match
//! both groups and invokes the corresponding callback.
//!
//! # Example
//!
//! ```ignore
//! fn ball_brick_callback(ball: Entity, brick: Entity, ctx: &mut CollisionContext) {
//!     // Reflect ball, damage brick, play sound, etc.
//! }
//!
//! commands.spawn((
//!     CollisionRule::new("ball", "brick", ball_brick_callback as CollisionCallback),
//!     Group::new("collision_rules"),
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::systems::collision`] – collision detection and observer systems
//! - [`crate::events::collision::CollisionEvent`] – event emitted on collisions
//! - [`super::group::Group`] – group tag used for rule matching

// use bevy_ecs::prelude::*;
use raylib::prelude::Rectangle;
use smallvec::SmallVec;

/* use crate::components::boxcollider::BoxCollider;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::events::audio::AudioCmd;
use crate::resources::worldsignals::WorldSignals;
 */
// Context passed into collision callbacks to access world state.
//
// Extend this struct as new queries/resources are needed by callbacks.
/* pub struct CollisionContext<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub groups: &'a Query<'w, 's, &'static Group>,
    pub positions: &'a mut Query<'w, 's, &'static mut MapPosition>,
    pub rigid_bodies: &'a mut Query<'w, 's, &'static mut RigidBody>,
    pub box_colliders: &'a Query<'w, 's, &'static BoxCollider>,
    pub signals: &'a mut Query<'w, 's, &'static mut Signals>,
    pub world_signals: &'a mut ResMut<'w, WorldSignals>,
    pub audio_cmds: &'a mut MessageWriter<'w, AudioCmd>,
}
 */
// Callback signature for collision components using a grouped context.
// The callback receives the two entities involved in the collision
// and a mutable reference to the collision context.
// Callbacks are created in the game code when defining collision rules.
/* pub type CollisionCallback =
   for<'a, 'w, 's> fn(a: Entity, b: Entity, ctx: &mut CollisionContext<'a, 'w, 's>);
*/
// Defines how collisions between two entity groups should be handled.
//
// When a collision is detected between entities with groups matching
// `group_a` and `group_b`, the `callback` function is invoked with
// the entities and a [`CollisionContext`].
/* #[derive(Component)]
pub struct CollisionRule {
    pub group_a: String,
    pub group_b: String,
    pub callback: CollisionCallback,
}
 */// TODO: Instead of using a fixed function signature for the callback,
// use a closure that can capture additional context if needed.

/* impl CollisionRule {
    /// Create a new collision rule for two groups with a callback.
    pub fn new(
        group_a: impl Into<String>,
        group_b: impl Into<String>,
        callback: CollisionCallback,
    ) -> Self {
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
        if self.group_a == group_a && self.group_b == group_b {
            Some((ent_a, ent_b))
        } else if self.group_a == group_b && self.group_b == group_a {
            Some((ent_b, ent_a))
        } else {
            None
        }
    }
}
 */
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
}
