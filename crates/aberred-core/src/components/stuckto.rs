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
//! ```
//! # use bevy_ecs::prelude::*;
//! # use aberred_core::components::stuckto::StuckTo;
//! # use aberred_core::components::timer::Timer;
//! # use aberred_core::math::Vec2;
//! # use aberred_core::resources::input::InputState;
//! # use aberred_core::systems::GameCtx;
//! // Timers repeat until removed, so the callback removes both components.
//! fn release_ball(ball: Entity, ctx: &mut GameCtx, _input: &InputState) {
//!     ctx.commands.entity(ball).remove::<(StuckTo, Timer)>();
//! }
//!
//! # let mut world = World::new();
//! # let player_entity = world.spawn_empty().id();
//! # let ball = world.spawn_empty().id();
//! // Attach ball to player, release after 2 seconds
//! world.entity_mut(ball).insert((
//!     StuckTo::follow_x_only(player_entity)
//!         .with_offset(Vec2 { x: 0.0, y: -12.0 })
//!         .with_stored_velocity(Vec2 { x: 300.0, y: -300.0 }),
//!     Timer::rust(2.0, release_ball),
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::systems::stuckto::stuck_to_entity_system`] – the system that updates positions
//! - [`super::timer::Timer`] – can be used to auto-remove `StuckTo` after a delay

use bevy_ecs::prelude::{Component, Entity};
use crate::math::Vec2;

/// Component that makes an entity follow another entity's position.
///
/// When attached to an entity, the `stuck_to_entity_system` will update
/// this entity's `MapPosition` to match the target's position plus the offset.
#[derive(Debug, Clone, Component)]
pub struct StuckTo {
    /// The entity to follow.
    pub target: Entity,
    /// Offset from the target's position.
    pub offset: Vec2,
    /// If true, only follow the X axis.
    pub follow_x: bool,
    /// If true, only follow the Y axis.
    pub follow_y: bool,
    /// Stored velocity to restore when unstuck (optional).
    pub stored_velocity: Option<Vec2>,
}

impl StuckTo {
    /// Create a new StuckTo component that follows both axes.
    pub fn new(target: Entity) -> Self {
        Self {
            target,
            offset: Vec2::ZERO,
            follow_x: true,
            follow_y: true,
            stored_velocity: None,
        }
    }

    /// Create a StuckTo that only follows the X axis.
    pub fn follow_x_only(target: Entity) -> Self {
        Self {
            target,
            offset: Vec2::ZERO,
            follow_x: true,
            follow_y: false,
            stored_velocity: None,
        }
    }

    /// Create a StuckTo that only follows the Y axis.
    pub fn follow_y_only(target: Entity) -> Self {
        Self {
            target,
            offset: Vec2::ZERO,
            follow_x: false,
            follow_y: true,
            stored_velocity: None,
        }
    }

    /// Set the offset from the target's position.
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// Store a velocity to restore when the component is removed.
    pub fn with_stored_velocity(mut self, velocity: Vec2) -> Self {
        self.stored_velocity = Some(velocity);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_select_followed_axes() {
        let target = Entity::from_bits(42);
        for (st, x, y) in [
            (StuckTo::new(target), true, true),
            (StuckTo::follow_x_only(target), true, false),
            (StuckTo::follow_y_only(target), false, true),
        ] {
            assert_eq!((st.follow_x, st.follow_y), (x, y));
            assert_eq!(st.offset, Vec2::ZERO);
            assert!(st.stored_velocity.is_none());
        }
    }
}
