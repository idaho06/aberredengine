//! Component for attaching an entity's position to another entity.
//!
//! When an entity has the [`StuckTo`] component, the
//! [`stuck_to_entity_system`](crate::systems::stuckto::stuck_to_entity_system)
//! will update its position to follow the target entity's position, optionally
//! with an offset.
//!
//! This is useful for:
//! - Attaching projectiles to moving platforms
//! - Making objects follow other entities (e.g., ball stuck to paddle)
//! - Temporary "sticky" effects in games
//!
//! # Integration with Timer
//!
//! Combine with [`Timer`](super::timer::Timer) to automatically release the
//! stuck entity after a duration. The `stored_velocity` field can preserve the
//! entity's velocity to restore when unstuck.
//!
//! # Example
//!
//! ```ignore
//! // Attach ball to player, release after 2 seconds
//! commands.entity(ball).insert((
//!     StuckTo::follow_x_only(player_entity)
//!         .with_offset(Vector2 { x: 0.0, y: -12.0 })
//!         .with_stored_velocity(Vector2 { x: 300.0, y: -300.0 }),
//!     Timer::new(2.0, "remove_stuck_to"),
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::systems::stuckto::stuck_to_entity_system`] – the system that updates positions
//! - [`super::timer::Timer`] – can be used to auto-remove `StuckTo` after a delay

use bevy_ecs::prelude::{Component, Entity};
use raylib::prelude::Vector2;

/// Component that makes an entity follow another entity's position.
///
/// When attached to an entity, the `stuck_to_entity_system` will update
/// this entity's `MapPosition` to match the target's position plus the offset.
#[derive(Debug, Clone, Component)]
pub struct StuckTo {
    /// The entity to follow.
    pub target: Entity,
    /// Offset from the target's position.
    pub offset: Vector2,
    /// If true, only follow the X axis.
    pub follow_x: bool,
    /// If true, only follow the Y axis.
    pub follow_y: bool,
    /// Stored velocity to restore when unstuck (optional).
    pub stored_velocity: Option<Vector2>,
}

#[allow(dead_code)]
impl StuckTo {
    /// Create a new StuckTo component that follows both axes.
    pub fn new(target: Entity) -> Self {
        Self {
            target,
            offset: Vector2::zero(),
            follow_x: true,
            follow_y: true,
            stored_velocity: None,
        }
    }

    /// Create a StuckTo that only follows the X axis.
    pub fn follow_x_only(target: Entity) -> Self {
        Self {
            target,
            offset: Vector2::zero(),
            follow_x: true,
            follow_y: false,
            stored_velocity: None,
        }
    }

    /// Create a StuckTo that only follows the Y axis.
    pub fn follow_y_only(target: Entity) -> Self {
        Self {
            target,
            offset: Vector2::zero(),
            follow_x: false,
            follow_y: true,
            stored_velocity: None,
        }
    }

    /// Set the offset from the target's position.
    pub fn with_offset(mut self, offset: Vector2) -> Self {
        self.offset = offset;
        self
    }

    /// Store a velocity to restore when the component is removed.
    pub fn with_stored_velocity(mut self, velocity: Vector2) -> Self {
        self.stored_velocity = Some(velocity);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_entity() -> Entity {
        Entity::from_bits(42)
    }

    #[test]
    fn test_new_follows_both_axes() {
        let st = StuckTo::new(dummy_entity());
        assert!(st.follow_x);
        assert!(st.follow_y);
        assert_eq!(st.offset.x, 0.0);
        assert_eq!(st.offset.y, 0.0);
        assert!(st.stored_velocity.is_none());
    }

    #[test]
    fn test_follow_x_only() {
        let st = StuckTo::follow_x_only(dummy_entity());
        assert!(st.follow_x);
        assert!(!st.follow_y);
    }

    #[test]
    fn test_follow_y_only() {
        let st = StuckTo::follow_y_only(dummy_entity());
        assert!(!st.follow_x);
        assert!(st.follow_y);
    }

    #[test]
    fn test_with_offset() {
        let st = StuckTo::new(dummy_entity()).with_offset(Vector2 { x: 5.0, y: -10.0 });
        assert_eq!(st.offset.x, 5.0);
        assert_eq!(st.offset.y, -10.0);
    }

    #[test]
    fn test_with_stored_velocity() {
        let st = StuckTo::new(dummy_entity())
            .with_stored_velocity(Vector2 { x: 100.0, y: -200.0 });
        let vel = st.stored_velocity.unwrap();
        assert_eq!(vel.x, 100.0);
        assert_eq!(vel.y, -200.0);
    }

    #[test]
    fn test_builder_chaining() {
        let st = StuckTo::follow_x_only(dummy_entity())
            .with_offset(Vector2 { x: 1.0, y: 2.0 })
            .with_stored_velocity(Vector2 { x: 3.0, y: 4.0 });
        assert!(st.follow_x);
        assert!(!st.follow_y);
        assert_eq!(st.offset.x, 1.0);
        assert_eq!(st.offset.y, 2.0);
        assert!(st.stored_velocity.is_some());
    }

    #[test]
    fn test_target_entity_stored() {
        let entity = Entity::from_bits(99);
        let st = StuckTo::new(entity);
        assert_eq!(st.target, entity);
    }
}
