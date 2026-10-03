//! Engine-wide cleanup of [`WorldSignals`] entity registrations.

use bevy_ecs::entity::Entities;
use bevy_ecs::prelude::*;

use crate::resources::worldsignals::WorldSignals;

/// Drop [`WorldSignals`] entity registrations that point at despawned entities.
///
/// Runs at the end of every sim tick, so a registration for an entity
/// despawned during the tick (by Rust or Lua code alike) is gone before the
/// tick's snapshot and the next tick's logic. Uses [`Entities::contains`], a
/// generation check, so an entity that `Commands` has reserved but not yet
/// spawned still counts as alive. Leaves change detection untouched when
/// nothing is removed.
pub fn prune_dead_entity_registrations(mut signals: ResMut<WorldSignals>, entities: &Entities) {
    if signals
        .bypass_change_detection()
        .remove_dead_entity_registrations(|e| entities.contains(e))
    {
        signals.set_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    fn world_with_signals() -> World {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world
    }

    #[test]
    fn prunes_registrations_of_despawned_entities() {
        let mut world = world_with_signals();
        let dead = world.spawn_empty().id();
        let live = world.spawn_empty().id();
        {
            let mut ws = world.resource_mut::<WorldSignals>();
            ws.set_entity("dead", dead);
            ws.set_entity("live", live);
        }
        world.despawn(dead);

        world
            .run_system_once(prune_dead_entity_registrations)
            .unwrap();

        let ws = world.resource::<WorldSignals>();
        assert!(ws.get_entity("dead").is_none());
        assert_eq!(ws.get_entity("live"), Some(live));
    }

    #[test]
    fn keeps_registrations_of_reserved_but_unspawned_entities() {
        let mut world = world_with_signals();
        // `Commands::spawn` hands out an allocated id before the command is
        // applied; a registration made in that window must survive the sweep.
        let reserved = world.entity_allocator().alloc();
        assert!(!world.entities().contains_spawned(reserved));
        world
            .resource_mut::<WorldSignals>()
            .set_entity("reserved", reserved);

        world
            .run_system_once(prune_dead_entity_registrations)
            .unwrap();

        assert_eq!(
            world.resource::<WorldSignals>().get_entity("reserved"),
            Some(reserved)
        );
    }

    #[test]
    fn untouched_when_nothing_is_dead() {
        let mut world = world_with_signals();
        let live = world.spawn_empty().id();
        world
            .resource_mut::<WorldSignals>()
            .set_entity("live", live);
        let tick = world.change_tick();
        world.increment_change_tick();

        world
            .run_system_once(prune_dead_entity_registrations)
            .unwrap();

        assert!(
            !world
                .resource_ref::<WorldSignals>()
                .last_changed()
                .is_newer_than(tick, world.change_tick()),
            "a sweep that removes nothing must not mark WorldSignals changed"
        );
    }
}
