//! Shared command processing utilities for Lua-Rust communication.
//!
//! This module provides unified command processors used by various Lua callback
//! contexts (scene setup, phase callbacks, timer callbacks, etc.).
//!
//! # Command Types
//!
//! - [`EntityCmd`](crate::resources::lua_runtime::EntityCmd) – Runtime entity manipulation
//! - [`SpawnCmd`](crate::resources::lua_runtime::SpawnCmd) – Entity spawning
//!
//! # Functions
//!
//! - [`process_entity_commands`] – Process all EntityCmd variants
//! - [`process_spawn_command`] – Process a single SpawnCmd to create an entity
//! - [`process_signal_command`] – Process a single signal command
//! - [`process_phase_command`] – Process a single phase command
//! - [`process_audio_command`] – Process a single audio command
//! - [`parse_tween_easing`] – Convert string to Easing enum
//! - [`parse_tween_loop_mode`] – Convert string to LoopMode enum

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::group::Group;
use crate::components::luaphase::{LuaPhase, PhaseCallbacks};
use crate::components::luatimer::LuaTimer;
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
use crate::components::tween::{Easing, LoopMode, TweenPosition, TweenRotation, TweenScale};
use crate::components::zindex::ZIndex;
use crate::events::audio::AudioCmd;
use crate::resources::lua_runtime::{AudioLuaCmd, EntityCmd, SignalCmd, SpawnCmd};
use crate::resources::worldsignals::WorldSignals;
use raylib::prelude::Color;

/// Process a single audio command from Lua and write to the audio command channel.
///
/// This function converts Lua audio commands (AudioLuaCmd) into engine audio
/// commands (AudioCmd) and writes them to the message channel for processing
/// by the audio system.
///
/// # Parameters
///
/// - `audio_cmd_writer` - MessageWriter for sending AudioCmd messages
/// - `cmd` - The AudioLuaCmd to process
pub fn process_audio_command(audio_cmd_writer: &mut MessageWriter<AudioCmd>, cmd: AudioLuaCmd) {
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

pub fn process_signal_command(world_signals: &mut WorldSignals, cmd: SignalCmd) {
    match cmd {
        SignalCmd::SetScalar { key, value } => {
            world_signals.set_scalar(&key, value);
        }
        SignalCmd::SetInteger { key, value } => {
            world_signals.set_integer(&key, value);
        }
        SignalCmd::SetFlag { key } => {
            world_signals.set_flag(&key);
        }
        SignalCmd::ClearFlag { key } => {
            world_signals.clear_flag(&key);
        }
        SignalCmd::SetString { key, value } => {
            world_signals.set_string(&key, &value);
        }
    }
}

/// Process a single phase command from Lua and apply it to the appropriate entity.
///
/// This function converts Lua phase commands (PhaseCmd) into entity state changes
/// by updating the LuaPhase component's next phase field.
///
/// # Parameters
///
/// - `luaphase_query` - Query for accessing and modifying LuaPhase components
/// - `cmd` - The PhaseCmd to process
pub fn process_phase_command(
    luaphase_query: &mut Query<(Entity, &mut crate::components::luaphase::LuaPhase)>,
    cmd: crate::resources::lua_runtime::PhaseCmd,
) {
    match cmd {
        crate::resources::lua_runtime::PhaseCmd::TransitionTo { entity_id, phase } => {
            let entity = Entity::from_bits(entity_id);
            if let Ok((_, mut lua_phase)) = luaphase_query.get_mut(entity) {
                lua_phase.next = Some(phase);
            }
        }
    }
}

/// Process all EntityCmd commands queued by Lua.
///
/// This function handles all runtime entity manipulation commands including:
/// - Component insertion/removal (StuckTo, LuaTimer, Tweens)
/// - Entity state changes (velocity, animation, signals)
///
/// # Parameters
///
/// - `commands` - Bevy Commands for entity manipulation
/// - `entity_commands` - Iterator of EntityCmd variants to process
/// - `stuckto_query` - Query for reading StuckTo components
/// - `signals_query` - Query for modifying Signals components
/// - `animation_query` - Query for modifying Animation components
pub fn process_entity_commands(
    commands: &mut Commands,
    entity_commands: impl IntoIterator<Item = EntityCmd>,
    stuckto_query: &Query<&StuckTo>,
    signals_query: &mut Query<&mut Signals>,
    animation_query: &mut Query<&mut Animation>,
) {
    for cmd in entity_commands {
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
                commands
                    .entity(entity)
                    .insert(LuaTimer::new(duration, callback));
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
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = parse_tween_easing(&easing);
                let parsed_loop = parse_tween_loop_mode(&loop_mode);
                commands.entity(entity).insert(
                    TweenPosition::new(
                        Vector2 {
                            x: from_x,
                            y: from_y,
                        },
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
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = parse_tween_easing(&easing);
                let parsed_loop = parse_tween_loop_mode(&loop_mode);
                commands.entity(entity).insert(
                    TweenRotation::new(from, to, duration)
                        .with_easing(parsed_easing)
                        .with_loop_mode(parsed_loop),
                );
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
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = parse_tween_easing(&easing);
                let parsed_loop = parse_tween_loop_mode(&loop_mode);
                commands.entity(entity).insert(
                    TweenScale::new(
                        Vector2 {
                            x: from_x,
                            y: from_y,
                        },
                        Vector2 { x: to_x, y: to_y },
                        duration,
                    )
                    .with_easing(parsed_easing)
                    .with_loop_mode(parsed_loop),
                );
            }
            EntityCmd::RemoveTweenPosition { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<TweenPosition>();
            }
            EntityCmd::RemoveTweenRotation { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<TweenRotation>();
            }
            EntityCmd::RemoveTweenScale { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<TweenScale>();
            }
        }
    }
}

/// Process a spawn command from Lua and create the corresponding entity.
///
/// This function creates a new entity with all components specified in the
/// SpawnCmd. It handles component insertion, signals, and entity registration.
///
/// # Parameters
///
/// - `commands` - Bevy Commands for entity creation
/// - `cmd` - The SpawnCmd containing all entity configuration
/// - `world_signals` - WorldSignals for entity registration
pub fn process_spawn_command(
    commands: &mut Commands,
    cmd: SpawnCmd,
    world_signals: &mut WorldSignals,
) {
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
        use crate::components::inputcontrolled::MouseControlled;
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

    // Menu (Menu + MenuActions)
    if let Some(menu_data) = cmd.menu {
        use crate::components::menu::{Menu, MenuAction, MenuActions};
        let labels: Vec<(&str, &str)> = menu_data
            .items
            .iter()
            .map(|(id, label)| (id.as_str(), label.as_str()))
            .collect();

        let mut menu = Menu::new(
            &labels,
            Vector2 {
                x: menu_data.origin_x,
                y: menu_data.origin_y,
            },
            menu_data.font,
            menu_data.font_size,
            menu_data.item_spacing,
            menu_data.use_screen_space,
        );

        if let (Some(normal), Some(selected)) = (menu_data.normal_color, menu_data.selected_color) {
            menu = menu.with_colors(
                Color::new(normal.r, normal.g, normal.b, normal.a),
                Color::new(selected.r, selected.g, selected.b, selected.a),
            );
        }

        if let Some(dynamic) = menu_data.dynamic_text {
            menu = menu.with_dynamic_text(dynamic);
        }

        if let Some(sound) = menu_data.selection_change_sound {
            menu = menu.with_selection_sound(sound);
        }

        if let Some(cursor_key) = menu_data.cursor_entity_key {
            if let Some(cursor_entity) = world_signals.get_entity(&cursor_key).copied() {
                menu = menu.with_cursor(cursor_entity);
            } else {
                eprintln!(
                    "[Rust] Menu cursor entity key '{}' not found in WorldSignals",
                    cursor_key
                );
            }
        }

        let mut actions = MenuActions::new();
        for (item_id, action_data) in menu_data.actions {
            let action = match action_data {
                crate::resources::lua_runtime::MenuActionData::SetScene { scene } => {
                    MenuAction::SetScene(scene)
                }
                crate::resources::lua_runtime::MenuActionData::ShowSubMenu { menu } => {
                    MenuAction::ShowSubMenu(menu)
                }
                crate::resources::lua_runtime::MenuActionData::QuitGame => MenuAction::QuitGame,
            };
            actions = actions.with(item_id, action);
        }

        entity_commands.insert((menu, actions));
    }

    // LuaCollisionRule
    if let Some(rule_data) = cmd.lua_collision_rule {
        use crate::components::luacollision::LuaCollisionRule;
        entity_commands.insert(LuaCollisionRule::new(
            rule_data.group_a,
            rule_data.group_b,
            rule_data.callback,
        ));
    }

    // Animation
    if let Some(anim_data) = cmd.animation {
        entity_commands.insert(Animation::new(anim_data.animation_key));
    }

    // AnimationController
    if let Some(controller_data) = cmd.animation_controller {
        use crate::components::animation::AnimationController;
        let mut controller = AnimationController::new(&controller_data.fallback_key);
        for rule in controller_data.rules {
            let condition = convert_animation_condition(rule.condition);
            controller = controller.with_rule(condition, rule.set_key);
        }
        entity_commands.insert(controller);
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

    // TweenPosition
    if let Some(tween_data) = cmd.tween_position {
        let easing = parse_tween_easing(&tween_data.easing);
        let loop_mode = parse_tween_loop_mode(&tween_data.loop_mode);
        entity_commands.insert(
            TweenPosition::new(
                Vector2 {
                    x: tween_data.from_x,
                    y: tween_data.from_y,
                },
                Vector2 {
                    x: tween_data.to_x,
                    y: tween_data.to_y,
                },
                tween_data.duration,
            )
            .with_easing(easing)
            .with_loop_mode(loop_mode),
        );
    }

    // TweenRotation
    if let Some(tween_data) = cmd.tween_rotation {
        let easing = parse_tween_easing(&tween_data.easing);
        let loop_mode = parse_tween_loop_mode(&tween_data.loop_mode);
        entity_commands.insert(
            TweenRotation::new(tween_data.from, tween_data.to, tween_data.duration)
                .with_easing(easing)
                .with_loop_mode(loop_mode),
        );
    }

    // TweenScale
    if let Some(tween_data) = cmd.tween_scale {
        let easing = parse_tween_easing(&tween_data.easing);
        let loop_mode = parse_tween_loop_mode(&tween_data.loop_mode);
        entity_commands.insert(
            TweenScale::new(
                Vector2 {
                    x: tween_data.from_x,
                    y: tween_data.from_y,
                },
                Vector2 {
                    x: tween_data.to_x,
                    y: tween_data.to_y,
                },
                tween_data.duration,
            )
            .with_easing(easing)
            .with_loop_mode(loop_mode),
        );
    }

    // Register entity in WorldSignals if requested
    if let Some(key) = cmd.register_as {
        world_signals.set_entity(&key, entity);
    }
}

/// Parse easing string into Easing enum.
///
/// Converts string representations like "linear", "quad_in", etc. into the
/// corresponding Easing variant. Unknown strings default to Linear.
pub fn parse_tween_easing(easing: &str) -> Easing {
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
///
/// Converts string representations like "once", "loop", "ping_pong" into the
/// corresponding LoopMode variant. Unknown strings default to Once.
pub fn parse_tween_loop_mode(loop_mode: &str) -> LoopMode {
    match loop_mode {
        "once" => LoopMode::Once,
        "loop" => LoopMode::Loop,
        "ping_pong" => LoopMode::PingPong,
        _ => LoopMode::Once, // Default to once for unknown
    }
}

/// Parse comparison operator string into CmpOp enum.
fn parse_cmp_op(op: &str) -> crate::components::animation::CmpOp {
    use crate::components::animation::CmpOp;
    match op {
        "lt" => CmpOp::Lt,
        "le" => CmpOp::Le,
        "gt" => CmpOp::Gt,
        "ge" => CmpOp::Ge,
        "eq" => CmpOp::Eq,
        "ne" => CmpOp::Ne,
        _ => CmpOp::Eq,
    }
}

/// Convert AnimationConditionData from Lua into Condition enum.
fn convert_animation_condition(
    data: crate::resources::lua_runtime::AnimationConditionData,
) -> crate::components::animation::Condition {
    use crate::components::animation::Condition;
    use crate::resources::lua_runtime::AnimationConditionData;
    match data {
        AnimationConditionData::ScalarCmp { key, op, value } => Condition::ScalarCmp {
            key,
            op: parse_cmp_op(&op),
            value,
        },
        AnimationConditionData::ScalarRange {
            key,
            min,
            max,
            inclusive,
        } => Condition::ScalarRange {
            key,
            min,
            max,
            inclusive,
        },
        AnimationConditionData::IntegerCmp { key, op, value } => Condition::IntegerCmp {
            key,
            op: parse_cmp_op(&op),
            value,
        },
        AnimationConditionData::IntegerRange {
            key,
            min,
            max,
            inclusive,
        } => Condition::IntegerRange {
            key,
            min,
            max,
            inclusive,
        },
        AnimationConditionData::HasFlag { key } => Condition::HasFlag { key },
        AnimationConditionData::LacksFlag { key } => Condition::LacksFlag { key },
        AnimationConditionData::All(conditions) => Condition::All(
            conditions
                .into_iter()
                .map(convert_animation_condition)
                .collect(),
        ),
        AnimationConditionData::Any(conditions) => Condition::Any(
            conditions
                .into_iter()
                .map(convert_animation_condition)
                .collect(),
        ),
        AnimationConditionData::Not(inner) => {
            Condition::Not(Box::new(convert_animation_condition(*inner)))
        }
    }
}
