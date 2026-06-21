//! Runtime entity manipulation command processing.
//!
//! [`process_entity_commands`] dispatches all [`EntityCmd`] variants to modify
//! live entities — physics, signals, transforms, animation, shaders, tweens, etc.

use std::sync::Arc;

use log::warn;

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::cameratarget::CameraTarget;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::luatimer::{LuaTimer, LuaTimerCallback};
use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::stuckto::StuckTo;
use crate::components::tint::Tint;
use crate::components::ttl::Ttl;
use crate::components::tween::{Tween, TweenValue};

use crate::resources::animationstore::AnimationStore;
use crate::resources::lua_runtime::{EntityCmd, TweenConfig, UniformValue};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;

use super::EntityCmdQueries;

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

/// Get `EntityCommands` for a live entity, warning and returning None if despawned.
fn get_entity_cmd<'a>(entity: Entity, commands: &'a mut Commands) -> Option<EntityCommands<'a>> {
    match commands.get_entity(entity) {
        Ok(entity_cmds) => Some(entity_cmds),
        Err(_) => {
            warn!("Cannot apply command to entity {:?}: entity was despawned", entity);
            None
        }
    }
}

/// Run `f` against the live `EntityCommands` for `entity_id`.
///
/// No-ops (with a warn log) if `entity_id` has invalid bits, or if the entity
/// was already despawned in a prior frame's flush. `f` must use
/// `try_insert`/`try_remove`/`try_despawn` (not the panicking
/// `insert`/`remove`/`despawn`) so that an entity despawned *earlier in the
/// same drained batch* (e.g. `Despawn{id}` then `SetRotation{id, ..}`) no-ops
/// silently at apply time instead of panicking via Bevy's default (panic)
/// error handler.
fn with_entity_cmd(commands: &mut Commands, entity_id: u64, f: impl FnOnce(&mut EntityCommands)) {
    let Some(entity) = resolve_entity(entity_id) else { return; };
    with_entity_cmds(commands, entity, f);
}

/// Same as [`with_entity_cmd`], for callers that already hold a resolved
/// `Entity` (avoids re-resolving it from bits).
fn with_entity_cmds(commands: &mut Commands, entity: Entity, f: impl FnOnce(&mut EntityCommands)) {
    if let Some(mut entity_cmds) = get_entity_cmd(entity, commands) {
        f(&mut entity_cmds);
    }
}

pub fn process_entity_commands(
    commands: &mut Commands,
    entity_commands: impl IntoIterator<Item = EntityCmd>,
    world_signals: &mut WorldSignals,
    queries: &mut EntityCmdQueries,
    systems_store: &SystemsStore,
    anim_store: &AnimationStore,
) {
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
            | EntityCmd::SetForceValue { .. }) => process_physics_cmd(cmd, queries),

            cmd @ (EntityCmd::SignalSetFlag { .. }
            | EntityCmd::SignalClearFlag { .. }
            | EntityCmd::SignalToggleFlag { .. }
            | EntityCmd::SignalSetScalar { .. }
            | EntityCmd::SignalClearScalar { .. }
            | EntityCmd::SignalSetString { .. }
            | EntityCmd::SignalClearString { .. }
            | EntityCmd::SignalSetInteger { .. }
            | EntityCmd::SignalClearInteger { .. }) => process_signal_cmd(cmd, queries),

            cmd @ (EntityCmd::RestartAnimation { .. }
            | EntityCmd::SetAnimation { .. }
            | EntityCmd::SetSpriteFlip { .. }) => process_animation_cmd(cmd, queries, anim_store),

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
            | EntityCmd::RemoveTint { .. }) => process_shader_cmd(cmd, commands, queries),

            cmd @ (EntityCmd::SetPosition { .. }
            | EntityCmd::SetScreenPosition { .. }
            | EntityCmd::RemoveScreenPosition { .. }
            | EntityCmd::SetRotation { .. }
            | EntityCmd::SetScale { .. }
            | EntityCmd::SetCameraTarget { .. }
            | EntityCmd::RemoveCameraTarget { .. }) => {
                process_transform_cmd(cmd, commands, queries)
            }

            cmd @ (EntityCmd::SetParent { .. }
            | EntityCmd::RemoveParent { .. }
            | EntityCmd::InsertStuckTo { .. }
            | EntityCmd::ReleaseStuckTo { .. }) => process_hierarchy_cmd(cmd, commands, queries),

            cmd @ (EntityCmd::InsertLuaTimer { .. }
            | EntityCmd::RemoveLuaTimer { .. }
            | EntityCmd::Despawn { .. }
            | EntityCmd::MenuDespawn { .. }
            | EntityCmd::InsertTtl { .. }) => {
                process_lifecycle_cmd(cmd, commands, world_signals, systems_store)
            }
        }
    }
}

fn process_physics_cmd(cmd: EntityCmd, queries: &mut EntityCmdQueries) {
    match cmd {
        EntityCmd::SetVelocity { entity_id, vx, vy } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.velocity = Vector2 { x: vx, y: vy };
            }
        }
        EntityCmd::SetSpeed { entity_id, speed } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.set_speed(speed);
            }
        }
        EntityCmd::SetFriction {
            entity_id,
            friction,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.friction = friction;
            }
        }
        EntityCmd::SetMaxSpeed {
            entity_id,
            max_speed,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.max_speed = max_speed;
            }
        }
        EntityCmd::FreezeEntity { entity_id } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.freeze();
            }
        }
        EntityCmd::UnfreezeEntity { entity_id } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.unfreeze();
            }
        }
        EntityCmd::AddForce {
            entity_id,
            name,
            x,
            y,
            enabled,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.add_force_with_state(&name, Vector2 { x, y }, enabled);
            }
        }
        EntityCmd::RemoveForce { entity_id, name } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.remove_force(&name);
            }
        }
        EntityCmd::SetForceEnabled {
            entity_id,
            name,
            enabled,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.set_force_enabled(&name, enabled);
            }
        }
        EntityCmd::SetForceValue {
            entity_id,
            name,
            x,
            y,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                rb.set_force_value(&name, Vector2 { x, y });
            }
        }
        _ => unreachable!(),
    }
}

fn process_signal_cmd(cmd: EntityCmd, queries: &mut EntityCmdQueries) {
    match cmd {
        EntityCmd::SignalSetFlag { entity_id, flag } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.set_flag(&flag);
            }
        }
        EntityCmd::SignalClearFlag { entity_id, flag } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.clear_flag(&flag);
            }
        }
        EntityCmd::SignalToggleFlag { entity_id, flag } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.toggle_flag(&flag);
            }
        }
        EntityCmd::SignalSetScalar {
            entity_id,
            key,
            value,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.set_scalar(&key, value);
            }
        }
        EntityCmd::SignalClearScalar { entity_id, key } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.clear_scalar(&key);
            }
        }
        EntityCmd::SignalSetString {
            entity_id,
            key,
            value,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.set_string(&key, &value);
            }
        }
        EntityCmd::SignalClearString { entity_id, key } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.remove_string(&key);
            }
        }
        EntityCmd::SignalSetInteger {
            entity_id,
            key,
            value,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.set_integer(&key, value);
            }
        }
        EntityCmd::SignalClearInteger { entity_id, key } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut signals) = queries.signals.get_mut(entity) {
                signals.clear_integer(&key);
            }
        }
        _ => unreachable!(),
    }
}

fn process_animation_cmd(
    cmd: EntityCmd,
    queries: &mut EntityCmdQueries,
    anim_store: &AnimationStore,
) {
    match cmd {
        EntityCmd::RestartAnimation { entity_id } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut animation) = queries.animation.get_mut(entity) {
                animation.frame_index = 0;
                animation.elapsed_time = 0.0;
                animation.finished = false;
            }
        }
        EntityCmd::SetAnimation {
            entity_id,
            animation_key,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut animation) = queries.animation.get_mut(entity) {
                animation.animation_key = animation_key.clone();
                animation.frame_index = 0;
                animation.elapsed_time = 0.0;
                animation.finished = false;
            }
            // Also update the sprite's texture to match the new animation
            if let Some(anim_res) = anim_store.animations.get(&animation_key)
                && let Ok(mut sprite) = queries.sprites.get_mut(entity)
            {
                sprite.tex_key = anim_res.tex_key.clone();
            }
        }
        EntityCmd::SetSpriteFlip {
            entity_id,
            flip_h,
            flip_v,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut sprite) = queries.sprites.get_mut(entity) {
                sprite.flip_h = flip_h;
                sprite.flip_v = flip_v;
            }
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
            MapPosition::from_vec(Vector2 {
                x: from_x,
                y: from_y,
            }),
            MapPosition::from_vec(Vector2 { x: to_x, y: to_y }),
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
            let from = ScreenPosition::from_vec(Vector2 {
                x: from_x,
                y: from_y,
            });
            let to = ScreenPosition::from_vec(Vector2 { x: to_x, y: to_y });
            let tween = super::build_tween(from, to, &config);
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert(from);
                ec.try_insert(tween);
            });
        }
        EntityCmd::RemoveTweenPosition { entity_id } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_remove::<Tween<MapPosition>>();
            });
        }
        EntityCmd::RemoveTweenRotation { entity_id } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_remove::<Tween<Rotation>>();
            });
        }
        EntityCmd::RemoveTweenScale { entity_id } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_remove::<Tween<Scale>>();
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
    with_entity_cmd(commands, entity_id, |ec| {
        ec.try_insert(tween);
    });
}

fn process_shader_cmd(cmd: EntityCmd, commands: &mut Commands, queries: &mut EntityCmdQueries) {
    match cmd {
        EntityCmd::SetShader { entity_id, key } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert(EntityShader::new(key));
            });
        }
        EntityCmd::RemoveShader { entity_id } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_remove::<EntityShader>();
            });
        }
        cmd @ (EntityCmd::ShaderSetFloat { .. }
        | EntityCmd::ShaderSetInt { .. }
        | EntityCmd::ShaderSetVec2 { .. }
        | EntityCmd::ShaderSetVec4 { .. }) => shader_set_uniform(cmd, queries),
        EntityCmd::ShaderClearUniform { entity_id, name } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut shader) = queries.shaders.get_mut(entity) {
                shader.uniforms_mut().remove(name.as_str());
            }
        }
        EntityCmd::ShaderClearUniforms { entity_id } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut shader) = queries.shaders.get_mut(entity) {
                shader.uniforms_mut().clear();
            }
        }
        EntityCmd::SetTint {
            entity_id,
            r,
            g,
            b,
            a,
        } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert(Tint::new(r, g, b, a));
            });
        }
        EntityCmd::RemoveTint { entity_id } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_remove::<Tint>();
            });
        }
        _ => unreachable!(),
    }
}

fn shader_set_uniform(cmd: EntityCmd, queries: &mut EntityCmdQueries) {
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
    let Some(entity) = resolve_entity(entity_id) else { return; };
    if let Ok(mut shader) = queries.shaders.get_mut(entity) {
        shader.uniforms_mut().insert(Arc::from(name), value);
    }
}

fn process_transform_cmd(cmd: EntityCmd, commands: &mut Commands, queries: &mut EntityCmdQueries) {
    match cmd {
        EntityCmd::SetPosition { entity_id, x, y } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut pos) = queries.positions.get_mut(entity) {
                pos.pos.x = x;
                pos.pos.y = y;
            }
        }
        EntityCmd::SetScreenPosition { entity_id, x, y } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Ok(mut pos) = queries.screen_positions.get_mut(entity) {
                pos.pos.x = x;
                pos.pos.y = y;
            }
        }
        EntityCmd::RemoveScreenPosition { entity_id } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_remove::<ScreenPosition>();
            });
        }
        EntityCmd::SetRotation { entity_id, degrees } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert(Rotation { degrees });
            });
        }
        EntityCmd::SetScale { entity_id, sx, sy } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert(Scale::new(sx, sy));
            });
        }
        EntityCmd::SetCameraTarget {
            entity_id,
            priority,
            zoom,
        } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            let existing = queries
                .camera_targets
                .get(entity)
                .copied()
                .unwrap_or_default();
            with_entity_cmds(commands, entity, |ec| {
                ec.try_insert(CameraTarget {
                    priority: priority.unwrap_or(existing.priority),
                    zoom: zoom.unwrap_or(existing.zoom).max(f32::EPSILON),
                });
            });
        }
        EntityCmd::RemoveCameraTarget { entity_id } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_remove::<CameraTarget>();
            });
        }
        _ => unreachable!(),
    }
}

fn process_hierarchy_cmd(cmd: EntityCmd, commands: &mut Commands, queries: &mut EntityCmdQueries) {
    match cmd {
        EntityCmd::SetParent {
            entity_id,
            parent_id,
        } => {
            let Some(parent) = resolve_entity(parent_id) else { return; };
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert((ChildOf(parent), GlobalTransform2D::default()));
            });
            // Ensure parent also has GlobalTransform2D
            if queries.global_transforms.get(parent).is_err() {
                with_entity_cmds(commands, parent, |ec| {
                    ec.try_insert(GlobalTransform2D::default());
                });
            }
        }
        EntityCmd::RemoveParent { entity_id } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            // Snap to world transform before detaching
            let world_transform = queries
                .global_transforms
                .get(entity)
                .ok()
                .map(|gt| (gt.position, gt.rotation_degrees, gt.scale));
            if let Some((position, rotation_degrees, scale)) = world_transform {
                if let Ok(mut pos) = queries.positions.get_mut(entity) {
                    pos.pos = position;
                }
                with_entity_cmds(commands, entity, |ec| {
                    ec.try_insert(Rotation {
                        degrees: rotation_degrees,
                    })
                    .try_insert(Scale::new(scale.x, scale.y))
                    .try_remove::<ChildOf>()
                    .try_remove::<GlobalTransform2D>();
                });
            } else {
                with_entity_cmds(commands, entity, |ec| {
                    ec.try_remove::<ChildOf>().try_remove::<GlobalTransform2D>();
                });
            }
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
            let Some(target) = resolve_entity(target_id) else { return; };
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert(StuckTo {
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
                })
                .try_remove::<crate::components::rigidbody::RigidBody>();
            });
        }
        EntityCmd::ReleaseStuckTo { entity_id } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            let stored_velocity = queries
                .stuckto
                .get(entity)
                .ok()
                .and_then(|stuckto| stuckto.stored_velocity);
            with_entity_cmds(commands, entity, |ec| {
                if let Some(velocity) = stored_velocity {
                    let mut rb = crate::components::rigidbody::RigidBody::new();
                    rb.velocity = velocity;
                    ec.try_insert(rb);
                }
                ec.try_remove::<StuckTo>();
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
        } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert(LuaTimer::new(duration, LuaTimerCallback { name: callback.into() }));
            });
        }
        EntityCmd::RemoveLuaTimer { entity_id } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_remove::<LuaTimer>();
            });
        }
        EntityCmd::Despawn { entity_id } => {
            if let Some(entity) = resolve_entity(entity_id) {
                world_signals.remove_entity_registrations_for(entity);
                with_entity_cmds(commands, entity, |ec| {
                    ec.try_despawn();
                });
            }
        }
        EntityCmd::MenuDespawn { entity_id } => {
            let Some(entity) = resolve_entity(entity_id) else { return; };
            if let Some(system_id) = systems_store.get_entity_system("menu_despawn") {
                commands.run_system_with(*system_id, entity);
            }
        }
        EntityCmd::InsertTtl { entity_id, seconds } => {
            with_entity_cmd(commands, entity_id, |ec| {
                ec.try_insert(Ttl::new(seconds));
            });
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn run_entity_cmd(world: &mut World, world_signals: &mut WorldSignals, cmd: EntityCmd) {
        use bevy_ecs::system::SystemState;

        let systems_store = SystemsStore::default();
        let anim_store = AnimationStore::default();

        let mut system_state = SystemState::<(Commands, EntityCmdQueries)>::new(world);
        {
            let (mut commands, mut queries) = system_state.get_mut(world);
            process_entity_commands(
                &mut commands,
                [cmd],
                world_signals,
                &mut queries,
                &systems_store,
                &anim_store,
            );
        }
        system_state.apply(world);
    }

    #[test]
    fn despawn_removes_world_signals_registration_and_entity() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();

        let mut world_signals = WorldSignals::default();
        world_signals.set_entity("tpl", entity);

        run_entity_cmd(
            &mut world,
            &mut world_signals,
            EntityCmd::Despawn {
                entity_id: entity.to_bits(),
            },
        );

        assert!(world.get_entity(entity).is_err());
        assert!(world_signals.get_entity("tpl").is_none());
    }

    fn run_camera_target_cmd(world: &mut World, cmd: EntityCmd) {
        run_entity_cmd(world, &mut WorldSignals::default(), cmd);
    }

    #[test]
    fn set_camera_target_defaults_when_absent() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();

        run_camera_target_cmd(
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
            .spawn(CameraTarget { priority: 5, zoom: 2.0 })
            .id();

        run_camera_target_cmd(
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
            .spawn(CameraTarget { priority: 5, zoom: 2.0 })
            .id();

        run_camera_target_cmd(
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

    fn run_screen_position_cmd(world: &mut World, cmd: EntityCmd) {
        run_entity_cmd(world, &mut WorldSignals::default(), cmd);
    }

    #[test]
    fn insert_tween_screen_position_adds_both_components_when_missing() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        assert!(world.get::<ScreenPosition>(entity).is_none());

        run_screen_position_cmd(
            &mut world,
            EntityCmd::InsertTweenScreenPosition {
                entity_id: entity.to_bits(),
                from_x: 10.0,
                from_y: 400.0,
                to_x: 10.0,
                to_y: 260.0,
                config: TweenConfig {
                    duration: 1.0,
                    easing: "linear".to_string(),
                    loop_mode: "once".to_string(),
                    backwards: false,
                },
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
    fn insert_tween_screen_position_overwrites_existing_position() {
        let mut world = World::new();
        let entity = world
            .spawn(ScreenPosition::from_vec(Vector2 { x: 10.0, y: 260.0 }))
            .id();

        run_screen_position_cmd(
            &mut world,
            EntityCmd::InsertTweenScreenPosition {
                entity_id: entity.to_bits(),
                from_x: 10.0,
                from_y: 260.0,
                to_x: 10.0,
                to_y: 400.0,
                config: TweenConfig {
                    duration: 1.0,
                    easing: "linear".to_string(),
                    loop_mode: "once".to_string(),
                    backwards: false,
                },
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
            .spawn(ScreenPosition::from_vec(Vector2 { x: 1.0, y: 2.0 }))
            .id();

        run_screen_position_cmd(
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

        run_screen_position_cmd(
            &mut world,
            EntityCmd::RemoveScreenPosition {
                entity_id: entity.to_bits(),
            },
        );

        assert!(world.get_entity(entity).is_ok());
        assert!(world.get::<ScreenPosition>(entity).is_none());
    }
}
