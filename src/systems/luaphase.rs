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
//! function my_enter_callback(entity_id, input, previous_phase)
//! function my_update_callback(entity_id, input, time_in_phase, dt)
//! function my_exit_callback(entity_id)
//! ```

use bevy_ecs::prelude::*;
use mlua::prelude::*;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::group::Group;
use crate::components::inputcontrolled::MouseControlled;
use crate::components::luaphase::{LuaPhase, PhaseCallbacks};
use crate::components::mapposition::MapPosition;
use crate::components::persistent::Persistent;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signalbinding::SignalBinding;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::components::timer::Timer;
use crate::components::zindex::ZIndex;
use crate::events::audio::AudioCmd;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::{InputSnapshot, LuaRuntime};
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    process_audio_command, process_camera_command, process_entity_commands, process_phase_command,
    process_signal_command, process_spawn_command,
};
use raylib::prelude::{Color, Vector2};

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
pub fn lua_phase_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut LuaPhase)>,
    stuckto_query: Query<&StuckTo>,
    mut signals_query: Query<&mut Signals>,
    mut animation_query: Query<&mut Animation>,
    mut rigid_bodies_query: Query<&mut RigidBody>,
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

    for (entity, mut lua_phase) in query.iter_mut() {
        let entity_id = entity.to_bits();

        // Handle initial enter callback: (entity_id, input, nil)
        if lua_phase.needs_enter_callback {
            lua_phase.needs_enter_callback = false;
            if let Some(callbacks) = lua_phase.get_callbacks(&lua_phase.current) {
                if let Some(ref fn_name) = callbacks.on_enter {
                    call_phase_enter(&lua_runtime, fn_name, entity_id, &input_table, None);
                }
            }
        }

        // Handle pending transition
        if let Some(next_phase) = lua_phase.next.take() {
            let old_phase = std::mem::replace(&mut lua_phase.current, next_phase.clone());
            lua_phase.previous = Some(old_phase.clone());
            lua_phase.time_in_phase = 0.0;

            // Call exit callback for old phase: (entity_id) - no input, housekeeping only
            if let Some(callbacks) = lua_phase.get_callbacks(&old_phase) {
                if let Some(ref fn_name) = callbacks.on_exit {
                    call_phase_exit(&lua_runtime, fn_name, entity_id);
                }
            }

            // Call enter callback for new phase: (entity_id, input, previous_phase)
            if let Some(callbacks) = lua_phase.get_callbacks(&lua_phase.current) {
                if let Some(ref fn_name) = callbacks.on_enter {
                    call_phase_enter(&lua_runtime, fn_name, entity_id, &input_table, Some(&old_phase));
                }
            }
        }

        // Call update callback: (entity_id, input, time_in_phase, dt)
        if let Some(callbacks) = lua_phase.current_callbacks() {
            if let Some(ref fn_name) = callbacks.on_update {
                call_phase_update(&lua_runtime, fn_name, entity_id, &input_table, lua_phase.time_in_phase, delta);
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
    );

    // Process camera commands from Lua
    for cmd in lua_runtime.drain_camera_commands() {
        process_camera_command(&mut commands, cmd);
    }
}
