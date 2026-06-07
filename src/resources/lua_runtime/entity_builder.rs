//! Entity builder for fluent entity construction from Lua.
//!
//! This module provides the `LuaEntityBuilder` struct which implements
//! a fluent interface for building entities from Lua scripts using method chaining.
//!
//! The builder supports both spawning new entities and cloning existing ones,
//! in both regular and collision contexts.

use super::commands::{CloneCmd, UniformValue};
use super::runtime::LuaAppData;
use super::spawn_data::*;
use super::stub_meta::BuilderMethodDef;
use mlua::prelude::*;
use mlua::MaybeSend;

/// Parse a Lua value into a UniformValue.
///
/// Numbers are treated as Float, tables of length 2 as Vec2, and tables of length 4 as Vec4.
fn parse_uniform_value(val: LuaValue) -> LuaResult<UniformValue> {
    match val {
        LuaValue::Number(n) => Ok(UniformValue::Float(n as f32)),
        LuaValue::Integer(n) => Ok(UniformValue::Float(n as f32)),
        LuaValue::Table(t) => {
            let len = t.raw_len();
            match len {
                2 => {
                    let x: f32 = t.get(1)?;
                    let y: f32 = t.get(2)?;
                    Ok(UniformValue::Vec2 { x, y })
                }
                4 => {
                    let x: f32 = t.get(1)?;
                    let y: f32 = t.get(2)?;
                    let z: f32 = t.get(3)?;
                    let w: f32 = t.get(4)?;
                    Ok(UniformValue::Vec4 { x, y, z, w })
                }
                _ => Err(LuaError::runtime(
                    "Uniform table must be array of length 2 (vec2) or 4 (vec4)",
                )),
            }
        }
        _ => Err(LuaError::runtime(
            "Uniform value must be number or array table",
        )),
    }
}

/// Builder mode: spawn a new entity or clone an existing one.
#[derive(Debug, Clone, Copy, Default)]
pub enum BuilderMode {
    /// Spawn a new entity from scratch
    #[default]
    Spawn,
    /// Clone an existing entity (looked up by WorldSignals key)
    Clone,
}

/// Builder context: regular or collision callback.
#[derive(Debug, Clone, Copy, Default)]
pub enum BuilderContext {
    /// Regular context (scene setup, phase callbacks, timer callbacks)
    #[default]
    Regular,
    /// Collision callback context (processed immediately after collision)
    Collision,
}

/// Entity builder exposed to Lua for fluent entity construction.
///
/// This struct implements `UserData` so Lua can call methods on it using
/// the colon syntax: `engine.spawn():with_position(x, y):build()`
///
/// Each `with_*` method returns `Self` to allow chaining.
/// The `build()` method queues the entity for spawning or cloning.
#[derive(Debug, Clone, Default)]
pub struct LuaEntityBuilder {
    mode: BuilderMode,
    context: BuilderContext,
    /// Only used in Clone mode - WorldSignals key for source entity
    source_key: Option<String>,
    cmd: SpawnCmd,
}

impl LuaEntityBuilder {
    /// Create a new spawn builder (regular context).
    pub fn new() -> Self {
        Self {
            mode: BuilderMode::Spawn,
            context: BuilderContext::Regular,
            source_key: None,
            cmd: SpawnCmd::default(),
        }
    }

    /// Create a new spawn builder (collision context).
    pub fn new_collision() -> Self {
        Self {
            mode: BuilderMode::Spawn,
            context: BuilderContext::Collision,
            source_key: None,
            cmd: SpawnCmd::default(),
        }
    }

    /// Create a new clone builder (regular context).
    pub fn new_clone(source_key: String) -> Self {
        Self {
            mode: BuilderMode::Clone,
            context: BuilderContext::Regular,
            source_key: Some(source_key),
            cmd: SpawnCmd::default(),
        }
    }

    /// Create a new clone builder (collision context).
    pub fn new_collision_clone(source_key: String) -> Self {
        Self {
            mode: BuilderMode::Clone,
            context: BuilderContext::Collision,
            source_key: Some(source_key),
            cmd: SpawnCmd::default(),
        }
    }
}

/// Registers a `with_*` builder method and, when a metadata collector is present, records its
/// stub info. The PARAMS const inside the macro body is `'static` because const items always are.
macro_rules! builder_method {
    (
        $methods:expr, $meta:expr,
        $name:literal, $desc:literal,
        [$( ($pname:literal, $ptype:literal) ),* $(,)?],
        $closure:expr
    ) => {
        $methods.add_method_mut($name, $closure);
        if let Some(collector) = $meta.as_mut() {
            const PARAMS: &[(&str, &str)] = &[$( ($pname, $ptype) ),*];
            collector.push(($name, $desc, PARAMS));
        }
    };
}

/// No-op `UserDataMethods` impl used only by `collect_builder_meta` to harvest stub metadata
/// without performing real method registration.
struct DummyMethods;

impl LuaUserDataMethods<LuaEntityBuilder> for DummyMethods {
    fn add_method<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: Fn(&Lua, &LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_method_mut<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: FnMut(&Lua, &mut LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_function<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: Fn(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        unimplemented!()
    }

    fn add_function_mut<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: FnMut(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        unimplemented!()
    }

    fn add_meta_method<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: Fn(&Lua, &LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        unimplemented!()
    }

    fn add_meta_method_mut<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: FnMut(&Lua, &mut LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        unimplemented!()
    }

    fn add_meta_function<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: Fn(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        unimplemented!()
    }

    fn add_meta_function_mut<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: FnMut(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        unimplemented!()
    }
}

/// Collect stub metadata for all builder methods.
/// Used by the stub generator path only; has no effect during normal gameplay.
pub fn collect_builder_meta() -> Vec<BuilderMethodDef> {
    let mut meta = Some(Vec::new());
    let mut dummy = DummyMethods;
    register_methods(&mut dummy, &mut meta);
    let mut v = meta.unwrap();
    // register_as and build are not with_* methods so the macro doesn't capture them;
    // append their entries manually so the stub generator includes them.
    const REGISTER_AS_PARAMS: &[(&str, &str)] = &[("key", "string")];
    v.push(("register_as", "Register entity in WorldSignals for later retrieval", REGISTER_AS_PARAMS));
    v.push(("build", "Queue entity for spawning or cloning", &[]));
    v
}

fn register_methods<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods, meta,
        "with_group", "Set entity group",
        [("name", "string")],
        |_, this, name: String| {
            this.cmd.group = Some(name);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_position", "Set world position",
        [("x", "number"), ("y", "number")],
        |_, this, (x, y): (f32, f32)| {
            this.cmd.position = Some((x, y));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_sprite", "Set sprite",
        [
            ("tex_key", "string"),
            ("width", "number"),
            ("height", "number"),
            ("origin_x", "number"),
            ("origin_y", "number"),
        ],
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
        }
    );

    builder_method!(
        methods, meta,
        "with_sprite_offset", "Set sprite offset",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this, (offset_x, offset_y): (f32, f32)| {
            if let Some(ref mut sprite) = this.cmd.sprite {
                sprite.offset_x = offset_x;
                sprite.offset_y = offset_y;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_sprite_flip", "Set sprite flipping",
        [("flip_h", "boolean"), ("flip_v", "boolean")],
        |_, this, (flip_h, flip_v): (bool, bool)| {
            if let Some(ref mut sprite) = this.cmd.sprite {
                sprite.flip_h = flip_h;
                sprite.flip_v = flip_v;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_zindex", "Set render order",
        [("z", "number")],
        |_, this, z: f32| {
            this.cmd.zindex = Some(z);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_velocity", "Set velocity (creates RigidBody if needed)",
        [("vx", "number"), ("vy", "number")],
        |_, this, (vx, vy): (f32, f32)| {
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
        }
    );

    builder_method!(
        methods, meta,
        "with_friction", "Set friction (creates RigidBody if needed)",
        [("friction", "number")],
        |_, this, friction: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.friction = friction;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    friction,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_max_speed", "Set max speed clamp (creates RigidBody if needed)",
        [("speed", "number")],
        |_, this, speed: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.max_speed = Some(speed);
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    max_speed: Some(speed),
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_accel", "Add a named acceleration force",
        [
            ("name", "string"),
            ("x", "number"),
            ("y", "number"),
            ("enabled", "boolean"),
        ],
        |_, this, (name, x, y, enabled): (String, f32, f32, bool)| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.forces.push(ForceData { name, x, y, enabled });
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    forces: vec![ForceData { name, x, y, enabled }],
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_frozen", "Mark entity as frozen (physics skipped)",
        [],
        |_, this, (): ()| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.frozen = true;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    frozen: true,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_collider", "Set box collider",
        [
            ("width", "number"),
            ("height", "number"),
            ("origin_x", "number"),
            ("origin_y", "number"),
        ],
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
        }
    );

    builder_method!(
        methods, meta,
        "with_collider_offset", "Set collider offset",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this, (offset_x, offset_y): (f32, f32)| {
            if let Some(ref mut collider) = this.cmd.collider {
                collider.offset_x = offset_x;
                collider.offset_y = offset_y;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_mouse_controlled", "Enable mouse position tracking",
        [("follow_x", "boolean"), ("follow_y", "boolean")],
        |_, this, (follow_x, follow_y): (bool, bool)| {
            this.cmd.mouse_controlled = Some((follow_x, follow_y));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_rotation", "Set rotation in degrees",
        [("degrees", "number")],
        |_, this, degrees: f32| {
            this.cmd.rotation = Some(degrees);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_scale", "Set scale",
        [("sx", "number"), ("sy", "number")],
        |_, this, (sx, sy): (f32, f32)| {
            this.cmd.scale = Some((sx, sy));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_persistent", "Survive scene transitions",
        [],
        |_, this, (): ()| {
            this.cmd.persistent = true;
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_scalar", "Add a scalar signal",
        [("key", "string"), ("value", "number")],
        |_, this, (key, value): (String, f32)| {
            this.cmd.signal_scalars.push((key, value));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_integer", "Add an integer signal",
        [("key", "string"), ("value", "integer")],
        |_, this, (key, value): (String, i32)| {
            this.cmd.signal_integers.push((key, value));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_flag", "Add a flag signal",
        [("key", "string")],
        |_, this, key: String| {
            this.cmd.signal_flags.push(key);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_string", "Add a string signal",
        [("key", "string"), ("value", "string")],
        |_, this, (key, value): (String, String)| {
            this.cmd.signal_strings.push((key, value));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_screen_position", "Set screen position (UI elements)",
        [("x", "number"), ("y", "number")],
        |_, this, (x, y): (f32, f32)| {
            this.cmd.screen_position = Some((x, y));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_text", "Set DynamicText component",
        [
            ("content", "string"),
            ("font", "string"),
            ("font_size", "number"),
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer"),
        ],
        |_, this, (content, font, font_size, r, g, b, a): (String, String, f32, u8, u8, u8, u8)| {
            this.cmd.text = Some(TextData { content, font, font_size, r, g, b, a });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu", "Add interactive menu",
        [
            ("items", "table"),
            ("origin_x", "number"),
            ("origin_y", "number"),
            ("font", "string"),
            ("font_size", "number"),
            ("item_spacing", "number"),
            ("use_screen_space", "boolean"),
        ],
        |_, this, (items_table, origin_x, origin_y, font, font_size, item_spacing, use_screen_space): (LuaTable, f32, f32, String, f32, f32, bool)| {
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
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_colors", "Set menu normal/selected colors (RGBA)",
        [
            ("nr", "integer"),
            ("ng", "integer"),
            ("nb", "integer"),
            ("na", "integer"),
            ("sr", "integer"),
            ("sg", "integer"),
            ("sb", "integer"),
            ("sa", "integer"),
        ],
        |_, this, (nr, ng, nb, na, sr, sg, sb, sa): (u8, u8, u8, u8, u8, u8, u8, u8)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_colors() requires with_menu() first",
                ));
            };
            menu.normal_color = Some(ColorData { r: nr, g: ng, b: nb, a: na });
            menu.selected_color = Some(ColorData { r: sr, g: sg, b: sb, a: sa });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_dynamic_text", "Enable dynamic text updates for menu items",
        [("dynamic", "boolean")],
        |_, this, dynamic: bool| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_dynamic_text() requires with_menu() first",
                ));
            };
            menu.dynamic_text = Some(dynamic);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_cursor", "Set cursor entity for menu",
        [("key", "string")],
        |_, this, key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_cursor() requires with_menu() first",
                ));
            };
            menu.cursor_entity_key = Some(key);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_selection_sound", "Set sound for menu selection changes",
        [("sound_key", "string")],
        |_, this, sound_key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_selection_sound() requires with_menu() first",
                ));
            };
            menu.selection_change_sound = Some(sound_key);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_action_set_scene", "Set scene-switch action for menu item",
        [("item_id", "string"), ("scene", "string")],
        |_, this, (item_id, scene): (String, String)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_set_scene() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::SetScene { scene }));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_action_show_submenu", "Set submenu action for menu item",
        [("item_id", "string"), ("submenu", "string")],
        |_, this, (item_id, submenu): (String, String)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_show_submenu() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::ShowSubMenu { menu: submenu }));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_action_quit", "Set quit action for menu item",
        [("item_id", "string")],
        |_, this, item_id: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_quit() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::QuitGame));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_callback", "Set Lua callback for menu selection",
        [("callback", "string")],
        |_, this, callback: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_callback() requires with_menu() first",
                ));
            };
            menu.on_select_callback = Some(callback);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_visible_count", "Set max visible menu items (enables scrolling)",
        [("count", "integer")],
        |_, this, count: usize| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_visible_count() requires with_menu() first",
                ));
            };
            menu.visible_count = Some(count);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_signals", "Add empty Signals component",
        [],
        |_, this, (): ()| {
            this.cmd.has_signals = true;
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_phase",
        "Add phase state machine\n\nExample:\n```lua\nengine.spawn()\n    :with_phase({\n        initial = \"idle\",\n        phases = {\n            idle = {\n                on_enter = \"on_idle_enter\",\n                on_update = \"on_idle_update\",\n                on_exit = \"on_idle_exit\"\n            },\n            moving = { on_enter = \"on_moving_enter\" }\n        }\n    })\n    :build()\n```",
        [("table", "table")],
        |_, this, table: LuaTable| {
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
        }
    );

    builder_method!(
        methods, meta,
        "with_stuckto", "Attach entity to a target entity",
        [
            ("target_entity_id", "integer"),
            ("follow_x", "boolean"),
            ("follow_y", "boolean"),
        ],
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
        }
    );

    builder_method!(
        methods, meta,
        "with_stuckto_offset", "Set offset for StuckTo",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this, (offset_x, offset_y): (f32, f32)| {
            if let Some(ref mut stuckto) = this.cmd.stuckto {
                stuckto.offset_x = offset_x;
                stuckto.offset_y = offset_y;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_stuckto_stored_velocity", "Set velocity to restore when unstuck",
        [("vx", "number"), ("vy", "number")],
        |_, this, (vx, vy): (f32, f32)| {
            if let Some(ref mut stuckto) = this.cmd.stuckto {
                stuckto.stored_velocity = Some((vx, vy));
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_lua_timer", "Add a Lua timer callback",
        [("duration", "number"), ("callback", "string")],
        |_, this, (duration, callback): (f32, String)| {
            this.cmd.lua_timer = Some((duration, callback));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_ttl", "Set time-to-live (auto-despawn)",
        [("seconds", "number")],
        |_, this, seconds: f32| {
            this.cmd.ttl = Some(seconds);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_binding", "Bind text to a WorldSignal value",
        [("key", "string")],
        |_, this, key: String| {
            this.cmd.signal_binding = Some((key, None));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_binding_format", "Set format string for signal binding (use {} as placeholder)",
        [("format", "string")],
        |_, this, format: String| {
            if let Some((key, _)) = this.cmd.signal_binding.take() {
                this.cmd.signal_binding = Some((key, Some(format)));
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_grid_layout", "Spawn entities from a JSON grid layout",
        [("path", "string"), ("group", "string"), ("zindex", "number")],
        |_, this, (path, group, zindex): (String, String, f32)| {
            this.cmd.grid_layout = Some((path, group, zindex));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_position", "Add position tween animation",
        [
            ("from_x", "number"),
            ("from_y", "number"),
            ("to_x", "number"),
            ("to_y", "number"),
            ("duration", "number"),
        ],
        |_, this, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
            this.cmd.tween_position = Some(TweenPositionData {
                from_x,
                from_y,
                to_x,
                to_y,
                config: TweenConfig::new(duration),
            });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_position_easing", "Set easing for position tween",
        [("easing", "string")],
        |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_position {
                tween.config.easing = easing;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_position_loop", "Set loop mode for position tween",
        [("loop_mode", "string")],
        |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_position {
                tween.config.loop_mode = loop_mode;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_position_backwards", "Start position tween in reverse",
        [],
        |_, this, (): ()| {
            if let Some(ref mut tween) = this.cmd.tween_position {
                tween.config.backwards = true;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation", "Add rotation tween animation",
        [("from", "number"), ("to", "number"), ("duration", "number")],
        |_, this, (from, to, duration): (f32, f32, f32)| {
            this.cmd.tween_rotation = Some(TweenRotationData {
                from,
                to,
                config: TweenConfig::new(duration),
            });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation_easing", "Set easing for rotation tween",
        [("easing", "string")],
        |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_rotation {
                tween.config.easing = easing;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation_loop", "Set loop mode for rotation tween",
        [("loop_mode", "string")],
        |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_rotation {
                tween.config.loop_mode = loop_mode;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation_backwards", "Start rotation tween in reverse",
        [],
        |_, this, (): ()| {
            if let Some(ref mut tween) = this.cmd.tween_rotation {
                tween.config.backwards = true;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_scale", "Add scale tween animation",
        [
            ("from_x", "number"),
            ("from_y", "number"),
            ("to_x", "number"),
            ("to_y", "number"),
            ("duration", "number"),
        ],
        |_, this, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
            this.cmd.tween_scale = Some(TweenScaleData {
                from_x,
                from_y,
                to_x,
                to_y,
                config: TweenConfig::new(duration),
            });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_scale_easing", "Set easing for scale tween",
        [("easing", "string")],
        |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_scale {
                tween.config.easing = easing;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_scale_loop", "Set loop mode for scale tween",
        [("loop_mode", "string")],
        |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_scale {
                tween.config.loop_mode = loop_mode;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_scale_backwards", "Start scale tween in reverse",
        [],
        |_, this, (): ()| {
            if let Some(ref mut tween) = this.cmd.tween_scale {
                tween.config.backwards = true;
            }
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_lua_collision_rule", "Add collision callback between two groups",
        [("group_a", "string"), ("group_b", "string"), ("callback", "string")],
        |_, this, (group_a, group_b, callback): (String, String, String)| {
            this.cmd.lua_collision_rule = Some(LuaCollisionRuleData {
                group_a,
                group_b,
                callback,
            });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_animation", "Set animation by key",
        [("animation_key", "string")],
        |_, this, animation_key: String| {
            this.cmd.animation = Some(AnimationData { animation_key });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_animation_controller", "Add animation controller with fallback",
        [("fallback_key", "string")],
        |_, this, fallback_key: String| {
            this.cmd.animation_controller = Some(AnimationControllerData {
                fallback_key,
                rules: Vec::new(),
            });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_animation_rule", "Add animation rule to controller",
        [("condition_table", "table"), ("set_key", "string")],
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
                        Ok(AnimationConditionData::ScalarRange { key, min, max, inclusive })
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
                        Ok(AnimationConditionData::IntegerRange { key, min, max, inclusive })
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
            controller.rules.push(AnimationRuleData { condition, set_key });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_particle_emitter", "Add particle emitter",
        [("table", "table")],
        |_, this, table: LuaTable| {
            use super::spawn_data::{ParticleEmitterData, ParticleEmitterShapeData, ParticleTtlData};

            let mut data = ParticleEmitterData::default();

            if let Ok(templates_table) = table.get::<LuaTable>("templates") {
                let mut keys = Vec::new();
                for key in templates_table.sequence_values::<String>().flatten() {
                    keys.push(key);
                }
                data.template_keys = keys;
            }

            if let Ok(shape_value) = table.get::<LuaValue>("shape") {
                match shape_value {
                    LuaValue::String(s) if s.to_string_lossy() == "point" => {
                        data.shape = ParticleEmitterShapeData::Point;
                    }
                    LuaValue::Table(shape_table) => {
                        let kind: String = shape_table
                            .get("kind")
                            .or_else(|_| shape_table.get("type"))
                            .unwrap_or_default();
                        if kind == "rect" {
                            let width: f32 = shape_table.get("width").unwrap_or(0.0);
                            let height: f32 = shape_table.get("height").unwrap_or(0.0);
                            data.shape = ParticleEmitterShapeData::Rect { width, height };
                        }
                    }
                    _ => {}
                }
            }

            if let Ok(offset_table) = table.get::<LuaTable>("offset") {
                data.offset_x = offset_table.get("x").unwrap_or(0.0);
                data.offset_y = offset_table.get("y").unwrap_or(0.0);
            }

            if let Ok(v) = table.get::<u32>("particles_per_emission") {
                data.particles_per_emission = v;
            }
            if let Ok(v) = table.get::<f32>("emissions_per_second") {
                data.emissions_per_second = v;
            }
            if let Ok(v) = table.get::<u32>("emissions_remaining") {
                data.emissions_remaining = v;
            }

            if let Ok(arc_table) = table.get::<LuaTable>("arc") {
                let min: f32 = arc_table.get(1).unwrap_or(0.0);
                let max: f32 = arc_table.get(2).unwrap_or(360.0);
                if min <= max {
                    data.arc_min_deg = min;
                    data.arc_max_deg = max;
                } else {
                    data.arc_min_deg = max;
                    data.arc_max_deg = min;
                }
            }

            if let Ok(speed_table) = table.get::<LuaTable>("speed") {
                let min: f32 = speed_table.get(1).unwrap_or(50.0);
                let max: f32 = speed_table.get(2).unwrap_or(100.0);
                if min <= max {
                    data.speed_min = min;
                    data.speed_max = max;
                } else {
                    data.speed_min = max;
                    data.speed_max = min;
                }
            }

            if let Ok(ttl_value) = table.get::<LuaValue>("ttl") {
                match ttl_value {
                    LuaValue::String(s) if s.to_string_lossy() == "none" => {
                        data.ttl = ParticleTtlData::None;
                    }
                    LuaValue::Number(n) => {
                        data.ttl = ParticleTtlData::Fixed((n as f32).max(0.0));
                    }
                    LuaValue::Integer(n) => {
                        data.ttl = ParticleTtlData::Fixed((n as f32).max(0.0));
                    }
                    LuaValue::Table(ttl_table) => {
                        let min: f32 = ttl_table.get("min").unwrap_or(0.0);
                        let max: f32 = ttl_table.get("max").unwrap_or(0.0);
                        let (min, max) = if min <= max { (min, max) } else { (max, min) };
                        data.ttl = ParticleTtlData::Range {
                            min: min.max(0.0),
                            max: max.max(0.0),
                        };
                    }
                    _ => {}
                }
            }

            this.cmd.particle_emitter = Some(data);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tint", "Set color tint (RGBA 0-255)",
        [("r", "integer"), ("g", "integer"), ("b", "integer"), ("a", "integer")],
        |_, this, (r, g, b, a): (u8, u8, u8, u8)| {
            this.cmd.tint = Some((r, g, b, a));
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_shader", "Set per-entity shader with optional uniforms",
        [("shader_key", "string"), ("uniforms", "table?")],
        |_, this, args: mlua::MultiValue| {
            let mut iter = args.into_iter();
            let key_val = iter
                .next()
                .ok_or_else(|| LuaError::runtime("with_shader requires shader_key"))?;
            let shader_key: String = key_val
                .as_string()
                .and_then(|s| s.to_str().ok())
                .ok_or_else(|| LuaError::runtime("shader_key must be string"))?
                .to_string();

            let mut uniforms = Vec::new();

            if let Some(table_val) = iter.next()
                && let Some(table) = table_val.as_table()
            {
                for pair in table.pairs::<String, LuaValue>() {
                    let (name, val) = pair?;
                    let uniform = parse_uniform_value(val)?;
                    uniforms.push((name, uniform));
                }
            }

            this.cmd.shader = Some(EntityShaderData { key: shader_key, uniforms });
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_parent", "Set parent entity for transform hierarchy",
        [("parent_id", "integer")],
        |_, this, parent_id: u64| {
            this.cmd.parent = Some(parent_id);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_tilemap",
        "Spawn a tilemap root. All tile entities become ChildOf children so the root's position/scale/rotation transforms the whole tilemap.",
        [("path", "string")],
        |_, this, path: String| {
            this.cmd.tilemap_path = Some(path);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_lua_setup",
        "Attach a one-shot Lua setup callback. The named function is called once (Added<LuaSetup>) with the entity context. Fires the frame after spawn; child entities added inside the callback appear the following frame.",
        [("callback", "string")],
        |_, this, callback: String| {
            this.cmd.lua_setup = Some(callback);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_on_animation_end",
        "Attach a callback fired exactly once when the entity's non-looped animation first reaches its last frame. Signature: fn(ctx, input). Looped animations never trigger it.",
        [("fn_name", "string")],
        |_, this, callback: String| {
            this.cmd.lua_on_animation_end = Some(callback);
            Ok(this.clone())
        }
    );

    builder_method!(
        methods, meta,
        "with_camera_target",
        "Mark entity as camera follow target (higher priority wins). zoom is the desired camera zoom when this target wins (default 1.0).",
        [("priority", "integer?"), ("zoom", "number?")],
        |_, this, (priority, zoom): (Option<u8>, Option<f32>)| {
            this.cmd.camera_target = Some(priority.unwrap_or(0));
            this.cmd.camera_target_zoom = zoom;
            Ok(this.clone())
        }
    );

    // Non-builder-stub methods — kept as plain registrations.
    // These have their own entries in stub_meta's non-BUILDER_METHODS sections.

    methods.add_method_mut("register_as", |_, this, key: String| {
        this.cmd.register_as = Some(key);
        Ok(this.clone())
    });

    methods.add_method("build", |lua, this, ()| {
        let app_data = lua
            .app_data_ref::<LuaAppData>()
            .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;

        match (this.mode, this.context) {
            (BuilderMode::Spawn, BuilderContext::Regular) => {
                app_data.spawn_commands.borrow_mut().push(this.cmd.clone());
            }
            (BuilderMode::Spawn, BuilderContext::Collision) => {
                app_data
                    .collision_spawn_commands
                    .borrow_mut()
                    .push(this.cmd.clone());
            }
            (BuilderMode::Clone, BuilderContext::Regular) => {
                let source_key = this.source_key.clone().unwrap_or_default();
                app_data.clone_commands.borrow_mut().push(CloneCmd {
                    source_key,
                    overrides: this.cmd.clone(),
                });
            }
            (BuilderMode::Clone, BuilderContext::Collision) => {
                let source_key = this.source_key.clone().unwrap_or_default();
                app_data
                    .collision_clone_commands
                    .borrow_mut()
                    .push(CloneCmd {
                        source_key,
                        overrides: this.cmd.clone(),
                    });
            }
        }
        Ok(())
    });
}

impl LuaUserData for LuaEntityBuilder {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        register_methods(methods, &mut None);
    }
}
