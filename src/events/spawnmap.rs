//! Event to request spawning all assets and entities from a [`MapData`].
//!
//! Trigger this event after loading a map with
//! [`crate::resources::mapdata::load_map`] to have the engine populate the
//! asset stores and spawn all entities defined in the map.
//!
//! The built-in [`crate::systems::mapspawn::spawn_map_observer`] handles this
//! event automatically — no manual registration needed for standard usage.
//!
//! # Example
//!
//! ```rust,no_run
//! # use aberredengine::resources::mapdata::load_map;
//! # use aberredengine::events::spawnmap::SpawnMapRequested;
//! # use bevy_ecs::prelude::Commands;
//! # fn example(mut commands: Commands) {
//! let map = load_map("assets/levels/level01.json").unwrap();
//! commands.trigger(SpawnMapRequested { map });
//! # }
//! ```

use bevy_ecs::prelude::Event;

use crate::resources::mapdata::MapData;

/// Trigger this event to load all assets in a [`MapData`] into the engine
/// stores and spawn all entity definitions.
#[derive(Event)]
pub struct SpawnMapRequested {
    pub map: MapData,
}
