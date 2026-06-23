//! Entity builder for fluent entity construction from Lua.
//!
//! This module provides the `LuaEntityBuilder` struct which implements
//! a fluent interface for building entities from Lua scripts using method chaining.
//!
//! The builder supports both spawning new entities and cloning existing ones,
//! in both regular and collision contexts.

use crate::components::guibutton::GuiButton;
use crate::components::guiimage::GuiImage;
use crate::components::guilabel::GuiLabel;
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
        $methods.add_function($name, |lua, (ud, args): (LuaAnyUserData, _)| {
            let mut this = ud.borrow_mut::<LuaEntityBuilder>()?;
            let f: fn(&Lua, &mut LuaEntityBuilder, _) -> LuaResult<()> = $closure;
            f(lua, &mut *this, args)?;
            Ok(ud)
        });
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
    }

    fn add_function_mut<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: FnMut(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_meta_method<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: Fn(&Lua, &LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_meta_method_mut<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: FnMut(&Lua, &mut LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_meta_function<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: Fn(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_meta_function_mut<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: FnMut(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
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
        |_, this: &mut LuaEntityBuilder, name: String| {
            this.cmd.group = Some(name);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_position", "Set world position",
        [("x", "number"), ("y", "number")],
        |_, this: &mut LuaEntityBuilder, (x, y): (f32, f32)| {
            this.cmd.position = Some((x, y));
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (tex_key, width, height, origin_x, origin_y): (String, f32, f32, f32, f32)| {
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
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_sprite_offset", "Set sprite offset",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(ref mut sprite) = this.cmd.sprite else {
                return Err(LuaError::runtime(
                    "with_sprite_offset() requires with_sprite() first",
                ));
            };
            sprite.offset_x = offset_x;
            sprite.offset_y = offset_y;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_sprite_flip", "Set sprite flipping",
        [("flip_h", "boolean"), ("flip_v", "boolean")],
        |_, this: &mut LuaEntityBuilder, (flip_h, flip_v): (bool, bool)| {
            let Some(ref mut sprite) = this.cmd.sprite else {
                return Err(LuaError::runtime(
                    "with_sprite_flip() requires with_sprite() first",
                ));
            };
            sprite.flip_h = flip_h;
            sprite.flip_v = flip_v;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_zindex", "Set render order",
        [("z", "number")],
        |_, this: &mut LuaEntityBuilder, z: f32| {
            this.cmd.zindex = Some(z);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_velocity", "Set velocity (creates RigidBody if needed)",
        [("vx", "number"), ("vy", "number")],
        |_, this: &mut LuaEntityBuilder, (vx, vy): (f32, f32)| {
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
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_friction", "Set friction (creates RigidBody if needed)",
        [("friction", "number")],
        |_, this: &mut LuaEntityBuilder, friction: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.friction = friction;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    friction,
                    ..RigidBodyData::default()
                });
            }
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_max_speed", "Set max speed clamp (creates RigidBody if needed)",
        [("speed", "number")],
        |_, this: &mut LuaEntityBuilder, speed: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.max_speed = Some(speed);
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    max_speed: Some(speed),
                    ..RigidBodyData::default()
                });
            }
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (name, x, y, enabled): (String, f32, f32, bool)| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.forces.push(ForceData { name, x, y, enabled });
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    forces: vec![ForceData { name, x, y, enabled }],
                    ..RigidBodyData::default()
                });
            }
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_frozen", "Mark entity as frozen (physics skipped)",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.frozen = true;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    frozen: true,
                    ..RigidBodyData::default()
                });
            }
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (width, height, origin_x, origin_y): (f32, f32, f32, f32)| {
            this.cmd.collider = Some(ColliderData {
                width,
                height,
                offset_x: 0.0,
                offset_y: 0.0,
                origin_x,
                origin_y,
            });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_collider_offset", "Set collider offset",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(ref mut collider) = this.cmd.collider else {
                return Err(LuaError::runtime(
                    "with_collider_offset() requires with_collider() first",
                ));
            };
            collider.offset_x = offset_x;
            collider.offset_y = offset_y;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_mouse_controlled", "Enable mouse position tracking",
        [("follow_x", "boolean"), ("follow_y", "boolean")],
        |_, this: &mut LuaEntityBuilder, (follow_x, follow_y): (bool, bool)| {
            this.cmd.mouse_controlled = Some((follow_x, follow_y));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_rotation", "Set rotation in degrees",
        [("degrees", "number")],
        |_, this: &mut LuaEntityBuilder, degrees: f32| {
            this.cmd.rotation = Some(degrees);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_scale", "Set scale",
        [("sx", "number"), ("sy", "number")],
        |_, this: &mut LuaEntityBuilder, (sx, sy): (f32, f32)| {
            this.cmd.scale = Some((sx, sy));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_persistent", "Survive scene transitions",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            this.cmd.persistent = true;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_scalar", "Add a scalar signal",
        [("key", "string"), ("value", "number")],
        |_, this: &mut LuaEntityBuilder, (key, value): (String, f32)| {
            this.cmd.signal_scalars.push((key, value));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_integer", "Add an integer signal",
        [("key", "string"), ("value", "integer")],
        |_, this: &mut LuaEntityBuilder, (key, value): (String, i32)| {
            this.cmd.signal_integers.push((key, value));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_flag", "Add a flag signal",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            this.cmd.signal_flags.push(key);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_string", "Add a string signal",
        [("key", "string"), ("value", "string")],
        |_, this: &mut LuaEntityBuilder, (key, value): (String, String)| {
            this.cmd.signal_strings.push((key, value));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_screen_position", "Set screen position (UI elements). Requires :with_zindex() to render -- screen-space rendering requires ZIndex (mirrors world-space); entities without it are silently excluded, not an error.",
        [("x", "number"), ("y", "number")],
        |_, this: &mut LuaEntityBuilder, (x, y): (f32, f32)| {
            this.cmd.screen_position = Some((x, y));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_gui_window", "Set GuiWindow component (themed panel, drawn via the global GuiTheme). Requires :with_screen_position() and :with_zindex() to render.",
        [("width", "number"), ("height", "number")],
        |_, this: &mut LuaEntityBuilder, (width, height): (f32, f32)| {
            this.cmd.gui_window = Some((width, height));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_gui_offset", "Set GuiOffset (position relative to the parent, resolved each frame by gui_layout_system). Requires :with_parent() first.",
        [("x", "number"), ("y", "number")],
        |_, this: &mut LuaEntityBuilder, (x, y): (f32, f32)| {
            if this.cmd.parent.is_none() {
                return Err(LuaError::runtime(
                    "with_gui_offset() requires with_parent() first",
                ));
            }
            this.cmd.gui_offset = Some((x, y));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_gui_button", "Set GuiButton component; gui_button_spawn_system spawns a co-located GuiInteractable plus a caption DynamicText child on Added<GuiButton>, themed via GuiTheme.font/font_size/text_color (see engine.set_gui_theme_font). An empty `label` skips spawning the caption entirely (captionless button). Requires :with_screen_position() (or :with_parent()+:with_gui_offset()) and :with_zindex() to render.",
        [("width", "number"), ("height", "number"), ("label", "string"), ("callback_name", "string")],
        |_, this: &mut LuaEntityBuilder, (width, height, label, callback_name): (f32, f32, String, String)| {
            this.cmd.gui_button = Some(GuiButton::with_lua_callback(width, height, label, callback_name));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_gui_button_disabled", "Mark a GuiButton authored-disabled — gui_button_spawn_system applies this to the spawned GuiInteractable's state. Requires :with_gui_button() first.",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(btn) = this.cmd.gui_button.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_button_disabled() requires with_gui_button() first",
                ));
            };
            btn.disabled = true;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_gui_label", "Set GuiLabel component; gui_label_spawn_system spawns a caption DynamicText child on Added<GuiLabel>, themed via GuiTheme.font/font_size/text_color (see engine.set_gui_theme_font). An empty `text` skips spawning the caption entirely (captionless label). Requires :with_screen_position() (or :with_parent()+:with_gui_offset()) and :with_zindex() to render.",
        [("width", "number"), ("height", "number"), ("text", "string")],
        |_, this: &mut LuaEntityBuilder, (width, height, text): (f32, f32, String)| {
            this.cmd.gui_label = Some(GuiLabel::new(width, height, text));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_gui_image", "Set GuiImage component; gui_image_spawn_system spawns a co-located GuiInteractable + Sprite on Added<GuiImage> (no caption child, unlike GuiButton/GuiLabel). An empty `callback_name` skips wiring a click callback (the image still hit-tests/hovers/presses, it just has nothing to dispatch). Requires :with_screen_position() (or :with_parent()+:with_gui_offset()) and :with_zindex() to render.",
        [("width", "number"), ("height", "number"), ("tex_key", "string"), ("callback_name", "string")],
        |_, this: &mut LuaEntityBuilder, (width, height, tex_key, callback_name): (f32, f32, String, String)| {
            this.cmd.gui_image = Some(GuiImage::with_lua_callback(width, height, tex_key, callback_name));
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (content, font, font_size, r, g, b, a): (String, String, f32, u8, u8, u8, u8)| {
            this.cmd.text = Some(TextData { content, font, font_size, r, g, b, a });
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (items_table, origin_x, origin_y, font, font_size, item_spacing, use_screen_space): (LuaTable, f32, f32, String, f32, f32, bool)| {
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
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (nr, ng, nb, na, sr, sg, sb, sa): (u8, u8, u8, u8, u8, u8, u8, u8)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_colors() requires with_menu() first",
                ));
            };
            menu.normal_color = Some(ColorData { r: nr, g: ng, b: nb, a: na });
            menu.selected_color = Some(ColorData { r: sr, g: sg, b: sb, a: sa });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_dynamic_text", "Enable dynamic text updates for menu items",
        [("dynamic", "boolean")],
        |_, this: &mut LuaEntityBuilder, dynamic: bool| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_dynamic_text() requires with_menu() first",
                ));
            };
            menu.dynamic_text = Some(dynamic);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_cursor", "Set cursor entity for menu",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_cursor() requires with_menu() first",
                ));
            };
            menu.cursor_entity_key = Some(key);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_selection_sound", "Set sound for menu selection changes",
        [("sound_key", "string")],
        |_, this: &mut LuaEntityBuilder, sound_key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_selection_sound() requires with_menu() first",
                ));
            };
            menu.selection_change_sound = Some(sound_key);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_action_set_scene", "Set scene-switch action for menu item",
        [("item_id", "string"), ("scene", "string")],
        |_, this: &mut LuaEntityBuilder, (item_id, scene): (String, String)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_set_scene() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::SetScene { scene }));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_action_show_submenu", "Set submenu action for menu item",
        [("item_id", "string"), ("submenu", "string")],
        |_, this: &mut LuaEntityBuilder, (item_id, submenu): (String, String)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_show_submenu() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::ShowSubMenu { menu: submenu }));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_action_quit", "Set quit action for menu item",
        [("item_id", "string")],
        |_, this: &mut LuaEntityBuilder, item_id: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_quit() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::QuitGame));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_callback", "Set Lua callback for menu selection",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_callback() requires with_menu() first",
                ));
            };
            menu.on_select_callback = Some(callback);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_menu_visible_count", "Set max visible menu items (enables scrolling)",
        [("count", "integer")],
        |_, this: &mut LuaEntityBuilder, count: usize| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_visible_count() requires with_menu() first",
                ));
            };
            menu.visible_count = Some(count);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_signals", "Add empty Signals component",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            this.cmd.has_signals = true;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_phase",
        "Add phase state machine\n\nExample:\n```lua\nengine.spawn()\n    :with_phase({\n        initial = \"idle\",\n        phases = {\n            idle = {\n                on_enter = \"on_idle_enter\",\n                on_update = \"on_idle_update\",\n                on_exit = \"on_idle_exit\"\n            },\n            moving = { on_enter = \"on_moving_enter\" }\n        }\n    })\n    :build()\n```",
        [("table", "table")],
        |_, this: &mut LuaEntityBuilder, table: LuaTable| {
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
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (target_entity_id, follow_x, follow_y): (u64, bool, bool)| {
            this.cmd.stuckto = Some(StuckToData {
                target_entity_id,
                offset_x: 0.0,
                offset_y: 0.0,
                follow_x,
                follow_y,
                stored_velocity: None,
            });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_stuckto_offset", "Set offset for StuckTo",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(ref mut stuckto) = this.cmd.stuckto else {
                return Err(LuaError::runtime(
                    "with_stuckto_offset() requires with_stuckto() first",
                ));
            };
            stuckto.offset_x = offset_x;
            stuckto.offset_y = offset_y;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_stuckto_stored_velocity", "Set velocity to restore when unstuck",
        [("vx", "number"), ("vy", "number")],
        |_, this: &mut LuaEntityBuilder, (vx, vy): (f32, f32)| {
            let Some(ref mut stuckto) = this.cmd.stuckto else {
                return Err(LuaError::runtime(
                    "with_stuckto_stored_velocity() requires with_stuckto() first",
                ));
            };
            stuckto.stored_velocity = Some((vx, vy));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_lua_timer", "Add a Lua timer callback",
        [("duration", "number"), ("callback", "string")],
        |_, this: &mut LuaEntityBuilder, (duration, callback): (f32, String)| {
            this.cmd.lua_timer = Some((duration, callback));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_ttl", "Set time-to-live (auto-despawn)",
        [("seconds", "number")],
        |_, this: &mut LuaEntityBuilder, seconds: f32| {
            this.cmd.ttl = Some(seconds);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_binding", "Bind text to a WorldSignal value",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            this.cmd.signal_binding = Some((key, None));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_signal_binding_format", "Set format string for signal binding (use {} as placeholder)",
        [("format", "string")],
        |_, this: &mut LuaEntityBuilder, format: String| {
            let Some((_, ref mut fmt)) = this.cmd.signal_binding else {
                return Err(LuaError::runtime(
                    "with_signal_binding_format() requires with_signal_binding() first",
                ));
            };
            *fmt = Some(format);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_grid_layout", "Spawn entities from a JSON grid layout",
        [("path", "string"), ("group", "string"), ("zindex", "number")],
        |_, this: &mut LuaEntityBuilder, (path, group, zindex): (String, String, f32)| {
            this.cmd.grid_layout = Some((path, group, zindex));
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
            this.cmd.tween_position = Some(TweenPositionData {
                from_x,
                from_y,
                to_x,
                to_y,
                config: TweenConfig::new(duration),
            });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_position_easing", "Set easing for position tween",
        [("easing", "string")],
        |_, this: &mut LuaEntityBuilder, easing: String| {
            let Some(ref mut tween) = this.cmd.tween_position else {
                return Err(LuaError::runtime(
                    "with_tween_position_easing() requires with_tween_position() first",
                ));
            };
            tween.config.easing = easing;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_position_loop", "Set loop mode for position tween",
        [("loop_mode", "string")],
        |_, this: &mut LuaEntityBuilder, loop_mode: String| {
            let Some(ref mut tween) = this.cmd.tween_position else {
                return Err(LuaError::runtime(
                    "with_tween_position_loop() requires with_tween_position() first",
                ));
            };
            tween.config.loop_mode = loop_mode;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_position_backwards", "Start position tween in reverse",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(ref mut tween) = this.cmd.tween_position else {
                return Err(LuaError::runtime(
                    "with_tween_position_backwards() requires with_tween_position() first",
                ));
            };
            tween.config.backwards = true;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_position_on_finished", "Set a Lua callback to call when the position tween finishes",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut tween) = this.cmd.tween_position else {
                return Err(LuaError::runtime(
                    "with_tween_position_on_finished() requires with_tween_position() first",
                ));
            };
            tween.config.callback = callback;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_screen_position", "Add screen position tween animation",
        [
            ("from_x", "number"),
            ("from_y", "number"),
            ("to_x", "number"),
            ("to_y", "number"),
            ("duration", "number"),
        ],
        |_, this: &mut LuaEntityBuilder, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
            this.cmd.tween_screen_position = Some(TweenScreenPositionData {
                from_x,
                from_y,
                to_x,
                to_y,
                config: TweenConfig::new(duration),
            });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_screen_position_easing", "Set easing for screen position tween",
        [("easing", "string")],
        |_, this: &mut LuaEntityBuilder, easing: String| {
            let Some(ref mut tween) = this.cmd.tween_screen_position else {
                return Err(LuaError::runtime(
                    "with_tween_screen_position_easing() requires with_tween_screen_position() first",
                ));
            };
            tween.config.easing = easing;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_screen_position_loop", "Set loop mode for screen position tween",
        [("loop_mode", "string")],
        |_, this: &mut LuaEntityBuilder, loop_mode: String| {
            let Some(ref mut tween) = this.cmd.tween_screen_position else {
                return Err(LuaError::runtime(
                    "with_tween_screen_position_loop() requires with_tween_screen_position() first",
                ));
            };
            tween.config.loop_mode = loop_mode;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_screen_position_backwards", "Start screen position tween in reverse",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(ref mut tween) = this.cmd.tween_screen_position else {
                return Err(LuaError::runtime(
                    "with_tween_screen_position_backwards() requires with_tween_screen_position() first",
                ));
            };
            tween.config.backwards = true;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_screen_position_on_finished", "Set a Lua callback to call when the screen position tween finishes",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut tween) = this.cmd.tween_screen_position else {
                return Err(LuaError::runtime(
                    "with_tween_screen_position_on_finished() requires with_tween_screen_position() first",
                ));
            };
            tween.config.callback = callback;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation", "Add rotation tween animation",
        [("from", "number"), ("to", "number"), ("duration", "number")],
        |_, this: &mut LuaEntityBuilder, (from, to, duration): (f32, f32, f32)| {
            this.cmd.tween_rotation = Some(TweenRotationData {
                from,
                to,
                config: TweenConfig::new(duration),
            });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation_easing", "Set easing for rotation tween",
        [("easing", "string")],
        |_, this: &mut LuaEntityBuilder, easing: String| {
            let Some(ref mut tween) = this.cmd.tween_rotation else {
                return Err(LuaError::runtime(
                    "with_tween_rotation_easing() requires with_tween_rotation() first",
                ));
            };
            tween.config.easing = easing;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation_loop", "Set loop mode for rotation tween",
        [("loop_mode", "string")],
        |_, this: &mut LuaEntityBuilder, loop_mode: String| {
            let Some(ref mut tween) = this.cmd.tween_rotation else {
                return Err(LuaError::runtime(
                    "with_tween_rotation_loop() requires with_tween_rotation() first",
                ));
            };
            tween.config.loop_mode = loop_mode;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation_backwards", "Start rotation tween in reverse",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(ref mut tween) = this.cmd.tween_rotation else {
                return Err(LuaError::runtime(
                    "with_tween_rotation_backwards() requires with_tween_rotation() first",
                ));
            };
            tween.config.backwards = true;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_rotation_on_finished", "Set a Lua callback to call when the rotation tween finishes",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut tween) = this.cmd.tween_rotation else {
                return Err(LuaError::runtime(
                    "with_tween_rotation_on_finished() requires with_tween_rotation() first",
                ));
            };
            tween.config.callback = callback;
            Ok(())
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
        |_, this: &mut LuaEntityBuilder, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
            this.cmd.tween_scale = Some(TweenScaleData {
                from_x,
                from_y,
                to_x,
                to_y,
                config: TweenConfig::new(duration),
            });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_scale_easing", "Set easing for scale tween",
        [("easing", "string")],
        |_, this: &mut LuaEntityBuilder, easing: String| {
            let Some(ref mut tween) = this.cmd.tween_scale else {
                return Err(LuaError::runtime(
                    "with_tween_scale_easing() requires with_tween_scale() first",
                ));
            };
            tween.config.easing = easing;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_scale_loop", "Set loop mode for scale tween",
        [("loop_mode", "string")],
        |_, this: &mut LuaEntityBuilder, loop_mode: String| {
            let Some(ref mut tween) = this.cmd.tween_scale else {
                return Err(LuaError::runtime(
                    "with_tween_scale_loop() requires with_tween_scale() first",
                ));
            };
            tween.config.loop_mode = loop_mode;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_scale_backwards", "Start scale tween in reverse",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(ref mut tween) = this.cmd.tween_scale else {
                return Err(LuaError::runtime(
                    "with_tween_scale_backwards() requires with_tween_scale() first",
                ));
            };
            tween.config.backwards = true;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tween_scale_on_finished", "Set a Lua callback to call when the scale tween finishes",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut tween) = this.cmd.tween_scale else {
                return Err(LuaError::runtime(
                    "with_tween_scale_on_finished() requires with_tween_scale() first",
                ));
            };
            tween.config.callback = callback;
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_lua_collision_rule", "Add collision callback between two groups",
        [("group_a", "string"), ("group_b", "string"), ("callback", "string")],
        |_, this: &mut LuaEntityBuilder, (group_a, group_b, callback): (String, String, String)| {
            this.cmd.lua_collision_rule = Some(LuaCollisionRuleData {
                group_a,
                group_b,
                callback,
            });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_animation", "Set animation by key",
        [("animation_key", "string")],
        |_, this: &mut LuaEntityBuilder, animation_key: String| {
            this.cmd.animation = Some(AnimationData { animation_key });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_animation_controller", "Add animation controller with fallback",
        [("fallback_key", "string")],
        |_, this: &mut LuaEntityBuilder, fallback_key: String| {
            this.cmd.animation_controller = Some(AnimationControllerData {
                fallback_key,
                rules: Vec::new(),
            });
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_animation_rule", "Add animation rule to controller",
        [("condition_table", "table"), ("set_key", "string")],
        |_, this: &mut LuaEntityBuilder, (condition_table, set_key): (LuaTable, String)| {
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
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_particle_emitter", "Add particle emitter",
        [("table", "table")],
        |_, this: &mut LuaEntityBuilder, table: LuaTable| {
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
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tint", "Set color tint (RGBA 0-255)",
        [("r", "integer"), ("g", "integer"), ("b", "integer"), ("a", "integer")],
        |_, this: &mut LuaEntityBuilder, (r, g, b, a): (u8, u8, u8, u8)| {
            this.cmd.tint = Some((r, g, b, a));
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_shader", "Set per-entity shader with optional uniforms",
        [("shader_key", "string"), ("uniforms", "table?")],
        |_, this: &mut LuaEntityBuilder, args: mlua::MultiValue| {
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
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_parent", "Set parent entity for transform hierarchy",
        [("parent_id", "integer")],
        |_, this: &mut LuaEntityBuilder, parent_id: u64| {
            this.cmd.parent = Some(parent_id);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_tilemap",
        "Spawn a tilemap root. All tile entities become ChildOf children so the root's position/scale/rotation transforms the whole tilemap.",
        [("path", "string")],
        |_, this: &mut LuaEntityBuilder, path: String| {
            this.cmd.tilemap_path = Some(path);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_lua_setup",
        "Attach a one-shot Lua setup callback. The named function is called once (Added<LuaSetup>) with the entity context. Fires the frame after spawn; child entities added inside the callback appear the following frame.",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            this.cmd.lua_setup = Some(callback);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_on_animation_end",
        "Attach a callback fired exactly once when the entity's non-looped animation first reaches its last frame. Signature: fn(ctx, input). Looped animations never trigger it.",
        [("fn_name", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            this.cmd.lua_on_animation_end = Some(callback);
            Ok(())
        }
    );

    builder_method!(
        methods, meta,
        "with_camera_target",
        "Mark entity as camera follow target (higher priority wins). zoom is the desired camera zoom when this target wins (default 1.0).",
        [("priority", "integer?"), ("zoom", "number?")],
        |_, this: &mut LuaEntityBuilder, (priority, zoom): (Option<u8>, Option<f32>)| {
            this.cmd.camera_target = Some(priority.unwrap_or(0));
            this.cmd.camera_target_zoom = zoom;
            Ok(())
        }
    );

    // Non-builder-stub methods — kept as plain registrations.
    // These have their own entries in stub_meta's non-BUILDER_METHODS sections.

    methods.add_function("register_as", |_, (ud, key): (LuaAnyUserData, String)| {
        let mut this = ud.borrow_mut::<LuaEntityBuilder>()?;
        this.cmd.register_as = Some(key);
        Ok(ud)
    });

    methods.add_method_mut("build", |lua, this, ()| {
        let app_data = lua
            .app_data_ref::<LuaAppData>()
            .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;

        // Take the built command out of the builder rather than cloning it — the
        // builder is consumed by build() in normal (single-call) usage.
        match (this.mode, this.context) {
            (BuilderMode::Spawn, BuilderContext::Regular) => {
                app_data
                    .spawn_commands
                    .borrow_mut()
                    .push(std::mem::take(&mut this.cmd));
            }
            (BuilderMode::Spawn, BuilderContext::Collision) => {
                app_data
                    .collision_spawn_commands
                    .borrow_mut()
                    .push(std::mem::take(&mut this.cmd));
            }
            (BuilderMode::Clone, BuilderContext::Regular) => {
                let source_key = this.source_key.take().unwrap_or_default();
                app_data.clone_commands.borrow_mut().push(CloneCmd {
                    source_key,
                    overrides: std::mem::take(&mut this.cmd),
                });
            }
            (BuilderMode::Clone, BuilderContext::Collision) => {
                let source_key = this.source_key.take().unwrap_or_default();
                app_data
                    .collision_clone_commands
                    .borrow_mut()
                    .push(CloneCmd {
                        source_key,
                        overrides: std::mem::take(&mut this.cmd),
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

#[cfg(test)]
mod tests {
    use crate::resources::lua_runtime::LuaRuntime;

    fn assert_runtime_error(script: &str, expected_msg: &str) {
        let runtime = LuaRuntime::new().unwrap();
        let err = runtime
            .lua()
            .load(script)
            .exec()
            .expect_err("expected script to raise an error");
        let message = err.to_string();
        assert!(
            message.contains(expected_msg),
            "expected error containing {expected_msg:?}, got {message:?}"
        );
    }

    #[test]
    fn with_sprite_offset_requires_with_sprite() {
        assert_runtime_error(
            "engine.spawn():with_sprite_offset(1, 1)",
            "with_sprite_offset() requires with_sprite() first",
        );
    }

    #[test]
    fn with_collider_offset_requires_with_collider() {
        assert_runtime_error(
            "engine.spawn():with_collider_offset(1, 1)",
            "with_collider_offset() requires with_collider() first",
        );
    }

    #[test]
    fn with_stuckto_offset_requires_with_stuckto() {
        assert_runtime_error(
            "engine.spawn():with_stuckto_offset(1, 1)",
            "with_stuckto_offset() requires with_stuckto() first",
        );
    }

    #[test]
    fn with_signal_binding_format_requires_with_signal_binding() {
        assert_runtime_error(
            "engine.spawn():with_signal_binding_format('{}')",
            "with_signal_binding_format() requires with_signal_binding() first",
        );
    }

    #[test]
    fn with_tween_position_easing_requires_with_tween_position() {
        assert_runtime_error(
            "engine.spawn():with_tween_position_easing('linear')",
            "with_tween_position_easing() requires with_tween_position() first",
        );
    }

    #[test]
    fn with_tween_rotation_loop_requires_with_tween_rotation() {
        assert_runtime_error(
            "engine.spawn():with_tween_rotation_loop('loop')",
            "with_tween_rotation_loop() requires with_tween_rotation() first",
        );
    }

    #[test]
    fn with_tween_scale_backwards_requires_with_tween_scale() {
        assert_runtime_error(
            "engine.spawn():with_tween_scale_backwards()",
            "with_tween_scale_backwards() requires with_tween_scale() first",
        );
    }

    /// `with_*` chaining must return the *same* userdata handle (in-place mutation),
    /// not a clone, otherwise the O(n) chain cost regresses back to O(n^2).
    #[test]
    fn chaining_returns_same_userdata() {
        let runtime = LuaRuntime::new().unwrap();
        let same: bool = runtime
            .lua()
            .load(
                "local b = engine.spawn() \
                 return rawequal(b, b:with_position(1, 2):with_velocity(0, 0))",
            )
            .eval()
            .unwrap();
        assert!(same, "chained with_* calls must return the same userdata");
    }

    #[test]
    fn long_chain_builds_expected_spawn_cmd() {
        use super::super::runtime::LuaAppData;

        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load(
                "engine.spawn() \
                    :with_group('asteroids') \
                    :with_position(10, 20) \
                    :with_sprite('rock', 64, 64, 32, 32) \
                    :with_rotation(45) \
                    :with_velocity(1, 2) \
                    :with_zindex(5) \
                    :with_collider(40, 40, 20, 20) \
                    :with_signal_integer('hp', 3) \
                    :build()",
            )
            .exec()
            .unwrap();

        let app_data = runtime.lua().app_data_ref::<LuaAppData>().unwrap();
        let queued = app_data.spawn_commands.borrow();
        assert_eq!(queued.len(), 1, "expected exactly one queued spawn command");
        let cmd = &queued[0];
        assert_eq!(cmd.group.as_deref(), Some("asteroids"));
        assert_eq!(cmd.position, Some((10.0, 20.0)));
        assert!(cmd.sprite.is_some());
        assert_eq!(cmd.rotation, Some(45.0));
        assert_eq!(cmd.zindex, Some(5.0));
        assert!(cmd.collider.is_some());
        assert_eq!(cmd.signal_integers, vec![("hp".to_string(), 3)]);
    }
}
