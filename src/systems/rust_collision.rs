//! Rust collision observer and callback dispatch.
//!
//! This module provides the Rust-native collision handling:
//!
//! - [`rust_collision_observer`] – receives [`CollisionEvent`](crate::events::collision::CollisionEvent)s
//!   and dispatches to [`CollisionRule`](crate::components::collision::CollisionRule) callbacks
//!
//! Callbacks receive `&mut `[`GameCtx`](crate::systems::GameCtx) for full ECS access.
//!
//! # Collision Flow
//!
//! 1. [`collision_detector`](crate::systems::collision_detector::collision_detector) detects overlaps
//!    and emits `CollisionEvent`s
//! 2. `rust_collision_observer` looks up matching Rust collision rules by
//!    [`Group`](crate::components::group::Group) names
//! 3. For each match, computes collision sides and calls the Rust callback
//!
//! # Callback Signature
//!
//! ```ignore
//! fn my_collision(
//!     a: Entity,
//!     b: Entity,
//!     sides_a: &BoxSides,
//!     sides_b: &BoxSides,
//!     ctx: &mut GameCtx,
//! ) {
//!     // Full ECS access via ctx
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – pure Rust collision detection
//! - [`crate::components::collision::CollisionRule`] – defines Rust collision handlers
//! - [`crate::components::collision::CollisionCallback`] – callback type alias
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision::CollisionEvent`] – emitted on each collision

use bevy_ecs::prelude::*;

use crate::components::collision::{get_colliding_sides, CollisionRule};
use crate::events::collision::CollisionEvent;
use crate::systems::GameCtx;

/// Observer that handles Rust collision rules.
///
/// When a [`CollisionEvent`] is triggered:
///
/// 1. Looks up [`Group`] names for both entities (returns early if missing)
/// 2. Queries all [`CollisionRule`] entities for a matching rule
/// 3. Computes collision sides via [`get_colliding_sides`]
/// 4. Calls the matched callback with `(ent_a, ent_b, &sides_a, &sides_b, &mut ctx)`
pub fn rust_collision_observer(
    trigger: On<CollisionEvent>,
    rules: Query<&CollisionRule>,
    mut ctx: GameCtx,
) {
    if rules.is_empty() {
        return;
    }

    let a = trigger.event().a;
    let b = trigger.event().b;

    let ga = if let Ok(group) = ctx.groups.get(a) {
        group.name()
    } else {
        return;
    };
    let gb = if let Ok(group) = ctx.groups.get(b) {
        group.name()
    } else {
        return;
    };

    for rule in rules.iter() {
        if let Some((ent_a, ent_b)) = rule.match_and_order(a, b, ga, gb) {
            // Resolve world positions using GlobalTransform2D when available
            let pos_a = ctx
                .positions
                .get(ent_a)
                .ok()
                .map(|p| {
                    ctx.global_transforms
                        .get(ent_a)
                        .ok()
                        .map_or(p.pos, |gt| gt.position)
                });
            let pos_b = ctx
                .positions
                .get(ent_b)
                .ok()
                .map(|p| {
                    ctx.global_transforms
                        .get(ent_b)
                        .ok()
                        .map_or(p.pos, |gt| gt.position)
                });

            // Compute collider rectangles for side detection
            let rect_a = ctx
                .box_colliders
                .get(ent_a)
                .ok()
                .and_then(|c| pos_a.map(|pos| c.as_rectangle(pos)));
            let rect_b = ctx
                .box_colliders
                .get(ent_b)
                .ok()
                .and_then(|c| pos_b.map(|pos| c.as_rectangle(pos)));

            let (sides_a, sides_b) = match (rect_a, rect_b) {
                (Some(ra), Some(rb)) => get_colliding_sides(&ra, &rb).unwrap_or_default(),
                _ => Default::default(),
            };

            let callback = rule.callback;
            callback(ent_a, ent_b, &sides_a, &sides_b, &mut ctx);
            return;
        }
    }
}
