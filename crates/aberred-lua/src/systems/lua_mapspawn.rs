//! Lua-only map-spawning glue.
//!
//! [`aberred_core::systems::mapspawn::spawn_map`] cannot attach
//! `LuaSetup`/`LuaOnAnimationEnd` itself -- those component types are
//! Lua-only and `aberred-core` cannot name them. This module's
//! `spawn_map_observer` wraps core's `spawn_map`, zipping its returned
//! entities against `MapData::entities` to attach them. Re-exported by
//! the facade's `systems::mapspawn` under `#[cfg(feature = "lua")]`.
//! `process_lua_map_commands` -- draining `engine.load_map()` queue entries
//! into `SpawnMapRequested` triggers -- lives here entirely, since it needs
//! `LuaRuntime`.

use crate::components::lua_on_animation_end::LuaOnAnimationEnd;
use crate::components::luasetup::LuaSetup;
use crate::resources::lua_runtime::{LuaRuntime, MapLuaCmd};
use aberred_core::events::spawnmap::SpawnMapRequested;
use aberred_core::protocol::render_assets::RenderAssetCmd;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::mapdata::load_map;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_core::systems::mapspawn::spawn_map;
use bevy_ecs::prelude::*;

/// Bevy observer registered by the engine. Fires on `SpawnMapRequested` and
/// delegates to core's `spawn_map`, then attaches the Lua-only per-entity
/// components core couldn't.
#[allow(clippy::too_many_arguments)]
pub fn spawn_map_observer(
    trigger: On<SpawnMapRequested>,
    mut commands: Commands,
    mut animation_store: ResMut<AnimationStore>,
    mut world_signals: ResMut<WorldSignals>,
    mut render_asset_cmd_writer: MessageWriter<RenderAssetCmd>,
) {
    let mut render_asset_cmds = Vec::new();
    let map = &trigger.event().map;
    let entities = spawn_map(
        &mut commands,
        &mut animation_store,
        map,
        &mut world_signals,
        &mut render_asset_cmds,
    );
    for (entity, def) in entities.iter().zip(map.entities.iter()) {
        let mut ec = commands.entity(*entity);
        if let Some(ref callback) = def.lua_setup {
            ec.insert(LuaSetup::new(callback.as_str()));
        }
        if let Some(ref callback) = def.on_animation_end {
            ec.insert(LuaOnAnimationEnd::new(callback.as_str()));
        }
    }
    for cmd in render_asset_cmds {
        render_asset_cmd_writer.write(cmd);
    }
}

/// Drains `engine.load_map()` commands queued by Lua and fires
/// `SpawnMapRequested` for each, letting `spawn_map_observer` handle the
/// Raylib-dependent asset loading and entity spawning.
///
/// Registered by the facade's `EngineBuilder::with_lua`. Runs on the
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
