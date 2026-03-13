//! Entity spawn and clone command processing.
//!
//! - [`process_spawn_command`] – create a new entity from a [`SpawnCmd`]
//! - [`process_clone_command`] – clone an existing entity with optional overrides
//! - [`apply_components`] – shared helper that applies all `SpawnCmd` fields to an entity

use std::sync::Arc;

use bevy_ecs::prelude::*;
use raylib::prelude::{Color, Vector2};

use crate::components::animation::{Animation, AnimationController};
use crate::components::boxcollider::BoxCollider;
use crate::components::cameratarget::CameraTarget;
use crate::components::dynamictext::DynamicText;
use crate::components::entityshader::EntityShader;
use crate::components::group::Group;
use crate::components::luaphase::{LuaPhase, PhaseCallbacks};
use crate::components::luatimer::{LuaTimer, LuaTimerCallback};
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
use crate::components::tint::Tint;
use crate::components::ttl::Ttl;
use crate::components::tween::{Easing, LoopMode, Tween};
use crate::components::zindex::ZIndex;

use crate::resources::lua_runtime::{
    AnimationControllerData, AnimationData, CloneCmd, ColliderData, EntityShaderData,
    LuaCollisionRuleData, MenuActionData, MenuData, ParticleEmitterData, PhaseData, RigidBodyData,
    SpawnCmd, SpriteData, StuckToData, TextData, TweenPositionData, TweenRotationData,
    TweenScaleData,
};
use crate::resources::worldsignals::WorldSignals;
use crate::systems::propagate_transforms::ComputeInitialGlobalTransform;

use super::parse::convert_animation_condition;

use log::warn;

/// Process a spawn command from Lua and create the corresponding entity.
///
/// Creates a new entity and delegates all component insertion to [`apply_components`].
pub fn process_spawn_command(
    commands: &mut Commands,
    cmd: SpawnCmd,
    world_signals: &mut WorldSignals,
) {
    let mut entity_commands = commands.spawn_empty();
    let entity = entity_commands.id();
    apply_components(&mut entity_commands, cmd, world_signals, entity);
}

pub(super) fn apply_components(
    entity_commands: &mut EntityCommands,
    cmd: SpawnCmd,
    world_signals: &mut WorldSignals,
    entity: Entity,
) {
    // Trivial one-component insertions kept inline
    if let Some(group_name) = cmd.group {
        entity_commands.insert(Group::new(&group_name));
    }
    if cmd.persistent {
        entity_commands.insert(Persistent);
    }
    if let Some(seconds) = cmd.ttl {
        entity_commands.insert(Ttl::new(seconds));
    }

    apply_transform_components(
        entity_commands,
        cmd.position,
        cmd.screen_position,
        cmd.rotation,
        cmd.scale,
        cmd.parent,
        cmd.stuckto,
        cmd.camera_target,
    );
    apply_physics_components(entity_commands, cmd.rigidbody, cmd.collider);
    apply_render_components(entity_commands, cmd.sprite, cmd.zindex, cmd.shader, cmd.tint);
    apply_animation_components(
        entity_commands,
        cmd.animation,
        cmd.animation_controller,
        cmd.tween_position,
        cmd.tween_rotation,
        cmd.tween_scale,
    );
    apply_signal_components(
        entity_commands,
        cmd.has_signals,
        cmd.signal_scalars,
        cmd.signal_integers,
        cmd.signal_flags,
        cmd.signal_strings,
        cmd.signal_binding,
    );
    apply_behavior_components(
        entity_commands,
        cmd.phase_data,
        cmd.lua_timer,
        cmd.lua_collision_rule,
    );
    apply_ui_components(
        entity_commands,
        world_signals,
        cmd.text,
        cmd.menu,
        cmd.grid_layout,
        cmd.mouse_controlled,
    );
    apply_particle_emitter(entity_commands, world_signals, cmd.particle_emitter);

    // Register entity in WorldSignals if requested
    if let Some(key) = cmd.register_as {
        world_signals.set_entity(&key, entity);
    }
}

fn apply_transform_components(
    entity_commands: &mut EntityCommands,
    position: Option<(f32, f32)>,
    screen_position: Option<(f32, f32)>,
    rotation: Option<f32>,
    scale: Option<(f32, f32)>,
    parent: Option<u64>,
    stuckto: Option<StuckToData>,
    camera_target: Option<u8>,
) {
    if let Some((x, y)) = position {
        entity_commands.insert(MapPosition::new(x, y));
    }
    if let Some((x, y)) = screen_position {
        entity_commands.insert(ScreenPosition::new(x, y));
    }
    if let Some(degrees) = rotation {
        entity_commands.insert(Rotation { degrees });
    }
    if let Some((sx, sy)) = scale {
        entity_commands.insert(Scale { scale: Vector2 { x: sx, y: sy } });
    }
    // Set ChildOf and immediately compute the correct initial GlobalTransform2D
    // so the child renders at the right world position on its very first frame
    // (avoids a one-frame flash at world origin).
    if let Some(parent_id) = parent {
        entity_commands.insert(ChildOf(Entity::from_bits(parent_id)));
        entity_commands.queue(ComputeInitialGlobalTransform);
    }
    if let Some(stuckto_data) = stuckto {
        let target = Entity::from_bits(stuckto_data.target_entity_id);
        let mut stuckto = StuckTo::new(target);
        stuckto.offset = Vector2 { x: stuckto_data.offset_x, y: stuckto_data.offset_y };
        stuckto.follow_x = stuckto_data.follow_x;
        stuckto.follow_y = stuckto_data.follow_y;
        stuckto.stored_velocity = stuckto_data
            .stored_velocity
            .map(|(vx, vy)| Vector2 { x: vx, y: vy });
        entity_commands.insert(stuckto);
    }
    if let Some(priority) = camera_target {
        entity_commands.insert(CameraTarget { priority });
    }
}

fn apply_physics_components(
    entity_commands: &mut EntityCommands,
    rigidbody: Option<RigidBodyData>,
    collider: Option<ColliderData>,
) {
    if let Some(rb_data) = rigidbody {
        let mut rb = RigidBody::with_physics(rb_data.friction, rb_data.max_speed);
        rb.velocity = Vector2 { x: rb_data.velocity_x, y: rb_data.velocity_y };
        rb.frozen = rb_data.frozen;
        for force in rb_data.forces {
            rb.add_force_with_state(&force.name, Vector2 { x: force.x, y: force.y }, force.enabled);
        }
        entity_commands.insert(rb);
    }
    if let Some(collider_data) = collider {
        entity_commands.insert(BoxCollider {
            size: Vector2 { x: collider_data.width, y: collider_data.height },
            offset: Vector2 { x: collider_data.offset_x, y: collider_data.offset_y },
            origin: Vector2 { x: collider_data.origin_x, y: collider_data.origin_y },
        });
    }
}

fn apply_render_components(
    entity_commands: &mut EntityCommands,
    sprite: Option<SpriteData>,
    zindex: Option<f32>,
    shader: Option<EntityShaderData>,
    tint: Option<(u8, u8, u8, u8)>,
) {
    if let Some(sprite_data) = sprite {
        entity_commands.insert(Sprite {
            tex_key: Arc::from(sprite_data.tex_key),
            width: sprite_data.width,
            height: sprite_data.height,
            origin: Vector2 { x: sprite_data.origin_x, y: sprite_data.origin_y },
            offset: Vector2 { x: sprite_data.offset_x, y: sprite_data.offset_y },
            flip_h: sprite_data.flip_h,
            flip_v: sprite_data.flip_v,
        });
    }
    if let Some(z) = zindex {
        entity_commands.insert(ZIndex(z));
    }
    if let Some(shader_data) = shader {
        let mut entity_shader = EntityShader::new(shader_data.key);
        for (name, value) in shader_data.uniforms {
            entity_shader.uniforms.insert(Arc::from(name), value);
        }
        entity_commands.insert(entity_shader);
    }
    if let Some((r, g, b, a)) = tint {
        entity_commands.insert(Tint::new(r, g, b, a));
    }
}

fn apply_animation_components(
    entity_commands: &mut EntityCommands,
    animation: Option<AnimationData>,
    animation_controller: Option<AnimationControllerData>,
    tween_position: Option<TweenPositionData>,
    tween_rotation: Option<TweenRotationData>,
    tween_scale: Option<TweenScaleData>,
) {
    if let Some(anim_data) = animation {
        entity_commands.insert(Animation::new(anim_data.animation_key));
    }
    if let Some(controller_data) = animation_controller {
        let mut controller = AnimationController::new(&controller_data.fallback_key);
        for rule in controller_data.rules {
            let condition = convert_animation_condition(rule.condition);
            controller = controller.with_rule(condition, rule.set_key);
        }
        entity_commands.insert(controller);
    }
    if let Some(tween_data) = tween_position {
        let easing = tween_data.easing.parse::<Easing>().unwrap();
        let loop_mode = tween_data.loop_mode.parse::<LoopMode>().unwrap();
        let mut tween = Tween::new(
            MapPosition::from_vec(Vector2 { x: tween_data.from_x, y: tween_data.from_y }),
            MapPosition::from_vec(Vector2 { x: tween_data.to_x, y: tween_data.to_y }),
            tween_data.duration,
        )
        .with_easing(easing)
        .with_loop_mode(loop_mode);
        if tween_data.backwards {
            tween = tween.with_backwards();
        }
        entity_commands.insert(tween);
    }
    if let Some(tween_data) = tween_rotation {
        let easing = tween_data.easing.parse::<Easing>().unwrap();
        let loop_mode = tween_data.loop_mode.parse::<LoopMode>().unwrap();
        let mut tween = Tween::new(
            Rotation { degrees: tween_data.from },
            Rotation { degrees: tween_data.to },
            tween_data.duration,
        )
        .with_easing(easing)
        .with_loop_mode(loop_mode);
        if tween_data.backwards {
            tween = tween.with_backwards();
        }
        entity_commands.insert(tween);
    }
    if let Some(tween_data) = tween_scale {
        let easing = tween_data.easing.parse::<Easing>().unwrap();
        let loop_mode = tween_data.loop_mode.parse::<LoopMode>().unwrap();
        let mut tween = Tween::new(
            Scale::new(tween_data.from_x, tween_data.from_y),
            Scale::new(tween_data.to_x, tween_data.to_y),
            tween_data.duration,
        )
        .with_easing(easing)
        .with_loop_mode(loop_mode);
        if tween_data.backwards {
            tween = tween.with_backwards();
        }
        entity_commands.insert(tween);
    }
}

fn apply_signal_components(
    entity_commands: &mut EntityCommands,
    has_signals: bool,
    signal_scalars: Vec<(String, f32)>,
    signal_integers: Vec<(String, i32)>,
    signal_flags: Vec<String>,
    signal_strings: Vec<(String, String)>,
    signal_binding: Option<(String, Option<String>)>,
) {
    if has_signals
        || !signal_scalars.is_empty()
        || !signal_integers.is_empty()
        || !signal_flags.is_empty()
        || !signal_strings.is_empty()
    {
        let mut signals = Signals::default();
        for (key, value) in signal_scalars {
            signals.set_scalar(&key, value);
        }
        for (key, value) in signal_integers {
            signals.set_integer(&key, value);
        }
        for flag in signal_flags {
            signals.set_flag(&flag);
        }
        for (key, value) in signal_strings {
            signals.set_string(&key, &value);
        }
        entity_commands.insert(signals);
    }
    if let Some((key, format)) = signal_binding {
        let mut binding = SignalBinding::new(&key);
        if let Some(fmt) = format {
            binding = binding.with_format(fmt);
        }
        entity_commands.insert(binding);
    }
}

fn apply_behavior_components(
    entity_commands: &mut EntityCommands,
    phase_data: Option<PhaseData>,
    lua_timer: Option<(f32, String)>,
    lua_collision_rule: Option<LuaCollisionRuleData>,
) {
    if let Some(phase_data) = phase_data {
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
    if let Some((duration, callback)) = lua_timer {
        entity_commands.insert(LuaTimer::new(duration, LuaTimerCallback { name: callback }));
    }
    if let Some(rule_data) = lua_collision_rule {
        use crate::components::collision::CollisionRule;
        use crate::components::luacollision::LuaCollisionCallback;
        entity_commands.insert(CollisionRule::new(
            rule_data.group_a,
            rule_data.group_b,
            LuaCollisionCallback { name: rule_data.callback },
        ));
    }
}

fn apply_ui_components(
    entity_commands: &mut EntityCommands,
    world_signals: &mut WorldSignals,
    text: Option<TextData>,
    menu: Option<MenuData>,
    grid_layout: Option<(String, String, f32)>,
    mouse_controlled: Option<(bool, bool)>,
) {
    if let Some(text_data) = text {
        entity_commands.insert(DynamicText::new(
            text_data.content,
            text_data.font,
            text_data.font_size,
            Color::new(text_data.r, text_data.g, text_data.b, text_data.a),
        ));
    }
    if let Some(menu_data) = menu {
        use crate::components::menu::{Menu, MenuAction, MenuActions};
        let labels: Vec<(&str, &str)> = menu_data
            .items
            .iter()
            .map(|(id, label)| (id.as_str(), label.as_str()))
            .collect();
        let mut menu_component = Menu::new(
            &labels,
            Vector2 { x: menu_data.origin_x, y: menu_data.origin_y },
            menu_data.font,
            menu_data.font_size,
            menu_data.item_spacing,
            menu_data.use_screen_space,
        );
        if let (Some(normal), Some(selected)) = (menu_data.normal_color, menu_data.selected_color) {
            menu_component = menu_component.with_colors(
                Color::new(normal.r, normal.g, normal.b, normal.a),
                Color::new(selected.r, selected.g, selected.b, selected.a),
            );
        }
        if let Some(dynamic) = menu_data.dynamic_text {
            menu_component = menu_component.with_dynamic_text(dynamic);
        }
        if let Some(sound) = menu_data.selection_change_sound {
            menu_component = menu_component.with_selection_sound(sound);
        }
        if let Some(cursor_key) = menu_data.cursor_entity_key {
            if let Some(cursor_entity) = world_signals.get_entity(&cursor_key).copied() {
                menu_component = menu_component.with_cursor(cursor_entity);
            } else {
                warn!(
                    "Menu cursor entity key '{}' not found in WorldSignals",
                    cursor_key
                );
            }
        }
        if let Some(callback) = menu_data.on_select_callback {
            menu_component = menu_component.with_on_select_callback(callback);
        }
        if let Some(count) = menu_data.visible_count {
            menu_component = menu_component.with_visible_count(count);
        }
        let mut actions = MenuActions::new();
        for (item_id, action_data) in menu_data.actions {
            let action = match action_data {
                MenuActionData::SetScene { scene } => MenuAction::SetScene(scene),
                MenuActionData::ShowSubMenu { menu } => MenuAction::ShowSubMenu(menu),
                MenuActionData::QuitGame => MenuAction::QuitGame,
            };
            actions = actions.with(item_id, action);
        }
        entity_commands.insert((menu_component, actions));
    }
    if let Some((path, group, zindex)) = grid_layout {
        use crate::components::gridlayout::GridLayout;
        entity_commands.insert(GridLayout::new(path, group, zindex));
    }
    if let Some((follow_x, follow_y)) = mouse_controlled {
        use crate::components::inputcontrolled::MouseControlled;
        entity_commands.insert(MouseControlled { follow_x, follow_y });
    }
}

fn apply_particle_emitter(
    entity_commands: &mut EntityCommands,
    world_signals: &mut WorldSignals,
    particle_emitter: Option<ParticleEmitterData>,
) {
    let Some(emitter_data) = particle_emitter else { return };

    use crate::components::particleemitter::{EmitterShape, ParticleEmitter, TtlSpec};
    use crate::resources::lua_runtime::{ParticleEmitterShapeData, ParticleTtlData};

    // Resolve template keys to Entity IDs
    let mut templates = Vec::new();
    for key in &emitter_data.template_keys {
        if let Some(entity) = world_signals.get_entity(key).copied() {
            templates.push(entity);
        } else {
            warn!(
                "ParticleEmitter template key '{}' not found in WorldSignals; ignoring",
                key
            );
        }
    }
    if templates.is_empty() && !emitter_data.template_keys.is_empty() {
        warn!("ParticleEmitter: no valid templates resolved; emitter will not emit");
    }

    // Convert shape
    let shape = match emitter_data.shape {
        ParticleEmitterShapeData::Point => EmitterShape::Point,
        ParticleEmitterShapeData::Rect { width, height } => EmitterShape::Rect { width, height },
    };

    // Convert TTL
    let ttl = match emitter_data.ttl {
        ParticleTtlData::None => TtlSpec::None,
        ParticleTtlData::Fixed(v) => TtlSpec::Fixed(v),
        ParticleTtlData::Range { min, max } => TtlSpec::Range { min, max },
    };

    // Normalize arc and speed (swap if needed)
    let arc_degrees = if emitter_data.arc_min_deg <= emitter_data.arc_max_deg {
        (emitter_data.arc_min_deg, emitter_data.arc_max_deg)
    } else {
        (emitter_data.arc_max_deg, emitter_data.arc_min_deg)
    };
    let speed_range = if emitter_data.speed_min <= emitter_data.speed_max {
        (emitter_data.speed_min, emitter_data.speed_max)
    } else {
        (emitter_data.speed_max, emitter_data.speed_min)
    };

    entity_commands.insert(ParticleEmitter {
        templates,
        shape,
        offset: Vector2 { x: emitter_data.offset_x, y: emitter_data.offset_y },
        particles_per_emission: emitter_data.particles_per_emission,
        emissions_per_second: emitter_data.emissions_per_second,
        emissions_remaining: emitter_data.emissions_remaining,
        arc_degrees,
        speed_range,
        ttl,
        time_since_emit: 0.0,
    });
}

/// EntityCommand that resets an `Animation` component to frame 0.
/// Used when cloning entities to ensure the animation starts fresh.
struct ResetAnimationCommand;

impl bevy_ecs::system::EntityCommand for ResetAnimationCommand {
    fn apply(self, mut entity: bevy_ecs::world::EntityWorldMut<'_>) {
        if let Some(mut animation) = entity.get_mut::<Animation>() {
            animation.frame_index = 0;
            animation.elapsed_time = 0.0;
        }
    }
}

/// Process a clone command from Lua and create a cloned entity.
///
/// Clones an existing entity (looked up by [`WorldSignals`] key) and applies
/// component overrides from the [`CloneCmd`]. Animation is always reset to frame 0
/// unless an animation override is explicitly provided.
pub fn process_clone_command(
    commands: &mut Commands,
    cmd: CloneCmd,
    world_signals: &mut WorldSignals,
) {
    // 1. Look up source entity from WorldSignals
    let Some(source_entity) = world_signals.get_entity(&cmd.source_key).copied() else {
        log::error!(
            "Clone source '{}' not found in WorldSignals",
            cmd.source_key
        );
        return;
    };

    // 2. Clone entity using Bevy's clone_and_spawn API
    let mut source_commands = commands.entity(source_entity);
    let mut entity_commands = source_commands.clone_and_spawn();
    let cloned_entity = entity_commands.id();

    // 3. Check if animation override is provided before moving overrides
    let has_animation_override = cmd.overrides.animation.is_some();

    // 4. Apply all component overrides (same logic as spawn)
    apply_components(
        &mut entity_commands,
        cmd.overrides,
        world_signals,
        cloned_entity,
    );

    // 5. If no animation override was provided, reset to frame 0
    if !has_animation_override {
        entity_commands.queue(ResetAnimationCommand);
    }
}
