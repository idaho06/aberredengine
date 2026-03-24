//! Lua timer systems.
//!
//! This module provides systems for processing [`LuaTimer`](crate::components::luatimer::LuaTimer) components:
//!
//! - [`update_lua_timers`] – updates timer elapsed time and emits events when they expire
//! - [`lua_timer_observer`] – observer that calls Lua functions when timer events fire
//!
//! # System Flow
//!
//! Each frame:
//!
//! 1. `update_lua_timers` accumulates delta time on all LuaTimer components
//! 2. When `elapsed >= duration`, emits `LuaTimerEvent` and resets timer
//! 3. `lua_timer_observer` receives events and calls the named Lua function
//! 4. Lua callback executes with full engine API access
//! 5. Commands queued by Lua are processed (spawns, audio, signals, entity ops)
//!
//! # Lua Callback Signature
//!
//! ```lua
//! function callback_name(ctx, input)
//!     -- ctx is the entity context table with all component data
//!     -- input is the input table with digital and analog inputs
//!     -- Full access to engine API
//! end
//! ```
//!
//! # Performance
//!
//! Context tables are pooled and reused across callbacks to reduce Lua GC pressure.
//! See [`EntityCtxPool`](crate::resources::lua_runtime::EntityCtxTables) in runtime.rs.

use bevy_ecs::prelude::*;
use mlua::prelude::*;

use crate::components::luaphase::LuaPhase;
use crate::components::luatimer::{LuaTimer, LuaTimerCallback};
use crate::events::audio::AudioCmd;
use crate::events::luatimer::LuaTimerEvent;
use crate::resources::animationstore::AnimationStore;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::{InputSnapshot, LuaPhaseSnapshot, LuaRuntime};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    ContextQueries, EntityCmdQueries, build_entity_context, process_audio_command,
    process_camera_command, process_clone_command, process_entity_commands, process_phase_command,
    process_signal_command, process_spawn_command,
};
use log::{error, warn};

use super::timer_core::{TimerRunner, run_timer_update};

struct LuaTimerRunner<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
}

impl<'a, 'w, 's> TimerRunner<LuaTimerCallback> for LuaTimerRunner<'a, 'w, 's> {
    fn on_fire(&mut self, entity: Entity, callback: &LuaTimerCallback) {
        self.commands.trigger(LuaTimerEvent {
            entity,
            callback: callback.name.clone(),
        });
    }
}

/// Update all Lua timer components and emit events when they expire.
///
/// Accumulates delta time on each [`LuaTimer`](crate::components::luatimer::LuaTimer)
/// and triggers a [`LuaTimerEvent`](crate::events::luatimer::LuaTimerEvent) when
/// `elapsed >= duration`. The timer resets by subtracting duration, allowing for
/// consistent periodic timing.
pub fn update_lua_timers(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut LuaTimer)>,
    mut commands: Commands,
) {
    let delta = world_time.delta;
    let mut runner = LuaTimerRunner {
        commands: &mut commands,
    };
    run_timer_update(delta, &mut query, &mut runner);
}

fn build_timer_context(
    lua_runtime: &LuaRuntime,
    entity: Entity,
    ctx_queries: &ContextQueries,
    cmd_queries: &EntityCmdQueries,
    luaphase_query: &Query<(Entity, &mut LuaPhase)>,
) -> LuaResult<LuaTable> {
    let lua_phase_snapshot = luaphase_query
        .get(entity)
        .ok()
        .map(|(_, p)| LuaPhaseSnapshot {
            current: p.current.as_str(),
            time_in_phase: p.time_in_phase,
        });
    build_entity_context(
        lua_runtime,
        entity,
        ctx_queries,
        cmd_queries,
        lua_phase_snapshot,
        None,
    )
}

/// Observer that handles Lua timer events by calling Lua functions.
///
/// When a [`LuaTimerEvent`](crate::events::luatimer::LuaTimerEvent) is triggered:
///
/// 1. Builds entity context with all component data
/// 2. Checks if the Lua function exists
/// 3. Calls it with `(ctx, input)` as parameters
/// 4. Processes all commands queued by the Lua function:
///    - Audio commands (play music/sounds)
///    - Signal commands (modify WorldSignals)
///    - Phase commands (trigger phase transitions)
///    - Spawn commands (create new entities)
///    - Entity commands (modify components)
///
/// If the Lua function doesn't exist, logs a warning but doesn't crash.
#[allow(clippy::too_many_arguments)]
pub fn lua_timer_observer(
    trigger: On<LuaTimerEvent>,
    mut commands: Commands,
    input: Res<InputState>,
    // Bundled read-only queries for context building
    ctx_queries: ContextQueries,
    // Bundled mutable queries for command processing
    mut cmd_queries: EntityCmdQueries,
    mut luaphase_query: Query<(Entity, &mut LuaPhase)>,
    // Resources
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    systems_store: Res<SystemsStore>,
    animation_store: Res<AnimationStore>,
) {
    let event = trigger.event();
    let entity = event.entity;

    // Update signal cache so Lua can read current values
    lua_runtime.update_signal_cache(world_signals.snapshot());

    // Create input snapshot and table
    let input_snapshot = InputSnapshot::from_input_state(&input);
    let input_table = match lua_runtime.update_input_table(&input_snapshot) {
        Ok(table) => table,
        Err(e) => {
            error!("Error creating input table for timer callback: {}", e);
            return;
        }
    };

    // Build entity context
    let ctx_table = match build_timer_context(
        &lua_runtime,
        entity,
        &ctx_queries,
        &cmd_queries,
        &luaphase_query,
    ) {
        Ok(ctx) => ctx,
        Err(e) => {
            error!("Error building context for timer callback: {}", e);
            return;
        }
    };

    // Call the Lua callback with (ctx, input)
    match lua_runtime.get_function(&event.callback) {
        Ok(Some(func)) => {
            if let Err(e) = func.call::<()>((ctx_table, input_table)) {
                error!(target: "lua", "Error in {}(): {}", event.callback, e);
            }
        }
        Ok(None) => {
            warn!(target: "lua", "Timer callback '{}' not found", event.callback);
        }
        Err(e) => {
            error!(target: "lua", "Error resolving {}(): {}", event.callback, e);
        }
    }

    // Process phase commands from Lua
    for cmd in lua_runtime.drain_phase_commands() {
        process_phase_command(&mut luaphase_query, cmd);
    }

    // Process audio commands from Lua
    for cmd in lua_runtime.drain_audio_commands() {
        process_audio_command(&mut audio_cmd_writer, cmd);
    }

    // Process signal commands from Lua
    for cmd in lua_runtime.drain_signal_commands() {
        process_signal_command(&mut world_signals, cmd);
    }

    // Process spawn commands from Lua
    for cmd in lua_runtime.drain_spawn_commands() {
        process_spawn_command(&mut commands, cmd, &mut world_signals);
    }

    // Process clone commands from Lua
    for cmd in lua_runtime.drain_clone_commands() {
        process_clone_command(&mut commands, cmd, &mut world_signals);
    }

    // Process entity commands from Lua
    process_entity_commands(
        &mut commands,
        lua_runtime.drain_entity_commands(),
        &mut cmd_queries,
        &systems_store,
        &animation_store,
    );

    // Process camera commands from Lua
    for cmd in lua_runtime.drain_camera_commands() {
        process_camera_command(&mut commands, cmd);
    }
}
