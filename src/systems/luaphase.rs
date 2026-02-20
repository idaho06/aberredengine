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
use bevy_ecs::system::{Local, SystemParam};
use mlua::prelude::*;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::group::Group;
use crate::components::luaphase::LuaPhase;
use crate::components::luatimer::LuaTimer;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::entityshader::EntityShader;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::events::audio::AudioCmd;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::{
    AnimationSnapshot, InputSnapshot, LuaPhaseSnapshot, LuaRuntime, LuaTimerSnapshot,
    RigidBodySnapshot, SpriteSnapshot, build_entity_context_pooled,
};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    process_audio_command, process_camera_command, process_clone_command, process_entity_commands,
    process_phase_command, process_signal_command, process_spawn_command,
};
use log::{error, warn};

/// Bundled read-only queries for building entity context.
///
/// This SystemParam bundles component queries that don't conflict with
/// the mutable queries used for command processing.
/// Note: Signals, RigidBody, MapPosition, and Animation are NOT included here
/// because they conflict with the mutable queries in the main system.
#[derive(SystemParam)]
pub struct ContextQueries<'w, 's> {
    pub groups: Query<'w, 's, &'static Group>,
    pub screen_positions: Query<'w, 's, &'static ScreenPosition>,
    pub rotations: Query<'w, 's, &'static Rotation>,
    pub scales: Query<'w, 's, &'static Scale>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub sprites: Query<'w, 's, &'static Sprite>,
    pub lua_timers: Query<'w, 's, &'static LuaTimer>,
}

/// Build entity context for phase callbacks using pooled tables.
///
/// Gathers all component data for the given entity and builds a Lua context table.
/// Uses pooled tables to reduce allocations.
/// Note: Some queries are passed separately because they conflict with mutable queries
/// used for command processing.
#[allow(clippy::too_many_arguments)]
fn build_phase_context(
    lua_runtime: &LuaRuntime,
    entity: Entity,
    lua_phase: &LuaPhase,
    previous_phase: Option<&str>,
    ctx_queries: &ContextQueries,
    // These are passed separately because they conflict with mutable queries
    positions_query: &Query<&mut MapPosition>,
    rigid_bodies_query: &Query<&mut RigidBody>,
    animation_query: &Query<&mut Animation>,
    signals_query: &Query<&mut Signals>,
) -> LuaResult<LuaTable> {
    let lua = lua_runtime.lua();
    let tables = lua_runtime.get_entity_ctx_pool()?;
    let entity_id = entity.to_bits();

    // Query all optional components
    let group = ctx_queries.groups.get(entity).ok().map(|g| g.name());

    let map_pos = positions_query.get(entity).ok().map(|p| (p.pos.x, p.pos.y));

    let screen_pos = ctx_queries
        .screen_positions
        .get(entity)
        .ok()
        .map(|p| (p.pos.x, p.pos.y));

    let rigid_body = rigid_bodies_query
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
        positions_query.get(entity).ok().map(|pos| {
            let rect = bc.as_rectangle(pos.pos);
            (rect.x, rect.y, rect.width, rect.height)
        })
    });

    let sprite = ctx_queries
        .sprites
        .get(entity)
        .ok()
        .map(|s| SpriteSnapshot {
            tex_key: s.tex_key.as_ref(),
            flip_h: s.flip_h,
            flip_v: s.flip_v,
        });

    let animation = animation_query.get(entity).ok().map(|a| AnimationSnapshot {
        key: a.animation_key.as_str(),
        frame_index: a.frame_index,
        elapsed: a.elapsed_time,
    });

    let signals_ref = signals_query.get(entity).ok();

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
            callback: t.callback.as_str(),
        });

    build_entity_context_pooled(
        lua,
        &tables,
        entity_id,
        group,
        map_pos,
        screen_pos,
        rigid_body.as_ref(),
        rotation,
        scale,
        rect,
        sprite.as_ref(),
        animation.as_ref(),
        signals_ref,
        lua_phase_snapshot.as_ref(),
        lua_timer.as_ref(),
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
    // Mutable queries for command processing
    stuckto_query: Query<&StuckTo>,
    mut signals_query: Query<&mut Signals>,
    mut animation_query: Query<&mut Animation>,
    mut rigid_bodies_query: Query<&mut RigidBody>,
    mut positions_query: Query<&mut MapPosition>,
    mut shader_query: Query<&mut EntityShader>,
    // Resources
    time: Res<WorldTime>,
    input: Res<InputState>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    systems_store: Res<SystemsStore>,
    // Local resource to avoid per-frame allocation for callback return transitions
    mut callback_transitions: Local<Vec<(u64, String)>>,
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

    for (entity, mut lua_phase) in query.iter_mut() {
        let entity_id = entity.to_bits();

        // Handle initial enter callback: (ctx, input) with previous_phase = nil
        if lua_phase.needs_enter_callback {
            lua_phase.needs_enter_callback = false;
            if let Some(callbacks) = lua_phase.get_callbacks(&lua_phase.current)
                && let Some(ref fn_name) = callbacks.on_enter
            {
                // Build context with no previous_phase
                match build_phase_context(
                    &lua_runtime,
                    entity,
                    &lua_phase,
                    None,
                    &ctx_queries,
                    &positions_query,
                    &rigid_bodies_query,
                    &animation_query,
                    &signals_query,
                ) {
                    Ok(ctx) => {
                        if let Some(next) = call_phase_enter(
                            &lua_runtime,
                            fn_name,
                            &ctx,
                            &input_table,
                            &lua_phase.current,
                        ) {
                            callback_transitions.push((entity_id, next));
                        }
                    }
                    Err(e) => error!("Error building context: {}", e),
                }
            }
        }

        // Handle pending transition
        if let Some(next_phase) = lua_phase.next.take() {
            let old_phase = std::mem::replace(&mut lua_phase.current, next_phase.clone());
            lua_phase.previous = Some(old_phase.clone());
            lua_phase.time_in_phase = 0.0;

            // Call exit callback for old phase: (ctx)
            if let Some(callbacks) = lua_phase.get_callbacks(&old_phase)
                && let Some(ref fn_name) = callbacks.on_exit
            {
                match build_phase_context(
                    &lua_runtime,
                    entity,
                    &lua_phase,
                    None,
                    &ctx_queries,
                    &positions_query,
                    &rigid_bodies_query,
                    &animation_query,
                    &signals_query,
                ) {
                    Ok(ctx) => call_phase_exit(&lua_runtime, fn_name, &ctx),
                    Err(e) => error!("Error building context: {}", e),
                }
            }

            // Call enter callback for new phase: (ctx, input) with previous_phase set
            if let Some(callbacks) = lua_phase.get_callbacks(&lua_phase.current)
                && let Some(ref fn_name) = callbacks.on_enter
            {
                match build_phase_context(
                    &lua_runtime,
                    entity,
                    &lua_phase,
                    Some(&old_phase),
                    &ctx_queries,
                    &positions_query,
                    &rigid_bodies_query,
                    &animation_query,
                    &signals_query,
                ) {
                    Ok(ctx) => {
                        if let Some(next) = call_phase_enter(
                            &lua_runtime,
                            fn_name,
                            &ctx,
                            &input_table,
                            &lua_phase.current,
                        ) {
                            callback_transitions.push((entity_id, next));
                        }
                    }
                    Err(e) => error!("Error building context: {}", e),
                }
            }
        }

        // Call update callback: (ctx, input, dt)
        if let Some(callbacks) = lua_phase.current_callbacks()
            && let Some(ref fn_name) = callbacks.on_update
        {
            match build_phase_context(
                &lua_runtime,
                entity,
                &lua_phase,
                None,
                &ctx_queries,
                &positions_query,
                &rigid_bodies_query,
                &animation_query,
                &signals_query,
            ) {
                Ok(ctx) => {
                    if let Some(next) = call_phase_update(
                        &lua_runtime,
                        fn_name,
                        &ctx,
                        &input_table,
                        delta,
                        &lua_phase.current,
                    ) {
                        callback_transitions.push((entity_id, next));
                    }
                }
                Err(e) => error!("Error building context: {}", e),
            }
        }

        // Increment time
        lua_phase.time_in_phase += delta;
    }

    // Process phase commands from Lua (from engine.phase_transition calls)
    for cmd in lua_runtime.drain_phase_commands() {
        process_phase_command(&mut query, cmd);
    }

    // Apply return value transitions (these take precedence over PhaseCmd)
    for (entity_id, next_phase) in (*callback_transitions).drain(..) {
        let entity = Entity::from_bits(entity_id);
        if let Ok((_, mut lua_phase)) = query.get_mut(entity) {
            lua_phase.next = Some(next_phase);
        }
    }

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
        &stuckto_query,
        &mut signals_query,
        &mut animation_query,
        &mut rigid_bodies_query,
        &mut positions_query,
        &mut shader_query,
        &systems_store,
    );

    // Process camera commands from Lua
    for cmd in lua_runtime.drain_camera_commands() {
        process_camera_command(&mut commands, cmd);
    }
}
