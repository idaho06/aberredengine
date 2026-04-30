//! One-shot Lua entity setup system.
//!
//! [`lua_setup_entity_system`] reacts to every entity that gains a
//! [`LuaSetup`] component and calls the named Lua function once, passing the
//! standard entity context table. It is ordered before `animation_controller`
//! so that setup callbacks can set animation state in the same frame.
//!
//! For timing and contract details see [`crate::components::luasetup`].

use bevy_ecs::prelude::*;
use log::{error, warn};

use crate::components::luaphase::LuaPhase;
use crate::components::luasetup::LuaSetup;
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::AnimationStore;
use crate::resources::lua_runtime::LuaRuntime;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::systems::lua_commands::{
    ContextQueries, DrainScope, EntityCmdQueries, build_entity_context,
    drain_and_process_effect_commands, process_phase_command,
};

/// Call the named Lua setup function for every newly added [`LuaSetup`] entity.
///
/// Runs during `Playing` state, after `check_pending_state` and before
/// `animation_controller`.
#[allow(clippy::too_many_arguments)]
pub fn lua_setup_entity_system(
    query: Query<(Entity, &LuaSetup), Added<LuaSetup>>,
    ctx_queries: ContextQueries,
    mut cmd_queries: EntityCmdQueries,
    mut luaphase_query: Query<(Entity, &mut LuaPhase)>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut commands: Commands,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    systems_store: Res<SystemsStore>,
    animation_store: Res<AnimationStore>,
) {
    if query.is_empty() {
        return;
    }

    lua_runtime.update_signal_cache(world_signals.snapshot());

    for (entity, lua_setup) in &query {
        let ctx_table = match build_entity_context(
            &lua_runtime,
            entity,
            &ctx_queries,
            &cmd_queries,
            None,
            None,
        ) {
            Ok(table) => table,
            Err(e) => {
                error!(
                    target: "lua",
                    "lua_setup_entity: error building context for {:?}: {}",
                    entity, e
                );
                continue;
            }
        };

        match lua_runtime.get_function(&lua_setup.callback) {
            Ok(Some(func)) => {
                if let Err(e) = func.call::<()>(ctx_table) {
                    error!(target: "lua", "Error in {}(): {}", lua_setup.callback, e);
                }
            }
            Ok(None) => {
                warn!(
                    target: "lua",
                    "LuaSetup callback '{}' not found",
                    lua_setup.callback
                );
            }
            Err(e) => {
                error!(
                    target: "lua",
                    "Error resolving {}(): {}",
                    lua_setup.callback, e
                );
            }
        }
    }

    for cmd in lua_runtime.drain_phase_commands() {
        process_phase_command(&mut luaphase_query, cmd);
    }

    drain_and_process_effect_commands(
        &lua_runtime,
        DrainScope::Regular,
        &mut commands,
        &mut world_signals,
        &mut cmd_queries,
        &mut audio_cmd_writer,
        &systems_store,
        &animation_store,
    );
}
