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
//! function callback_name(entity_id)
//!     -- entity_id is a u64 number
//!     -- Full access to engine API
//! end
//! ```

use bevy_ecs::prelude::*;

use crate::components::animation::Animation;
use crate::components::luaphase::LuaPhase;
use crate::components::luatimer::LuaTimer;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::components::stuckto::StuckTo;
use crate::events::audio::AudioCmd;
use crate::events::luatimer::LuaTimerEvent;
use crate::resources::lua_runtime::{LuaRuntime, PhaseCmd};
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    process_audio_command, process_camera_command, process_entity_commands, process_phase_command,
    process_signal_command, process_spawn_command,
};
use raylib::prelude::Vector2;

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

/// Observer that handles Lua timer events by calling Lua functions.
///
/// When a [`LuaTimerEvent`](crate::events::luatimer::LuaTimerEvent) is triggered:
///
/// 1. Checks if the Lua function exists
/// 2. Calls it with `(entity_id)` as parameter
/// 3. Processes all commands queued by the Lua function:
///    - Audio commands (play music/sounds)
///    - Signal commands (modify WorldSignals)
///    - Phase commands (trigger phase transitions)
///    - Spawn commands (create new entities)
///    - Entity commands (modify components)
///
/// If the Lua function doesn't exist, logs a warning but doesn't crash.
pub fn lua_timer_observer(
    trigger: On<LuaTimerEvent>,
    mut commands: Commands,
    stuckto_query: Query<&StuckTo>,
    mut signals_query: Query<&mut Signals>,
    mut animation_query: Query<&mut Animation>,
    mut rigid_bodies_query: Query<&mut RigidBody>,
    mut luaphase_query: Query<(Entity, &mut LuaPhase)>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    let event = trigger.event();
    let entity_id = event.entity.to_bits();

    // Update signal cache so Lua can read current values
    lua_runtime.update_signal_cache(world_signals.snapshot());

    // Call the Lua callback
    if lua_runtime.has_function(&event.callback) {
        if let Err(e) = lua_runtime.call_function::<_, ()>(&event.callback, entity_id) {
            eprintln!("[Lua] Error in {}(): {}", event.callback, e);
        }
    } else {
        eprintln!(
            "[Lua] Warning: timer callback '{}' not found",
            event.callback
        );
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

    // Process entity commands from Lua
    process_entity_commands(
        &mut commands,
        lua_runtime.drain_entity_commands(),
        &stuckto_query,
        &mut signals_query,
        &mut animation_query,
        &mut rigid_bodies_query,
    );

    // Process camera commands from Lua
    for cmd in lua_runtime.drain_camera_commands() {
        process_camera_command(&mut commands, cmd);
    }
}
