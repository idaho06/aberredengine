//! Lua-based phase state machine systems.
//!
//! This module provides systems for processing [`LuaPhase`] components:
//!
//! - [`lua_phase_system`] – runs Lua callbacks for phase enter/update/exit
//!
//! Unlike the Rust-based [`phase`](super::phase) system, this system delegates
//! all callback logic to Lua scripts via named function references.
//!
//! # System Flow
//!
//! Each frame, for each entity with a `LuaPhase` component:
//!
//! 1. If `needs_enter_callback` is set, call the on_enter function for current phase
//! 2. If `next` is set (transition requested):
//!    - Call on_exit for old phase
//!    - Swap phases, reset time
//!    - Call on_enter for new phase
//! 3. Call on_update for current phase
//! 4. Increment `time_in_phase` by delta
//! 5. Process any phase transition commands from Lua
//!
//! # Callback Signatures (Lua side)
//!
//! ```lua
//! function my_enter_callback(ctx, input)      -- ctx.previous_phase available
//! function my_update_callback(ctx, input, dt) -- ctx.time_in_phase in ctx
//! function my_exit_callback(ctx)
//! ```
//!
//! # Performance
//!
//! Context tables are pooled and reused across callbacks to reduce Lua GC pressure.
//! See [`EntityCtxPool`](crate::resources::lua_runtime::EntityCtxTables) in runtime.rs.

use bevy_ecs::prelude::*;
use bevy_ecs::system::Local;
use mlua::prelude::*;

use crate::components::luaphase::LuaPhase;
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::AnimationStore;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::{InputSnapshot, LuaPhaseSnapshot, LuaRuntime};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    ContextQueries, DrainScope, EntityCmdQueries, build_entity_context,
    drain_and_process_effect_commands, process_phase_command,
};
use crate::systems::phase_core::{PhaseRunner, apply_callback_transitions, run_phase_callbacks};
use log::{error, warn};

fn build_phase_context(
    lua_runtime: &LuaRuntime,
    entity: Entity,
    lua_phase: &LuaPhase,
    previous_phase: Option<&str>,
    ctx_queries: &ContextQueries,
    cmd_queries: &EntityCmdQueries,
) -> LuaResult<LuaTable> {
    let lua_phase_snapshot = Some(LuaPhaseSnapshot {
        current: lua_phase.current.as_str(),
        time_in_phase: lua_phase.time_in_phase,
    });
    build_entity_context(
        lua_runtime,
        entity,
        ctx_queries,
        cmd_queries,
        lua_phase_snapshot,
        previous_phase,
    )
}

/// Process the return value from a phase callback.
/// Returns Some(phase_name) if a valid transition was requested (different from current phase).
fn process_callback_return(
    result: LuaResult<LuaValue>,
    current_phase: &str,
    fn_name: &str,
) -> Option<String> {
    match result {
        Ok(LuaValue::String(s)) => {
            match s.to_str() {
                Ok(phase) => {
                    if phase != current_phase {
                        Some(phase.to_string())
                    } else {
                        None // Same phase, ignore
                    }
                }
                Err(e) => {
                    error!(target: "lua", "Error converting return value in {}(): {}", fn_name, e);
                    None
                }
            }
        }
        Ok(LuaValue::Nil) => None,
        Ok(_) => {
            warn!(target: "lua", "Phase callback '{}' returned non-string, non-nil value", fn_name);
            None
        }
        Err(e) => {
            error!(target: "lua", "Error in {}(): {}", fn_name, e);
            None
        }
    }
}

/// Call phase enter callback: (ctx, input)
/// Returns Some(phase_name) if the callback returned a phase transition request.
fn call_phase_enter(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    ctx_table: &LuaTable,
    input_table: &LuaTable,
    current_phase: &str,
) -> Option<String> {
    match lua_runtime.get_function(fn_name) {
        Ok(Some(func)) => {
            let result = func.call::<LuaValue>((ctx_table.clone(), input_table.clone()));
            process_callback_return(result, current_phase, fn_name)
        }
        Ok(None) => {
            warn!(target: "lua", "Phase callback '{}' not found", fn_name);
            None
        }
        Err(e) => {
            error!(target: "lua", "Error resolving {}(): {}", fn_name, e);
            None
        }
    }
}

/// Call phase update callback: (ctx, input, dt)
/// Returns Some(phase_name) if the callback returned a phase transition request.
fn call_phase_update(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    ctx_table: &LuaTable,
    input_table: &LuaTable,
    dt: f32,
    current_phase: &str,
) -> Option<String> {
    match lua_runtime.get_function(fn_name) {
        Ok(Some(func)) => {
            let result = func.call::<LuaValue>((ctx_table.clone(), input_table.clone(), dt));
            process_callback_return(result, current_phase, fn_name)
        }
        Ok(None) => {
            warn!(target: "lua", "Phase callback '{}' not found", fn_name);
            None
        }
        Err(e) => {
            error!(target: "lua", "Error resolving {}(): {}", fn_name, e);
            None
        }
    }
}

/// Call phase exit callback: (ctx)
fn call_phase_exit(lua_runtime: &LuaRuntime, fn_name: &str, ctx_table: &LuaTable) {
    match lua_runtime.get_function(fn_name) {
        Ok(Some(func)) => {
            if let Err(e) = func.call::<()>(ctx_table.clone()) {
                error!(target: "lua", "Error in {}(): {}", fn_name, e);
            }
        }
        Ok(None) => {
            warn!(target: "lua", "Phase callback '{}' not found", fn_name);
        }
        Err(e) => {
            error!(target: "lua", "Error resolving {}(): {}", fn_name, e);
        }
    }
}

struct LuaPhaseRunner<'a, 'w, 's> {
    lua_runtime: &'a LuaRuntime,
    input_table: &'a LuaTable,
    ctx_queries: &'a ContextQueries<'w, 's>,
    cmd_queries: &'a EntityCmdQueries<'w, 's>,
}

impl<'a, 'w, 's> PhaseRunner<crate::components::luaphase::PhaseCallbacks>
    for LuaPhaseRunner<'a, 'w, 's>
{
    fn call_enter(
        &mut self,
        entity: Entity,
        lua_phase: &LuaPhase,
        callbacks: &crate::components::luaphase::PhaseCallbacks,
    ) -> Option<String> {
        let fn_name = callbacks.on_enter.as_deref()?;

        match build_phase_context(
            self.lua_runtime,
            entity,
            lua_phase,
            lua_phase.previous.as_deref(),
            self.ctx_queries,
            self.cmd_queries,
        ) {
            Ok(ctx) => call_phase_enter(
                self.lua_runtime,
                fn_name,
                &ctx,
                self.input_table,
                &lua_phase.current,
            ),
            Err(e) => {
                error!("Error building context: {}", e);
                None
            }
        }
    }

    fn call_update(
        &mut self,
        entity: Entity,
        lua_phase: &LuaPhase,
        callbacks: &crate::components::luaphase::PhaseCallbacks,
        delta: f32,
    ) -> Option<String> {
        let fn_name = callbacks.on_update.as_deref()?;

        match build_phase_context(
            self.lua_runtime,
            entity,
            lua_phase,
            None,
            self.ctx_queries,
            self.cmd_queries,
        ) {
            Ok(ctx) => call_phase_update(
                self.lua_runtime,
                fn_name,
                &ctx,
                self.input_table,
                delta,
                &lua_phase.current,
            ),
            Err(e) => {
                error!("Error building context: {}", e);
                None
            }
        }
    }

    fn call_exit(
        &mut self,
        entity: Entity,
        lua_phase: &LuaPhase,
        callbacks: &crate::components::luaphase::PhaseCallbacks,
    ) {
        let Some(fn_name) = callbacks.on_exit.as_deref() else {
            return;
        };

        match build_phase_context(
            self.lua_runtime,
            entity,
            lua_phase,
            None,
            self.ctx_queries,
            self.cmd_queries,
        ) {
            Ok(ctx) => call_phase_exit(self.lua_runtime, fn_name, &ctx),
            Err(e) => error!("Error building context: {}", e),
        }
    }
}

/// Process Lua-based phase state machines.
///
/// This system:
/// 1. Updates signal cache for Lua to read
/// 2. Runs Lua phase callbacks (enter/update/exit) via named functions
/// 3. Processes commands queued by Lua (audio, signals, phases, spawns, entity ops)
/// 4. Handles phase transitions
#[allow(clippy::too_many_arguments)]
pub fn lua_phase_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut LuaPhase)>,
    // Bundled read-only queries for context building
    ctx_queries: ContextQueries,
    // Bundled mutable queries for command processing
    mut cmd_queries: EntityCmdQueries,
    // Resources
    time: Res<WorldTime>,
    input: Res<InputState>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    systems_store: Res<SystemsStore>,
    animation_store: Res<AnimationStore>,
    // Local resource to avoid per-frame allocation for callback return transitions
    mut callback_transitions: Local<Vec<(Entity, String)>>,
    mut phase_entities: Local<Vec<Entity>>,
) {
    // Clear previous frame's transitions (reuses allocated capacity)
    callback_transitions.clear();
    phase_entities.clear();

    // Update signal cache so Lua can read current values
    lua_runtime.update_signal_cache(world_signals.snapshot());

    // Create input snapshot once for all callbacks this frame
    let input_snapshot = InputSnapshot::from_input_state(&input);
    let input_table = match lua_runtime.update_input_table(&input_snapshot) {
        Ok(table) => table,
        Err(e) => {
            error!("Error creating input table for phase system: {}", e);
            return;
        }
    };

    let delta = time.delta;
    let mut runner = LuaPhaseRunner {
        lua_runtime: &lua_runtime,
        input_table: &input_table,
        ctx_queries: &ctx_queries,
        cmd_queries: &cmd_queries,
    };

    run_phase_callbacks(
        &mut query,
        delta,
        &mut callback_transitions,
        &mut phase_entities,
        &mut runner,
    );

    for cmd in lua_runtime.drain_phase_commands() {
        process_phase_command(&mut query, cmd);
    }

    // Apply return value transitions after phase drain — return values take
    // precedence over engine.phase_transition() calls in the same callback.
    apply_callback_transitions(&mut query, &mut callback_transitions);

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
