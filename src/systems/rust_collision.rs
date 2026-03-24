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

use crate::components::collision::CollisionRule;
use crate::events::collision::CollisionEvent;
use crate::systems::GameCtx;
use crate::systems::collision::{compute_sides, resolve_collider_rect, resolve_groups};

/// Observer that handles Rust collision rules.
///
/// When a [`CollisionEvent`] is triggered:
///
/// 1. Looks up [`Group`] names for both entities (returns early if missing)
/// 2. Queries all [`CollisionRule`] entities for a matching rule
/// 3. Computes collision sides via [`compute_sides`]
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

    let (ga, gb) = match resolve_groups(&ctx.groups, a, b) {
        Some(names) => names,
        None => return,
    };

    for rule in rules.iter() {
        if let Some((ent_a, ent_b)) = rule.match_and_order(a, b, ga, gb) {
            let rect_a = resolve_collider_rect(
                &ctx.positions.as_readonly(),
                &ctx.global_transforms,
                &ctx.box_colliders,
                ent_a,
            );
            let rect_b = resolve_collider_rect(
                &ctx.positions.as_readonly(),
                &ctx.global_transforms,
                &ctx.box_colliders,
                ent_b,
            );
            let (sides_a, sides_b) = compute_sides(rect_a, rect_b);

            let callback = rule.callback;
            callback(ent_a, ent_b, &sides_a, &sides_b, &mut ctx);
            return;
        }
    }
}
