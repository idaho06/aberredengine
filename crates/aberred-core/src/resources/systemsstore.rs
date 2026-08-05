//! Registry for dynamically addressable systems.
//!
//! Allows systems to be registered under string keys and looked up later to
//! run via their [`bevy_ecs::system::SystemId`]. This is useful for invoking
//! state-specific setup/teardown hooks without tight coupling.

use bevy_ecs::prelude::{Entity, In, Resource};
use bevy_ecs::system::SystemId;
use rustc_hash::FxHashMap;

/// Map of string names to system IDs.
#[derive(Resource)]
pub struct SystemsStore {
    /// Systems that take no input.
    pub map: FxHashMap<String, SystemId>,
    /// Systems that take an Entity as input (via `In<Entity>`).
    pub entity_map: FxHashMap<String, SystemId<In<Entity>>>,
}

impl Default for SystemsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemsStore {
    /// Create an empty store.
    pub fn new() -> Self {
        SystemsStore {
            map: FxHashMap::default(),
            entity_map: FxHashMap::default(),
        }
    }

    /// Insert a system ID under a human-readable name.
    pub fn insert(&mut self, name: impl Into<String>, id: SystemId) {
        self.map.insert(name.into(), id);
    }

    /// Retrieve a system ID by name, if present.
    pub fn get(&self, name: impl AsRef<str>) -> Option<&SystemId> {
        self.map.get(name.as_ref())
    }

    /// Insert a system ID that takes an Entity as input.
    pub fn insert_entity_system(&mut self, name: impl Into<String>, id: SystemId<In<Entity>>) {
        self.entity_map.insert(name.into(), id);
    }

    /// Retrieve a system ID that takes an Entity as input.
    pub fn get_entity_system(&self, name: impl AsRef<str>) -> Option<&SystemId<In<Entity>>> {
        self.entity_map.get(name.as_ref())
    }
}

// Well-known SystemsStore keys the engine itself registers/looks up.
//
// Use these constants everywhere a SystemsStore key is written or read to
// get compile-time-checked references and a single rename point (mirrors
// src/resources/signal_keys.rs's convention for WorldSignals keys -- this is
// a distinct namespace even where a string value happens to coincide, e.g.
// "switch_scene"/"quit_game" are also WorldSignals flag names; do not
// conflate the two).

/// One-shot asset-loading hook, called during the `Setup` game state.
pub const SETUP: &str = "setup";

/// Hook called once when transitioning to `Playing`.
pub const ENTER_PLAY: &str = "enter_play";

/// Hook called when a scene transition is requested.
pub const SWITCH_SCENE: &str = "switch_scene";

/// Engine-internal teardown hook run on a clean shutdown request.
pub const QUIT_GAME: &str = "quit_game";

/// Engine-internal hook that despawns all non-persistent entities.
pub const CLEAN_ALL_ENTITIES: &str = "clean_all_entities";

/// Entity-input system (`SystemId<In<Entity>>`) that despawns a menu.
pub const MENU_DESPAWN: &str = "menu_despawn";
