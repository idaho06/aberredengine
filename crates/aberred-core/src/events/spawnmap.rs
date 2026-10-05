//! Event to request spawning all assets and entities from a [`MapData`].
//!
//! Trigger this event after loading a map with
//! [`crate::resources::mapdata::load_map`] to have the engine queue the map's
//! assets and spawn its entities (see [`SpawnMapRequested`]).
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

/// Trigger this event to queue every texture and font in a [`MapData`], fill
/// `AnimationStore` with its animations, and spawn its entity definitions.
///
/// Don't also queue the map's assets yourself: textures always reload, and a
/// font queued after the map reloads (the map skips fonts already loaded).
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
