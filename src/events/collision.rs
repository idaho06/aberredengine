use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;

use crate::components::group::Group;

/// Event fired when two entities with BoxCollider overlap.
#[derive(Event, Debug, Clone, Copy)]
pub struct CollisionEvent {
    pub a: Entity,
    pub b: Entity,
}

/// Global observer that despawns both collided entities when a CollisionEvent is triggered.
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
