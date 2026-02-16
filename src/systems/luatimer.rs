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
use crate::components::entityshader::EntityShader;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::events::audio::AudioCmd;
use crate::events::luatimer::LuaTimerEvent;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::{
    build_entity_context_pooled, AnimationSnapshot, InputSnapshot, LuaPhaseSnapshot, LuaRuntime,
    LuaTimerSnapshot, RigidBodySnapshot, SpriteSnapshot,
};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    process_audio_command, process_camera_command, process_clone_command, process_entity_commands,
    process_phase_command, process_signal_command, process_spawn_command,
};
use log::{error, warn};

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
    for (entity, mut timer) in query.iter_mut() {
        timer.elapsed += world_time.delta;
        if timer.elapsed >= timer.duration {
            // Emit timer event
            commands.trigger(LuaTimerEvent {
                entity,
                callback: timer.callback.clone(),
            });
            // Reset timer (subtract duration instead of zeroing)
            timer.reset();
        }
    }
}

/// Bundled read-only queries for building entity context in timer callbacks.
///
/// This SystemParam bundles component queries that don't conflict with
/// the mutable queries used for command processing.
/// Note: Signals, RigidBody, MapPosition, Animation, and LuaPhase are NOT included here
/// because they conflict with the mutable queries in the observer.
#[derive(SystemParam)]
pub struct TimerContextQueries<'w, 's> {
    pub groups: Query<'w, 's, &'static Group>,
    pub screen_positions: Query<'w, 's, &'static ScreenPosition>,
    pub rotations: Query<'w, 's, &'static Rotation>,
    pub scales: Query<'w, 's, &'static Scale>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub sprites: Query<'w, 's, &'static Sprite>,
    pub lua_timers: Query<'w, 's, &'static LuaTimer>,
}

/// Build entity context for timer callbacks using pooled tables.
///
/// Gathers all component data for the given entity and builds a Lua context table.
/// Uses pooled tables to reduce allocations.
/// Note: Some queries are passed separately because they conflict with mutable queries
/// used for command processing.
#[allow(clippy::too_many_arguments)]
fn build_timer_context(
    lua_runtime: &LuaRuntime,
    entity: Entity,
    ctx_queries: &TimerContextQueries,
    // These are passed separately because they conflict with mutable queries
    positions_query: &Query<&mut MapPosition>,
    rigid_bodies_query: &Query<&mut RigidBody>,
    animation_query: &Query<&mut Animation>,
    signals_query: &Query<&mut Signals>,
    luaphase_query: &Query<(Entity, &mut LuaPhase)>,
) -> LuaResult<LuaTable> {
    let lua = lua_runtime.lua();
    let tables = lua_runtime.get_entity_ctx_pool()?;
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

    let lua_phase_snapshot = luaphase_query
        .get(entity)
        .ok()
        .map(|(_, p)| LuaPhaseSnapshot {
            current: p.current.clone(),
            time_in_phase: p.time_in_phase,
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
        None, // previous_phase is only for phase enter callbacks
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
    ctx_queries: TimerContextQueries,
    // Mutable queries for command processing
    stuckto_query: Query<&StuckTo>,
    mut signals_query: Query<&mut Signals>,
    mut animation_query: Query<&mut Animation>,
    mut rigid_bodies_query: Query<&mut RigidBody>,
    mut positions_query: Query<&mut MapPosition>,
    mut shader_query: Query<&mut EntityShader>,
    mut luaphase_query: Query<(Entity, &mut LuaPhase)>,
    // Resources
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    systems_store: Res<SystemsStore>,
) {
    let event = trigger.event();
    let entity = event.entity;

    // Update signal cache so Lua can read current values
    lua_runtime.update_signal_cache(world_signals.snapshot());

    // Create input snapshot and table
    let input_snapshot = InputSnapshot::from_input_state(&input);
    let input_table = match lua_runtime.create_input_table(&input_snapshot) {
        Ok(table) => table,
        Err(e) => {
            error!("Error creating input table for timer callback: {}", e);
            return;
        }
    };

    // Build entity context
    let ctx_table = match build_timer_context(
        &lua_runtime, entity, &ctx_queries,
        &positions_query, &rigid_bodies_query, &animation_query, &signals_query, &luaphase_query,
    ) {
        Ok(ctx) => ctx,
        Err(e) => {
            error!("Error building context for timer callback: {}", e);
            return;
        }
    };

    // Call the Lua callback with (ctx, input)
    if lua_runtime.has_function(&event.callback) {
        if let Err(e) =
            lua_runtime.call_function::<_, ()>(&event.callback, (ctx_table, input_table))
        {
            error!(target: "lua", "Error in {}(): {}", event.callback, e);
        }
    } else {
        warn!(target: "lua", "Timer callback '{}' not found", event.callback);
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
