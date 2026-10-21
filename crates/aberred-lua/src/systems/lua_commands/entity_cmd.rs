//! Runtime entity manipulation command processing.
//!
//! [`process_entity_commands`] dispatches all [`EntityCmd`] variants to modify
//! live entities — physics, signals, transforms, animation, shaders, tweens, etc.
//!
//! Every handler queues its work as a deferred `EntityWorldMut` command
//! ([`queue_on_entity`], [`queue_on_component`]), so one drained batch applies
//! in exactly the order the script made the calls, at the next command flush.
//! A handler never touches the world while it queues: anything it depends on
//! (an existing component, the `AnimationStore`) is read when the command
//! applies, so it sees the earlier commands of the same batch.

use log::warn;

use aberred_core::math::Vec2;
use bevy_ecs::component::Mutable;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;

use crate::components::lua_on_phase::{LuaOnPhaseEntered, LuaOnPhaseExited, LuaOnPhaseUpdate};
use crate::components::lua_on_timer_fired::LuaOnTimerFired;
use aberred_core::components::animation::Animation;
use aberred_core::components::cameratarget::CameraTarget;
use aberred_core::components::entityshader::EntityShader;
use aberred_core::components::globaltransform2d::GlobalTransform2D;
use aberred_core::components::guiinteractable::{GuiInteractable, GuiWidgetState};
use aberred_core::components::guiprogressbar::GuiProgressBar;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::phase::Phase;
use aberred_core::components::rigidbody::RigidBody;
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::components::screenposition::ScreenPosition;
use aberred_core::components::shadow::Shadow;
use aberred_core::components::signals::Signals;
use aberred_core::components::sprite::Sprite;
use aberred_core::components::stuckto::StuckTo;
use aberred_core::components::timer::Timer;
use aberred_core::components::tint::Tint;
use aberred_core::components::ttl::Ttl;
use aberred_core::components::tween::{Tween, TweenValue};
use aberred_core::systems::propagate_transforms::ComputeInitialGlobalTransform;
use bevy_ecs::system::EntityCommand;

use crate::resources::lua_runtime::{EntityCmd, LuaPhaseCallbackKind, TweenConfig, UniformValue};
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::systemsstore as hook_keys;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;

/// Resolve a Lua-supplied u64 entity ID, warning and returning None on invalid bits.
pub(super) fn resolve_entity(id: u64) -> Option<Entity> {
    match Entity::try_from_bits(id) {
        Some(entity) => Some(entity),
        None => {
            warn!("Invalid entity bits received from Lua script: {}", id);
            None
        }
    }
}

/// Queue `f` to run against `entity_id`'s `EntityWorldMut` at the next command
/// flush, in push order with every other queued command, so a write that
/// follows an insert in the same batch sees the inserted component.
///
/// No-ops (with a warn log) on invalid bits or an entity despawned in a prior
/// frame's flush. An entity despawned *earlier in the same drained batch* is
/// skipped silently at apply time.
fn queue_on_entity(
    commands: &mut Commands,
    entity_id: u64,
    f: impl FnOnce(EntityWorldMut) + Send + 'static,
) {
    if let Some(entity) = resolve_entity(entity_id) {
        queue_on(commands, entity, f);
    }
}

/// [`queue_on_entity`] for an already-resolved `Entity`.
pub(super) fn queue_on(
    commands: &mut Commands,
    entity: Entity,
    f: impl FnOnce(EntityWorldMut) + Send + 'static,
) {
    match commands.get_entity(entity) {
        Ok(mut entity_cmds) => {
            entity_cmds.queue_silenced(f);
        }
        Err(_) => warn!(
            "Cannot apply command to entity {:?}: entity was despawned",
            entity
        ),
    }
}

/// Drains the Lua entity command queue and dispatches each `EntityCmd` to the
/// matching ECS mutation (SetVelocity, SetAnimation, Despawn, etc.).
///
/// Nothing applies here: each command is queued on `commands` as deferred
/// entity work, in call order, and lands at the next command flush — right
/// after the observer for observer callbacks, at the next sync point after the
/// system for system callbacks. An entity despawned in an earlier frame is
/// warned about and skipped; one despawned earlier in the same batch is skipped
/// silently when the command applies. The one immediate effect is `Despawn`'s
/// removal of the entity's `WorldSignals` registrations, so a later command in
/// the same batch can't resolve a stale key.
///
/// `Commands` declare no component access, so the scheduler can't see these
/// writes: a system that reads a Lua-written component must be ordered after
/// the Lua drain (any ordering edge is a sync point). An unordered system in
/// the same set reads pre-flush state, and `assert_no_ambiguities` doesn't
/// flag it.
pub fn process_entity_commands(
    commands: &mut Commands,
    entity_commands: impl IntoIterator<Item = EntityCmd>,
    world_signals: &mut WorldSignals,
    systems_store: &SystemsStore,
) {
    aberred_core::tracy::tracy_span!("lua_entity_cmds");
    for cmd in entity_commands {
        match cmd {
            cmd @ (EntityCmd::SetVelocity { .. }
            | EntityCmd::SetSpeed { .. }
            | EntityCmd::SetFriction { .. }
            | EntityCmd::SetMaxSpeed { .. }
            | EntityCmd::FreezeEntity { .. }
            | EntityCmd::UnfreezeEntity { .. }
            | EntityCmd::AddForce { .. }
            | EntityCmd::RemoveForce { .. }
            | EntityCmd::SetForceEnabled { .. }
            | EntityCmd::SetForceValue { .. }) => process_physics_cmd(cmd, commands),

            cmd @ (EntityCmd::SignalSetFlag { .. }
            | EntityCmd::SignalClearFlag { .. }
            | EntityCmd::SignalToggleFlag { .. }
            | EntityCmd::SignalSetScalar { .. }
            | EntityCmd::SignalClearScalar { .. }
            | EntityCmd::SignalSetString { .. }
            | EntityCmd::SignalClearString { .. }
            | EntityCmd::SignalSetInteger { .. }
            | EntityCmd::SignalClearInteger { .. }) => process_signal_cmd(cmd, commands),

            cmd @ (EntityCmd::RestartAnimation { .. }
            | EntityCmd::SetAnimation { .. }
            | EntityCmd::SetSpriteFlip { .. }) => process_animation_cmd(cmd, commands),

            cmd @ (EntityCmd::InsertTweenPosition { .. }
            | EntityCmd::InsertTweenRotation { .. }
            | EntityCmd::InsertTweenScale { .. }
            | EntityCmd::InsertTweenScreenPosition { .. }
            | EntityCmd::RemoveTweenPosition { .. }
            | EntityCmd::RemoveTweenRotation { .. }
            | EntityCmd::RemoveTweenScale { .. }) => process_tween_cmd(cmd, commands),

            cmd @ (EntityCmd::SetShader { .. }
            | EntityCmd::RemoveShader { .. }
            | EntityCmd::ShaderSetFloat { .. }
            | EntityCmd::ShaderSetInt { .. }
            | EntityCmd::ShaderSetVec2 { .. }
            | EntityCmd::ShaderSetVec4 { .. }
            | EntityCmd::ShaderClearUniform { .. }
            | EntityCmd::ShaderClearUniforms { .. }
            | EntityCmd::SetTint { .. }
            | EntityCmd::RemoveTint { .. }
            | EntityCmd::SetShadow { .. }
            | EntityCmd::RemoveShadow { .. }) => process_shader_cmd(cmd, commands),

            cmd @ (EntityCmd::SetPosition { .. }
            | EntityCmd::SetScreenPosition { .. }
            | EntityCmd::RemoveScreenPosition { .. }
            | EntityCmd::SetRotation { .. }
            | EntityCmd::SetScale { .. }
            | EntityCmd::SetCameraTarget { .. }
            | EntityCmd::RemoveCameraTarget { .. }) => process_transform_cmd(cmd, commands),

            cmd @ (EntityCmd::SetParent { .. }
            | EntityCmd::RemoveParent { .. }
            | EntityCmd::InsertStuckTo { .. }
            | EntityCmd::ReleaseStuckTo { .. }) => process_hierarchy_cmd(cmd, commands),

            cmd @ (EntityCmd::InsertLuaTimer { .. }
            | EntityCmd::RemoveLuaTimer { .. }
            | EntityCmd::InsertPhase { .. }
            | EntityCmd::InsertLuaPhaseCallback { .. }
            | EntityCmd::Despawn { .. }
            | EntityCmd::MenuDespawn { .. }
            | EntityCmd::InsertTtl { .. }) => {
                process_lifecycle_cmd(cmd, commands, world_signals, systems_store)
            }

            EntityCmd::SetGuiDisabled {
                entity_id,
                disabled,
            } => {
                // Mutates `state` only — never inserts a fresh component, since
                // that would wipe the widget's `size`.
                queue_on_component::<GuiInteractable>(commands, entity_id, move |interactable| {
                    interactable.state = if disabled {
                        GuiWidgetState::Disabled
                    } else {
                        GuiWidgetState::Normal
                    };
                });
            }

            EntityCmd::SetGuiProgress { entity_id, value } => {
                queue_on_component::<GuiProgressBar>(commands, entity_id, move |bar| {
                    bar.value = value.clamp(0.0, bar.max);
                });
            }

            EntityCmd::SetGuiProgressMax { entity_id, max } => {
                queue_on_component::<GuiProgressBar>(commands, entity_id, move |bar| {
                    bar.max = max.max(0.0);
                    bar.value = bar.value.min(bar.max);
                });
            }
        }
    }
}

fn process_physics_cmd(cmd: EntityCmd, commands: &mut Commands) {
    match cmd {
        EntityCmd::SetVelocity { entity_id, vx, vy } => {
            queue_on_component::<RigidBody>(commands, entity_id, move |rb| {
                rb.velocity = Vec2 { x: vx, y: vy };
            });
        }
        EntityCmd::SetSpeed { entity_id, speed } => {
            queue_on_component::<RigidBody>(commands, entity_id, move |rb| rb.set_speed(speed));
        }
        EntityCmd::SetFriction {
            entity_id,
            friction,
        } => {
            queue_on_component::<RigidBody>(commands, entity_id, move |rb| rb.friction = friction);
        }
        EntityCmd::SetMaxSpeed {
            entity_id,
            max_speed,
        } => {
            queue_on_component::<RigidBody>(commands, entity_id, move |rb| {
                rb.max_speed = max_speed
            });
        }
        EntityCmd::FreezeEntity { entity_id } => {
            queue_on_component::<RigidBody>(commands, entity_id, RigidBody::freeze);
        }
        EntityCmd::UnfreezeEntity { entity_id } => {
            queue_on_component::<RigidBody>(commands, entity_id, RigidBody::unfreeze);
        }
        EntityCmd::AddForce {
            entity_id,
            name,
            x,
            y,
            enabled,
        } => {
            queue_on_component::<RigidBody>(commands, entity_id, move |rb| {
                rb.add_force_with_state(&name, Vec2 { x, y }, enabled);
            });
        }
        EntityCmd::RemoveForce { entity_id, name } => {
            queue_on_component::<RigidBody>(commands, entity_id, move |rb| {
                rb.remove_force(&name);
            });
        }
        EntityCmd::SetForceEnabled {
            entity_id,
            name,
            enabled,
        } => {
            queue_on_component::<RigidBody>(commands, entity_id, move |rb| {
                rb.set_force_enabled(&name, enabled);
            });
        }
        EntityCmd::SetForceValue {
            entity_id,
            name,
            x,
            y,
        } => {
            queue_on_component::<RigidBody>(commands, entity_id, move |rb| {
                rb.set_force_value(&name, Vec2 { x, y });
            });
        }
        _ => unreachable!(),
    }
}

/// [`queue_on_entity`] for a write to the entity's `C`; a no-op at apply time
/// if it has none (writes never insert a missing component).
fn queue_on_component<C: Component<Mutability = Mutable>>(
    commands: &mut Commands,
    entity_id: u64,
    f: impl FnOnce(&mut C) + Send + 'static,
) {
    queue_on_entity(commands, entity_id, move |mut e| {
        if let Some(mut c) = e.get_mut::<C>() {
            f(&mut c);
        }
    });
}

fn process_signal_cmd(cmd: EntityCmd, commands: &mut Commands) {
    match cmd {
        EntityCmd::SignalSetFlag { entity_id, flag } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| s.set_flag(flag));
        }
        EntityCmd::SignalClearFlag { entity_id, flag } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| {
                s.remove_flag(&flag);
            });
        }
        EntityCmd::SignalToggleFlag { entity_id, flag } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| s.toggle_flag(&flag));
        }
        EntityCmd::SignalSetScalar {
            entity_id,
            key,
            value,
        } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| s.set_scalar(key, value));
        }
        EntityCmd::SignalClearScalar { entity_id, key } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| {
                s.remove_scalar(&key);
            });
        }
        EntityCmd::SignalSetString {
            entity_id,
            key,
            value,
        } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| s.set_string(key, value));
        }
        EntityCmd::SignalClearString { entity_id, key } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| {
                s.remove_string(&key);
            });
        }
        EntityCmd::SignalSetInteger {
            entity_id,
            key,
            value,
        } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| s.set_integer(key, value));
        }
        EntityCmd::SignalClearInteger { entity_id, key } => {
            queue_on_component::<Signals>(commands, entity_id, move |s| {
                s.remove_integer(&key);
            });
        }
        _ => unreachable!(),
    }
}

fn process_animation_cmd(cmd: EntityCmd, commands: &mut Commands) {
    match cmd {
        EntityCmd::RestartAnimation { entity_id } => {
            queue_on_component::<Animation>(commands, entity_id, Animation::reset);
        }
        EntityCmd::SetAnimation {
            entity_id,
            animation_key,
        } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                // Also switch the sprite's texture to the new animation's sheet.
                let tex_key = e
                    .world()
                    .get_resource::<AnimationStore>()
                    .and_then(|store| store.animations.get(&animation_key))
                    .map(|anim| anim.tex_key.clone());
                if let Some(tex_key) = tex_key
                    && let Some(mut sprite) = e.get_mut::<Sprite>()
                {
                    sprite.tex_key = tex_key;
                }
                if let Some(mut animation) = e.get_mut::<Animation>() {
                    animation.animation_key = animation_key;
                    animation.reset();
                }
            });
        }
        EntityCmd::SetSpriteFlip {
            entity_id,
            flip_h,
            flip_v,
        } => {
            queue_on_component::<Sprite>(commands, entity_id, move |sprite| {
                sprite.flip_h = flip_h;
                sprite.flip_v = flip_v;
            });
        }
        _ => unreachable!(),
    }
}

fn process_tween_cmd(cmd: EntityCmd, commands: &mut Commands) {
    match cmd {
        EntityCmd::InsertTweenPosition {
            entity_id,
            from_x,
            from_y,
            to_x,
            to_y,
            config,
        } => insert_tween(
            commands,
            entity_id,
            MapPosition::from_vec(Vec2 {
                x: from_x,
                y: from_y,
            }),
            MapPosition::from_vec(Vec2 { x: to_x, y: to_y }),
            &config,
        ),
        EntityCmd::InsertTweenRotation {
            entity_id,
            from,
            to,
            config,
        } => insert_tween(
            commands,
            entity_id,
            Rotation { degrees: from },
            Rotation { degrees: to },
            &config,
        ),
        EntityCmd::InsertTweenScale {
            entity_id,
            from_x,
            from_y,
            to_x,
            to_y,
            config,
        } => insert_tween(
            commands,
            entity_id,
            Scale::new(from_x, from_y),
            Scale::new(to_x, to_y),
            &config,
        ),
        EntityCmd::InsertTweenScreenPosition {
            entity_id,
            from_x,
            from_y,
            to_x,
            to_y,
            config,
        } => {
            // Unlike MapPosition/Rotation/Scale, ScreenPosition is not
            // guaranteed to already exist — its presence/absence is the GUI
            // visibility toggle, so a hidden entity has none. Insert it
            // (seeded at `from`) alongside the tween in the same batch.
            let from = ScreenPosition::from_vec(Vec2 {
                x: from_x,
                y: from_y,
            });
            let to = ScreenPosition::from_vec(Vec2 { x: to_x, y: to_y });
            let tween = super::build_tween(from, to, &config);
            let set_callback = super::tween_finished_callback::<ScreenPosition>(&config);
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert((from, tween));
                set_callback(e);
            });
        }
        EntityCmd::RemoveTweenPosition { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<Tween<MapPosition>>();
            });
        }
        EntityCmd::RemoveTweenRotation { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<Tween<Rotation>>();
            });
        }
        EntityCmd::RemoveTweenScale { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<Tween<Scale>>();
            });
        }
        _ => unreachable!(),
    }
}

fn insert_tween<T>(commands: &mut Commands, entity_id: u64, from: T, to: T, config: &TweenConfig)
where
    T: TweenValue + Send + Sync + 'static,
{
    let tween = super::build_tween(from, to, config);
    let set_callback = super::tween_finished_callback::<T>(config);
    queue_on_entity(commands, entity_id, move |mut e| {
        e.insert(tween);
        set_callback(e);
    });
}

fn process_shader_cmd(cmd: EntityCmd, commands: &mut Commands) {
    match cmd {
        EntityCmd::SetShader { entity_id, key } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(EntityShader::new(key));
            });
        }
        EntityCmd::RemoveShader { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<EntityShader>();
            });
        }
        cmd @ (EntityCmd::ShaderSetFloat { .. }
        | EntityCmd::ShaderSetInt { .. }
        | EntityCmd::ShaderSetVec2 { .. }
        | EntityCmd::ShaderSetVec4 { .. }) => shader_set_uniform(cmd, commands),
        EntityCmd::ShaderClearUniform { entity_id, name } => {
            queue_on_component::<EntityShader>(commands, entity_id, move |shader| {
                shader.uniforms_mut().remove(name.as_str());
            });
        }
        EntityCmd::ShaderClearUniforms { entity_id } => {
            queue_on_component::<EntityShader>(commands, entity_id, |shader| {
                shader.uniforms_mut().clear();
            });
        }
        EntityCmd::SetTint {
            entity_id,
            r,
            g,
            b,
            a,
        } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(Tint::new(r, g, b, a));
            });
        }
        EntityCmd::RemoveTint { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<Tint>();
            });
        }
        EntityCmd::SetShadow {
            entity_id,
            dx,
            dy,
            r,
            g,
            b,
            a,
        } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(Shadow::new(dx, dy, r, g, b, a));
            });
        }
        EntityCmd::RemoveShadow { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<Shadow>();
            });
        }
        _ => unreachable!(),
    }
}

fn shader_set_uniform(cmd: EntityCmd, commands: &mut Commands) {
    let (entity_id, name, value) = match cmd {
        EntityCmd::ShaderSetFloat {
            entity_id,
            name,
            value,
        } => (entity_id, name, UniformValue::Float(value)),
        EntityCmd::ShaderSetInt {
            entity_id,
            name,
            value,
        } => (entity_id, name, UniformValue::Int(value)),
        EntityCmd::ShaderSetVec2 {
            entity_id,
            name,
            x,
            y,
        } => (entity_id, name, UniformValue::Vec2 { x, y }),
        EntityCmd::ShaderSetVec4 {
            entity_id,
            name,
            x,
            y,
            z,
            w,
        } => (entity_id, name, UniformValue::Vec4 { x, y, z, w }),
        _ => unreachable!(),
    };
    queue_on_component::<EntityShader>(commands, entity_id, move |shader| {
        shader.set_uniform(&name, value);
    });
}

fn process_transform_cmd(cmd: EntityCmd, commands: &mut Commands) {
    match cmd {
        EntityCmd::SetPosition { entity_id, x, y } => {
            queue_on_component::<MapPosition>(commands, entity_id, move |pos| {
                pos.pos = Vec2 { x, y };
            });
        }
        EntityCmd::SetScreenPosition { entity_id, x, y } => {
            queue_on_component::<ScreenPosition>(commands, entity_id, move |pos| {
                pos.pos = Vec2 { x, y };
            });
        }
        EntityCmd::RemoveScreenPosition { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<ScreenPosition>();
            });
        }
        EntityCmd::SetRotation { entity_id, degrees } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(Rotation { degrees });
            });
        }
        EntityCmd::SetScale { entity_id, sx, sy } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(Scale::new(sx, sy));
            });
        }
        EntityCmd::SetCameraTarget {
            entity_id,
            priority,
            zoom,
        } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                let existing = e.get::<CameraTarget>().copied().unwrap_or_default();
                e.insert(CameraTarget {
                    priority: priority.unwrap_or(existing.priority),
                    zoom: zoom.unwrap_or(existing.zoom).max(f32::EPSILON),
                });
            });
        }
        EntityCmd::RemoveCameraTarget { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<CameraTarget>();
            });
        }
        _ => unreachable!(),
    }
}

fn process_hierarchy_cmd(cmd: EntityCmd, commands: &mut Commands) {
    match cmd {
        EntityCmd::SetParent {
            entity_id,
            parent_id,
        } => {
            let Some(parent) = resolve_entity(parent_id) else {
                return;
            };
            queue_on_entity(commands, entity_id, move |mut child| {
                child.insert(ChildOf(parent));
                // Compute the real world transform now (no one-frame flash at the
                // origin). A standalone parent's transform is synthesized from its
                // local components; propagate_transforms gives it its own next tick.
                ComputeInitialGlobalTransform.apply(child);
            });
        }
        EntityCmd::RemoveParent { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                // Snap to the world transform before detaching.
                if let Some(gt) = e.get::<GlobalTransform2D>().copied() {
                    if let Some(mut pos) = e.get_mut::<MapPosition>() {
                        pos.pos = gt.position;
                    }
                    e.insert((
                        Rotation {
                            degrees: gt.rotation_degrees,
                        },
                        Scale::new(gt.scale.x, gt.scale.y),
                    ));
                }
                e.remove::<(ChildOf, GlobalTransform2D)>();
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
            let Some(target) = resolve_entity(target_id) else {
                return;
            };
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(StuckTo {
                    target,
                    offset: Vec2 {
                        x: offset_x,
                        y: offset_y,
                    },
                    follow_x,
                    follow_y,
                    stored_velocity: Some(Vec2 {
                        x: stored_vx,
                        y: stored_vy,
                    }),
                })
                .remove::<RigidBody>();
            });
        }
        EntityCmd::ReleaseStuckTo { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                if let Some(velocity) = e.take::<StuckTo>().and_then(|s| s.stored_velocity) {
                    let mut rb = RigidBody::new();
                    rb.velocity = velocity;
                    e.insert(rb);
                }
            });
        }
        _ => unreachable!(),
    }
}

fn process_lifecycle_cmd(
    cmd: EntityCmd,
    commands: &mut Commands,
    world_signals: &mut WorldSignals,
    systems_store: &SystemsStore,
) {
    match cmd {
        EntityCmd::InsertLuaTimer {
            entity_id,
            duration,
            callback,
            mode,
        } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(LuaOnTimerFired::timer(duration, callback, mode));
            });
        }
        EntityCmd::RemoveLuaTimer { entity_id } => {
            queue_on_entity(commands, entity_id, |mut e| {
                e.remove::<(Timer, LuaOnTimerFired)>();
            });
        }
        EntityCmd::InsertPhase { entity_id, initial } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(Phase::new(initial));
            });
        }
        EntityCmd::InsertLuaPhaseCallback {
            entity_id,
            kind,
            callback,
        } => {
            queue_on_entity(commands, entity_id, move |mut e| match kind {
                LuaPhaseCallbackKind::Entered => {
                    e.insert(LuaOnPhaseEntered::new(callback));
                }
                LuaPhaseCallbackKind::Exited => {
                    e.insert(LuaOnPhaseExited::new(callback));
                }
                LuaPhaseCallbackKind::Update => {
                    e.insert(LuaOnPhaseUpdate::new(callback));
                }
            });
        }
        EntityCmd::Despawn { entity_id } => {
            if let Some(entity) = resolve_entity(entity_id) {
                // Immediate removal, so a later command in this same drained batch
                // (e.g. a clone of the key) already sees it gone; the engine's
                // end-of-tick prune_dead_entity_registrations covers every other despawn.
                world_signals.unregister_entity_everywhere(entity);
                queue_on(commands, entity, |e| e.despawn());
            }
        }
        EntityCmd::MenuDespawn { entity_id } => {
            let Some(entity) = resolve_entity(entity_id) else {
                return;
            };
            if let Some(system_id) = systems_store.get_entity_system(hook_keys::MENU_DESPAWN) {
                commands.run_system_with(*system_id, entity);
            }
        }
        EntityCmd::InsertTtl { entity_id, seconds } => {
            queue_on_entity(commands, entity_id, move |mut e| {
                e.insert(Ttl::new(seconds));
            });
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::lua_on_tween_finished::LuaOnTweenFinished;
    use aberred_core::components::timer::TimerMode;
    use aberred_core::testing::approx_eq;

    #[test]
    fn resolve_entity_rejects_invalid_bits() {
        // Low 32 bits (entity index) of zero are invalid per `EntityIndex::try_from_bits`.
        assert_eq!(resolve_entity(0), None);
    }

    #[test]
    fn resolve_entity_accepts_valid_bits() {
        let entity = Entity::from_raw_u32(42).unwrap();
        assert_eq!(resolve_entity(entity.to_bits()), Some(entity));
    }

    /// Run a single `EntityCmd` through `process_entity_commands` against a
    /// fresh `World`, applying the resulting ECS commands before returning.
    fn run_entity_cmd(world: &mut World, cmd: EntityCmd) {
        run_entity_cmds(world, [cmd]);
    }

    /// Run a batch of `EntityCmd`s as ONE drained queue (one `Commands` flush), like a
    /// single frame's drain.
    fn run_entity_cmds(world: &mut World, cmds: impl IntoIterator<Item = EntityCmd>) {
        run_entity_cmds_in(
            world,
            &mut WorldSignals::default(),
            &SystemsStore::default(),
            cmds,
        );
    }

    /// [`run_entity_cmds`] with caller-provided `WorldSignals` and `SystemsStore`.
    /// `SetAnimation` reads the world's `AnimationStore` resource when it applies.
    fn run_entity_cmds_in(
        world: &mut World,
        world_signals: &mut WorldSignals,
        systems_store: &SystemsStore,
        cmds: impl IntoIterator<Item = EntityCmd>,
    ) {
        use bevy_ecs::system::SystemState;

        let mut system_state = SystemState::<Commands>::new(world);
        {
            let mut commands = system_state
                .get_mut(world)
                .expect("Entity command test params should fetch");
            process_entity_commands(&mut commands, cmds, world_signals, systems_store);
        }
        system_state.apply(world);
    }

    #[test]
    fn shader_set_float_cmd_updates_the_named_uniform_on_the_target_entity() {
        let mut world = World::new();
        let entity = world.spawn(EntityShader::new("test")).id();

        run_entity_cmd(
            &mut world,
            EntityCmd::ShaderSetFloat {
                entity_id: entity.to_bits(),
                name: "uIntensity".to_string(),
                value: 1.0,
            },
        );
        run_entity_cmd(
            &mut world,
            EntityCmd::ShaderSetFloat {
                entity_id: entity.to_bits(),
                name: "uIntensity".to_string(),
                value: 2.0,
            },
        );

        // Allocation behavior (no reallocated `Arc<str>` key on a repeat write) is
        // covered at the unit level by `EntityShader::set_uniform`'s own test
        // (components/entityshader.rs); this integration test only needs to prove
        // the command correctly reaches the target entity's shader.
        let shader = world.get::<EntityShader>(entity).unwrap();
        assert_eq!(shader.uniforms.len(), 1);
        assert!(matches!(
            shader.uniforms.get("uIntensity"),
            Some(UniformValue::Float(v)) if approx_eq(*v, 2.0)
        ));
    }

    #[test]
    fn despawn_removes_world_signals_registration_and_entity() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();

        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", entity);

        run_entity_cmds_in(
            &mut world,
            &mut world_signals,
            &SystemsStore::default(),
            [EntityCmd::Despawn {
                entity_id: entity.to_bits(),
            }],
        );

        assert!(world.get_entity(entity).is_err());
        assert!(world_signals.get_entity("tpl").is_none());
    }

    /// Every command whose handler inserts/removes components on `id`, plus
    /// `SetParent` onto `parent`.
    fn insert_remove_cmds_for(id: u64, parent: u64) -> Vec<EntityCmd> {
        vec![
            EntityCmd::InsertTweenPosition {
                entity_id: id,
                from_x: 0.0,
                from_y: 0.0,
                to_x: 1.0,
                to_y: 1.0,
                config: TweenConfig::new(1.0),
            },
            EntityCmd::InsertTweenRotation {
                entity_id: id,
                from: 0.0,
                to: 1.0,
                config: TweenConfig::new(1.0),
            },
            EntityCmd::InsertTweenScale {
                entity_id: id,
                from_x: 1.0,
                from_y: 1.0,
                to_x: 2.0,
                to_y: 2.0,
                config: TweenConfig::new(1.0),
            },
            EntityCmd::InsertTweenScreenPosition {
                entity_id: id,
                from_x: 0.0,
                from_y: 0.0,
                to_x: 1.0,
                to_y: 1.0,
                config: TweenConfig::new(1.0),
            },
            EntityCmd::RemoveTweenPosition { entity_id: id },
            EntityCmd::RemoveTweenRotation { entity_id: id },
            EntityCmd::RemoveTweenScale { entity_id: id },
            EntityCmd::SetShader {
                entity_id: id,
                key: "wave".into(),
            },
            EntityCmd::RemoveShader { entity_id: id },
            EntityCmd::SetTint {
                entity_id: id,
                r: 1,
                g: 2,
                b: 3,
                a: 4,
            },
            EntityCmd::RemoveTint { entity_id: id },
            EntityCmd::SetShadow {
                entity_id: id,
                dx: 1.0,
                dy: 1.0,
                r: 0,
                g: 0,
                b: 0,
                a: 128,
            },
            EntityCmd::RemoveShadow { entity_id: id },
            EntityCmd::RemoveScreenPosition { entity_id: id },
            EntityCmd::SetRotation {
                entity_id: id,
                degrees: 45.0,
            },
            EntityCmd::SetScale {
                entity_id: id,
                sx: 2.0,
                sy: 2.0,
            },
            EntityCmd::SetCameraTarget {
                entity_id: id,
                priority: Some(1),
                zoom: None,
            },
            EntityCmd::RemoveCameraTarget { entity_id: id },
            EntityCmd::SetParent {
                entity_id: id,
                parent_id: parent,
            },
            EntityCmd::RemoveParent { entity_id: id },
            EntityCmd::InsertStuckTo {
                entity_id: id,
                target_id: parent,
                follow_x: true,
                follow_y: true,
                offset_x: 0.0,
                offset_y: 0.0,
                stored_vx: 0.0,
                stored_vy: 0.0,
            },
            EntityCmd::ReleaseStuckTo { entity_id: id },
            EntityCmd::InsertLuaTimer {
                entity_id: id,
                duration: 1.0,
                callback: "cb".into(),
                mode: TimerMode::Repeat,
            },
            EntityCmd::RemoveLuaTimer { entity_id: id },
            EntityCmd::InsertPhase {
                entity_id: id,
                initial: "idle".into(),
            },
            EntityCmd::InsertLuaPhaseCallback {
                entity_id: id,
                kind: LuaPhaseCallbackKind::Entered,
                callback: "cb".into(),
            },
            EntityCmd::InsertTtl {
                entity_id: id,
                seconds: 1.0,
            },
            EntityCmd::Despawn { entity_id: id },
        ]
    }

    #[test]
    fn insert_remove_cmds_after_same_batch_despawn_are_silent_noops() {
        let mut world = World::new();
        let doomed = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let parent = world.spawn(MapPosition::new(0.0, 0.0)).id();

        let mut batch = vec![EntityCmd::Despawn {
            entity_id: doomed.to_bits(),
        }];
        batch.extend(insert_remove_cmds_for(doomed.to_bits(), parent.to_bits()));
        // Panics on apply if any handler writes a despawned entity unguarded.
        run_entity_cmds(&mut world, batch);

        assert!(world.get_entity(doomed).is_err(), "despawn still applied");
        assert!(world.get_entity(parent).is_ok());
        assert!(
            world.get::<Children>(parent).is_none(),
            "no ChildOf may point at the parent from the despawned entity"
        );
    }

    #[test]
    fn component_write_cmds_after_same_batch_despawn_are_silent_noops() {
        let mut world = World::new();
        let doomed = world
            .spawn((
                Signals::default(),
                GuiInteractable::new(10.0, 10.0),
                GuiProgressBar::new(10.0, 2.0, 5.0, 10.0),
                Animation::new("walk"),
                test_sprite("walk_sheet"),
                MapPosition::new(0.0, 0.0),
                ScreenPosition::from_vec(Vec2::ZERO),
                CameraTarget::default(),
            ))
            .id();
        let id = doomed.to_bits();

        // Panics on apply if any handler writes a despawned entity unguarded.
        run_entity_cmds(
            &mut world,
            [
                EntityCmd::Despawn { entity_id: id },
                EntityCmd::SignalSetFlag {
                    entity_id: id,
                    flag: "f".into(),
                },
                EntityCmd::SignalClearFlag {
                    entity_id: id,
                    flag: "f".into(),
                },
                EntityCmd::SignalToggleFlag {
                    entity_id: id,
                    flag: "f".into(),
                },
                EntityCmd::SignalSetScalar {
                    entity_id: id,
                    key: "s".into(),
                    value: 1.0,
                },
                EntityCmd::SignalClearScalar {
                    entity_id: id,
                    key: "s".into(),
                },
                EntityCmd::SignalSetString {
                    entity_id: id,
                    key: "t".into(),
                    value: "v".into(),
                },
                EntityCmd::SignalClearString {
                    entity_id: id,
                    key: "t".into(),
                },
                EntityCmd::SignalSetInteger {
                    entity_id: id,
                    key: "i".into(),
                    value: 1,
                },
                EntityCmd::SignalClearInteger {
                    entity_id: id,
                    key: "i".into(),
                },
                EntityCmd::SetGuiDisabled {
                    entity_id: id,
                    disabled: true,
                },
                EntityCmd::SetGuiProgress {
                    entity_id: id,
                    value: 1.0,
                },
                EntityCmd::SetGuiProgressMax {
                    entity_id: id,
                    max: 1.0,
                },
                EntityCmd::RestartAnimation { entity_id: id },
                EntityCmd::SetAnimation {
                    entity_id: id,
                    animation_key: "run".into(),
                },
                EntityCmd::SetSpriteFlip {
                    entity_id: id,
                    flip_h: true,
                    flip_v: false,
                },
                EntityCmd::SetPosition {
                    entity_id: id,
                    x: 1.0,
                    y: 1.0,
                },
                EntityCmd::SetScreenPosition {
                    entity_id: id,
                    x: 1.0,
                    y: 1.0,
                },
                set_camera_target(id, Some(1), None),
            ],
        );

        assert!(world.get_entity(doomed).is_err(), "despawn still applied");
    }

    #[test]
    fn insert_lua_timer_cmd_carries_its_mode() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();

        run_entity_cmd(
            &mut world,
            EntityCmd::InsertLuaTimer {
                entity_id: entity.to_bits(),
                duration: 1.5,
                callback: "boom".into(),
                mode: TimerMode::Once,
            },
        );

        let timer = world.get::<Timer>(entity).unwrap();
        assert_eq!(timer.mode, TimerMode::Once);
        assert_eq!(timer.duration, 1.5);
        assert_eq!(
            &*world.get::<LuaOnTimerFired>(entity).unwrap().callback,
            "boom"
        );
    }

    #[test]
    fn insert_phase_and_lua_phase_callbacks_on_a_live_entity() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let id = entity.to_bits();
        let callback = |kind, callback: &str| EntityCmd::InsertLuaPhaseCallback {
            entity_id: id,
            kind,
            callback: callback.into(),
        };

        run_entity_cmds(
            &mut world,
            vec![
                EntityCmd::InsertPhase {
                    entity_id: id,
                    initial: "idle".into(),
                },
                callback(LuaPhaseCallbackKind::Entered, "on_in"),
                callback(LuaPhaseCallbackKind::Exited, "on_out"),
                callback(LuaPhaseCallbackKind::Update, "on_tick"),
            ],
        );

        assert_eq!(world.get::<Phase>(entity).unwrap().current, "idle");
        assert_eq!(
            &*world.get::<LuaOnPhaseEntered>(entity).unwrap().callback,
            "on_in"
        );
        assert_eq!(
            &*world.get::<LuaOnPhaseExited>(entity).unwrap().callback,
            "on_out"
        );
        assert_eq!(
            &*world.get::<LuaOnPhaseUpdate>(entity).unwrap().callback,
            "on_tick"
        );
    }

    #[test]
    fn remove_lua_timer_cmd_removes_the_timer_and_its_callback() {
        let mut world = World::new();
        let entity = world
            .spawn((Timer::new(1.0), LuaOnTimerFired::new("boom")))
            .id();

        run_entity_cmd(
            &mut world,
            EntityCmd::RemoveLuaTimer {
                entity_id: entity.to_bits(),
            },
        );

        assert!(world.get::<Timer>(entity).is_none());
        assert!(world.get::<LuaOnTimerFired>(entity).is_none());
    }

    #[test]
    fn insert_remove_cmds_for_entity_despawned_in_a_prior_frame_are_skipped() {
        let mut world = World::new();
        let doomed = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let parent = world.spawn(MapPosition::new(0.0, 0.0)).id();
        run_entity_cmd(
            &mut world,
            EntityCmd::Despawn {
                entity_id: doomed.to_bits(),
            },
        );
        assert!(world.get_entity(doomed).is_err());

        let entities_before = world.entities().count_spawned();
        run_entity_cmds(
            &mut world,
            insert_remove_cmds_for(doomed.to_bits(), parent.to_bits()),
        );
        assert!(
            world.get_entity(doomed).is_err(),
            "no command resurrects it"
        );
        assert_eq!(world.entities().count_spawned(), entities_before);
    }

    #[test]
    fn set_parent_places_child_at_its_world_transform_immediately() {
        let mut world = World::new();
        let parent = world.spawn(MapPosition::new(10.0, 5.0)).id();
        let child = world.spawn(MapPosition::new(1.0, 2.0)).id();

        run_entity_cmd(
            &mut world,
            EntityCmd::SetParent {
                entity_id: child.to_bits(),
                parent_id: parent.to_bits(),
            },
        );

        assert_eq!(
            world.get::<ChildOf>(child).map(ChildOf::parent),
            Some(parent)
        );
        // No one-frame flash at the origin: the world transform is real at once.
        assert_eq!(
            world.get::<GlobalTransform2D>(child).unwrap().position,
            Vec2::new(11.0, 7.0)
        );
    }

    #[test]
    fn two_set_parents_onto_one_parent_in_a_batch_place_both_children() {
        let mut world = World::new();
        let parent = world.spawn(MapPosition::new(10.0, 5.0)).id();
        let first = world.spawn(MapPosition::new(1.0, 2.0)).id();
        let second = world.spawn(MapPosition::new(2.0, 3.0)).id();

        run_entity_cmds(
            &mut world,
            [first, second].map(|child| EntityCmd::SetParent {
                entity_id: child.to_bits(),
                parent_id: parent.to_bits(),
            }),
        );

        let world_pos = |e| world.get::<GlobalTransform2D>(e).unwrap().position;
        assert_eq!(world_pos(first), Vec2::new(11.0, 7.0));
        assert_eq!(world_pos(second), Vec2::new(12.0, 8.0));
    }

    #[test]
    fn set_parent_then_remove_parent_keeps_the_world_position() {
        let mut world = World::new();
        let parent = world.spawn(MapPosition::new(10.0, 5.0)).id();
        let child = world.spawn(MapPosition::new(1.0, 2.0)).id();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::SetParent {
                    entity_id: child.to_bits(),
                    parent_id: parent.to_bits(),
                },
                EntityCmd::RemoveParent {
                    entity_id: child.to_bits(),
                },
            ],
        );

        assert!(world.get::<ChildOf>(child).is_none());
        assert!(world.get::<GlobalTransform2D>(child).is_none());
        assert_eq!(
            world.get::<MapPosition>(child).unwrap().pos,
            Vec2::new(11.0, 7.0),
            "detaching keeps the world position it had as a child"
        );
    }

    #[test]
    fn set_parent_onto_parent_despawned_same_batch_leaves_child_unparented() {
        let mut world = World::new();
        let child = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let parent = world.spawn(MapPosition::new(0.0, 0.0)).id();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::Despawn {
                    entity_id: parent.to_bits(),
                },
                EntityCmd::SetParent {
                    entity_id: child.to_bits(),
                    parent_id: parent.to_bits(),
                },
            ],
        );

        assert!(world.get_entity(parent).is_err());
        assert!(world.get_entity(child).is_ok(), "child survives");
        assert!(
            world.get::<ChildOf>(child).is_none(),
            "a ChildOf to a dead parent must not stick"
        );
    }

    #[test]
    fn set_parent_from_dead_child_leaves_parent_untouched() {
        let mut world = World::new();
        let child = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let parent = world.spawn(MapPosition::new(0.0, 0.0)).id();
        run_entity_cmd(
            &mut world,
            EntityCmd::Despawn {
                entity_id: child.to_bits(),
            },
        );

        run_entity_cmd(
            &mut world,
            EntityCmd::SetParent {
                entity_id: child.to_bits(),
                parent_id: parent.to_bits(),
            },
        );

        assert!(
            world.get::<GlobalTransform2D>(parent).is_none(),
            "SetParent on a dead child must not add GlobalTransform2D to the parent"
        );
    }

    fn test_sprite(tex_key: &str) -> Sprite {
        Sprite::new(tex_key, 16.0, 16.0)
    }

    #[test]
    fn physics_cmds_mutate_the_rigidbody() {
        let mut world = World::new();
        let e = world.spawn(RigidBody::new()).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::SetVelocity {
                    entity_id: id,
                    vx: 3.0,
                    vy: 4.0,
                },
                EntityCmd::SetFriction {
                    entity_id: id,
                    friction: 0.5,
                },
                EntityCmd::SetMaxSpeed {
                    entity_id: id,
                    max_speed: Some(9.0),
                },
                EntityCmd::AddForce {
                    entity_id: id,
                    name: "gravity".into(),
                    x: 0.0,
                    y: 10.0,
                    enabled: true,
                },
                EntityCmd::AddForce {
                    entity_id: id,
                    name: "wind".into(),
                    x: 1.0,
                    y: 0.0,
                    enabled: true,
                },
                EntityCmd::RemoveForce {
                    entity_id: id,
                    name: "wind".into(),
                },
                EntityCmd::SetForceEnabled {
                    entity_id: id,
                    name: "gravity".into(),
                    enabled: false,
                },
                EntityCmd::SetForceValue {
                    entity_id: id,
                    name: "gravity".into(),
                    x: 0.0,
                    y: 20.0,
                },
                EntityCmd::FreezeEntity { entity_id: id },
            ],
        );
        let rb = world.get::<RigidBody>(e).unwrap();
        assert_eq!(rb.velocity, Vec2::new(3.0, 4.0));
        assert_eq!(
            (rb.friction, rb.max_speed, rb.frozen),
            (0.5, Some(9.0), true)
        );
        assert_eq!(rb.forces.len(), 1, "wind removed");
        let gravity = &rb.forces["gravity"];
        assert_eq!(
            (gravity.value, gravity.enabled),
            (Vec2::new(0.0, 20.0), false)
        );

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::UnfreezeEntity { entity_id: id },
                EntityCmd::SetSpeed {
                    entity_id: id,
                    speed: 10.0,
                },
                EntityCmd::SetMaxSpeed {
                    entity_id: id,
                    max_speed: None,
                },
            ],
        );
        let rb = world.get::<RigidBody>(e).unwrap();
        assert!(!rb.frozen);
        assert!(
            approx_eq(rb.velocity.x, 6.0) && approx_eq(rb.velocity.y, 8.0),
            "SetSpeed keeps direction: {:?}",
            rb.velocity
        );
        assert_eq!(rb.max_speed, None);
    }

    #[test]
    fn query_mutation_cmds_never_insert_missing_components() {
        let mut world = World::new();
        let e = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::SetVelocity {
                    entity_id: id,
                    vx: 1.0,
                    vy: 1.0,
                },
                EntityCmd::FreezeEntity { entity_id: id },
                EntityCmd::SignalSetFlag {
                    entity_id: id,
                    flag: "f".into(),
                },
                EntityCmd::RestartAnimation { entity_id: id },
                EntityCmd::SetSpriteFlip {
                    entity_id: id,
                    flip_h: true,
                    flip_v: true,
                },
            ],
        );
        assert!(world.get::<RigidBody>(e).is_none());
        assert!(world.get::<Signals>(e).is_none());
        assert!(world.get::<Animation>(e).is_none());
    }

    #[test]
    fn signal_cmds_set_clear_and_toggle_each_kind() {
        let mut world = World::new();
        let e = world.spawn(Signals::default()).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::SignalSetFlag {
                    entity_id: id,
                    flag: "kept".into(),
                },
                EntityCmd::SignalSetFlag {
                    entity_id: id,
                    flag: "cleared".into(),
                },
                EntityCmd::SignalClearFlag {
                    entity_id: id,
                    flag: "cleared".into(),
                },
                EntityCmd::SignalToggleFlag {
                    entity_id: id,
                    flag: "toggled_on".into(),
                },
                EntityCmd::SignalSetScalar {
                    entity_id: id,
                    key: "s".into(),
                    value: 1.5,
                },
                EntityCmd::SignalSetScalar {
                    entity_id: id,
                    key: "s_gone".into(),
                    value: 1.0,
                },
                EntityCmd::SignalClearScalar {
                    entity_id: id,
                    key: "s_gone".into(),
                },
                EntityCmd::SignalSetInteger {
                    entity_id: id,
                    key: "i".into(),
                    value: 7,
                },
                EntityCmd::SignalSetInteger {
                    entity_id: id,
                    key: "i_gone".into(),
                    value: 1,
                },
                EntityCmd::SignalClearInteger {
                    entity_id: id,
                    key: "i_gone".into(),
                },
                EntityCmd::SignalSetString {
                    entity_id: id,
                    key: "name".into(),
                    value: "bob".into(),
                },
                EntityCmd::SignalSetString {
                    entity_id: id,
                    key: "n_gone".into(),
                    value: "x".into(),
                },
                EntityCmd::SignalClearString {
                    entity_id: id,
                    key: "n_gone".into(),
                },
            ],
        );
        let s = world.get::<Signals>(e).unwrap();
        assert!(s.has_flag("kept") && !s.has_flag("cleared") && s.has_flag("toggled_on"));
        assert_eq!(
            (s.get_scalar("s"), s.get_scalar("s_gone")),
            (Some(1.5), None)
        );
        assert_eq!(
            (s.get_integer("i"), s.get_integer("i_gone")),
            (Some(7), None)
        );
        assert_eq!(s.get_string("name"), Some("bob"));
        assert!(s.get_string("n_gone").is_none());

        run_entity_cmd(
            &mut world,
            EntityCmd::SignalToggleFlag {
                entity_id: id,
                flag: "toggled_on".into(),
            },
        );
        assert!(!world.get::<Signals>(e).unwrap().has_flag("toggled_on"));
    }

    #[test]
    fn set_animation_resets_frame_and_syncs_sprite_texture_from_store() {
        use aberred_core::resources::animationstore::AnimationClip;
        let mut store = AnimationStore::default();
        store.animations.insert(
            "run".to_string(),
            AnimationClip::new("run_sheet", 16.0, 4, 10.0),
        );
        let mut world = World::new();
        // The texture lookup reads the world's store when the command applies.
        world.insert_resource(store);
        let mut running = Animation::new("walk");
        running.frame_index = 2;
        running.elapsed_time = 0.3;
        running.finished = true;
        let e = world.spawn((running, test_sprite("walk_sheet"))).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [EntityCmd::SetAnimation {
                entity_id: id,
                animation_key: "run".into(),
            }],
        );
        let anim = world.get::<Animation>(e).unwrap();
        assert_eq!(anim.animation_key, "run");
        assert_eq!(
            (anim.frame_index, anim.elapsed_time, anim.finished),
            (0, 0.0, false)
        );
        let sprite = world.get::<Sprite>(e).unwrap();
        assert_eq!(&*sprite.tex_key, "run_sheet");

        // A key missing from the store still switches the animation but keeps the texture.
        run_entity_cmds(
            &mut world,
            [EntityCmd::SetAnimation {
                entity_id: id,
                animation_key: "unknown".into(),
            }],
        );
        assert_eq!(world.get::<Animation>(e).unwrap().animation_key, "unknown");
        let sprite = world.get::<Sprite>(e).unwrap();
        assert_eq!(&*sprite.tex_key, "run_sheet");
    }

    #[test]
    fn restart_animation_and_sprite_flip() {
        let mut world = World::new();
        let mut running = Animation::new("walk");
        running.frame_index = 5;
        running.elapsed_time = 1.0;
        running.finished = true;
        let e = world.spawn((running, test_sprite("sheet"))).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::RestartAnimation { entity_id: id },
                EntityCmd::SetSpriteFlip {
                    entity_id: id,
                    flip_h: true,
                    flip_v: false,
                },
            ],
        );
        let anim = world.get::<Animation>(e).unwrap();
        assert_eq!(anim.animation_key, "walk");
        assert_eq!(
            (anim.frame_index, anim.elapsed_time, anim.finished),
            (0, 0.0, false)
        );
        let sprite = world.get::<Sprite>(e).unwrap();
        assert_eq!((sprite.flip_h, sprite.flip_v), (true, false));
    }

    #[test]
    fn gui_progress_value_clamps_and_max_lowers_value() {
        let mut world = World::new();
        let e = world.spawn(GuiProgressBar::new(100.0, 8.0, 5.0, 10.0)).id();
        let id = e.to_bits();
        let bar = |world: &World| {
            let b = world.get::<GuiProgressBar>(e).unwrap();
            (b.value, b.max)
        };

        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiProgress {
                entity_id: id,
                value: 25.0,
            },
        );
        assert_eq!(bar(&world), (10.0, 10.0), "value clamps to max");
        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiProgress {
                entity_id: id,
                value: -3.0,
            },
        );
        assert_eq!(bar(&world), (0.0, 10.0), "value clamps to 0");

        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiProgress {
                entity_id: id,
                value: 8.0,
            },
        );
        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiProgressMax {
                entity_id: id,
                max: 6.0,
            },
        );
        assert_eq!(bar(&world), (6.0, 6.0), "lowering max pulls value down");
        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiProgressMax {
                entity_id: id,
                max: 20.0,
            },
        );
        assert_eq!(bar(&world), (6.0, 20.0), "raising max keeps value");
        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiProgressMax {
                entity_id: id,
                max: -1.0,
            },
        );
        assert_eq!(bar(&world), (0.0, 0.0), "negative max clamps to 0");
    }

    #[test]
    fn shader_uniform_cmds_set_each_type_and_clear() {
        let mut world = World::new();
        let e = world.spawn(EntityShader::new("wave")).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::ShaderSetInt {
                    entity_id: id,
                    name: "mode".into(),
                    value: 2,
                },
                EntityCmd::ShaderSetVec2 {
                    entity_id: id,
                    name: "dir".into(),
                    x: 1.0,
                    y: 0.0,
                },
                EntityCmd::ShaderSetVec4 {
                    entity_id: id,
                    name: "tint".into(),
                    x: 1.0,
                    y: 0.5,
                    z: 0.25,
                    w: 1.0,
                },
                EntityCmd::ShaderClearUniform {
                    entity_id: id,
                    name: "dir".into(),
                },
            ],
        );
        let uniforms = &world.get::<EntityShader>(e).unwrap().uniforms;
        assert_eq!(uniforms.get("mode"), Some(&UniformValue::Int(2)));
        assert_eq!(uniforms.get("dir"), None, "cleared individually");
        assert_eq!(
            uniforms.get("tint"),
            Some(&UniformValue::Vec4 {
                x: 1.0,
                y: 0.5,
                z: 0.25,
                w: 1.0
            })
        );

        run_entity_cmd(&mut world, EntityCmd::ShaderClearUniforms { entity_id: id });
        assert!(world.get::<EntityShader>(e).unwrap().uniforms.is_empty());
    }

    fn set_shader(id: u64, key: &str) -> EntityCmd {
        EntityCmd::SetShader {
            entity_id: id,
            key: key.into(),
        }
    }

    fn set_float(id: u64, name: &str, value: f32) -> EntityCmd {
        EntityCmd::ShaderSetFloat {
            entity_id: id,
            name: name.into(),
            value,
        }
    }

    #[test]
    fn set_shader_then_set_uniform_keeps_uniform() {
        let mut world = World::new();
        let e = world.spawn_empty().id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [set_shader(id, "wave"), set_float(id, "u", 1.0)],
        );

        let shader = world.get::<EntityShader>(e).expect("shader inserted");
        assert_eq!(&*shader.shader_key, "wave");
        assert_eq!(shader.uniforms.get("u"), Some(&UniformValue::Float(1.0)));
    }

    #[test]
    fn set_shader_replacing_existing_then_set_uniform_keeps_uniform() {
        let mut world = World::new();
        let e = world.spawn(EntityShader::new("old")).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [set_shader(id, "wave"), set_float(id, "u", 1.0)],
        );

        let shader = world.get::<EntityShader>(e).unwrap();
        assert_eq!(&*shader.shader_key, "wave");
        assert_eq!(shader.uniforms.get("u"), Some(&UniformValue::Float(1.0)));
    }

    #[test]
    fn shader_clear_after_set_shader_applies_in_order() {
        let mut world = World::new();
        let e = world.spawn(EntityShader::new("old")).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                set_shader(id, "wave"),
                set_float(id, "a", 1.0),
                set_float(id, "b", 2.0),
                EntityCmd::ShaderClearUniform {
                    entity_id: id,
                    name: "a".into(),
                },
            ],
        );
        let uniforms = &world.get::<EntityShader>(e).unwrap().uniforms;
        assert_eq!(uniforms.get("a"), None, "cleared after the set");
        assert_eq!(uniforms.get("b"), Some(&UniformValue::Float(2.0)));

        run_entity_cmds(
            &mut world,
            [
                set_shader(id, "glow"),
                set_float(id, "c", 3.0),
                EntityCmd::ShaderClearUniforms { entity_id: id },
                set_float(id, "d", 4.0),
            ],
        );
        let shader = world.get::<EntityShader>(e).unwrap();
        assert_eq!(&*shader.shader_key, "glow");
        assert_eq!(shader.uniforms.len(), 1, "only the write after the clear");
        assert_eq!(shader.uniforms.get("d"), Some(&UniformValue::Float(4.0)));
    }

    #[test]
    fn release_stuckto_restores_stored_velocity_as_rigidbody() {
        let mut world = World::new();
        let target = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let e = world
            .spawn((MapPosition::new(0.0, 0.0), RigidBody::new()))
            .id();
        let id = e.to_bits();

        run_entity_cmd(
            &mut world,
            EntityCmd::InsertStuckTo {
                entity_id: id,
                target_id: target.to_bits(),
                follow_x: true,
                follow_y: false,
                offset_x: 2.0,
                offset_y: 0.0,
                stored_vx: 5.0,
                stored_vy: -1.0,
            },
        );
        assert!(
            world.get::<RigidBody>(e).is_none(),
            "sticking removes the RigidBody"
        );
        let stuck = world.get::<StuckTo>(e).unwrap();
        assert_eq!(stuck.target, target);
        assert_eq!(stuck.stored_velocity, Some(Vec2::new(5.0, -1.0)));

        run_entity_cmd(&mut world, EntityCmd::ReleaseStuckTo { entity_id: id });
        assert!(world.get::<StuckTo>(e).is_none());
        assert_eq!(
            world.get::<RigidBody>(e).unwrap().velocity,
            Vec2::new(5.0, -1.0)
        );
    }

    /// An entity stuck to a fresh target, with an optional stored velocity.
    fn spawn_stuck(world: &mut World, stored_velocity: Option<Vec2>) -> Entity {
        let target = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let mut stuck = StuckTo::new(target);
        stuck.stored_velocity = stored_velocity;
        world.spawn(stuck).id()
    }

    #[test]
    fn release_stuckto_without_stored_velocity_adds_no_rigidbody() {
        let mut world = World::new();
        let e = spawn_stuck(&mut world, None);

        run_entity_cmd(
            &mut world,
            EntityCmd::ReleaseStuckTo {
                entity_id: e.to_bits(),
            },
        );
        assert!(world.get::<StuckTo>(e).is_none());
        assert!(world.get::<RigidBody>(e).is_none());
    }

    #[test]
    fn release_stuckto_then_set_velocity_keeps_velocity() {
        let mut world = World::new();
        let e = spawn_stuck(&mut world, Some(Vec2::new(1.0, 1.0)));
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::ReleaseStuckTo { entity_id: id },
                EntityCmd::SetVelocity {
                    entity_id: id,
                    vx: 5.0,
                    vy: 0.0,
                },
            ],
        );
        assert!(world.get::<StuckTo>(e).is_none());
        assert_eq!(
            world.get::<RigidBody>(e).unwrap().velocity,
            Vec2::new(5.0, 0.0)
        );
    }

    #[test]
    fn release_stuckto_then_add_force_keeps_force() {
        let mut world = World::new();
        let e = spawn_stuck(&mut world, Some(Vec2::new(1.0, 1.0)));
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::ReleaseStuckTo { entity_id: id },
                EntityCmd::AddForce {
                    entity_id: id,
                    name: "gravity".into(),
                    x: 0.0,
                    y: 9.8,
                    enabled: true,
                },
            ],
        );
        let rb = world.get::<RigidBody>(e).unwrap();
        assert_eq!(rb.velocity, Vec2::new(1.0, 1.0), "stored velocity restored");
        assert!(rb.get_force("gravity").is_some());
    }

    #[test]
    fn insert_stuckto_then_release_in_one_batch_restores_stored_velocity() {
        let mut world = World::new();
        let target = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let e = world.spawn(RigidBody::new()).id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::InsertStuckTo {
                    entity_id: id,
                    target_id: target.to_bits(),
                    follow_x: true,
                    follow_y: true,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    stored_vx: 3.0,
                    stored_vy: 4.0,
                },
                EntityCmd::ReleaseStuckTo { entity_id: id },
            ],
        );
        assert!(world.get::<StuckTo>(e).is_none());
        assert_eq!(
            world.get::<RigidBody>(e).unwrap().velocity,
            Vec2::new(3.0, 4.0)
        );
    }

    #[derive(Resource, Default)]
    struct MenuDespawnCalls(Vec<Entity>);

    fn record_menu_despawn(In(entity): In<Entity>, mut calls: ResMut<MenuDespawnCalls>) {
        calls.0.push(entity);
    }

    #[test]
    fn menu_despawn_runs_the_registered_hook_with_the_entity() {
        let mut world = World::new();
        world.init_resource::<MenuDespawnCalls>();
        let mut signals = WorldSignals::default();
        let menu = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let mut systems_store = SystemsStore::default();
        systems_store.insert_entity_system(
            hook_keys::MENU_DESPAWN,
            world.register_system(record_menu_despawn),
        );

        run_entity_cmds_in(
            &mut world,
            &mut signals,
            &systems_store,
            [EntityCmd::MenuDespawn {
                entity_id: menu.to_bits(),
            }],
        );
        assert_eq!(world.resource::<MenuDespawnCalls>().0, [menu]);

        // Without a registered hook the command is a no-op.
        run_entity_cmd(
            &mut world,
            EntityCmd::MenuDespawn {
                entity_id: menu.to_bits(),
            },
        );
        assert_eq!(world.resource::<MenuDespawnCalls>().0.len(), 1);
        assert!(world.get_entity(menu).is_ok());
    }

    fn set_camera_target(id: u64, priority: Option<u8>, zoom: Option<f32>) -> EntityCmd {
        EntityCmd::SetCameraTarget {
            entity_id: id,
            priority,
            zoom,
        }
    }

    #[test]
    fn set_camera_target_twice_in_one_batch_merges_in_call_order() {
        let mut world = World::new();
        let id = world.spawn_empty().id().to_bits();

        run_entity_cmds(
            &mut world,
            [
                set_camera_target(id, Some(5), None),
                set_camera_target(id, None, Some(2.0)),
            ],
        );

        let ct = world.get::<CameraTarget>(Entity::from_bits(id)).unwrap();
        assert_eq!((ct.priority, ct.zoom), (5, 2.0));
    }

    #[test]
    fn remove_camera_target_then_set_camera_target_starts_from_defaults() {
        let mut world = World::new();
        let e = world
            .spawn(CameraTarget {
                priority: 5,
                zoom: 2.0,
            })
            .id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::RemoveCameraTarget { entity_id: id },
                set_camera_target(id, None, None),
            ],
        );

        let ct = world.get::<CameraTarget>(e).unwrap();
        let default = CameraTarget::default();
        assert_eq!((ct.priority, ct.zoom), (default.priority, default.zoom));
    }

    #[test]
    fn insert_tween_screen_position_then_set_screen_position_keeps_the_set() {
        let mut world = World::new();
        // A hidden GUI widget: no ScreenPosition until the tween seeds one.
        let e = world.spawn_empty().id();
        let id = e.to_bits();

        run_entity_cmds(
            &mut world,
            [
                EntityCmd::InsertTweenScreenPosition {
                    entity_id: id,
                    from_x: 0.0,
                    from_y: 0.0,
                    to_x: 100.0,
                    to_y: 0.0,
                    config: TweenConfig::new(1.0),
                },
                EntityCmd::SetScreenPosition {
                    entity_id: id,
                    x: 40.0,
                    y: 8.0,
                },
            ],
        );

        assert_eq!(
            world.get::<ScreenPosition>(e).unwrap().pos,
            Vec2::new(40.0, 8.0)
        );
    }

    #[test]
    fn set_camera_target_defaults_when_absent() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();

        run_entity_cmd(
            &mut world,
            EntityCmd::SetCameraTarget {
                entity_id: entity.to_bits(),
                priority: None,
                zoom: None,
            },
        );

        let ct = world.get::<CameraTarget>(entity).unwrap();
        assert_eq!(ct.priority, CameraTarget::default().priority);
        assert_eq!(ct.zoom, CameraTarget::default().zoom);
    }

    #[test]
    fn set_camera_target_preserves_existing_zoom_when_priority_only() {
        let mut world = World::new();
        let entity = world
            .spawn(CameraTarget {
                priority: 5,
                zoom: 2.0,
            })
            .id();

        run_entity_cmd(
            &mut world,
            EntityCmd::SetCameraTarget {
                entity_id: entity.to_bits(),
                priority: Some(10),
                zoom: None,
            },
        );

        let ct = world.get::<CameraTarget>(entity).unwrap();
        assert_eq!(ct.priority, 10);
        assert_eq!(ct.zoom, 2.0);
    }

    #[test]
    fn set_camera_target_preserves_existing_priority_when_zoom_only() {
        let mut world = World::new();
        let entity = world
            .spawn(CameraTarget {
                priority: 5,
                zoom: 2.0,
            })
            .id();

        run_entity_cmd(
            &mut world,
            EntityCmd::SetCameraTarget {
                entity_id: entity.to_bits(),
                priority: None,
                zoom: Some(3.0),
            },
        );

        let ct = world.get::<CameraTarget>(entity).unwrap();
        assert_eq!(ct.priority, 5);
        assert_eq!(ct.zoom, 3.0);
    }

    #[test]
    fn set_gui_disabled_true_sets_state_disabled() {
        let mut world = World::new();
        let entity = world.spawn(GuiInteractable::new(80.0, 24.0)).id();

        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiDisabled {
                entity_id: entity.to_bits(),
                disabled: true,
            },
        );

        let interactable = world.get::<GuiInteractable>(entity).unwrap();
        assert_eq!(interactable.state, GuiWidgetState::Disabled);
        assert_eq!(interactable.size, Vec2::new(80.0, 24.0));
    }

    #[test]
    fn set_gui_disabled_false_resets_to_normal() {
        let mut world = World::new();
        let entity = world
            .spawn(GuiInteractable::new(80.0, 24.0).with_disabled())
            .id();

        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiDisabled {
                entity_id: entity.to_bits(),
                disabled: false,
            },
        );

        let interactable = world.get::<GuiInteractable>(entity).unwrap();
        assert_eq!(interactable.state, GuiWidgetState::Normal);
    }

    #[test]
    fn set_gui_disabled_noop_on_missing_component() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();

        // Should not panic when the entity has no GuiInteractable (e.g. the
        // command races the one-frame-later gui_*_spawn_system insertion).
        run_entity_cmd(
            &mut world,
            EntityCmd::SetGuiDisabled {
                entity_id: entity.to_bits(),
                disabled: true,
            },
        );

        assert!(world.get::<GuiInteractable>(entity).is_none());
    }

    #[test]
    fn insert_tween_screen_position_adds_both_components_when_missing() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        assert!(world.get::<ScreenPosition>(entity).is_none());

        run_entity_cmd(
            &mut world,
            EntityCmd::InsertTweenScreenPosition {
                entity_id: entity.to_bits(),
                from_x: 10.0,
                from_y: 400.0,
                to_x: 10.0,
                to_y: 260.0,
                config: TweenConfig::new(1.0),
            },
        );

        let pos = world
            .get::<ScreenPosition>(entity)
            .expect("ScreenPosition should be inserted alongside the tween");
        assert_eq!(pos.pos.x, 10.0);
        assert_eq!(pos.pos.y, 400.0);
        let tween = world
            .get::<Tween<ScreenPosition>>(entity)
            .expect("Tween<ScreenPosition> should be inserted");
        assert_eq!(tween.to.pos.y, 260.0);
    }

    #[test]
    fn insert_tween_screen_position_without_callback_clears_stale_callback_from_prior_insert() {
        // Regression test: a hide-tween with a finished callback,
        // followed later by a callback-less show-tween on the same entity
        // (the gui_demo.lua Show/Hide pattern), must not leave the hide
        // callback attached — otherwise it fires again when the unrelated
        // show-tween finishes.
        let mut world = World::new();
        let entity = world
            .spawn(ScreenPosition::from_vec(Vec2 { x: 10.0, y: 260.0 }))
            .id();

        let mut hide_config = TweenConfig::new(1.0);
        hide_config.callback = "on_hide_done".to_string();
        run_entity_cmd(
            &mut world,
            EntityCmd::InsertTweenScreenPosition {
                entity_id: entity.to_bits(),
                from_x: 10.0,
                from_y: 260.0,
                to_x: 10.0,
                to_y: 400.0,
                config: hide_config,
            },
        );
        assert!(
            world
                .get::<LuaOnTweenFinished<ScreenPosition>>(entity)
                .is_some(),
            "hide-tween's callback component should be attached"
        );

        run_entity_cmd(
            &mut world,
            EntityCmd::InsertTweenScreenPosition {
                entity_id: entity.to_bits(),
                from_x: 10.0,
                from_y: 400.0,
                to_x: 10.0,
                to_y: 260.0,
                config: TweenConfig::new(1.0),
            },
        );
        assert!(
            world
                .get::<LuaOnTweenFinished<ScreenPosition>>(entity)
                .is_none(),
            "callback-less show-tween must clear the stale hide-tween callback"
        );
    }

    #[test]
    fn insert_tween_screen_position_overwrites_existing_position() {
        let mut world = World::new();
        let entity = world
            .spawn(ScreenPosition::from_vec(Vec2 { x: 10.0, y: 260.0 }))
            .id();

        run_entity_cmd(
            &mut world,
            EntityCmd::InsertTweenScreenPosition {
                entity_id: entity.to_bits(),
                from_x: 10.0,
                from_y: 260.0,
                to_x: 10.0,
                to_y: 400.0,
                config: TweenConfig::new(1.0),
            },
        );

        let pos = world.get::<ScreenPosition>(entity).unwrap();
        assert_eq!(pos.pos.y, 260.0);
        let tween = world.get::<Tween<ScreenPosition>>(entity).unwrap();
        assert_eq!(tween.to.pos.y, 400.0);
    }

    #[test]
    fn remove_screen_position_removes_component() {
        let mut world = World::new();
        let entity = world
            .spawn(ScreenPosition::from_vec(Vec2 { x: 1.0, y: 2.0 }))
            .id();

        run_entity_cmd(
            &mut world,
            EntityCmd::RemoveScreenPosition {
                entity_id: entity.to_bits(),
            },
        );

        assert!(world.get::<ScreenPosition>(entity).is_none());
    }

    #[test]
    fn remove_screen_position_noop_on_entity_without_one() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();

        run_entity_cmd(
            &mut world,
            EntityCmd::RemoveScreenPosition {
                entity_id: entity.to_bits(),
            },
        );

        assert!(world.get_entity(entity).is_ok());
        assert!(world.get::<ScreenPosition>(entity).is_none());
    }
}
