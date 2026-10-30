//! Entity spawn and clone command processing.
//!
//! - [`process_spawn_command`] – create a new entity from a [`SpawnCmd`]
//! - [`process_clone_command`] – clone an existing entity with optional overrides
//! - [`apply_components`] – shared helper that applies all `SpawnCmd` fields to an entity

use aberred_core::math::Vec2;
use bevy_ecs::prelude::*;

use crate::components::lua_on_gui_clicked::LuaOnGuiClicked;
use crate::components::lua_on_menu_selected::LuaOnMenuSelected;
use crate::components::lua_on_phase::{LuaOnPhaseEntered, LuaOnPhaseExited, LuaOnPhaseUpdate};
use crate::components::lua_on_timer_fired::LuaOnTimerFired;
use crate::components::lua_setup::LuaSetup;
use aberred_core::components::animation::{Animation, AnimationController};
use aberred_core::components::box_collider::BoxCollider;
use aberred_core::components::camera_target::CameraTarget;
use aberred_core::components::dynamic_text::DynamicText;
use aberred_core::components::entity_shader::EntityShader;
use aberred_core::components::group::Group;
use aberred_core::components::gui_offset::GuiOffset;
use aberred_core::components::persistent::Persistent;
use aberred_core::components::phase::Phase;
use aberred_core::components::rigid_body::RigidBody;
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::components::screen_position::ScreenPosition;
use aberred_core::components::shadow::Shadow;
use aberred_core::components::signal_binding::SignalBinding;
use aberred_core::components::signals::Signals;
use aberred_core::components::sprite::Sprite;
use aberred_core::components::stuck_to::StuckTo;
use aberred_core::components::tilemap::Tilemap;
use aberred_core::components::tint::Tint;
use aberred_core::components::ttl::Ttl;
use aberred_core::components::world_position::WorldPosition;
use aberred_core::components::z_index::ZIndex;
use aberred_core::math::Color;

use crate::resources::lua_runtime::{
    AnimationControllerData, AnimationData, CloneCmd, ColliderData, EntityShaderData,
    LuaCollisionRuleData, LuaPhaseCallbacksSpawn, LuaTimerSpawn, MenuData, ParticleEmitterData,
    RigidBodyData, SpawnCmd, SpriteData, StuckToData, TextData, TweenPositionData,
    TweenRotationData, TweenScaleData, TweenScreenPositionData, TweenTintData,
};
use aberred_core::resources::world_signals::WorldSignals;
use aberred_core::systems::propagate_transforms::ComputeInitialGlobalTransform;

use super::parse::convert_animation_condition;

use log::warn;
/// Process a spawn command from Lua and create the corresponding entity.
///
/// Creates a new entity and delegates all component insertion to
/// `apply_components`. `GuiButton`/`GuiLabel`/`GuiImage` are inserted as
/// plain components — their caption/`GuiInteractable`/`Sprite` companions
/// are spawned by `gui_button_spawn_system`/`gui_label_spawn_system`/
/// `gui_image_spawn_system` (`systems/gui_spawn.rs`) reacting on
/// `Added<T>`, not by this function.
pub fn process_spawn_command(
    commands: &mut Commands,
    cmd: Box<SpawnCmd>,
    world_signals: &mut WorldSignals,
) {
    let mut entity_commands = commands.spawn_empty();
    let entity = entity_commands.id();
    apply_components(&mut entity_commands, cmd, world_signals, entity);
}

pub(super) fn apply_components(
    entity_commands: &mut EntityCommands,
    cmd: Box<SpawnCmd>,
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
    if let Some(path) = cmd.tilemap_path {
        entity_commands.insert(Tilemap::new(path));
    }
    if let Some(window) = cmd.gui_window {
        entity_commands.insert(window);
    }
    if let Some(shape) = cmd.shape {
        entity_commands.insert(shape);
    }
    // GuiButton/GuiLabel/GuiImage carry all their own spawn data; the
    // co-located GuiInteractable/caption/Sprite are spawned by
    // gui_button_spawn_system/gui_label_spawn_system/gui_image_spawn_system
    // (systems/gui_spawn.rs) reacting on Added<T>.
    // GuiProgressBar is inserted as-is; rendered directly by render_system.
    if let Some(btn) = cmd.gui_button {
        entity_commands.insert(btn);
    }
    if let Some(lbl) = cmd.gui_label {
        entity_commands.insert(lbl);
    }
    if let Some(img) = cmd.gui_image {
        entity_commands.insert(img);
    }
    if let Some(callback) = cmd.lua_on_gui_clicked {
        entity_commands.insert(LuaOnGuiClicked::new(callback));
    }
    if let Some(bar) = cmd.gui_progress_bar {
        entity_commands.insert(bar);
    }

    apply_transform_components(
        entity_commands,
        TransformComponents {
            position: cmd.position,
            screen_position: cmd.screen_position,
            rotation: cmd.rotation,
            scale: cmd.scale,
            parent: cmd.parent,
            gui_offset: cmd.gui_offset,
            stuck_to: cmd.stuck_to,
            camera_target: cmd.camera_target,
            camera_target_zoom: cmd.camera_target_zoom,
        },
    );
    apply_physics_components(entity_commands, cmd.rigidbody, cmd.collider);
    apply_render_components(
        entity_commands,
        cmd.sprite,
        cmd.z_index,
        cmd.shader,
        cmd.tint,
        cmd.shadow,
    );
    apply_animation_components(
        entity_commands,
        cmd.animation,
        cmd.animation_controller,
        cmd.tween_position,
        cmd.tween_screen_position,
        cmd.tween_rotation,
        cmd.tween_scale,
    );
    apply_tint_tween(entity_commands, cmd.tween_tint);
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
        BehaviorComponents {
            phase: cmd.phase,
            lua_phase_callbacks: cmd.lua_phase_callbacks,
            lua_timer: cmd.lua_timer,
            lua_collision_rule: cmd.lua_collision_rule,
            lua_setup: cmd.lua_setup,
            lua_on_animation_finished: cmd.lua_on_animation_finished,
        },
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
        world_signals.register_entity(key, entity);
    }
}

struct TransformComponents {
    position: Option<(f32, f32)>,
    screen_position: Option<(f32, f32)>,
    rotation: Option<f32>,
    scale: Option<(f32, f32)>,
    parent: Option<u64>,
    gui_offset: Option<(f32, f32)>,
    stuck_to: Option<StuckToData>,
    camera_target: Option<u8>,
    camera_target_zoom: Option<f32>,
}

fn apply_transform_components(
    entity_commands: &mut EntityCommands,
    transform: TransformComponents,
) {
    if let Some((x, y)) = transform.position {
        entity_commands.insert(WorldPosition::new(x, y));
    }
    if let Some((x, y)) = transform.screen_position {
        entity_commands.insert(ScreenPosition::new(x, y));
    }
    if let Some(degrees) = transform.rotation {
        entity_commands.insert(Rotation { degrees });
    }
    if let Some((sx, sy)) = transform.scale {
        entity_commands.insert(Scale {
            scale: Vec2 { x: sx, y: sy },
        });
    }
    // Set ChildOf and immediately compute the correct initial GlobalTransform2D
    // so the child renders at the right world position on its very first frame
    // (avoids a one-frame flash at world origin).
    if let Some(parent_id) = transform.parent
        && let Some(parent) = super::entity_cmd::resolve_entity(parent_id)
    {
        entity_commands.insert(ChildOf(parent));
        entity_commands.queue(ComputeInitialGlobalTransform);
    }
    if let Some((x, y)) = transform.gui_offset {
        entity_commands.insert(GuiOffset(Vec2 { x, y }));
    }
    if let Some(stuck_to_data) = transform.stuck_to
        && let Some(target) = super::entity_cmd::resolve_entity(stuck_to_data.target_entity_id)
    {
        let mut stuck_to = StuckTo::new(target);
        stuck_to.offset = Vec2 {
            x: stuck_to_data.offset_x,
            y: stuck_to_data.offset_y,
        };
        stuck_to.follow_x = stuck_to_data.follow_x;
        stuck_to.follow_y = stuck_to_data.follow_y;
        stuck_to.stored_velocity = stuck_to_data
            .stored_velocity
            .map(|(vx, vy)| Vec2 { x: vx, y: vy });
        entity_commands.insert(stuck_to);
    }
    if let Some(priority) = transform.camera_target {
        let zoom = transform.camera_target_zoom.unwrap_or(1.0);
        entity_commands.insert(CameraTarget::new(priority).with_zoom(zoom));
    }
}

fn apply_physics_components(
    entity_commands: &mut EntityCommands,
    rigidbody: Option<RigidBodyData>,
    collider: Option<ColliderData>,
) {
    if let Some(rb_data) = rigidbody {
        let mut rb = RigidBody::with_physics(rb_data.friction, rb_data.max_speed);
        rb.velocity = Vec2 {
            x: rb_data.velocity_x,
            y: rb_data.velocity_y,
        };
        rb.frozen = rb_data.frozen;
        for force in rb_data.forces {
            rb.add_force_with_state(
                &force.name,
                Vec2 {
                    x: force.x,
                    y: force.y,
                },
                force.enabled,
            );
        }
        entity_commands.insert(rb);
    }
    if let Some(collider_data) = collider {
        entity_commands.insert(BoxCollider {
            size: Vec2 {
                x: collider_data.width,
                y: collider_data.height,
            },
            offset: Vec2 {
                x: collider_data.offset_x,
                y: collider_data.offset_y,
            },
            origin: Vec2 {
                x: collider_data.origin_x,
                y: collider_data.origin_y,
            },
        });
    }
}

fn apply_render_components(
    entity_commands: &mut EntityCommands,
    sprite: Option<SpriteData>,
    z_index: Option<f32>,
    shader: Option<EntityShaderData>,
    tint: Option<(u8, u8, u8, u8)>,
    shadow: Option<(f32, f32, u8, u8, u8, u8)>,
) {
    if let Some(sprite_data) = sprite {
        entity_commands.insert(
            Sprite::new(sprite_data.tex_key, sprite_data.width, sprite_data.height)
                .with_origin(Vec2::new(sprite_data.origin_x, sprite_data.origin_y))
                .with_offset(Vec2::new(sprite_data.offset_x, sprite_data.offset_y))
                .with_flip(sprite_data.flip_h, sprite_data.flip_v),
        );
    }
    if let Some(z) = z_index {
        entity_commands.insert(ZIndex(z));
    }
    if let Some(shader_data) = shader {
        let mut entity_shader = EntityShader::new(shader_data.key);
        for (name, value) in shader_data.uniforms {
            entity_shader.set_uniform(&name, value);
        }
        entity_commands.insert(entity_shader);
    }
    if let Some((r, g, b, a)) = tint {
        entity_commands.insert(Tint::new(r, g, b, a));
    }
    if let Some((dx, dy, r, g, b, a)) = shadow {
        entity_commands.insert(Shadow::new(dx, dy, r, g, b, a));
    }
}

fn apply_animation_components(
    entity_commands: &mut EntityCommands,
    animation: Option<AnimationData>,
    animation_controller: Option<AnimationControllerData>,
    tween_position: Option<TweenPositionData>,
    tween_screen_position: Option<TweenScreenPositionData>,
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
    if let Some(td) = tween_position {
        entity_commands.insert(super::build_tween(
            WorldPosition::from_vec(Vec2 {
                x: td.from_x,
                y: td.from_y,
            }),
            WorldPosition::from_vec(Vec2 {
                x: td.to_x,
                y: td.to_y,
            }),
            &td.config,
        ));
        entity_commands.queue_silenced(super::tween_finished_callback::<WorldPosition>(&td.config));
    }
    if let Some(td) = tween_screen_position {
        entity_commands.insert(super::build_tween(
            ScreenPosition::from_vec(Vec2 {
                x: td.from_x,
                y: td.from_y,
            }),
            ScreenPosition::from_vec(Vec2 {
                x: td.to_x,
                y: td.to_y,
            }),
            &td.config,
        ));
        entity_commands
            .queue_silenced(super::tween_finished_callback::<ScreenPosition>(&td.config));
    }
    if let Some(td) = tween_rotation {
        entity_commands.insert(super::build_tween(
            Rotation { degrees: td.from },
            Rotation { degrees: td.to },
            &td.config,
        ));
        entity_commands.queue_silenced(super::tween_finished_callback::<Rotation>(&td.config));
    }
    if let Some(td) = tween_scale {
        entity_commands.insert(super::build_tween(
            Scale::new(td.from_x, td.from_y),
            Scale::new(td.to_x, td.to_y),
            &td.config,
        ));
        entity_commands.queue_silenced(super::tween_finished_callback::<Scale>(&td.config));
    }
}

/// Inserts the tint tween. A tween needs a `Tint` to overwrite, so one seeded at `from`
/// is added when the entity has none; an explicit `Tint` (already inserted by
/// `apply_render_components`) wins.
fn apply_tint_tween(entity_commands: &mut EntityCommands, tween_tint: Option<TweenTintData>) {
    let Some(td) = tween_tint else {
        return;
    };
    entity_commands.insert_if_new(td.from);
    entity_commands.insert(super::build_tween(td.from, td.to, &td.config));
    entity_commands.queue_silenced(super::tween_finished_callback::<Tint>(&td.config));
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
            signals.set_scalar(key, value);
        }
        for (key, value) in signal_integers {
            signals.set_integer(key, value);
        }
        for flag in signal_flags {
            signals.set_flag(flag);
        }
        for (key, value) in signal_strings {
            signals.set_string(key, value);
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

struct BehaviorComponents {
    phase: Option<String>,
    lua_phase_callbacks: LuaPhaseCallbacksSpawn,
    lua_timer: Option<LuaTimerSpawn>,
    lua_collision_rule: Option<LuaCollisionRuleData>,
    lua_setup: Option<String>,
    lua_on_animation_finished: Option<String>,
}

fn apply_behavior_components(entity_commands: &mut EntityCommands, b: BehaviorComponents) {
    let BehaviorComponents {
        phase,
        lua_phase_callbacks,
        lua_timer,
        lua_collision_rule,
        lua_setup,
        lua_on_animation_finished,
    } = b;
    if let Some(initial) = phase {
        entity_commands.insert(Phase::new(initial));
    }
    if let Some(callback) = lua_phase_callbacks.entered {
        entity_commands.insert(LuaOnPhaseEntered::new(callback));
    }
    if let Some(callback) = lua_phase_callbacks.exited {
        entity_commands.insert(LuaOnPhaseExited::new(callback));
    }
    if let Some(callback) = lua_phase_callbacks.update {
        entity_commands.insert(LuaOnPhaseUpdate::new(callback));
    }
    if let Some(timer) = lua_timer {
        entity_commands.insert(LuaOnTimerFired::timer(
            timer.duration,
            timer.callback,
            timer.mode,
        ));
    }
    if let Some(rule) = lua_collision_rule {
        use crate::components::lua_on_collision::LuaOnCollision;
        entity_commands.insert(LuaOnCollision::rule(
            rule.group_a,
            rule.group_b,
            rule.callbacks,
        ));
    }
    if let Some(callback) = lua_setup {
        entity_commands.insert(LuaSetup::new(callback));
    }
    if let Some(callback) = lua_on_animation_finished {
        use crate::components::lua_on_animation_finished::LuaOnAnimationFinished;
        entity_commands.insert(LuaOnAnimationFinished::new(callback));
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
        use aberred_core::components::menu::{Menu, MenuActions};
        let labels: Vec<(&str, &str)> = menu_data
            .items
            .iter()
            .map(|(id, label)| (id.as_str(), label.as_str()))
            .collect();
        let mut menu_component = Menu::new(
            &labels,
            Vec2 {
                x: menu_data.origin_x,
                y: menu_data.origin_y,
            },
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
            if let Some(cursor_entity) = world_signals.get_entity(&cursor_key) {
                menu_component = menu_component.with_cursor(cursor_entity);
            } else {
                warn!(
                    "Menu cursor entity key '{}' not found in WorldSignals",
                    cursor_key
                );
            }
        }
        if let Some(count) = menu_data.visible_count {
            menu_component = menu_component.with_visible_count(count);
        }
        if let Some(callback) = menu_data.lua_on_menu_selected {
            if !menu_data.actions.is_empty() {
                warn!(
                    "Menu has both with_lua_menu_selected('{callback}') and menu actions; \
                     the callback handles selection and the actions are discarded"
                );
            }
            entity_commands.insert((menu_component, LuaOnMenuSelected::new(callback)));
        } else {
            let mut actions = MenuActions::new();
            for (item_id, action) in menu_data.actions {
                actions = actions.with(item_id, action);
            }
            entity_commands.insert((menu_component, actions));
        }
    }
    if let Some((path, group, z_index)) = grid_layout {
        use aberred_core::components::grid_layout::GridLayout;
        entity_commands.insert(GridLayout::new(path, group, z_index));
    }
    if let Some((follow_x, follow_y)) = mouse_controlled {
        use aberred_core::components::input_controlled::MouseControlled;
        entity_commands.insert(MouseControlled { follow_x, follow_y });
    }
}

fn apply_particle_emitter(
    entity_commands: &mut EntityCommands,
    world_signals: &mut WorldSignals,
    particle_emitter: Option<ParticleEmitterData>,
) {
    let Some(emitter_data) = particle_emitter else {
        return;
    };

    use crate::resources::lua_runtime::{ParticleEmitterShapeData, ParticleTtlData};
    use aberred_core::components::particle_emitter::{EmitterShape, ParticleEmitter, TtlSpec};

    // Resolve template keys to Entity IDs
    let mut templates = Vec::new();
    for key in &emitter_data.template_keys {
        if let Some(entity) = world_signals.get_entity(key) {
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
        offset: Vec2 {
            x: emitter_data.offset_x,
            y: emitter_data.offset_y,
        },
        particles_per_emission: emitter_data.particles_per_emission,
        emissions_per_second: emitter_data.emissions_per_second,
        emissions_remaining: emitter_data.emissions_remaining,
        initial_emissions_remaining: emitter_data.emissions_remaining,
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
    type Out = ();

    fn apply(self, mut entity: bevy_ecs::world::EntityWorldMut<'_>) {
        if let Some(mut animation) = entity.get_mut::<Animation>() {
            animation.reset();
        }
    }
}

/// Process a clone command from Lua and create a cloned entity.
///
/// Clones an existing entity (looked up by [`WorldSignals`] key) and applies
/// component overrides from the [`CloneCmd`]. Animation is always reset to frame 0
/// unless an animation override is explicitly provided. A core [`Phase`] restarts
/// in its current phase (core's clone behavior), or starts in the override's, and
/// fires its initial enter.
pub fn process_clone_command(
    commands: &mut Commands,
    cmd: CloneCmd,
    world_signals: &mut WorldSignals,
) {
    // 1. Look up source entity from WorldSignals
    let Some(source_entity) = world_signals.get_entity(&cmd.source_key) else {
        log::error!(
            "Clone source '{}' not found in WorldSignals",
            cmd.source_key
        );
        return;
    };

    if commands.get_entity(source_entity).is_err() {
        log::warn!(
            "Clone source '{}' refers to a despawned entity; skipping clone",
            cmd.source_key
        );
        world_signals.unregister_entity(&cmd.source_key);
        return;
    }

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

#[cfg(test)]
mod tests {
    use bevy_ecs::system::SystemState;

    use super::*;
    use crate::resources::lua_runtime::{AnimationConditionData, AnimationRuleData};
    use aberred_core::components::animation::{CmpOp, Condition};
    use aberred_core::components::timer::Timer;
    use aberred_core::components::tween::{Easing, LoopMode, Tween};
    use aberred_core::events::phase::PhaseEntered;
    use aberred_core::resources::uniform_value::UniformValue;
    use aberred_core::resources::world_time::WorldTime;
    use aberred_core::systems::phase::phase_system;
    use bevy_ecs::system::RunSystemOnce;

    #[test]
    fn clone_of_despawned_source_skips_and_cleans_registry() {
        let mut world = World::new();
        let source = world.spawn(WorldPosition::new(1.0, 2.0)).id();
        world.despawn(source);

        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let mut system_state = SystemState::<Commands>::new(&mut world);
        {
            let mut commands = system_state
                .get_mut(&mut world)
                .expect("Commands should fetch in clone test");
            process_clone_command(
                &mut commands,
                CloneCmd {
                    source_key: "tpl".to_string(),
                    overrides: Box::new(SpawnCmd::default()),
                },
                &mut world_signals,
            );
        }
        system_state.apply(&mut world);

        assert!(
            world_signals.get_entity("tpl").is_none(),
            "stale registry entry should be removed"
        );
    }

    #[derive(Resource, Default)]
    struct EnterLog(Vec<(Entity, String, Option<String>)>);

    /// A source already in "run", plus a log of every `PhaseEntered` after
    /// the source's own enters.
    fn phase_source(world: &mut World) -> Entity {
        world.init_resource::<WorldTime>();
        world.init_resource::<EnterLog>();
        world.add_observer(|ev: On<PhaseEntered>, mut log: ResMut<EnterLog>| {
            log.0.push((
                ev.entity,
                ev.name.to_string(),
                ev.previous.as_deref().map(str::to_owned),
            ));
        });
        let mut phase = Phase::new("idle");
        phase.next = Some("run".into());
        let source = world.spawn(phase).id();
        world.run_system_once(phase_system).unwrap();
        world.resource_mut::<EnterLog>().0.clear();
        source
    }

    #[test]
    fn cloned_phase_entity_fires_its_initial_enter() {
        let mut world = World::new();
        let source = phase_source(&mut world);
        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let clone = clone_tpl(&mut world, &mut world_signals, SpawnCmd::default());
        world.run_system_once(phase_system).unwrap();

        let entered = &world.resource::<EnterLog>().0;
        assert_eq!(
            entered,
            &[(clone, "run".to_string(), None)],
            "only the clone enters"
        );
        assert_eq!(world.get::<Phase>(clone).unwrap().current, "run");
    }

    #[test]
    fn cloned_phase_override_starts_in_the_override() {
        let mut world = World::new();
        let source = phase_source(&mut world);
        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let clone = clone_tpl(
            &mut world,
            &mut world_signals,
            SpawnCmd {
                phase: Some("jump".into()),
                ..SpawnCmd::default()
            },
        );
        world.run_system_once(phase_system).unwrap();

        let entered = &world.resource::<EnterLog>().0;
        assert_eq!(entered, &[(clone, "jump".to_string(), None)]);
        assert_eq!(world.get::<Phase>(clone).unwrap().current, "jump");
    }

    #[test]
    fn clone_of_live_source_spawns_new_entity() {
        let mut world = World::new();
        let source = world.spawn(WorldPosition::new(1.0, 2.0)).id();

        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let mut system_state = SystemState::<Commands>::new(&mut world);
        {
            let mut commands = system_state
                .get_mut(&mut world)
                .expect("Commands should fetch in clone test");
            process_clone_command(
                &mut commands,
                CloneCmd {
                    source_key: "tpl".to_string(),
                    overrides: Box::new(SpawnCmd::default()),
                },
                &mut world_signals,
            );
        }
        system_state.apply(&mut world);

        // Source entity plus the newly cloned entity should both exist.
        let mut query = world.query::<&WorldPosition>();
        assert_eq!(query.iter(&world).count(), 2);
    }

    /// Runs one `CloneCmd` of the entity registered as `"tpl"` and returns the new entity.
    fn clone_tpl(
        world: &mut World,
        world_signals: &mut WorldSignals,
        overrides: SpawnCmd,
    ) -> Entity {
        let before: Vec<Entity> = world.query::<Entity>().iter(world).collect();
        let mut system_state = SystemState::<Commands>::new(world);
        {
            let mut commands = system_state.get_mut(world).unwrap();
            process_clone_command(
                &mut commands,
                CloneCmd {
                    source_key: "tpl".to_string(),
                    overrides: Box::new(overrides),
                },
                world_signals,
            );
        }
        system_state.apply(world);
        let spawned: Vec<Entity> = world
            .query::<Entity>()
            .iter(world)
            .filter(|e| !before.contains(e))
            .collect();
        assert_eq!(spawned.len(), 1, "exactly one entity spawned");
        spawned[0]
    }

    #[test]
    fn clone_overrides_win_and_source_is_untouched() {
        let mut world = World::new();
        let source = world
            .spawn((
                WorldPosition::new(1.0, 2.0),
                Group::new("enemy"),
                ZIndex(3.0),
            ))
            .id();
        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let clone = clone_tpl(
            &mut world,
            &mut world_signals,
            SpawnCmd {
                position: Some((10.0, 20.0)),
                group: Some("boss".to_string()),
                ..SpawnCmd::default()
            },
        );

        assert_eq!(
            world.get::<WorldPosition>(clone).unwrap().pos,
            Vec2::new(10.0, 20.0)
        );
        assert_eq!(world.get::<Group>(clone).unwrap().0, "boss");
        assert_eq!(
            world.get::<ZIndex>(clone).unwrap().0,
            3.0,
            "components without an override are cloned"
        );
        assert_eq!(
            world.get::<WorldPosition>(source).unwrap().pos,
            Vec2::new(1.0, 2.0)
        );
        assert_eq!(world.get::<Group>(source).unwrap().0, "enemy");
    }

    #[test]
    fn clone_resets_animation_to_frame_zero_without_touching_source() {
        let mut world = World::new();
        let mut running = Animation::new("walk");
        running.frame_index = 3;
        running.elapsed_time = 0.4;
        running.finished = true;
        let source = world.spawn(running).id();
        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let clone = clone_tpl(&mut world, &mut world_signals, SpawnCmd::default());

        let anim = world.get::<Animation>(clone).unwrap();
        assert_eq!(anim.animation_key, "walk");
        assert_eq!(
            (anim.frame_index, anim.elapsed_time, anim.finished),
            (0, 0.0, false)
        );
        let src = world.get::<Animation>(source).unwrap();
        assert_eq!((src.frame_index, src.finished), (3, true));
    }

    #[test]
    fn clone_with_animation_override_starts_the_override_fresh() {
        let mut world = World::new();
        let mut running = Animation::new("walk");
        running.frame_index = 3;
        let source = world.spawn(running).id();
        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let clone = clone_tpl(
            &mut world,
            &mut world_signals,
            SpawnCmd {
                animation: Some(AnimationData {
                    animation_key: "run".to_string(),
                }),
                ..SpawnCmd::default()
            },
        );

        let anim = world.get::<Animation>(clone).unwrap();
        assert_eq!((anim.animation_key.as_str(), anim.frame_index), ("run", 0));
    }

    #[test]
    fn clone_register_as_stores_the_clone_not_the_source() {
        let mut world = World::new();
        let source = world.spawn(WorldPosition::new(0.0, 0.0)).id();
        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let clone = clone_tpl(
            &mut world,
            &mut world_signals,
            SpawnCmd {
                register_as: Some("copy".to_string()),
                ..SpawnCmd::default()
            },
        );

        assert_ne!(clone, source);
        assert_eq!(world_signals.get_entity("copy"), Some(clone));
        assert_eq!(world_signals.get_entity("tpl"), Some(source));
    }

    #[test]
    fn clone_of_unregistered_key_spawns_nothing() {
        let mut world = World::new();
        world.spawn(WorldPosition::new(0.0, 0.0));
        let mut world_signals = WorldSignals::default();
        let before = world.entities().count_spawned();

        let mut system_state = SystemState::<Commands>::new(&mut world);
        {
            let mut commands = system_state.get_mut(&mut world).unwrap();
            process_clone_command(
                &mut commands,
                CloneCmd {
                    source_key: "missing".to_string(),
                    overrides: Box::new(SpawnCmd::default()),
                },
                &mut world_signals,
            );
        }
        system_state.apply(&mut world);
        assert_eq!(world.entities().count_spawned(), before);
    }

    #[test]
    fn spawn_with_animation_controller_inserts_converted_rules_in_order() {
        let mut world = World::new();
        let mut world_signals = WorldSignals::default();
        let mut system_state = SystemState::<Commands>::new(&mut world);
        {
            let mut commands = system_state.get_mut(&mut world).unwrap();
            process_spawn_command(
                &mut commands,
                Box::new(SpawnCmd {
                    animation_controller: Some(AnimationControllerData {
                        fallback_key: "idle".to_string(),
                        rules: vec![
                            AnimationRuleData {
                                condition: AnimationConditionData::HasFlag {
                                    key: "jumping".into(),
                                },
                                set_key: "jump".to_string(),
                            },
                            AnimationRuleData {
                                condition: AnimationConditionData::ScalarCmp {
                                    key: "speed".into(),
                                    op: "gt".into(),
                                    value: 1.0,
                                },
                                set_key: "run".to_string(),
                            },
                        ],
                    }),
                    ..SpawnCmd::default()
                }),
                &mut world_signals,
            );
        }
        system_state.apply(&mut world);

        let controller = world
            .query::<&AnimationController>()
            .single(&world)
            .expect("one controller spawned");
        assert_eq!(controller.fallback_key, "idle");
        let keys: Vec<&str> = controller
            .rules
            .iter()
            .map(|r| r.set_key.as_str())
            .collect();
        assert_eq!(
            keys,
            ["jump", "run"],
            "rule order is first-match-wins order"
        );
        assert!(
            matches!(&controller.rules[0].when, Condition::HasFlag { key } if key == "jumping")
        );
        assert!(matches!(
            &controller.rules[1].when,
            Condition::ScalarCmp { op: CmpOp::Gt, .. }
        ));
    }

    /// Runs `script` (one `engine.spawn()...:build()`) through the real Lua builder, feeds the
    /// queued `SpawnCmd` through `process_spawn_command`, and returns the spawned entity.
    fn spawn_from_lua(world: &mut World, world_signals: &mut WorldSignals, script: &str) -> Entity {
        use crate::resources::lua_runtime::LuaRuntime;
        let runtime = LuaRuntime::new().unwrap();
        runtime.lua().load(script).exec().unwrap();
        let mut queued = Vec::new();
        runtime.drain_spawn_commands_into(&mut queued);
        assert_eq!(queued.len(), 1, "script must build exactly one entity");

        let before: Vec<Entity> = world.query::<Entity>().iter(world).collect();
        let mut system_state = SystemState::<Commands>::new(world);
        {
            let mut commands = system_state.get_mut(world).unwrap();
            process_spawn_command(&mut commands, queued.pop().unwrap().cmd, world_signals);
        }
        system_state.apply(world);
        let spawned: Vec<Entity> = world
            .query::<Entity>()
            .iter(world)
            .filter(|e| !before.contains(e))
            .collect();
        assert_eq!(spawned.len(), 1, "exactly one entity spawned");
        spawned[0]
    }

    #[test]
    fn lua_spawn_applies_transform_components() {
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let target = world.spawn(WorldPosition::new(0.0, 0.0)).id();
        let script = format!(
            "engine.spawn():with_position(1, 2):with_screen_position(3, 4):with_rotation(45)\
             :with_scale(2, 3):with_stuck_to({}, true, false):with_stuck_to_offset(5, 6)\
             :with_stuck_to_stored_velocity(7, 8):with_camera_target(4):build()",
            target.to_bits()
        );
        let e = spawn_from_lua(&mut world, &mut signals, &script);

        assert_eq!(
            world.get::<WorldPosition>(e).unwrap().pos,
            Vec2::new(1.0, 2.0)
        );
        assert_eq!(
            world.get::<ScreenPosition>(e).unwrap().pos,
            Vec2::new(3.0, 4.0)
        );
        assert_eq!(world.get::<Rotation>(e).unwrap().degrees, 45.0);
        assert_eq!(world.get::<Scale>(e).unwrap().scale, Vec2::new(2.0, 3.0));
        let stuck = world.get::<StuckTo>(e).unwrap();
        assert_eq!(stuck.target, target);
        assert_eq!((stuck.follow_x, stuck.follow_y), (true, false));
        assert_eq!(stuck.offset, Vec2::new(5.0, 6.0));
        assert_eq!(stuck.stored_velocity, Some(Vec2::new(7.0, 8.0)));
        let cam = world.get::<CameraTarget>(e).unwrap();
        assert_eq!((cam.priority, cam.zoom), (4, 1.0), "zoom defaults to 1.0");
    }

    #[test]
    fn lua_spawn_with_parent_inserts_childof_and_gui_offset() {
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let parent = world.spawn(WorldPosition::new(10.0, 0.0)).id();
        let script = format!(
            "engine.spawn():with_position(1, 0):with_parent({}):with_gui_offset(3, 4)\
             :with_camera_target(1, 2.5):build()",
            parent.to_bits()
        );
        let e = spawn_from_lua(&mut world, &mut signals, &script);

        assert_eq!(world.get::<ChildOf>(e).unwrap().parent(), parent);
        assert_eq!(world.get::<GuiOffset>(e).unwrap().0, Vec2::new(3.0, 4.0));
        assert_eq!(world.get::<CameraTarget>(e).unwrap().zoom, 2.5);
    }

    #[test]
    fn lua_spawn_with_invalid_parent_or_stuck_to_target_skips_them() {
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        // Entity bits with a zero index are rejected by resolve_entity.
        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_position(1, 2):with_parent(0):with_stuck_to(0, true, true):build()",
        );
        assert!(world.get::<ChildOf>(e).is_none());
        assert!(world.get::<StuckTo>(e).is_none());
        assert_eq!(
            world.get::<WorldPosition>(e).unwrap().pos,
            Vec2::new(1.0, 2.0)
        );
    }

    #[test]
    fn lua_spawn_applies_physics_and_render_components() {
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_velocity(1, 2):with_friction(0.5):with_max_speed(9)\
             :with_acceleration('gravity', 0, 10, false):with_frozen()\
             :with_box_collider(20, 10, 5, 2):with_box_collider_offset(3, 4)\
             :with_sprite('hero', 32, 48, 16, 24):with_sprite_offset(64, 0):with_sprite_flip(true, false)\
             :with_z_index(7):with_shader('wave', { amp = 2 }):with_tint(1, 2, 3, 4)\
             :with_shadow(2, 3, 5, 6, 7, 8):build()",
        );

        let rb = world.get::<RigidBody>(e).unwrap();
        assert_eq!(rb.velocity, Vec2::new(1.0, 2.0));
        assert_eq!(
            (rb.friction, rb.max_speed, rb.frozen),
            (0.5, Some(9.0), true)
        );
        let gravity = &rb.forces["gravity"];
        assert_eq!(
            (gravity.value, gravity.enabled),
            (Vec2::new(0.0, 10.0), false)
        );

        let collider = world.get::<BoxCollider>(e).unwrap();
        assert_eq!(
            (collider.size, collider.origin, collider.offset),
            (
                Vec2::new(20.0, 10.0),
                Vec2::new(5.0, 2.0),
                Vec2::new(3.0, 4.0)
            )
        );

        let sprite = world.get::<Sprite>(e).unwrap();
        assert_eq!(&*sprite.tex_key, "hero");
        assert_eq!((sprite.width, sprite.height), (32.0, 48.0));
        assert_eq!(
            (sprite.origin, sprite.offset),
            (Vec2::new(16.0, 24.0), Vec2::new(64.0, 0.0))
        );
        assert_eq!((sprite.flip_h, sprite.flip_v), (true, false));

        assert_eq!(world.get::<ZIndex>(e).unwrap().0, 7.0);
        let shader = world.get::<EntityShader>(e).unwrap();
        assert_eq!(&*shader.shader_key, "wave");
        assert_eq!(shader.uniforms.get("amp"), Some(&UniformValue::Float(2.0)));
        assert_eq!(world.get::<Tint>(e).unwrap().color, Color::new(1, 2, 3, 4));
        let shadow = world.get::<Shadow>(e).unwrap();
        assert_eq!(
            (shadow.offset, shadow.color),
            (Vec2::new(2.0, 3.0), Color::new(5, 6, 7, 8))
        );
    }

    #[test]
    fn lua_spawn_applies_tweens_and_their_finished_callbacks() {
        use crate::components::lua_on_tween_finished::LuaOnTweenFinished;
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn()\
             :with_tween_position(0, 0, 10, 20, 1):with_tween_position_easing('quad_in')\
             :with_tween_position_loop('ping_pong'):with_lua_tween_position_finished('pos_done')\
             :with_tween_screen_position(1, 2, 3, 4, 2)\
             :with_tween_rotation(0, 90, 3):with_tween_rotation_backwards()\
             :with_tween_scale(1, 1, 2, 2, 4):with_lua_tween_scale_finished('scale_done')\
             :build()",
        );

        let pos = world.get::<Tween<WorldPosition>>(e).unwrap();
        assert_eq!(
            (pos.from.pos, pos.to.pos),
            (Vec2::ZERO, Vec2::new(10.0, 20.0))
        );
        assert_eq!(pos.duration, 1.0);
        assert!(matches!(pos.easing, Easing::QuadIn));
        assert!(matches!(pos.loop_mode, LoopMode::PingPong));
        let screen = world.get::<Tween<ScreenPosition>>(e).unwrap();
        assert_eq!(
            (screen.from.pos, screen.to.pos),
            (Vec2::new(1.0, 2.0), Vec2::new(3.0, 4.0))
        );
        let rot = world.get::<Tween<Rotation>>(e).unwrap();
        assert_eq!(
            (rot.from.degrees, rot.to.degrees, rot.duration),
            (0.0, 90.0, 3.0)
        );
        assert!(!rot.forward, "backwards tween starts reversed");
        let scale = world.get::<Tween<Scale>>(e).unwrap();
        assert_eq!(scale.to.scale, Vec2::new(2.0, 2.0));

        assert_eq!(
            &*world
                .get::<LuaOnTweenFinished<WorldPosition>>(e)
                .unwrap()
                .callback,
            "pos_done"
        );
        assert_eq!(
            &*world.get::<LuaOnTweenFinished<Scale>>(e).unwrap().callback,
            "scale_done"
        );
        assert!(world.get::<LuaOnTweenFinished<ScreenPosition>>(e).is_none());
        assert!(world.get::<LuaOnTweenFinished<Rotation>>(e).is_none());
    }

    #[test]
    fn lua_spawn_tint_tween_seeds_a_tint_only_when_the_entity_has_none() {
        use crate::components::lua_on_tween_finished::LuaOnTweenFinished;
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let bare = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_tween_tint(1, 2, 3, 4, 5, 6, 7, 8, 2)\
             :with_lua_tween_tint_finished('tint_done'):build()",
        );
        let tinted = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_tint(9, 9, 9, 9):with_tween_tint(1, 2, 3, 4, 5, 6, 7, 8, 2):build()",
        );

        let tween = world.get::<Tween<Tint>>(bare).unwrap();
        assert_eq!(
            (tween.from, tween.to, tween.duration),
            (Tint::new(1, 2, 3, 4), Tint::new(5, 6, 7, 8), 2.0)
        );
        assert_eq!(*world.get::<Tint>(bare).unwrap(), Tint::new(1, 2, 3, 4));
        assert_eq!(
            &*world
                .get::<LuaOnTweenFinished<Tint>>(bare)
                .unwrap()
                .callback,
            "tint_done"
        );
        assert_eq!(*world.get::<Tint>(tinted).unwrap(), Tint::new(9, 9, 9, 9));
        assert!(world.get::<LuaOnTweenFinished<Tint>>(tinted).is_none());
    }

    #[test]
    fn lua_spawn_inserts_the_built_shape() {
        use aberred_core::components::shape::{Shape, ShapeKind};

        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let rect = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_shape_rect(64, 32, 255, 200, 0, 255):with_shape_filled()\
             :with_shape_outline_color(255, 255, 255, 255):with_shape_origin(32, 16)\
             :with_position(100, 100):with_z_index(1):build()",
        );
        assert_eq!(
            *world.get::<Shape>(rect).unwrap(),
            Shape::rect(Vec2::new(64.0, 32.0), Color::new(255, 200, 0, 255))
                .filled()
                .with_outline_color(Color::WHITE)
                .with_origin(Vec2::new(32.0, 16.0))
        );

        let triangle = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_shape_polygon({ {x=0,y=0}, {x=4,y=0}, {x=2,y=3} }, 1, 2, 3, 4)\
             :build()",
        );
        let ShapeKind::Polygon(polygon) = &world.get::<Shape>(triangle).unwrap().kind else {
            panic!("a polygon shape");
        };
        assert_eq!(polygon.triangles().len(), 1, "triangulated at build time");
    }

    #[test]
    fn clone_copies_the_source_shape_and_an_override_replaces_it() {
        use aberred_core::components::shape::Shape;

        let mut world = World::new();
        let source = world.spawn(Shape::circle(5.0, Color::WHITE).filled()).id();
        let mut world_signals = WorldSignals::default();
        world_signals.register_entity("tpl", source);

        let copy = clone_tpl(&mut world, &mut world_signals, SpawnCmd::default());
        assert_eq!(
            *world.get::<Shape>(copy).unwrap(),
            Shape::circle(5.0, Color::WHITE).filled()
        );

        let replaced = clone_tpl(
            &mut world,
            &mut world_signals,
            SpawnCmd {
                shape: Some(Shape::point(Color::RED)),
                ..SpawnCmd::default()
            },
        );
        assert_eq!(
            *world.get::<Shape>(replaced).unwrap(),
            Shape::point(Color::RED)
        );
    }

    #[test]
    fn lua_spawn_inserts_signals_only_when_requested() {
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let bare = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_position(0, 0):build()",
        );
        assert!(world.get::<Signals>(bare).is_none());

        let empty = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_signals():build()",
        );
        assert!(
            world.get::<Signals>(empty).is_some(),
            "with_signals() alone adds an empty bag"
        );

        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_signal_scalar('s', 1.5):with_signal_integer('i', 3)\
             :with_signal_flag('f'):with_signal_string('n', 'bob')\
             :with_signal_binding('score'):with_signal_binding_format('Score: {}'):build()",
        );
        let s = world
            .get::<Signals>(e)
            .expect("any signal implies a Signals component");
        assert_eq!(
            (s.get_scalar("s"), s.get_integer("i")),
            (Some(1.5), Some(3))
        );
        assert!(s.has_flag("f"));
        assert_eq!(s.get_string("n"), Some("bob"));
        let binding = world.get::<SignalBinding>(e).unwrap();
        assert_eq!(binding.signal_key, "score");
        assert_eq!(binding.format.as_deref(), Some("Score: {}"));
    }

    #[test]
    fn lua_spawn_applies_core_phase_and_lua_phase_callbacks() {
        use crate::components::lua_on_phase::{
            LuaOnPhaseEntered, LuaOnPhaseExited, LuaOnPhaseUpdate,
        };
        use aberred_core::components::phase::Phase;
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_phase('idle')\
             :with_lua_phase_entered('on_in'):with_lua_phase_exited('on_out')\
             :with_lua_phase_update('on_tick'):build()",
        );

        assert_eq!(world.get::<Phase>(e).unwrap().current, "idle");
        assert_eq!(
            &*world.get::<LuaOnPhaseEntered>(e).unwrap().callback,
            "on_in"
        );
        assert_eq!(
            &*world.get::<LuaOnPhaseExited>(e).unwrap().callback,
            "on_out"
        );
        assert_eq!(
            &*world.get::<LuaOnPhaseUpdate>(e).unwrap().callback,
            "on_tick"
        );
    }

    #[test]
    fn lua_spawn_applies_behavior_components() {
        use crate::components::lua_on_animation_finished::LuaOnAnimationFinished;
        use crate::components::lua_on_collision::LuaOnCollision;
        use aberred_core::components::collision::CollisionRule;
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn()\
             :with_lua_timer(0.5, 'tick'):with_lua_collision_rule('player', 'enemy', 'on_hit')\
             :with_lua_setup('setup_fn'):with_lua_animation_finished('anim_done'):build()",
        );

        let timer = world.get::<Timer>(e).unwrap();
        let on_fired = world.get::<LuaOnTimerFired>(e).unwrap();
        assert_eq!((timer.duration, &*on_fired.callback), (0.5, "tick"));
        // A core CollisionRule (indexed by core) plus the Lua callbacks.
        let rule = world.get::<CollisionRule>(e).unwrap();
        assert_eq!(
            (rule.group_a.as_str(), rule.group_b.as_str()),
            ("player", "enemy")
        );
        let callbacks = world.get::<LuaOnCollision>(e).unwrap();
        assert_eq!(callbacks.ongoing.as_deref(), Some("on_hit"));
        assert_eq!(&*world.get::<LuaSetup>(e).unwrap().callback, "setup_fn");
        assert_eq!(
            &*world.get::<LuaOnAnimationFinished>(e).unwrap().callback,
            "anim_done"
        );
    }

    #[test]
    fn lua_spawn_puts_started_and_ended_callbacks_on_the_collision_rule() {
        use crate::components::lua_on_collision::LuaOnCollision;
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_lua_collision_rule('a', 'b', nil)\
             :with_lua_collision_started('in'):with_lua_collision_ended('out'):build()",
        );

        let callbacks = world.get::<LuaOnCollision>(e).unwrap();
        assert_eq!(
            (
                callbacks.started.as_deref(),
                callbacks.ongoing.as_deref(),
                callbacks.ended.as_deref()
            ),
            (Some("in"), None, Some("out"))
        );
    }

    #[test]
    fn lua_spawn_applies_text_grid_and_mouse_components() {
        use aberred_core::components::grid_layout::GridLayout;
        use aberred_core::components::input_controlled::MouseControlled;
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_text('Hi', 'arcade', 12, 1, 2, 3, 4)\
             :with_grid_layout('levels/l1.json', 'bricks', 3):with_mouse_controlled(true, false):build()",
        );

        let text = world.get::<DynamicText>(e).unwrap();
        assert_eq!(
            (&*text.text, &*text.font, text.font_size),
            ("Hi", "arcade", 12.0)
        );
        assert_eq!(text.color, Color::new(1, 2, 3, 4));
        let grid = world.get::<GridLayout>(e).unwrap();
        assert_eq!(
            (grid.path.as_str(), grid.group.as_str(), grid.z_index),
            ("levels/l1.json", "bricks", 3.0)
        );
        assert!(!grid.spawned);
        let mouse = world.get::<MouseControlled>(e).unwrap();
        assert_eq!((mouse.follow_x, mouse.follow_y), (true, false));
    }

    #[test]
    fn lua_spawn_builds_menu_with_resolved_cursor_and_actions() {
        use aberred_core::components::menu::{Menu, MenuAction, MenuActions};
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let cursor = world.spawn(WorldPosition::new(0.0, 0.0)).id();
        signals.register_entity("cursor", cursor);

        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_menu({ {id='play', label='Play'}, {id='opts', label='Options'}, \
                {id='quit', label='Quit'} }, 10, 20, 'arcade', 16, 24, true)\
             :with_menu_colors(1, 2, 3, 4, 5, 6, 7, 8):with_menu_cursor('cursor')\
             :with_menu_selection_sound('blip')\
             :with_menu_visible_count(2)\
             :with_menu_action_request_scene('play', 'level01')\
             :with_menu_action_request_scene('opts', 'options')\
             :with_menu_action_request_quit('quit'):build()",
        );

        let menu = world.get::<Menu>(e).unwrap();
        let ids: Vec<&str> = menu.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["play", "opts", "quit"]);
        assert_eq!(menu.items[1].label, "Options");
        assert_eq!(
            (menu.origin, menu.font.as_str()),
            (Vec2::new(10.0, 20.0), "arcade")
        );
        assert_eq!(
            (menu.font_size, menu.item_spacing, menu.use_screen_space),
            (16.0, 24.0, true)
        );
        assert_eq!(
            (menu.normal_color, menu.selected_color),
            (Color::new(1, 2, 3, 4), Color::new(5, 6, 7, 8))
        );
        assert_eq!(menu.cursor_entity, Some(cursor));
        assert_eq!(menu.selection_change_sound.as_deref(), Some("blip"));
        assert_eq!(menu.visible_count, Some(2));

        let actions = world.get::<MenuActions>(e).unwrap();
        assert!(matches!(actions.get("play"), MenuAction::RequestScene(s) if s == "level01"));
        assert!(matches!(actions.get("opts"), MenuAction::RequestScene(s) if s == "options"));
        assert!(matches!(actions.get("quit"), MenuAction::RequestQuit));
    }

    #[test]
    fn with_lua_gui_clicked_attaches_lua_on_gui_clicked() {
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let mut on_click = |script: &str| {
            let e = spawn_from_lua(&mut world, &mut signals, script);
            world
                .get::<LuaOnGuiClicked>(e)
                .map(|c| c.callback.to_string())
        };

        assert_eq!(
            on_click(
                "engine.spawn():with_gui_button(80, 20, 'Start')\
                 :with_lua_gui_clicked('on_start'):build()"
            ),
            Some("on_start".to_string())
        );
        assert_eq!(
            on_click(
                "engine.spawn():with_gui_image(16, 16, 'atlas', 0, 0)\
                 :with_lua_gui_clicked('on_icon'):build()"
            ),
            Some("on_icon".to_string())
        );
        assert_eq!(
            on_click("engine.spawn():with_gui_button(80, 20, 'Start'):build()"),
            None
        );
        assert_eq!(
            on_click("engine.spawn():with_gui_image(16, 16, 'atlas', 0, 0):build()"),
            None
        );
    }

    #[test]
    fn lua_menu_selected_becomes_lua_on_menu_selected_without_menu_actions() {
        use aberred_core::components::menu::MenuActions;
        let mut world = World::new();
        let mut signals = WorldSignals::default();

        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_menu({ {id='play', label='Play'} }, 0, 0, 'arcade', 16, 24, true)\
             :with_lua_menu_selected('on_menu')\
             :with_menu_action_request_scene('play', 'level01'):build()",
        );

        assert_eq!(
            world.get::<LuaOnMenuSelected>(e).map(|c| &*c.callback),
            Some("on_menu")
        );
        assert!(world.get::<MenuActions>(e).is_none());
    }

    #[test]
    fn lua_menu_without_callback_keeps_menu_actions() {
        use aberred_core::components::menu::MenuActions;
        let mut world = World::new();
        let mut signals = WorldSignals::default();

        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_menu({ {id='play', label='Play'} }, 0, 0, 'arcade', 16, 24, true)\
             :with_menu_action_request_scene('play', 'level01'):build()",
        );

        assert!(world.get::<LuaOnMenuSelected>(e).is_none());
        assert!(world.get::<MenuActions>(e).is_some());
    }

    #[test]
    fn lua_spawn_menu_with_unregistered_cursor_key_has_no_cursor() {
        use aberred_core::components::menu::Menu;
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_menu({ {id='a', label='A'} }, 0, 0, 'f', 16, 24, false)\
             :with_menu_cursor('missing'):build()",
        );
        assert_eq!(world.get::<Menu>(e).unwrap().cursor_entity, None);
    }

    #[test]
    fn lua_spawn_particle_emitter_resolves_registered_templates_only() {
        use aberred_core::components::particle_emitter::{EmitterShape, ParticleEmitter, TtlSpec};
        let mut world = World::new();
        let mut signals = WorldSignals::default();
        let spark = world.spawn(WorldPosition::new(0.0, 0.0)).id();
        signals.register_entity("spark", spark);

        let e = spawn_from_lua(
            &mut world,
            &mut signals,
            "engine.spawn():with_position(0, 0):with_particle_emitter({ \
                templates = {'spark', 'missing'}, shape = {kind='rect', width=8, height=4}, \
                offset = {x=1, y=2}, particles_per_emission = 3, emissions_per_second = 20, \
                emissions_remaining = 5, arc = {30, 60}, speed = {10, 20}, ttl = {min=1, max=2} })\
             :build()",
        );

        let emitter = world.get::<ParticleEmitter>(e).unwrap();
        assert_eq!(
            emitter.templates,
            [spark],
            "unregistered template keys are dropped"
        );
        assert!(matches!(
            emitter.shape,
            EmitterShape::Rect {
                width: 8.0,
                height: 4.0
            }
        ));
        assert_eq!(emitter.offset, Vec2::new(1.0, 2.0));
        assert_eq!(
            (emitter.particles_per_emission, emitter.emissions_per_second),
            (3, 20.0)
        );
        assert_eq!(
            (
                emitter.emissions_remaining,
                emitter.initial_emissions_remaining
            ),
            (5, 5)
        );
        assert_eq!(
            (emitter.arc_degrees, emitter.speed_range),
            ((30.0, 60.0), (10.0, 20.0))
        );
        assert!(matches!(emitter.ttl, TtlSpec::Range { min, max } if min == 1.0 && max == 2.0));
        assert_eq!(emitter.time_since_emit, 0.0);
    }
}
