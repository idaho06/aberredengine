//! Event to request spawning all assets and entities from a [`MapData`].
//!
//! Trigger this event after loading a map with
//! [`crate::resources::mapdata::load_map`] to have the engine populate the
//! asset stores and spawn all entities defined in the map.
//!
//! The built-in [`crate::systems::mapspawn::spawn_map_observer`] handles this
//! event automatically; no manual registration is needed. Once the map's
//! entities are spawned, the engine triggers [`MapSpawned`] with them.
//!
//! # Example
//!
//! ```rust,no_run
//! # use aberred_core::resources::mapdata::load_map;
//! # use aberred_core::events::spawnmap::SpawnMapRequested;
//! # use bevy_ecs::prelude::Commands;
//! # fn example(mut commands: Commands) {
//! let map = load_map("assets/levels/level01.json").unwrap();
//! commands.trigger(SpawnMapRequested { map });
//! # }
//! ```

use bevy_ecs::prelude::{Entity, Event};

use crate::resources::mapdata::MapData;

/// Trigger this event to load all assets in a [`MapData`] into the engine
/// stores and spawn all entity definitions.
#[derive(Event)]
pub struct SpawnMapRequested {
    pub map: MapData,
}

/// Triggered by [`spawn_map`](crate::systems::mapspawn::spawn_map) after it
/// spawns a map's entities (including for every [`SpawnMapRequested`]).
///
/// `spawned[i]` is the entity spawned for `map.entities[i]`, so an observer
/// can attach components from each entity definition.
#[derive(Event, Clone, Debug)]
pub struct MapSpawned {
    /// The spawned map.
    pub map: MapData,
    /// One entity per `map.entities` entry, in the same order.
    pub spawned: Vec<Entity>,
}
