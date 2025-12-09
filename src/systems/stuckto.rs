//! System for handling entities stuck to other entities.
//!
//! This system updates the position of entities with the [`StuckTo`] component
//! to follow their target entity's position.
//!
//! # Use Cases
//!
//! - Ball stuck to paddle at game start (follows X only)
//! - Objects attached to moving platforms
//! - Temporary "sticky" effects with auto-release via [`Timer`](crate::components::timer::Timer)
//!
//! # Related
//!
//! - [`StuckTo`](crate::components::stuckto::StuckTo) – the attachment component
//! - [`Timer`](crate::components::timer::Timer) – can auto-remove `StuckTo` after a delay

use bevy_ecs::prelude::*;

use crate::components::mapposition::MapPosition;
use crate::components::stuckto::StuckTo;

/// Updates positions of entities with `StuckTo` to follow their targets.
///
/// For each entity with a `StuckTo` component:
/// - Gets the target entity's `MapPosition`
/// - Updates this entity's position based on `follow_x` and `follow_y` flags
/// - Applies the offset
pub fn stuck_to_entity_system(
    mut followers: Query<(&StuckTo, &mut MapPosition)>,
    targets: Query<&MapPosition, Without<StuckTo>>,
) {
    for (stuck_to, mut follower_pos) in followers.iter_mut() {
        // Try to get the target's position
        if let Ok(target_pos) = targets.get(stuck_to.target) {
            if stuck_to.follow_x {
                follower_pos.pos.x = target_pos.pos.x + stuck_to.offset.x;
            }
            if stuck_to.follow_y {
                follower_pos.pos.y = target_pos.pos.y + stuck_to.offset.y;
            }
        }
    }
}
