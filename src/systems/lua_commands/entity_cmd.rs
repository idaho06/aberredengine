//! Runtime entity manipulation command processing.
//!
//! [`process_entity_commands`] dispatches all [`EntityCmd`] variants to modify
//! live entities — physics, signals, transforms, animation, shaders, tweens, etc.

use std::sync::Arc;

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::cameratarget::CameraTarget;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::luatimer::LuaTimer;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::stuckto::StuckTo;
use crate::components::tint::Tint;
use crate::components::ttl::Ttl;
use crate::components::tween::{Easing, LoopMode, TweenPosition, TweenRotation, TweenScale};

use crate::resources::animationstore::AnimationStore;
use crate::resources::lua_runtime::{EntityCmd, UniformValue};
use crate::resources::systemsstore::SystemsStore;

use super::EntityCmdQueries;

/// Process all `EntityCmd` commands queued by Lua.
///
/// Iterates over every command and applies the corresponding component
/// mutation or entity-command insertion. All queries are bundled in
/// `queries` to keep the call sites simple.
pub fn process_entity_commands(
    commands: &mut Commands,
    entity_commands: impl IntoIterator<Item = EntityCmd>,
    queries: &mut EntityCmdQueries,
    systems_store: &SystemsStore,
    anim_store: &AnimationStore,
) {
    for cmd in entity_commands {
        match cmd {
            EntityCmd::ReleaseStuckTo { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(stuckto) = queries.stuckto.get(entity)
                    && let Some(velocity) = stuckto.stored_velocity
                {
                    let mut rb = crate::components::rigidbody::RigidBody::new();
                    rb.velocity = velocity;
                    commands.entity(entity).insert(rb);
                }
                commands.entity(entity).remove::<StuckTo>();
            }
            EntityCmd::SignalSetFlag { entity_id, flag } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = queries.signals.get_mut(entity) {
                    signals.set_flag(&flag);
                }
            }
            EntityCmd::SignalClearFlag { entity_id, flag } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = queries.signals.get_mut(entity) {
                    signals.clear_flag(&flag);
                }
            }
            EntityCmd::SetVelocity { entity_id, vx, vy } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.velocity = Vector2 { x: vx, y: vy };
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
                commands
                    .entity(entity)
                    .remove::<crate::components::rigidbody::RigidBody>();
            }
            EntityCmd::RestartAnimation { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut animation) = queries.animation.get_mut(entity) {
                    animation.frame_index = 0;
                    animation.elapsed_time = 0.0;
                }
            }
            EntityCmd::SetAnimation {
                entity_id,
                animation_key,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut animation) = queries.animation.get_mut(entity) {
                    animation.animation_key = animation_key.clone();
                    animation.frame_index = 0;
                    animation.elapsed_time = 0.0;
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
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut sprite) = queries.sprites.get_mut(entity) {
                    sprite.flip_h = flip_h;
                    sprite.flip_v = flip_v;
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
                backwards,
            } => {
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = easing.parse::<Easing>().unwrap();
                let parsed_loop = loop_mode.parse::<LoopMode>().unwrap();
                let mut tween = TweenPosition::new(
                    Vector2 {
                        x: from_x,
                        y: from_y,
                    },
                    Vector2 { x: to_x, y: to_y },
                    duration,
                )
                .with_easing(parsed_easing)
                .with_loop_mode(parsed_loop);

                if backwards {
                    tween = tween.with_backwards();
                }

                commands.entity(entity).insert(tween);
            }
            EntityCmd::InsertTweenRotation {
                entity_id,
                from,
                to,
                duration,
                easing,
                loop_mode,
                backwards,
            } => {
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = easing.parse::<Easing>().unwrap();
                let parsed_loop = loop_mode.parse::<LoopMode>().unwrap();
                let mut tween = TweenRotation::new(from, to, duration)
                    .with_easing(parsed_easing)
                    .with_loop_mode(parsed_loop);

                if backwards {
                    tween = tween.with_backwards();
                }

                commands.entity(entity).insert(tween);
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
                backwards,
            } => {
                let entity = Entity::from_bits(entity_id);
                let parsed_easing = easing.parse::<Easing>().unwrap();
                let parsed_loop = loop_mode.parse::<LoopMode>().unwrap();
                let mut tween = TweenScale::new(
                    Vector2 {
                        x: from_x,
                        y: from_y,
                    },
                    Vector2 { x: to_x, y: to_y },
                    duration,
                )
                .with_easing(parsed_easing)
                .with_loop_mode(parsed_loop);

                if backwards {
                    tween = tween.with_backwards();
                }

                commands.entity(entity).insert(tween);
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
            EntityCmd::SetRotation { entity_id, degrees } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).insert(Rotation { degrees });
            }
            EntityCmd::SetScale { entity_id, sx, sy } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).insert(Scale::new(sx, sy));
            }
            EntityCmd::SignalSetScalar {
                entity_id,
                key,
                value,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = queries.signals.get_mut(entity) {
                    signals.set_scalar(&key, value);
                }
            }
            EntityCmd::SignalSetString {
                entity_id,
                key,
                value,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = queries.signals.get_mut(entity) {
                    signals.set_string(&key, &value);
                }
            }
            EntityCmd::AddForce {
                entity_id,
                name,
                x,
                y,
                enabled,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.add_force_with_state(&name, Vector2 { x, y }, enabled);
                }
            }
            EntityCmd::RemoveForce { entity_id, name } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.remove_force(&name);
                }
            }
            EntityCmd::SetForceEnabled {
                entity_id,
                name,
                enabled,
            } => {
                let entity = Entity::from_bits(entity_id);
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
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.set_force_value(&name, Vector2 { x, y });
                }
            }
            EntityCmd::SetFriction {
                entity_id,
                friction,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.friction = friction;
                }
            }
            EntityCmd::SetMaxSpeed {
                entity_id,
                max_speed,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.max_speed = max_speed;
                }
            }
            EntityCmd::FreezeEntity { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.freeze();
                }
            }
            EntityCmd::UnfreezeEntity { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.unfreeze();
                }
            }
            EntityCmd::SetSpeed { entity_id, speed } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = queries.rigid_bodies.get_mut(entity) {
                    rb.set_speed(speed);
                }
            }
            EntityCmd::SetPosition { entity_id, x, y } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut pos) = queries.positions.get_mut(entity) {
                    pos.pos.x = x;
                    pos.pos.y = y;
                }
            }
            EntityCmd::SetScreenPosition { entity_id, x, y } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut pos) = queries.screen_positions.get_mut(entity) {
                    pos.pos.x = x;
                    pos.pos.y = y;
                }
            }
            EntityCmd::Despawn { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).despawn();
            }
            EntityCmd::MenuDespawn { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Some(system_id) = systems_store.get_entity_system("menu_despawn") {
                    commands.run_system_with(*system_id, entity);
                }
            }
            EntityCmd::SignalSetInteger {
                entity_id,
                key,
                value,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = queries.signals.get_mut(entity) {
                    signals.set_integer(&key, value);
                }
            }
            EntityCmd::InsertTtl { entity_id, seconds } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).insert(Ttl::new(seconds));
            }
            EntityCmd::SetShader { entity_id, key } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut entity_cmds) = commands.get_entity(entity) {
                    entity_cmds.insert(EntityShader::new(key));
                }
            }
            EntityCmd::RemoveShader { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut entity_cmds) = commands.get_entity(entity) {
                    entity_cmds.remove::<EntityShader>();
                }
            }
            EntityCmd::ShaderSetFloat {
                entity_id,
                name,
                value,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut shader) = queries.shaders.get_mut(entity) {
                    shader
                        .uniforms
                        .insert(Arc::from(name), UniformValue::Float(value));
                }
            }
            EntityCmd::ShaderSetInt {
                entity_id,
                name,
                value,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut shader) = queries.shaders.get_mut(entity) {
                    shader
                        .uniforms
                        .insert(Arc::from(name), UniformValue::Int(value));
                }
            }
            EntityCmd::ShaderSetVec2 {
                entity_id,
                name,
                x,
                y,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut shader) = queries.shaders.get_mut(entity) {
                    shader
                        .uniforms
                        .insert(Arc::from(name), UniformValue::Vec2 { x, y });
                }
            }
            EntityCmd::ShaderSetVec4 {
                entity_id,
                name,
                x,
                y,
                z,
                w,
            } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut shader) = queries.shaders.get_mut(entity) {
                    shader
                        .uniforms
                        .insert(Arc::from(name), UniformValue::Vec4 { x, y, z, w });
                }
            }
            EntityCmd::ShaderClearUniform { entity_id, name } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut shader) = queries.shaders.get_mut(entity) {
                    shader.uniforms.remove(name.as_str());
                }
            }
            EntityCmd::ShaderClearUniforms { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut shader) = queries.shaders.get_mut(entity) {
                    shader.uniforms.clear();
                }
            }
            EntityCmd::SetTint {
                entity_id,
                r,
                g,
                b,
                a,
            } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).insert(Tint::new(r, g, b, a));
            }
            EntityCmd::RemoveTint { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<Tint>();
            }
            EntityCmd::SetParent {
                entity_id,
                parent_id,
            } => {
                let child = Entity::from_bits(entity_id);
                let parent = Entity::from_bits(parent_id);
                commands
                    .entity(child)
                    .insert((ChildOf(parent), GlobalTransform2D::default()));
                // Ensure parent also has GlobalTransform2D
                if queries.global_transforms.get(parent).is_err() {
                    commands.entity(parent).insert(GlobalTransform2D::default());
                }
            }
            EntityCmd::RemoveParent { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                // Snap to world transform before detaching
                if let Ok(gt) = queries.global_transforms.get(entity) {
                    if let Ok(mut pos) = queries.positions.get_mut(entity) {
                        pos.pos = gt.position;
                    }
                    commands.entity(entity).insert(Rotation {
                        degrees: gt.rotation_degrees,
                    });
                    commands
                        .entity(entity)
                        .insert(Scale::new(gt.scale.x, gt.scale.y));
                }
                commands.entity(entity).remove::<ChildOf>();
                commands.entity(entity).remove::<GlobalTransform2D>();
            }
            EntityCmd::SetCameraTarget {
                entity_id,
                priority,
            } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).insert(CameraTarget { priority });
            }
            EntityCmd::RemoveCameraTarget { entity_id } => {
                let entity = Entity::from_bits(entity_id);
                commands.entity(entity).remove::<CameraTarget>();
            }
        }
    }
}
