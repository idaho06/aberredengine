//! Lua-only map-spawning glue.
//!
//! [`lua_map_spawned_observer`] attaches `LuaSetup`/`LuaOnAnimationFinished`
//! to the entities core's `spawn_map` spawned, from each entity definition's
//! `lua_setup`/`on_animation_finished`. It is registered only when the game
//! runs a Lua script. `process_lua_map_commands` drains
//! `engine.load_map()` queue entries into `SpawnMapRequested` triggers.

use crate::components::lua_on_animation_finished::LuaOnAnimationFinished;
use crate::components::lua_setup::LuaSetup;
use crate::resources::lua_runtime::{LuaRuntime, MapLuaCmd};
use aberred_core::events::map_spawn::{MapSpawned, SpawnMapRequested};
use aberred_core::resources::map_data::load_map;
use bevy_ecs::prelude::*;

/// Attaches the Lua callback components named by each spawned map entity's
/// definition.
pub fn lua_map_spawned_observer(trigger: On<MapSpawned>, mut commands: Commands) {
    let MapSpawned { map, spawned } = trigger.event();
    for (&entity, def) in spawned.iter().zip(&map.entities) {
        let mut ec = commands.entity(entity);
        if let Some(callback) = &def.lua_setup {
            ec.insert(LuaSetup::new(callback.as_str()));
        }
        if let Some(callback) = &def.on_animation_finished {
            ec.insert(LuaOnAnimationFinished::new(callback.as_str()));
        }
    }
}

/// Drains `engine.load_map()` commands queued by Lua and fires
/// `SpawnMapRequested` for each, letting core's `spawn_map_observer` queue the
/// asset loads and spawn the entities.
///
/// Registered only when the game runs a Lua script. Runs on the
/// sim schedule's `SimSet::Bookkeeping`, so a map load queued from
/// `on_update_<scene>`/phase/timer/collision callbacks is picked up the same
/// tick it's queued.
pub fn process_lua_map_commands(
    mut commands: Commands,
    lua: NonSend<LuaRuntime>,
    mut buf: Local<Vec<MapLuaCmd>>,
) {
    lua.drain_map_commands_into(&mut buf);
    for cmd in buf.drain(..) {
        match cmd {
            MapLuaCmd::LoadMap { path } => match load_map(&path) {
                Ok(map) => commands.trigger(SpawnMapRequested { map }),
                Err(e) => log::error!("engine.load_map: failed to read '{path}': {e}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::resources::map_data::{EntityDef, MapData};

    #[test]
    fn map_spawned_attaches_lua_components_by_entity_def() {
        let mut world = World::new();
        world.add_observer(lua_map_spawned_observer);
        let with_callbacks = world.spawn_empty().id();
        let plain = world.spawn_empty().id();

        world.trigger(MapSpawned {
            map: MapData {
                entities: vec![
                    EntityDef {
                        lua_setup: Some("setup_fn".into()),
                        on_animation_finished: Some("anim_done".into()),
                        ..Default::default()
                    },
                    EntityDef::default(),
                ],
                ..Default::default()
            },
            spawned: vec![with_callbacks, plain],
        });
        world.flush();

        assert_eq!(
            &*world.get::<LuaSetup>(with_callbacks).unwrap().callback,
            "setup_fn"
        );
        assert_eq!(
            &*world
                .get::<LuaOnAnimationFinished>(with_callbacks)
                .unwrap()
                .callback,
            "anim_done"
        );
        assert!(world.get::<LuaSetup>(plain).is_none());
        assert!(world.get::<LuaOnAnimationFinished>(plain).is_none());
    }
}
