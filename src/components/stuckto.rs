//! Component for attaching an entity's position to another entity.
//!
//! When an entity has the [`StuckTo`] component, a system will update its
//! position to follow the target entity's position, optionally with an offset.
//!
//! This is useful for:
//! - Attaching projectiles to moving platforms
//! - Making objects follow other entities
//! - Temporary "sticky" effects in games

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
