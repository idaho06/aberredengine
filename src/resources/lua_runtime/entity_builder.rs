//! Entity builder for fluent entity construction from Lua.
//!
//! This module provides the `LuaEntityBuilder` struct which implements
//! a fluent interface for building entities from Lua scripts using method chaining.
//!
//! Also provides `LuaCollisionEntityBuilder` for spawning entities from collision callbacks.

use super::runtime::LuaAppData;
use super::spawn_data::*;
use mlua::prelude::*;

/// Entity builder exposed to Lua for fluent entity construction.
///
/// This struct implements `UserData` so Lua can call methods on it using
/// the colon syntax: `engine.spawn():with_position(x, y):build()`
///
/// Each `with_*` method returns `Self` to allow chaining.
/// The `build()` method queues the entity for spawning.
#[derive(Debug, Clone, Default)]
pub struct LuaEntityBuilder {
    cmd: SpawnCmd,
}

impl LuaEntityBuilder {
    pub fn new() -> Self {
        Self {
            cmd: SpawnCmd::default(),
        }
    }
}

impl LuaUserData for LuaEntityBuilder {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // :with_group(name) - Set entity group
        methods.add_method_mut("with_group", |_, this, name: String| {
            this.cmd.group = Some(name);
            Ok(this.clone())
        });

        // :with_position(x, y) - Set world position
        methods.add_method_mut("with_position", |_, this, (x, y): (f32, f32)| {
            this.cmd.position = Some((x, y));
            Ok(this.clone())
        });

        // :with_sprite(tex_key, width, height, origin_x, origin_y) - Set sprite
        methods.add_method_mut(
            "with_sprite",
            |_, this, (tex_key, width, height, origin_x, origin_y): (String, f32, f32, f32, f32)| {
                this.cmd.sprite = Some(SpriteData {
                    tex_key,
                    width,
                    height,
                    origin_x,
                    origin_y,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    flip_h: false,
                    flip_v: false,
                });
                Ok(this.clone())
            },
        );

        // :with_sprite_offset(offset_x, offset_y) - Set sprite offset
        methods.add_method_mut(
            "with_sprite_offset",
            |_, this, (offset_x, offset_y): (f32, f32)| {
                if let Some(ref mut sprite) = this.cmd.sprite {
                    sprite.offset_x = offset_x;
                    sprite.offset_y = offset_y;
                }
                Ok(this.clone())
            },
        );

        // :with_sprite_flip(flip_h, flip_v) - Set sprite flipping
        methods.add_method_mut(
            "with_sprite_flip",
            |_, this, (flip_h, flip_v): (bool, bool)| {
                if let Some(ref mut sprite) = this.cmd.sprite {
                    sprite.flip_h = flip_h;
                    sprite.flip_v = flip_v;
                }
                Ok(this.clone())
            },
        );

        // :with_zindex(z) - Set render order
        methods.add_method_mut("with_zindex", |_, this, z: i32| {
            this.cmd.zindex = Some(z);
            Ok(this.clone())
        });

        // :with_velocity(vx, vy) - Set RigidBody velocity
        // Creates a RigidBody if one doesn't exist, otherwise updates velocity
        methods.add_method_mut("with_velocity", |_, this, (vx, vy): (f32, f32)| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.velocity_x = vx;
                rb.velocity_y = vy;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    velocity_x: vx,
                    velocity_y: vy,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        });

        // :with_friction(friction) - Set RigidBody friction (velocity damping)
        // Creates a RigidBody if one doesn't exist
        methods.add_method_mut("with_friction", |_, this, friction: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.friction = friction;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    friction,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        });

        // :with_max_speed(speed) - Set RigidBody max_speed clamp
        // Creates a RigidBody if one doesn't exist
        methods.add_method_mut("with_max_speed", |_, this, speed: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.max_speed = Some(speed);
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    max_speed: Some(speed),
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        });

        // :with_accel(name, x, y, enabled) - Add a named acceleration force
        // Creates a RigidBody if one doesn't exist
        methods.add_method_mut(
            "with_accel",
            |_, this, (name, x, y, enabled): (String, f32, f32, bool)| {
                if let Some(ref mut rb) = this.cmd.rigidbody {
                    rb.forces.push(ForceData {
                        name,
                        x,
                        y,
                        enabled,
                    });
                } else {
                    this.cmd.rigidbody = Some(RigidBodyData {
                        forces: vec![ForceData {
                            name,
                            x,
                            y,
                            enabled,
                        }],
                        ..RigidBodyData::default()
                    });
                }
                Ok(this.clone())
            },
        );

        // :with_frozen() - Mark entity as frozen (physics skipped)
        // Creates a RigidBody if one doesn't exist
        methods.add_method_mut("with_frozen", |_, this, ()| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.frozen = true;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    frozen: true,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        });

        // :with_collider(width, height, origin_x, origin_y) - Set BoxCollider
        methods.add_method_mut(
            "with_collider",
            |_, this, (width, height, origin_x, origin_y): (f32, f32, f32, f32)| {
                this.cmd.collider = Some(ColliderData {
                    width,
                    height,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    origin_x,
                    origin_y,
                });
                Ok(this.clone())
            },
        );

        // :with_collider_offset(offset_x, offset_y) - Set collider offset
        methods.add_method_mut(
            "with_collider_offset",
            |_, this, (offset_x, offset_y): (f32, f32)| {
                if let Some(ref mut collider) = this.cmd.collider {
                    collider.offset_x = offset_x;
                    collider.offset_y = offset_y;
                }
                Ok(this.clone())
            },
        );

        // :with_mouse_controlled(follow_x, follow_y) - Enable mouse control
        methods.add_method_mut(
            "with_mouse_controlled",
            |_, this, (follow_x, follow_y): (bool, bool)| {
                this.cmd.mouse_controlled = Some((follow_x, follow_y));
                Ok(this.clone())
            },
        );

        // :with_rotation(degrees) - Set rotation
        methods.add_method_mut("with_rotation", |_, this, degrees: f32| {
            this.cmd.rotation = Some(degrees);
            Ok(this.clone())
        });

        // :with_scale(sx, sy) - Set scale
        methods.add_method_mut("with_scale", |_, this, (sx, sy): (f32, f32)| {
            this.cmd.scale = Some((sx, sy));
            Ok(this.clone())
        });

        // :with_persistent() - Mark entity as persistent across scene changes
        methods.add_method_mut("with_persistent", |_, this, ()| {
            this.cmd.persistent = true;
            Ok(this.clone())
        });

        // :with_signal_scalar(key, value) - Add a scalar signal
        methods.add_method_mut(
            "with_signal_scalar",
            |_, this, (key, value): (String, f32)| {
                this.cmd.signal_scalars.push((key, value));
                Ok(this.clone())
            },
        );

        // :with_signal_integer(key, value) - Add an integer signal
        methods.add_method_mut(
            "with_signal_integer",
            |_, this, (key, value): (String, i32)| {
                this.cmd.signal_integers.push((key, value));
                Ok(this.clone())
            },
        );

        // :with_signal_flag(key) - Add a flag signal
        methods.add_method_mut("with_signal_flag", |_, this, key: String| {
            this.cmd.signal_flags.push(key);
            Ok(this.clone())
        });

        // :with_signal_string(key, value) - Add a string signal
        methods.add_method_mut(
            "with_signal_string",
            |_, this, (key, value): (String, String)| {
                this.cmd.signal_strings.push((key, value));
                Ok(this.clone())
            },
        );

        // :with_screen_position(x, y) - Set screen position (for UI elements)
        methods.add_method_mut("with_screen_position", |_, this, (x, y): (f32, f32)| {
            this.cmd.screen_position = Some((x, y));
            Ok(this.clone())
        });

        // :with_text(content, font, font_size, r, g, b, a) - Set DynamicText
        methods.add_method_mut(
            "with_text",
            |_, this, (content, font, font_size, r, g, b, a): (String, String, f32, u8, u8, u8, u8)| {
                this.cmd.text = Some(TextData {
                    content,
                    font,
                    font_size,
                    r,
                    g,
                    b,
                    a,
                });
                Ok(this.clone())
            },
        );

        // :with_menu(items, origin_x, origin_y, font, font_size, item_spacing, use_screen_space)
        // items is an array-like table of { id = "...", label = "..." }
        methods.add_method_mut(
            "with_menu",
            |_, this,
             (items_table, origin_x, origin_y, font, font_size, item_spacing, use_screen_space): (
                LuaTable,
                f32,
                f32,
                String,
                f32,
                f32,
                bool,
            )| {
                let mut items: Vec<(String, String)> = Vec::new();
                for value in items_table.sequence_values::<LuaTable>() {
                    let item_table = value?;
                    let id: String = item_table.get("id")?;
                    let label: String = item_table.get("label")?;
                    items.push((id, label));
                }

                this.cmd.menu = Some(MenuData {
                    items,
                    origin_x,
                    origin_y,
                    font,
                    font_size,
                    item_spacing,
                    use_screen_space,
                    ..MenuData::default()
                });
                Ok(this.clone())
            },
        );

        // :with_menu_colors(normal_r, normal_g, normal_b, normal_a, selected_r, selected_g, selected_b, selected_a)
        methods.add_method_mut(
            "with_menu_colors",
            |_, this, (nr, ng, nb, na, sr, sg, sb, sa): (u8, u8, u8, u8, u8, u8, u8, u8)| {
                let Some(ref mut menu) = this.cmd.menu else {
                    return Err(LuaError::runtime(
                        "with_menu_colors() requires with_menu() first",
                    ));
                };
                menu.normal_color = Some(ColorData {
                    r: nr,
                    g: ng,
                    b: nb,
                    a: na,
                });
                menu.selected_color = Some(ColorData {
                    r: sr,
                    g: sg,
                    b: sb,
                    a: sa,
                });
                Ok(this.clone())
            },
        );

        // :with_menu_dynamic_text(dynamic)
        methods.add_method_mut("with_menu_dynamic_text", |_, this, dynamic: bool| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_dynamic_text() requires with_menu() first",
                ));
            };
            menu.dynamic_text = Some(dynamic);
            Ok(this.clone())
        });

        // :with_menu_cursor(worldsignals_key)
        methods.add_method_mut("with_menu_cursor", |_, this, key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_cursor() requires with_menu() first",
                ));
            };
            menu.cursor_entity_key = Some(key);
            Ok(this.clone())
        });

        // :with_menu_selection_sound(sound_key)
        methods.add_method_mut("with_menu_selection_sound", |_, this, sound_key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_selection_sound() requires with_menu() first",
                ));
            };
            menu.selection_change_sound = Some(sound_key);
            Ok(this.clone())
        });

        // :with_menu_action_set_scene(item_id, scene)
        methods.add_method_mut(
            "with_menu_action_set_scene",
            |_, this, (item_id, scene): (String, String)| {
                let Some(ref mut menu) = this.cmd.menu else {
                    return Err(LuaError::runtime(
                        "with_menu_action_set_scene() requires with_menu() first",
                    ));
                };
                menu.actions
                    .push((item_id, MenuActionData::SetScene { scene }));
                Ok(this.clone())
            },
        );

        // :with_menu_action_show_submenu(item_id, submenu)
        methods.add_method_mut(
            "with_menu_action_show_submenu",
            |_, this, (item_id, submenu): (String, String)| {
                let Some(ref mut menu) = this.cmd.menu else {
                    return Err(LuaError::runtime(
                        "with_menu_action_show_submenu() requires with_menu() first",
                    ));
                };
                menu.actions
                    .push((item_id, MenuActionData::ShowSubMenu { menu: submenu }));
                Ok(this.clone())
            },
        );

        // :with_menu_action_quit(item_id)
        methods.add_method_mut("with_menu_action_quit", |_, this, item_id: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_quit() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::QuitGame));
            Ok(this.clone())
        });

        // :with_signals() - Add empty Signals component
        methods.add_method_mut("with_signals", |_, this, ()| {
            this.cmd.has_signals = true;
            Ok(this.clone())
        });

        // :with_phase(table) - Add LuaPhase component with phase definitions
        // Table format: { initial = "phase_name", phases = { phase_name = { on_enter = "fn", on_update = "fn", on_exit = "fn" } } }
        methods.add_method_mut("with_phase", |_, this, table: LuaTable| {
            let initial: String = table.get("initial")?;
            let mut phases = rustc_hash::FxHashMap::default();

            if let Ok(phases_table) = table.get::<LuaTable>("phases") {
                for pair in phases_table.pairs::<String, LuaTable>() {
                    let (phase_name, callbacks_table) = pair?;
                    let callbacks = PhaseCallbackData {
                        on_enter: callbacks_table.get("on_enter").ok(),
                        on_update: callbacks_table.get("on_update").ok(),
                        on_exit: callbacks_table.get("on_exit").ok(),
                    };
                    phases.insert(phase_name, callbacks);
                }
            }

            this.cmd.phase_data = Some(PhaseData { initial, phases });
            Ok(this.clone())
        });

        // :with_stuckto(target_entity_id, follow_x, follow_y) - Attach entity to another entity
        // target_entity_id is obtained from engine.get_entity()
        methods.add_method_mut(
            "with_stuckto",
            |_, this, (target_entity_id, follow_x, follow_y): (u64, bool, bool)| {
                this.cmd.stuckto = Some(StuckToData {
                    target_entity_id,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    follow_x,
                    follow_y,
                    stored_velocity: None,
                });
                Ok(this.clone())
            },
        );

        // :with_stuckto_offset(offset_x, offset_y) - Set offset for StuckTo component
        methods.add_method_mut(
            "with_stuckto_offset",
            |_, this, (offset_x, offset_y): (f32, f32)| {
                if let Some(ref mut stuckto) = this.cmd.stuckto {
                    stuckto.offset_x = offset_x;
                    stuckto.offset_y = offset_y;
                }
                Ok(this.clone())
            },
        );

        // :with_stuckto_stored_velocity(vx, vy) - Set velocity to restore when unstuck
        methods.add_method_mut(
            "with_stuckto_stored_velocity",
            |_, this, (vx, vy): (f32, f32)| {
                if let Some(ref mut stuckto) = this.cmd.stuckto {
                    stuckto.stored_velocity = Some((vx, vy));
                }
                Ok(this.clone())
            },
        );

        // :with_lua_timer(duration, callback) - Add LuaTimer component
        // LuaTimer calls a Lua function after duration seconds
        methods.add_method_mut(
            "with_lua_timer",
            |_, this, (duration, callback): (f32, String)| {
                this.cmd.lua_timer = Some((duration, callback));
                Ok(this.clone())
            },
        );

        // :with_signal_binding(key) - Bind DynamicText to a WorldSignal value
        // The text content will auto-update when the signal changes
        methods.add_method_mut("with_signal_binding", |_, this, key: String| {
            this.cmd.signal_binding = Some((key, None));
            Ok(this.clone())
        });

        // :with_signal_binding_format(format) - Set format string for signal binding
        // Use {} as placeholder, e.g., "Score: {}"
        methods.add_method_mut("with_signal_binding_format", |_, this, format: String| {
            if let Some((key, _)) = this.cmd.signal_binding.take() {
                this.cmd.signal_binding = Some((key, Some(format)));
            }
            Ok(this.clone())
        });

        // :with_grid_layout(path, group, zindex) - Add GridLayout component
        // Spawns entities from a JSON grid layout file
        methods.add_method_mut(
            "with_grid_layout",
            |_, this, (path, group, zindex): (String, String, i32)| {
                this.cmd.grid_layout = Some((path, group, zindex));
                Ok(this.clone())
            },
        );

        // :with_tween_position(from_x, from_y, to_x, to_y, duration) - Add TweenPosition component
        // Animates MapPosition from (from_x, from_y) to (to_x, to_y) over duration seconds
        // Defaults: easing = "linear", loop_mode = "once"
        methods.add_method_mut(
            "with_tween_position",
            |_, this, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
                this.cmd.tween_position = Some(TweenPositionData {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    duration,
                    easing: "linear".to_string(),
                    loop_mode: "once".to_string(),
                    backwards: false,
                });
                Ok(this.clone())
            },
        );

        // :with_tween_position_easing(easing) - Set easing for TweenPosition
        // Valid values: "linear", "quad_in", "quad_out", "quad_in_out", "cubic_in", "cubic_out", "cubic_in_out"
        methods.add_method_mut("with_tween_position_easing", |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_position {
                tween.easing = easing;
            }
            Ok(this.clone())
        });

        // :with_tween_position_loop(loop_mode) - Set loop mode for TweenPosition
        // Valid values: "once", "loop", "ping_pong"
        methods.add_method_mut("with_tween_position_loop", |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_position {
                tween.loop_mode = loop_mode;
            }
            Ok(this.clone())
        });

        // :with_tween_position_backwards() - Start position tween from the end, playing in reverse
        methods.add_method_mut("with_tween_position_backwards", |_, this, ()| {
            if let Some(ref mut tween) = this.cmd.tween_position {
                tween.backwards = true;
            }
            Ok(this.clone())
        });

        // :with_tween_rotation(from, to, duration) - Add TweenRotation component
        // Animates Rotation from `from` to `to` degrees over duration seconds
        // Defaults: easing = "linear", loop_mode = "once"
        methods.add_method_mut(
            "with_tween_rotation",
            |_, this, (from, to, duration): (f32, f32, f32)| {
                this.cmd.tween_rotation = Some(TweenRotationData {
                    from,
                    to,
                    duration,
                    easing: "linear".to_string(),
                    loop_mode: "once".to_string(),
                    backwards: false,
                });
                Ok(this.clone())
            },
        );

        // :with_tween_rotation_easing(easing) - Set easing for TweenRotation
        methods.add_method_mut("with_tween_rotation_easing", |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_rotation {
                tween.easing = easing;
            }
            Ok(this.clone())
        });

        // :with_tween_rotation_loop(loop_mode) - Set loop mode for TweenRotation
        methods.add_method_mut("with_tween_rotation_loop", |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_rotation {
                tween.loop_mode = loop_mode;
            }
            Ok(this.clone())
        });

        // :with_tween_rotation_backwards() - Start rotation tween from the end, playing in reverse
        methods.add_method_mut("with_tween_rotation_backwards", |_, this, ()| {
            if let Some(ref mut tween) = this.cmd.tween_rotation {
                tween.backwards = true;
            }
            Ok(this.clone())
        });

        // :with_tween_scale(from_x, from_y, to_x, to_y, duration) - Add TweenScale component
        // Animates Scale from (from_x, from_y) to (to_x, to_y) over duration seconds
        // Defaults: easing = "linear", loop_mode = "once"
        methods.add_method_mut(
            "with_tween_scale",
            |_, this, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
                this.cmd.tween_scale = Some(TweenScaleData {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    duration,
                    easing: "linear".to_string(),
                    loop_mode: "once".to_string(),
                    backwards: false,
                });
                Ok(this.clone())
            },
        );

        // :with_tween_scale_easing(easing) - Set easing for TweenScale
        methods.add_method_mut("with_tween_scale_easing", |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_scale {
                tween.easing = easing;
            }
            Ok(this.clone())
        });

        // :with_tween_scale_loop(loop_mode) - Set loop mode for TweenScale
        methods.add_method_mut("with_tween_scale_loop", |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_scale {
                tween.loop_mode = loop_mode;
            }
            Ok(this.clone())
        });

        // :with_tween_scale_backwards() - Start scale tween from the end, playing in reverse
        methods.add_method_mut("with_tween_scale_backwards", |_, this, ()| {
            if let Some(ref mut tween) = this.cmd.tween_scale {
                tween.backwards = true;
            }
            Ok(this.clone())
        });

        // :with_lua_collision_rule(group_a, group_b, callback) - Add LuaCollisionRule component
        // Registers a collision callback between two entity groups
        // callback is the name of a Lua function to call when collision occurs
        methods.add_method_mut(
            "with_lua_collision_rule",
            |_, this, (group_a, group_b, callback): (String, String, String)| {
                this.cmd.lua_collision_rule = Some(LuaCollisionRuleData {
                    group_a,
                    group_b,
                    callback,
                });
                Ok(this.clone())
            },
        );

        // :with_animation(animation_key) - Add Animation component
        // The animation_key refers to an animation registered via engine.register_animation()
        methods.add_method_mut("with_animation", |_, this, animation_key: String| {
            this.cmd.animation = Some(AnimationData { animation_key });
            Ok(this.clone())
        });

        // :with_animation_controller(fallback_key) - Add AnimationController component
        // The fallback_key is the default animation when no rules match
        methods.add_method_mut(
            "with_animation_controller",
            |_, this, fallback_key: String| {
                this.cmd.animation_controller = Some(AnimationControllerData {
                    fallback_key,
                    rules: Vec::new(),
                });
                Ok(this.clone())
            },
        );

        // :with_animation_rule(condition_table, set_key) - Add rule to AnimationController
        // condition_table format: { type = "has_flag", key = "moving" }
        // or { type = "lacks_flag", key = "moving" }
        // or { type = "scalar_cmp", key = "speed", op = "gt", value = 50.0 }
        // or { type = "scalar_range", key = "speed", min = 5.0, max = 50.0, inclusive = true }
        // or { type = "integer_cmp", key = "hp", op = "le", value = 0 }
        // or { type = "integer_range", key = "hp", min = 0, max = 10, inclusive = true }
        // or { type = "all", conditions = { ... } }
        // or { type = "any", conditions = { ... } }
        // or { type = "not", condition = { ... } }
        methods.add_method_mut(
            "with_animation_rule",
            |_, this, (condition_table, set_key): (LuaTable, String)| {
                fn parse_condition(table: &LuaTable) -> LuaResult<AnimationConditionData> {
                    let cond_type: String = table.get("type")?;
                    match cond_type.as_str() {
                        "has_flag" => {
                            let key: String = table.get("key")?;
                            Ok(AnimationConditionData::HasFlag { key })
                        }
                        "lacks_flag" => {
                            let key: String = table.get("key")?;
                            Ok(AnimationConditionData::LacksFlag { key })
                        }
                        "scalar_cmp" => {
                            let key: String = table.get("key")?;
                            let op: String = table.get("op")?;
                            let value: f32 = table.get("value")?;
                            Ok(AnimationConditionData::ScalarCmp { key, op, value })
                        }
                        "scalar_range" => {
                            let key: String = table.get("key")?;
                            let min: f32 = table.get("min")?;
                            let max: f32 = table.get("max")?;
                            let inclusive: bool = table.get("inclusive").unwrap_or(true);
                            Ok(AnimationConditionData::ScalarRange {
                                key,
                                min,
                                max,
                                inclusive,
                            })
                        }
                        "integer_cmp" => {
                            let key: String = table.get("key")?;
                            let op: String = table.get("op")?;
                            let value: i32 = table.get("value")?;
                            Ok(AnimationConditionData::IntegerCmp { key, op, value })
                        }
                        "integer_range" => {
                            let key: String = table.get("key")?;
                            let min: i32 = table.get("min")?;
                            let max: i32 = table.get("max")?;
                            let inclusive: bool = table.get("inclusive").unwrap_or(true);
                            Ok(AnimationConditionData::IntegerRange {
                                key,
                                min,
                                max,
                                inclusive,
                            })
                        }
                        "all" => {
                            let conditions_table: LuaTable = table.get("conditions")?;
                            let mut conditions = Vec::new();
                            for value in conditions_table.sequence_values::<LuaTable>() {
                                conditions.push(parse_condition(&value?)?);
                            }
                            Ok(AnimationConditionData::All(conditions))
                        }
                        "any" => {
                            let conditions_table: LuaTable = table.get("conditions")?;
                            let mut conditions = Vec::new();
                            for value in conditions_table.sequence_values::<LuaTable>() {
                                conditions.push(parse_condition(&value?)?);
                            }
                            Ok(AnimationConditionData::Any(conditions))
                        }
                        "not" => {
                            let inner_table: LuaTable = table.get("condition")?;
                            let inner = parse_condition(&inner_table)?;
                            Ok(AnimationConditionData::Not(Box::new(inner)))
                        }
                        _ => Err(LuaError::runtime(format!(
                            "Unknown condition type: {}",
                            cond_type
                        ))),
                    }
                }

                let Some(ref mut controller) = this.cmd.animation_controller else {
                    return Err(LuaError::runtime(
                        "with_animation_rule() requires with_animation_controller() first",
                    ));
                };

                let condition = parse_condition(&condition_table)?;
                controller
                    .rules
                    .push(AnimationRuleData { condition, set_key });
                Ok(this.clone())
            },
        );

        // :register_as(key) - Register spawned entity in WorldSignals with this key
        // This allows Lua to retrieve the entity ID later via engine.get_entity(key)
        methods.add_method_mut("register_as", |_, this, key: String| {
            this.cmd.register_as = Some(key);
            Ok(this.clone())
        });

        // :build() - Queue the entity for spawning
        methods.add_method("build", |lua, this, ()| {
            lua.app_data_ref::<LuaAppData>()
                .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                .spawn_commands
                .borrow_mut()
                .push(this.cmd.clone());
            Ok(())
        });
    }
}

/// Entity builder for collision callbacks.
///
/// Similar to `LuaEntityBuilder` but pushes to the collision spawn queue
/// which is processed immediately after collision callbacks.
#[derive(Debug, Clone, Default)]
pub struct LuaCollisionEntityBuilder {
    cmd: SpawnCmd,
}

impl LuaCollisionEntityBuilder {
    pub fn new() -> Self {
        Self {
            cmd: SpawnCmd::default(),
        }
    }
}

impl LuaUserData for LuaCollisionEntityBuilder {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // :with_group(name) - Set entity group
        methods.add_method_mut("with_group", |_, this, name: String| {
            this.cmd.group = Some(name);
            Ok(this.clone())
        });

        // :with_position(x, y) - Set world position
        methods.add_method_mut("with_position", |_, this, (x, y): (f32, f32)| {
            this.cmd.position = Some((x, y));
            Ok(this.clone())
        });

        // :with_sprite(tex_key, width, height, origin_x, origin_y) - Set sprite
        methods.add_method_mut(
            "with_sprite",
            |_, this, (tex_key, width, height, origin_x, origin_y): (String, f32, f32, f32, f32)| {
                this.cmd.sprite = Some(SpriteData {
                    tex_key,
                    width,
                    height,
                    origin_x,
                    origin_y,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    flip_h: false,
                    flip_v: false,
                });
                Ok(this.clone())
            },
        );

        // :with_sprite_offset(offset_x, offset_y) - Set sprite offset
        methods.add_method_mut(
            "with_sprite_offset",
            |_, this, (offset_x, offset_y): (f32, f32)| {
                if let Some(ref mut sprite) = this.cmd.sprite {
                    sprite.offset_x = offset_x;
                    sprite.offset_y = offset_y;
                }
                Ok(this.clone())
            },
        );

        // :with_sprite_flip(flip_h, flip_v) - Set sprite flipping
        methods.add_method_mut(
            "with_sprite_flip",
            |_, this, (flip_h, flip_v): (bool, bool)| {
                if let Some(ref mut sprite) = this.cmd.sprite {
                    sprite.flip_h = flip_h;
                    sprite.flip_v = flip_v;
                }
                Ok(this.clone())
            },
        );

        // :with_zindex(z) - Set render order
        methods.add_method_mut("with_zindex", |_, this, z: i32| {
            this.cmd.zindex = Some(z);
            Ok(this.clone())
        });

        // :with_velocity(vx, vy) - Set RigidBody velocity
        // Creates a RigidBody if one doesn't exist, otherwise updates velocity
        methods.add_method_mut("with_velocity", |_, this, (vx, vy): (f32, f32)| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.velocity_x = vx;
                rb.velocity_y = vy;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    velocity_x: vx,
                    velocity_y: vy,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        });

        // :with_friction(friction) - Set RigidBody friction (velocity damping)
        // Creates a RigidBody if one doesn't exist
        methods.add_method_mut("with_friction", |_, this, friction: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.friction = friction;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    friction,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        });

        // :with_max_speed(speed) - Set RigidBody max_speed clamp
        // Creates a RigidBody if one doesn't exist
        methods.add_method_mut("with_max_speed", |_, this, speed: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.max_speed = Some(speed);
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    max_speed: Some(speed),
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        });

        // :with_accel(name, x, y, enabled) - Add a named acceleration force
        // Creates a RigidBody if one doesn't exist
        methods.add_method_mut(
            "with_accel",
            |_, this, (name, x, y, enabled): (String, f32, f32, bool)| {
                if let Some(ref mut rb) = this.cmd.rigidbody {
                    rb.forces.push(ForceData {
                        name,
                        x,
                        y,
                        enabled,
                    });
                } else {
                    this.cmd.rigidbody = Some(RigidBodyData {
                        forces: vec![ForceData {
                            name,
                            x,
                            y,
                            enabled,
                        }],
                        ..RigidBodyData::default()
                    });
                }
                Ok(this.clone())
            },
        );

        // :with_frozen() - Mark entity as frozen (physics skipped)
        // Creates a RigidBody if one doesn't exist
        methods.add_method_mut("with_frozen", |_, this, ()| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.frozen = true;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    frozen: true,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        });

        // :with_collider(width, height, origin_x, origin_y) - Set BoxCollider
        methods.add_method_mut(
            "with_collider",
            |_, this, (width, height, origin_x, origin_y): (f32, f32, f32, f32)| {
                this.cmd.collider = Some(ColliderData {
                    width,
                    height,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    origin_x,
                    origin_y,
                });
                Ok(this.clone())
            },
        );

        // :with_collider_offset(offset_x, offset_y) - Set collider offset
        methods.add_method_mut(
            "with_collider_offset",
            |_, this, (offset_x, offset_y): (f32, f32)| {
                if let Some(ref mut collider) = this.cmd.collider {
                    collider.offset_x = offset_x;
                    collider.offset_y = offset_y;
                }
                Ok(this.clone())
            },
        );

        // :with_rotation(degrees) - Set rotation
        methods.add_method_mut("with_rotation", |_, this, degrees: f32| {
            this.cmd.rotation = Some(degrees);
            Ok(this.clone())
        });

        // :with_scale(sx, sy) - Set scale
        methods.add_method_mut("with_scale", |_, this, (sx, sy): (f32, f32)| {
            this.cmd.scale = Some((sx, sy));
            Ok(this.clone())
        });

        // :with_signal_integer(key, value) - Add an integer signal
        methods.add_method_mut(
            "with_signal_integer",
            |_, this, (key, value): (String, i32)| {
                this.cmd.signal_integers.push((key, value));
                Ok(this.clone())
            },
        );

        // :with_signal_flag(key) - Add a flag signal
        methods.add_method_mut("with_signal_flag", |_, this, key: String| {
            this.cmd.signal_flags.push(key);
            Ok(this.clone())
        });

        // :with_signals() - Add empty Signals component
        methods.add_method_mut("with_signals", |_, this, ()| {
            this.cmd.has_signals = true;
            Ok(this.clone())
        });

        // :with_lua_timer(duration, callback) - Add LuaTimer component
        methods.add_method_mut(
            "with_lua_timer",
            |_, this, (duration, callback): (f32, String)| {
                this.cmd.lua_timer = Some((duration, callback));
                Ok(this.clone())
            },
        );

        // :with_animation(animation_key) - Add Animation component
        methods.add_method_mut("with_animation", |_, this, animation_key: String| {
            this.cmd.animation = Some(AnimationData { animation_key });
            Ok(this.clone())
        });

        // :build() - Queue the entity for spawning in collision context
        methods.add_method("build", |lua, this, ()| {
            lua.app_data_ref::<LuaAppData>()
                .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                .collision_spawn_commands
                .borrow_mut()
                .push(this.cmd.clone());
            Ok(())
        });
    }
}
