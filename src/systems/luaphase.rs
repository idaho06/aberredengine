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
//! function my_enter_callback(entity_id, previous_phase)
//! function my_update_callback(entity_id, time_in_phase)
//! function my_exit_callback(entity_id, next_phase)
//! ```

use bevy_ecs::prelude::*;

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
use crate::resources::lua_runtime::{
    AudioLuaCmd, EntityCmd, LuaRuntime, PhaseCmd, SignalCmd, SpawnCmd,
};
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use raylib::prelude::{Color, Vector2};

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
    time: Res<WorldTime>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    // Update signal cache so Lua can read current values
    let group_counts = world_signals.group_counts();
    let entities: rustc_hash::FxHashMap<String, u64> = world_signals
        .entities
        .iter()
        .map(|(k, v)| (k.clone(), v.to_bits()))
        .collect();
    lua_runtime.update_signal_cache(
        world_signals.scalars(),
        world_signals.integers(),
        world_signals.strings(),
        world_signals.flags(),
        &group_counts,
        &entities,
    );

    for (entity, mut lua_phase) in query.iter_mut() {
        let entity_id = entity.to_bits();

        // Handle initial enter callback
        if lua_phase.needs_enter_callback {
            lua_phase.needs_enter_callback = false;
            if let Some(callbacks) = lua_phase.get_callbacks(&lua_phase.current) {
                if let Some(ref fn_name) = callbacks.on_enter {
                    call_lua_callback(&lua_runtime, fn_name, entity_id, LuaNil);
                }
            }
        }

        // Handle pending transition
        if let Some(next_phase) = lua_phase.next.take() {
            let old_phase = std::mem::replace(&mut lua_phase.current, next_phase.clone());
            lua_phase.previous = Some(old_phase.clone());
            lua_phase.time_in_phase = 0.0;

            // Call exit callback for old phase
            if let Some(callbacks) = lua_phase.get_callbacks(&old_phase) {
                if let Some(ref fn_name) = callbacks.on_exit {
                    call_lua_callback(&lua_runtime, fn_name, entity_id, next_phase.as_str());
                }
            }

            // Call enter callback for new phase
            if let Some(callbacks) = lua_phase.get_callbacks(&lua_phase.current) {
                if let Some(ref fn_name) = callbacks.on_enter {
                    call_lua_callback(&lua_runtime, fn_name, entity_id, old_phase.as_str());
                }
            }
        }

        // Call update callback
        if let Some(callbacks) = lua_phase.current_callbacks() {
            if let Some(ref fn_name) = callbacks.on_update {
                call_lua_callback(&lua_runtime, fn_name, entity_id, lua_phase.time_in_phase);
            }
        }

        // Increment time
        lua_phase.time_in_phase += time.delta;
    }

    // Process phase commands from Lua
    for cmd in lua_runtime.drain_phase_commands() {
        match cmd {
            PhaseCmd::TransitionTo { entity_id, phase } => {
                // Find entity by ID and set its next phase
                let entity = Entity::from_bits(entity_id);
                if let Ok((_, mut lua_phase)) = query.get_mut(entity) {
                    lua_phase.next = Some(phase);
                }
            }
        }
    }

    // Process audio commands from Lua
    for cmd in lua_runtime.drain_audio_commands() {
        match cmd {
            AudioLuaCmd::PlayMusic { id, looped } => {
                audio_cmd_writer.write(AudioCmd::PlayMusic { id, looped });
            }
            AudioLuaCmd::PlaySound { id } => {
                audio_cmd_writer.write(AudioCmd::PlayFx { id });
            }
            AudioLuaCmd::StopAllMusic => {
                audio_cmd_writer.write(AudioCmd::StopAllMusic);
            }
            AudioLuaCmd::StopAllSounds => {
                audio_cmd_writer.write(AudioCmd::UnloadAllFx);
            }
        }
    }

    // Process signal commands from Lua
    for cmd in lua_runtime.drain_signal_commands() {
        match cmd {
            SignalCmd::SetScalar { key, value } => {
                world_signals.set_scalar(&key, value);
            }
            SignalCmd::SetInteger { key, value } => {
                world_signals.set_integer(&key, value);
            }
            SignalCmd::SetString { key, value } => {
                world_signals.set_string(&key, &value);
            }
            SignalCmd::SetFlag { key } => {
                world_signals.set_flag(&key);
            }
            SignalCmd::ClearFlag { key } => {
                world_signals.clear_flag(&key);
            }
        }
    }

    // Process spawn commands from Lua (entities spawned during phase callbacks)
    for cmd in lua_runtime.drain_spawn_commands() {
        process_spawn_cmd(&mut commands, cmd, &mut world_signals);
    }

    // Process entity commands from Lua (component manipulation)
    for cmd in lua_runtime.drain_entity_commands() {
        match cmd {
            EntityCmd::ReleaseStuckTo { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                // Get the stored velocity before removing StuckTo
                if let Ok(stuckto) = stuckto_query.get(entity) {
                    if let Some(velocity) = stuckto.stored_velocity {
                        // Add RigidBody with stored velocity
                        commands.entity(entity).insert(RigidBody { velocity });
                    }
                }
                // Remove StuckTo component
                commands.entity(entity).remove::<StuckTo>();
            }
            EntityCmd::SignalSetFlag { entity_id, flag } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = signals_query.get_mut(entity) {
                    signals.set_flag(&flag);
                }
            }
            EntityCmd::SignalClearFlag { entity_id, flag } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = signals_query.get_mut(entity) {
                    signals.clear_flag(&flag);
                }
            }
            EntityCmd::SetVelocity { entity_id, vx, vy } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).insert(RigidBody {
                    velocity: Vector2 { x: vx, y: vy },
                });
            }
            EntityCmd::InsertStuckTo {
                entity_id,
                target_id,
                follow_x,
                follow_y,
                offset_x,
                offset_y,
                stored_vx,
                stored_vy,
            } => {
                let entity = Entity::from_bits(entity_id);
                let target = Entity::from_bits(target_id);
                commands.entity(entity).insert(StuckTo {
                    target,
                    offset: Vector2 {
                        x: offset_x,
                        y: offset_y,
                    },
                    follow_x,
                    follow_y,
                    stored_velocity: Some(Vector2 {
                        x: stored_vx,
                        y: stored_vy,
                    }),
                });
                // Remove RigidBody when inserting StuckTo
                commands.entity(entity).remove::<RigidBody>();
            }
            EntityCmd::RestartAnimation { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut animation) = animation_query.get_mut(entity) {
                    animation.frame_index = 0;
                    animation.elapsed_time = 0.0;
                }
            }
            EntityCmd::SetAnimation {
                entity_id,
                animation_key,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut animation) = animation_query.get_mut(entity) {
                    animation.animation_key = animation_key;
                    animation.frame_index = 0;
                    animation.elapsed_time = 0.0;
                }
            }
        }
    }
}

use mlua::IntoLua;
use mlua::Nil as LuaNil;

/// Call a named Lua function with (entity_id, arg2).
fn call_lua_callback<'lua, T: IntoLua>(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    entity_id: u64,
    arg2: T,
) {
    if lua_runtime.has_function(fn_name) {
        if let Err(e) = lua_runtime.call_function::<_, ()>(fn_name, (entity_id, arg2)) {
            eprintln!("[Lua] Error in {}(): {}", fn_name, e);
        }
    } else {
        eprintln!("[Lua] Warning: phase callback '{}' not found", fn_name);
    }
}
/// Processes a SpawnCmd from Lua and spawns the corresponding entity.
fn process_spawn_cmd(commands: &mut Commands, cmd: SpawnCmd, world_signals: &mut WorldSignals) {
    let mut entity_commands = commands.spawn_empty();
    let entity = entity_commands.id();

    // Group
    if let Some(group_name) = cmd.group {
        entity_commands.insert(Group::new(&group_name));
    }

    // Position
    if let Some((x, y)) = cmd.position {
        entity_commands.insert(MapPosition::new(x, y));
    }

    // Sprite
    if let Some(sprite_data) = cmd.sprite {
        entity_commands.insert(Sprite {
            tex_key: sprite_data.tex_key,
            width: sprite_data.width,
            height: sprite_data.height,
            origin: Vector2 {
                x: sprite_data.origin_x,
                y: sprite_data.origin_y,
            },
            offset: Vector2 {
                x: sprite_data.offset_x,
                y: sprite_data.offset_y,
            },
            flip_h: sprite_data.flip_h,
            flip_v: sprite_data.flip_v,
        });
    }

    // ZIndex
    if let Some(z) = cmd.zindex {
        entity_commands.insert(ZIndex(z));
    }

    // RigidBody
    if let Some(rb_data) = cmd.rigidbody {
        entity_commands.insert(RigidBody {
            velocity: Vector2 {
                x: rb_data.velocity_x,
                y: rb_data.velocity_y,
            },
        });
    }

    // BoxCollider
    if let Some(collider_data) = cmd.collider {
        entity_commands.insert(BoxCollider {
            size: Vector2 {
                x: collider_data.width,
                y: collider_data.height,
            },
            offset: Vector2 {
                x: collider_data.offset_x,
                y: collider_data.offset_y,
            },
            origin: Vector2 {
                x: collider_data.origin_x,
                y: collider_data.origin_y,
            },
        });
    }

    // MouseControlled
    if let Some((follow_x, follow_y)) = cmd.mouse_controlled {
        entity_commands.insert(MouseControlled { follow_x, follow_y });
    }

    // Rotation
    if let Some(degrees) = cmd.rotation {
        entity_commands.insert(Rotation { degrees });
    }

    // Scale
    if let Some((sx, sy)) = cmd.scale {
        entity_commands.insert(Scale {
            scale: Vector2 { x: sx, y: sy },
        });
    }

    // Persistent
    if cmd.persistent {
        entity_commands.insert(Persistent);
    }

    // Signals
    if cmd.has_signals
        || !cmd.signal_scalars.is_empty()
        || !cmd.signal_integers.is_empty()
        || !cmd.signal_flags.is_empty()
        || !cmd.signal_strings.is_empty()
    {
        let mut signals = Signals::default();
        for (key, value) in cmd.signal_scalars {
            signals.set_scalar(&key, value);
        }
        for (key, value) in cmd.signal_integers {
            signals.set_integer(&key, value);
        }
        for flag in cmd.signal_flags {
            signals.set_flag(&flag);
        }
        for (key, value) in cmd.signal_strings {
            signals.set_string(&key, &value);
        }
        entity_commands.insert(signals);
    }

    // ScreenPosition (for UI elements)
    if let Some((x, y)) = cmd.screen_position {
        entity_commands.insert(ScreenPosition::new(x, y));
    }

    // DynamicText
    if let Some(text_data) = cmd.text {
        entity_commands.insert(DynamicText::new(
            text_data.content,
            text_data.font,
            text_data.font_size,
            Color::new(text_data.r, text_data.g, text_data.b, text_data.a),
        ));
    }

    // LuaPhase
    if let Some(phase_data) = cmd.phase_data {
        // Convert PhaseCallbackData to PhaseCallbacks
        let phases = phase_data
            .phases
            .into_iter()
            .map(|(name, data)| {
                (
                    name,
                    PhaseCallbacks {
                        on_enter: data.on_enter,
                        on_update: data.on_update,
                        on_exit: data.on_exit,
                    },
                )
            })
            .collect();
        entity_commands.insert(LuaPhase::new(phase_data.initial, phases));
    }

    // StuckTo
    if let Some(stuckto_data) = cmd.stuckto {
        let target = Entity::from_bits(stuckto_data.target_entity_id);
        let mut stuckto = StuckTo::new(target);
        stuckto.offset = Vector2 {
            x: stuckto_data.offset_x,
            y: stuckto_data.offset_y,
        };
        stuckto.follow_x = stuckto_data.follow_x;
        stuckto.follow_y = stuckto_data.follow_y;
        stuckto.stored_velocity = stuckto_data
            .stored_velocity
            .map(|(vx, vy)| Vector2 { x: vx, y: vy });
        entity_commands.insert(stuckto);
    }

    // Timer
    if let Some((duration, signal)) = cmd.timer {
        entity_commands.insert(Timer::new(duration, signal));
    }

    // SignalBinding
    if let Some((key, format)) = cmd.signal_binding {
        let mut binding = SignalBinding::new(&key);
        if let Some(fmt) = format {
            binding = binding.with_format(fmt);
        }
        entity_commands.insert(binding);
    }

    // GridLayout
    if let Some((path, group, zindex)) = cmd.grid_layout {
        use crate::components::gridlayout::GridLayout;
        entity_commands.insert(GridLayout::new(path, group, zindex));
    }

    // Register entity in WorldSignals if requested
    if let Some(key) = cmd.register_as {
        world_signals.set_entity(&key, entity);
    }
}
