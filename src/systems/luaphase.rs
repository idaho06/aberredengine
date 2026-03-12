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
use crate::resources::lua_runtime::{
    AnimationSnapshot, EntitySnapshot, InputSnapshot, LuaPhaseSnapshot, LuaRuntime,
    LuaTimerSnapshot, RigidBodySnapshot, SpriteSnapshot, build_entity_context_pooled,
};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    ContextQueries, EntityCmdQueries, process_audio_command, process_camera_command,
    process_clone_command, process_entity_commands, process_phase_command, process_signal_command,
    process_spawn_command,
};
use crate::systems::phase_core::{
    PhaseRunner, apply_callback_transitions, run_phase_callbacks,
};
use log::{error, warn};

/// Build entity context for phase callbacks using pooled tables.
///
/// Gathers all component data for the given entity and builds a Lua context table.
/// Uses pooled tables to reduce allocations.
fn build_phase_context(
    lua_runtime: &LuaRuntime,
    entity: Entity,
    lua_phase: &LuaPhase,
    previous_phase: Option<&str>,
    ctx_queries: &ContextQueries,
    cmd_queries: &EntityCmdQueries,
) -> LuaResult<LuaTable> {
    let lua = lua_runtime.lua();
    let tables = lua_runtime.get_entity_ctx_pool()?;

    // Query all optional components
    let group = ctx_queries.groups.get(entity).ok().map(|g| g.name());

    let map_pos = cmd_queries
        .positions
        .get(entity)
        .ok()
        .map(|p| (p.pos.x, p.pos.y));

    let screen_pos = cmd_queries
        .screen_positions
        .get(entity)
        .ok()
        .map(|p| (p.pos.x, p.pos.y));

    let rigid_body = cmd_queries
        .rigid_bodies
        .get(entity)
        .ok()
        .map(|rb| RigidBodySnapshot {
            velocity: (rb.velocity.x, rb.velocity.y),
            speed_sq: rb.velocity.length_sqr(),
            frozen: rb.frozen,
        });

    let rotation = ctx_queries.rotations.get(entity).ok().map(|r| r.degrees);

    let scale = ctx_queries
        .scales
        .get(entity)
        .ok()
        .map(|s| (s.scale.x, s.scale.y));

    // Compute collider rect using position
    let rect = ctx_queries.box_colliders.get(entity).ok().and_then(|bc| {
        cmd_queries.positions.get(entity).ok().map(|pos| {
            let rect = bc.as_rectangle(pos.pos);
            (rect.x, rect.y, rect.width, rect.height)
        })
    });

    let sprite = cmd_queries
        .sprites
        .get(entity)
        .ok()
        .map(|s| SpriteSnapshot {
            tex_key: s.tex_key.as_ref(),
            flip_h: s.flip_h,
            flip_v: s.flip_v,
        });

    let animation = cmd_queries
        .animation
        .get(entity)
        .ok()
        .map(|a| AnimationSnapshot {
            key: a.animation_key.as_str(),
            frame_index: a.frame_index,
            elapsed: a.elapsed_time,
        });

    let signals_ref = cmd_queries.signals.get(entity).ok();

    let lua_phase_snapshot = Some(LuaPhaseSnapshot {
        current: lua_phase.current.as_str(),
        time_in_phase: lua_phase.time_in_phase,
    });

    let lua_timer = ctx_queries
        .lua_timers
        .get(entity)
        .ok()
        .map(|t| LuaTimerSnapshot {
            duration: t.duration,
            elapsed: t.elapsed,
            callback: t.callback.name.as_str(),
        });

    // World transform from GlobalTransform2D (hierarchy)
    let gt = ctx_queries.global_transforms.get(entity).ok();
    let world_pos = gt.map(|g| (g.position.x, g.position.y));
    let world_rotation = gt.map(|g| g.rotation_degrees);
    let world_scale = gt.map(|g| (g.scale.x, g.scale.y));

    // Parent entity ID from ChildOf
    let parent_id = ctx_queries.child_of.get(entity).ok().map(|c| c.0.to_bits());

    let snapshot = EntitySnapshot {
        entity_id: entity.to_bits(),
        group,
        map_pos,
        screen_pos,
        rigid_body,
        rotation,
        scale,
        rect,
        sprite,
        animation,
        signals: signals_ref,
        lua_phase: lua_phase_snapshot,
        lua_timer,
        previous_phase,
        world_pos,
        world_rotation,
        world_scale,
        parent_id,
    };

    build_entity_context_pooled(lua, &tables, &snapshot)
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
    if lua_runtime.has_function(fn_name) {
        let result = lua_runtime
            .call_function::<_, LuaValue>(fn_name, (ctx_table.clone(), input_table.clone()));
        process_callback_return(result, current_phase, fn_name)
    } else {
        warn!(target: "lua", "Phase callback '{}' not found", fn_name);
        None
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
    if lua_runtime.has_function(fn_name) {
        let result = lua_runtime
            .call_function::<_, LuaValue>(fn_name, (ctx_table.clone(), input_table.clone(), dt));
        process_callback_return(result, current_phase, fn_name)
    } else {
        warn!(target: "lua", "Phase callback '{}' not found", fn_name);
        None
    }
}

/// Call phase exit callback: (ctx)
fn call_phase_exit(lua_runtime: &LuaRuntime, fn_name: &str, ctx_table: &LuaTable) {
    if lua_runtime.has_function(fn_name) {
        if let Err(e) = lua_runtime.call_function::<_, ()>(fn_name, ctx_table.clone()) {
            error!(target: "lua", "Error in {}(): {}", fn_name, e);
        }
    } else {
        warn!(target: "lua", "Phase callback '{}' not found", fn_name);
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
) {
    // Clear previous frame's transitions (reuses allocated capacity)
    callback_transitions.clear();

    // Update signal cache so Lua can read current values
    lua_runtime.update_signal_cache(world_signals.snapshot());

    // Create input snapshot once for all callbacks this frame
    let input_snapshot = InputSnapshot::from_input_state(&input);
    let input_table = match lua_runtime.create_input_table(&input_snapshot) {
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
        &mut runner,
    );

    // Process phase commands from Lua (from engine.phase_transition calls)
    for cmd in lua_runtime.drain_phase_commands() {
        process_phase_command(&mut query, cmd);
    }

    // Apply return value transitions (these take precedence over PhaseCmd)
    apply_callback_transitions(&mut query, &mut callback_transitions);

    // Process audio commands from Lua
    for cmd in lua_runtime.drain_audio_commands() {
        process_audio_command(&mut audio_cmd_writer, cmd);
    }

    // Process signal commands from Lua
    for cmd in lua_runtime.drain_signal_commands() {
        process_signal_command(&mut world_signals, cmd);
    }

    // Process spawn commands from Lua (entities spawned during phase callbacks)
    for cmd in lua_runtime.drain_spawn_commands() {
        process_spawn_command(&mut commands, cmd, &mut world_signals);
    }

    // Process clone commands from Lua (entities cloned during phase callbacks)
    for cmd in lua_runtime.drain_clone_commands() {
        process_clone_command(&mut commands, cmd, &mut world_signals);
    }

    // Process entity commands from Lua (component manipulation)
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
