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

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
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
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::events::audio::AudioCmd;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::{
    build_entity_context, AnimationSnapshot, InputSnapshot, LuaPhaseSnapshot, LuaRuntime,
    LuaTimerSnapshot, RigidBodySnapshot, SpriteSnapshot,
};
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    process_audio_command, process_camera_command, process_entity_commands, process_phase_command,
    process_signal_command, process_spawn_command,
};

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

/// Build entity context for phase callbacks.
///
/// Gathers all component data for the given entity and builds a Lua context table.
/// Note: Some queries are passed separately because they conflict with mutable queries
/// used for command processing.
#[allow(clippy::too_many_arguments)]
fn build_phase_context(
    lua: &Lua,
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
    let entity_id = entity.to_bits();

    // Query all optional components
    let group = ctx_queries.groups.get(entity).ok().map(|g| g.name());

    let map_pos = positions_query
        .get(entity)
        .ok()
        .map(|p| (p.pos.x, p.pos.y));

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

    let sprite = ctx_queries.sprites.get(entity).ok().map(|s| SpriteSnapshot {
        tex_key: s.tex_key.to_string(),
        flip_h: s.flip_h,
        flip_v: s.flip_v,
    });

    let animation = animation_query
        .get(entity)
        .ok()
        .map(|a| AnimationSnapshot {
            key: a.animation_key.clone(),
            frame_index: a.frame_index,
            elapsed: a.elapsed_time,
        });

    let signals_ref = signals_query.get(entity).ok();

    let lua_phase_snapshot = Some(LuaPhaseSnapshot {
        current: lua_phase.current.clone(),
        time_in_phase: lua_phase.time_in_phase,
    });

    let lua_timer = ctx_queries
        .lua_timers
        .get(entity)
        .ok()
        .map(|t| LuaTimerSnapshot {
            duration: t.duration,
            elapsed: t.elapsed,
            callback: t.callback.clone(),
        });

    build_entity_context(
        lua,
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
        signals_ref.as_deref(),
        lua_phase_snapshot.as_ref(),
        lua_timer.as_ref(),
        previous_phase,
    )
}

/// Call phase enter callback: (ctx, input)
fn call_phase_enter(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    ctx_table: &LuaTable,
    input_table: &LuaTable,
) {
    if lua_runtime.has_function(fn_name) {
        if let Err(e) =
            lua_runtime.call_function::<_, ()>(fn_name, (ctx_table.clone(), input_table.clone()))
        {
            eprintln!("[Lua] Error in {}(): {}", fn_name, e);
        }
    } else {
        eprintln!("[Lua] Warning: phase callback '{}' not found", fn_name);
    }
}

/// Call phase update callback: (ctx, input, dt)
fn call_phase_update(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    ctx_table: &LuaTable,
    input_table: &LuaTable,
    dt: f32,
) {
    if lua_runtime.has_function(fn_name) {
        if let Err(e) = lua_runtime
            .call_function::<_, ()>(fn_name, (ctx_table.clone(), input_table.clone(), dt))
        {
            eprintln!("[Lua] Error in {}(): {}", fn_name, e);
        }
    } else {
        eprintln!("[Lua] Warning: phase callback '{}' not found", fn_name);
    }
}

/// Call phase exit callback: (ctx)
fn call_phase_exit(lua_runtime: &LuaRuntime, fn_name: &str, ctx_table: &LuaTable) {
    if lua_runtime.has_function(fn_name) {
        if let Err(e) = lua_runtime.call_function::<_, ()>(fn_name, ctx_table.clone()) {
            eprintln!("[Lua] Error in {}(): {}", fn_name, e);
        }
    } else {
        eprintln!("[Lua] Warning: phase callback '{}' not found", fn_name);
    }
}

/// Call phase enter callback: (entity_id, input, previous_phase)
fn call_phase_enter(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    entity_id: u64,
    input_table: &LuaTable,
    previous_phase: Option<&str>,
) {
    if lua_runtime.has_function(fn_name) {
        let result = match previous_phase {
            Some(phase) => {
                lua_runtime.call_function::<_, ()>(fn_name, (entity_id, input_table.clone(), phase))
            }
            None => lua_runtime.call_function::<_, ()>(fn_name, (entity_id, input_table.clone(), LuaNil)),
        };
        if let Err(e) = result {
            eprintln!("[Lua] Error in {}(): {}", fn_name, e);
        }
    } else {
        eprintln!("[Lua] Warning: phase callback '{}' not found", fn_name);
    }
}

/// Call phase update callback: (entity_id, input, time_in_phase, dt)
fn call_phase_update(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    entity_id: u64,
    input_table: &LuaTable,
    time_in_phase: f32,
    dt: f32,
) {
    if lua_runtime.has_function(fn_name) {
        if let Err(e) = lua_runtime.call_function::<_, ()>(
            fn_name,
            (entity_id, input_table.clone(), time_in_phase, dt),
        ) {
            eprintln!("[Lua] Error in {}(): {}", fn_name, e);
        }
    } else {
        eprintln!("[Lua] Warning: phase callback '{}' not found", fn_name);
    }
}

/// Call phase exit callback: (entity_id) - no input, housekeeping only
fn call_phase_exit(lua_runtime: &LuaRuntime, fn_name: &str, entity_id: u64) {
    if lua_runtime.has_function(fn_name) {
        if let Err(e) = lua_runtime.call_function::<_, ()>(fn_name, entity_id) {
            eprintln!("[Lua] Error in {}(): {}", fn_name, e);
        }
    } else {
        eprintln!("[Lua] Warning: phase callback '{}' not found", fn_name);
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
    // Resources
    time: Res<WorldTime>,
    input: Res<InputState>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    // Update signal cache so Lua can read current values
    lua_runtime.update_signal_cache(world_signals.snapshot());

    // Create input snapshot once for all callbacks this frame
    let input_snapshot = InputSnapshot::from_input_state(&input);
    let input_table = match lua_runtime.create_input_table(&input_snapshot) {
        Ok(table) => table,
        Err(e) => {
            eprintln!("[Rust] Error creating input table for phase system: {}", e);
            return;
        }
    };

    let delta = time.delta;
    let lua = lua_runtime.lua();

    for (entity, mut lua_phase) in query.iter_mut() {
        // Handle initial enter callback: (ctx, input) with previous_phase = nil
        if lua_phase.needs_enter_callback {
            lua_phase.needs_enter_callback = false;
            if let Some(callbacks) = lua_phase.get_callbacks(&lua_phase.current) {
                if let Some(ref fn_name) = callbacks.on_enter {
                    // Build context with no previous_phase
                    match build_phase_context(
                        lua, entity, &lua_phase, None, &ctx_queries,
                        &positions_query, &rigid_bodies_query, &animation_query, &signals_query,
                    ) {
                        Ok(ctx) => call_phase_enter(&lua_runtime, fn_name, &ctx, &input_table),
                        Err(e) => eprintln!("[Rust] Error building context: {}", e),
                    }
                }
            }
        }

        // Handle pending transition
        if let Some(next_phase) = lua_phase.next.take() {
            let old_phase = std::mem::replace(&mut lua_phase.current, next_phase.clone());
            lua_phase.previous = Some(old_phase.clone());
            lua_phase.time_in_phase = 0.0;

            // Call exit callback for old phase: (ctx)
            if let Some(callbacks) = lua_phase.get_callbacks(&old_phase) {
                if let Some(ref fn_name) = callbacks.on_exit {
                    match build_phase_context(
                        lua, entity, &lua_phase, None, &ctx_queries,
                        &positions_query, &rigid_bodies_query, &animation_query, &signals_query,
                    ) {
                        Ok(ctx) => call_phase_exit(&lua_runtime, fn_name, &ctx),
                        Err(e) => eprintln!("[Rust] Error building context: {}", e),
                    }
                }
            }

            // Call enter callback for new phase: (ctx, input) with previous_phase set
            if let Some(callbacks) = lua_phase.get_callbacks(&lua_phase.current) {
                if let Some(ref fn_name) = callbacks.on_enter {
                    match build_phase_context(
                        lua, entity, &lua_phase, Some(&old_phase), &ctx_queries,
                        &positions_query, &rigid_bodies_query, &animation_query, &signals_query,
                    ) {
                        Ok(ctx) => call_phase_enter(&lua_runtime, fn_name, &ctx, &input_table),
                        Err(e) => eprintln!("[Rust] Error building context: {}", e),
                    }
                }
            }
        }

        // Call update callback: (ctx, input, dt)
        if let Some(callbacks) = lua_phase.current_callbacks() {
            if let Some(ref fn_name) = callbacks.on_update {
                match build_phase_context(
                    lua, entity, &lua_phase, None, &ctx_queries,
                    &positions_query, &rigid_bodies_query, &animation_query, &signals_query,
                ) {
                    Ok(ctx) => call_phase_update(&lua_runtime, fn_name, &ctx, &input_table, delta),
                    Err(e) => eprintln!("[Rust] Error building context: {}", e),
                }
            }
        }

        // Increment time
        lua_phase.time_in_phase += delta;
    }

    // Process phase commands from Lua
    for cmd in lua_runtime.drain_phase_commands() {
        process_phase_command(&mut query, cmd);
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

    // Process entity commands from Lua (component manipulation)
    process_entity_commands(
        &mut commands,
        lua_runtime.drain_entity_commands(),
        &stuckto_query,
        &mut signals_query,
        &mut animation_query,
        &mut rigid_bodies_query,
        &mut positions_query,
    );

    // Process camera commands from Lua
    for cmd in lua_runtime.drain_camera_commands() {
        process_camera_command(&mut commands, cmd);
    }
}
