//! Collision event types and a simple observer.
//!
//! The collision system emits [`CollisionEvent`] whenever two entities with
//! compatible colliders overlap. Observers can subscribe to this event to
//! react in a decoupled manner (damage, sound, despawn, etc.).
//!
//! This module also includes an example observer,
//! [`observe_kill_on_collision`], which despawns both entities if at least one
//! belongs to the `"player"` [`Group`]. Use this as a reference or replace it
//! with your own game-specific logic.
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;

use crate::components::group::Group;

/// Event fired when two entities with BoxCollider overlap.
///
/// The two fields, [`CollisionEvent::a`] and [`CollisionEvent::b`], are the
/// entity IDs of the participants. No ordering guarantees are provided.
/// Additional collision details (normals, penetration, etc.) can be added by
/// extending this type when needed.
#[derive(Event, Debug, Clone, Copy)]
pub struct CollisionEvent {
    pub a: Entity,
    pub b: Entity,
}

/// Global observer that despawns both collided entities when a CollisionEvent is triggered.
///
/// Behavior
/// - If neither entity is in the `"player"` group, the function returns early
///   and does nothing.
/// - Otherwise, both entities are despawned immediately.
///
/// Notes
/// - This is a simple example intended for debugging or arcade-like rules. In
///   a real game you might filter by different groups, apply damage, trigger
///   invulnerability frames, or play sounds instead of despawning.
/// - The observer runs in the immediate observer flow, so both entities are
///   expected to still exist at this point.
pub fn observe_kill_on_collision(
    trigger: On<CollisionEvent>,
    mut commands: Commands,
    groups: Query<&Group>,
) {
    let a = trigger.event().a;
    let b = trigger.event().b;

    // Return early if none belong to the "player" group.
    let is_player = |e: Entity| {
        groups
            .get(e)
            //.map(|g| *g == Group("player"))
            .map(|g| g.name() == "player") // Use the string directly
            .unwrap_or(false)
    };
    if !is_player(a) && !is_player(b) {
        return;
    }
    println!("Collision detected: {:?} and {:?}", a, b);

    // Despawn both. In this immediate observer flow, entities should still exist.
    commands.entity(a).despawn();
    commands.entity(b).despawn();
}
