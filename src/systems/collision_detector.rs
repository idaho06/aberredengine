//! Collision detection system.
//!
//! This module provides the [`collision_detector`] system which performs pairwise
//! AABB overlap checks and emits [`CollisionEvent`](crate::events::collision::CollisionEvent)
//! for each detected collision.
//!
//! This system is pure Rust with no Lua dependency and is shared by both
//! the Lua and Rust game paths.
//!
//! # Related
//!
//! - [`crate::systems::lua_collision`] – Lua-based collision observer
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision::CollisionEvent`] – emitted on each collision

use bevy_ecs::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::mapposition::MapPosition;
use crate::events::collision::CollisionEvent;

/// Broad-phase pairwise overlap test with event emission.
///
/// Uses ECS `iter_combinations_mut()` to efficiently iterate unique pairs,
/// checks overlap, and triggers an event for each collision. Observers can
/// react to despawn, apply damage, or play sounds.
pub fn collision_detector(
    mut query: Query<(
        Entity,
        &MapPosition,
        &BoxCollider,
        Option<&GlobalTransform2D>,
    )>,
    mut commands: Commands,
) {
    let mut combos = query.iter_combinations_mut();
    while let Some(
        [
            (entity_a, position_a, collider_a, maybe_gt_a),
            (entity_b, position_b, collider_b, maybe_gt_b),
        ],
    ) = combos.fetch_next()
    {
        // Use world position from GlobalTransform2D when available, fall back to local
        let world_pos_a = maybe_gt_a.map_or(position_a.pos, |gt| gt.position);
        let world_pos_b = maybe_gt_b.map_or(position_b.pos, |gt| gt.position);
        let rect_a = collider_a.as_rectangle(world_pos_a);
        let rect_b = collider_b.as_rectangle(world_pos_b);
        if rect_a.check_collision_recs(&rect_b) {
            commands.trigger(CollisionEvent {
                a: entity_a,
                b: entity_b,
            });
        }
    }
}
