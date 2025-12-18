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
use crate::components::luatimer::LuaTimer;
use crate::components::signals::Signals;
use crate::components::stuckto::StuckTo;
use crate::components::rigidbody::RigidBody;
use crate::events::audio::AudioCmd;
use crate::events::luatimer::LuaTimerEvent;
use crate::resources::lua_runtime::{
    AudioLuaCmd, EntityCmd, LuaRuntime, PhaseCmd, SignalCmd, SpawnCmd,
};
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
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
    trigger: Trigger<LuaTimerEvent>,
    mut commands: Commands,
    stuckto_query: Query<&StuckTo>,
    mut signals_query: Query<&mut Signals>,
    mut animation_query: Query<&mut Animation>,
    mut luaphase_query: Query<&mut crate::components::luaphase::LuaPhase>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    let event = trigger.event();
    let entity_id = event.entity.to_bits();

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
        match cmd {
            PhaseCmd::TransitionTo { entity_id, phase } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut lua_phase) = luaphase_query.get_mut(entity) {
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

    // Process spawn commands from Lua
    for cmd in lua_runtime.drain_spawn_commands() {
        process_spawn_cmd(&mut commands, cmd, &mut world_signals);
    }

    // Process entity commands from Lua
    for cmd in lua_runtime.drain_entity_commands() {
        match cmd {
            EntityCmd::ReleaseStuckTo { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(stuckto) = stuckto_query.get(entity) {
                    if let Some(velocity) = stuckto.stored_velocity {
                        commands.entity(entity).insert(RigidBody { velocity });
                    }
                }
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
            EntityCmd::InsertLuaTimer {
                entity_id,
                duration,
                callback,
            } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).insert(LuaTimer::new(duration, callback));
            }
            EntityCmd::RemoveLuaTimer { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<LuaTimer>();
            }
            EntityCmd::InsertTweenPosition {
                entity_id,
                from_x,
                from_y,
                to_x,
                to_y,
                duration,
                easing,
                loop_mode,
            } => {
                use crate::components::tween::TweenPosition;
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = parse_tween_easing(&easing);
                let parsed_loop = parse_tween_loop_mode(&loop_mode);
                commands.entity(entity).insert(
                    TweenPosition::new(
                        Vector2 { x: from_x, y: from_y },
                        Vector2 { x: to_x, y: to_y },
                        duration,
                    )
                    .with_easing(parsed_easing)
                    .with_loop_mode(parsed_loop),
                );
            }
            EntityCmd::InsertTweenRotation {
                entity_id,
                from,
                to,
                duration,
                easing,
                loop_mode,
            } => {
                use crate::components::tween::TweenRotation;
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = parse_tween_easing(&easing);
                let parsed_loop = parse_tween_loop_mode(&loop_mode);
                commands
                    .entity(entity)
                    .insert(TweenRotation::new(from, to, duration).with_easing(parsed_easing).with_loop_mode(parsed_loop));
            }
            EntityCmd::InsertTweenScale {
                entity_id,
                from_x,
                from_y,
                to_x,
                to_y,
                duration,
                easing,
                loop_mode,
            } => {
                use crate::components::tween::TweenScale;
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = parse_tween_easing(&easing);
                let parsed_loop = parse_tween_loop_mode(&loop_mode);
                commands.entity(entity).insert(
                    TweenScale::new(
                        Vector2 { x: from_x, y: from_y },
                        Vector2 { x: to_x, y: to_y },
                        duration,
                    )
                    .with_easing(parsed_easing)
                    .with_loop_mode(parsed_loop),
                );
            }
            EntityCmd::RemoveTweenPosition { entity_id } => {
                use crate::components::tween::TweenPosition;
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<TweenPosition>();
            }
            EntityCmd::RemoveTweenRotation { entity_id } => {
                use crate::components::tween::TweenRotation;
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<TweenRotation>();
            }
            EntityCmd::RemoveTweenScale { entity_id } => {
                use crate::components::tween::TweenScale;
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<TweenScale>();
            }
        }
    }
}

/// Parse easing string into Easing enum.
fn parse_tween_easing(easing: &str) -> crate::components::tween::Easing {
    use crate::components::tween::Easing;
    match easing {
        "linear" => Easing::Linear,
        "quad_in" => Easing::QuadIn,
        "quad_out" => Easing::QuadOut,
        "quad_in_out" => Easing::QuadInOut,
        "cubic_in" => Easing::CubicIn,
        "cubic_out" => Easing::CubicOut,
        "cubic_in_out" => Easing::CubicInOut,
        _ => Easing::Linear, // Default to linear for unknown
    }
}

/// Parse loop mode string into LoopMode enum.
fn parse_tween_loop_mode(loop_mode: &str) -> crate::components::tween::LoopMode {
    use crate::components::tween::LoopMode;
    match loop_mode {
        "once" => LoopMode::Once,
        "loop" => LoopMode::Loop,
        "ping_pong" => LoopMode::PingPong,
        _ => LoopMode::Once, // Default to once for unknown
    }
}

/// Process a spawn command from Lua and create the corresponding entity.
///
/// This is a copy of the function from lua_phase_system, used to process
/// entity spawns requested by Lua timer callbacks.
fn process_spawn_cmd(commands: &mut Commands, cmd: SpawnCmd, world_signals: &mut WorldSignals) {
    use crate::components::boxcollider::BoxCollider;
    use crate::components::dynamictext::DynamicText;
    use crate::components::group::Group;
    use crate::components::inputcontrolled::MouseControlled;
    use crate::components::luaphase::{LuaPhase, PhaseCallbacks};
    use crate::components::mapposition::MapPosition;
    use crate::components::persistent::Persistent;
    use crate::components::rotation::Rotation;
    use crate::components::scale::Scale;
    use crate::components::screenposition::ScreenPosition;
    use crate::components::signalbinding::SignalBinding;
    use crate::components::sprite::Sprite;
    use crate::components::timer::Timer;
    use crate::components::zindex::ZIndex;
    use raylib::prelude::Color;

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

    // LuaTimer
    if let Some((duration, callback)) = cmd.lua_timer {
        entity_commands.insert(LuaTimer::new(duration, callback));
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
