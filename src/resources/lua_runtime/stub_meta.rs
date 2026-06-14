//! Stub metadata registration for the Lua runtime.
//!
//! Populates `engine.__meta` tables used by the stub generator (`--create-lua-stubs`).

use super::entity_builder::collect_builder_meta;
use super::runtime::LuaRuntime;
use mlua::prelude::*;

/// (param_name, lua_type)
pub type BuilderMethodParam = (&'static str, &'static str);
/// (method_name, description, params)
pub type BuilderMethodDef = (&'static str, &'static str, &'static [BuilderMethodParam]);

/// (field_name, lua_type, is_optional, description)
type TypeFieldDef = (&'static str, &'static str, bool, Option<&'static str>);
/// (type_name, description, fields)
type LuaTypeDef = (&'static str, &'static str, &'static [TypeFieldDef]);

/// Helper to push a type field entry to a fields table.
fn push_type_field(
    lua: &Lua,
    fields: &LuaTable,
    index: usize,
    name: &str,
    typ: &str,
    optional: bool,
    description: Option<&str>,
) -> LuaResult<()> {
    let f = lua.create_table()?;
    f.set("name", name)?;
    f.set("type", typ)?;
    f.set("optional", optional)?;
    if let Some(desc) = description {
        f.set("description", desc)?;
    }
    fields.set(index + 1, f)?;
    Ok(())
}

impl LuaRuntime {
    /// Registers builder class metadata in `engine.__meta.classes`.
    pub(super) fn register_builder_meta(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_classes: LuaTable = meta.get("classes")?;

        let builder_methods = collect_builder_meta();

        let schema_refs: &[(&str, &str, &str)] = &[
            ("with_phase", "table", "PhaseDefinition"),
            ("with_particle_emitter", "table", "ParticleEmitterConfig"),
            (
                "with_animation_rule",
                "condition_table",
                "AnimationRuleCondition",
            ),
            ("with_menu", "items", "MenuItem[]"),
        ];

        for class_name in &["EntityBuilder", "CollisionEntityBuilder"] {
            let class_tbl = self.lua.create_table()?;
            class_tbl.set(
                "description",
                format!(
                    "Fluent builder for entity construction ({})",
                    if *class_name == "EntityBuilder" {
                        "regular context"
                    } else {
                        "collision context"
                    }
                ),
            )?;

            let methods_tbl = self.lua.create_table()?;
            for (name, desc, params) in &builder_methods {
                let method_tbl = self.lua.create_table()?;
                method_tbl.set("description", *desc)?;
                let params_tbl = self.lua.create_table()?;
                for (i, (pname, ptype)) in params.iter().enumerate() {
                    let p = self.lua.create_table()?;
                    p.set("name", *pname)?;
                    p.set("type", *ptype)?;
                    for (method, param, schema) in schema_refs {
                        if *method == *name && *param == *pname {
                            p.set("schema", *schema)?;
                        }
                    }
                    params_tbl.set(i + 1, p)?;
                }
                method_tbl.set("params", params_tbl)?;
                if *name != "build" {
                    let ret = self.lua.create_table()?;
                    ret.set("type", *class_name)?;
                    method_tbl.set("returns", ret)?;
                }
                methods_tbl.set(*name, method_tbl)?;
            }
            class_tbl.set("methods", methods_tbl)?;
            meta_classes.set(*class_name, class_tbl)?;
        }

        Ok(())
    }

    /// Registers type shape definitions in `engine.__meta.types`.
    pub(super) fn register_types_meta(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_types: LuaTable = meta.get("types")?;

        let type_defs: &[LuaTypeDef] = &[
            (
                "Vector2",
                "2D vector / point",
                &[("x", "number", false, None), ("y", "number", false, None)],
            ),
            (
                "Rect",
                "Axis-aligned rectangle",
                &[
                    ("x", "number", false, None),
                    ("y", "number", false, None),
                    ("w", "number", false, None),
                    ("h", "number", false, None),
                ],
            ),
            (
                "CameraState",
                "Camera state returned by engine.get_camera()",
                &[
                    ("target_x", "number", false, Some("Camera target world X")),
                    ("target_y", "number", false, Some("Camera target world Y")),
                    ("offset_x", "number", false, Some("Camera screen offset X")),
                    ("offset_y", "number", false, Some("Camera screen offset Y")),
                    ("rotation", "number", false, Some("Camera rotation in degrees")),
                    ("zoom", "number", false, Some("Camera zoom factor")),
                ],
            ),
            (
                "CameraViewRect",
                "Visible world-space rectangle returned by engine.get_camera_view_rect(). Assumes zero rotation.",
                &[
                    ("x", "number", false, Some("Left edge in world space")),
                    ("y", "number", false, Some("Top edge in world space")),
                    ("w", "number", false, Some("Width in world units")),
                    ("h", "number", false, Some("Height in world units")),
                ],
            ),
            (
                "SpriteInfo",
                "Sprite state snapshot",
                &[
                    ("tex_key", "string", false, None),
                    ("flip_h", "boolean", false, None),
                    ("flip_v", "boolean", false, None),
                ],
            ),
            (
                "AnimationInfo",
                "Animation state snapshot",
                &[
                    ("key", "string", false, None),
                    ("frame_index", "integer", false, None),
                    ("elapsed", "number", false, None),
                ],
            ),
            (
                "TimerInfo",
                "Lua timer state snapshot",
                &[
                    ("duration", "number", false, None),
                    ("elapsed", "number", false, None),
                    ("callback", "string", false, None),
                ],
            ),
            (
                "SignalSet",
                "Entity signal snapshot",
                &[
                    ("flags", "string[]", false, None),
                    ("integers", "{[string]: integer}", false, None),
                    ("scalars", "{[string]: number}", false, None),
                    ("strings", "{[string]: string}", false, None),
                ],
            ),
            (
                "EntityContext",
                "Entity state passed to phase/timer callbacks",
                &[
                    ("id", "integer", false, Some("Entity ID")),
                    ("group", "string", true, None),
                    ("pos", "Vector2", true, None),
                    ("screen_pos", "Vector2", true, None),
                    ("vel", "Vector2", true, None),
                    ("speed_sq", "number", true, None),
                    ("frozen", "boolean", true, None),
                    ("rotation", "number", true, None),
                    ("scale", "Vector2", true, None),
                    ("rect", "Rect", true, None),
                    ("sprite", "SpriteInfo", true, None),
                    ("animation", "AnimationInfo", true, None),
                    ("signals", "SignalSet", true, None),
                    ("phase", "string", true, None),
                    ("time_in_phase", "number", true, None),
                    ("previous_phase", "string", true, Some("Only in on_enter")),
                    ("timer", "TimerInfo", true, None),
                    (
                        "world_pos",
                        "Vector2",
                        true,
                        Some("World position from hierarchy"),
                    ),
                    (
                        "world_rotation",
                        "number",
                        true,
                        Some("World rotation from hierarchy"),
                    ),
                    (
                        "world_scale",
                        "Vector2",
                        true,
                        Some("World scale from hierarchy"),
                    ),
                    (
                        "parent_id",
                        "integer",
                        true,
                        Some("Parent entity ID if in hierarchy"),
                    ),
                ],
            ),
            (
                "CollisionEntity",
                "Entity data in a collision context",
                &[
                    ("id", "integer", false, Some("Entity ID")),
                    ("group", "string", false, None),
                    ("pos", "Vector2", false, None),
                    ("vel", "Vector2", false, None),
                    ("speed_sq", "number", false, None),
                    ("rect", "Rect", false, None),
                    ("signals", "SignalSet", false, None),
                ],
            ),
            (
                "CollisionSides",
                "Collision contact sides",
                &[
                    ("a", "string[]", false, Some("Sides of entity A in contact")),
                    ("b", "string[]", false, Some("Sides of entity B in contact")),
                ],
            ),
            (
                "CollisionContext",
                "Context passed to collision callbacks",
                &[
                    ("a", "CollisionEntity", false, None),
                    ("b", "CollisionEntity", false, None),
                    ("sides", "CollisionSides", false, None),
                ],
            ),
            (
                "DigitalButtonState",
                "State of a single digital button",
                &[
                    ("pressed", "boolean", false, None),
                    ("just_pressed", "boolean", false, None),
                    ("just_released", "boolean", false, None),
                ],
            ),
            (
                "DigitalInputs",
                "All digital button states",
                &[
                    ("up", "DigitalButtonState", false, None),
                    ("down", "DigitalButtonState", false, None),
                    ("left", "DigitalButtonState", false, None),
                    ("right", "DigitalButtonState", false, None),
                    ("action_1", "DigitalButtonState", false, None),
                    ("action_2", "DigitalButtonState", false, None),
                    ("action_3", "DigitalButtonState", false, None),
                    ("back", "DigitalButtonState", false, None),
                    ("special", "DigitalButtonState", false, None),
                    (
                        "main_up",
                        "DigitalButtonState",
                        false,
                        Some("Raw WASD up (W key)"),
                    ),
                    (
                        "main_down",
                        "DigitalButtonState",
                        false,
                        Some("Raw WASD down (S key)"),
                    ),
                    (
                        "main_left",
                        "DigitalButtonState",
                        false,
                        Some("Raw WASD left (A key)"),
                    ),
                    (
                        "main_right",
                        "DigitalButtonState",
                        false,
                        Some("Raw WASD right (D key)"),
                    ),
                    (
                        "secondary_up",
                        "DigitalButtonState",
                        false,
                        Some("Raw arrow up key"),
                    ),
                    (
                        "secondary_down",
                        "DigitalButtonState",
                        false,
                        Some("Raw arrow down key"),
                    ),
                    (
                        "secondary_left",
                        "DigitalButtonState",
                        false,
                        Some("Raw arrow left key"),
                    ),
                    (
                        "secondary_right",
                        "DigitalButtonState",
                        false,
                        Some("Raw arrow right key"),
                    ),
                    (
                        "debug",
                        "DigitalButtonState",
                        false,
                        Some("Debug toggle (F11)"),
                    ),
                    (
                        "fullscreen",
                        "DigitalButtonState",
                        false,
                        Some("Fullscreen toggle (F10)"),
                    ),
                ],
            ),
            (
                "AnalogInputs",
                "Analog input values (mouse, scroll)",
                &[
                    (
                        "scroll_y",
                        "number",
                        false,
                        Some("Mouse wheel delta (positive=up, negative=down)"),
                    ),
                    (
                        "mouse_x",
                        "number",
                        false,
                        Some("Cursor X in game-space (0..render_width, letterbox-corrected)"),
                    ),
                    (
                        "mouse_y",
                        "number",
                        false,
                        Some("Cursor Y in game-space (0..render_height, letterbox-corrected)"),
                    ),
                    (
                        "mouse_world_x",
                        "number",
                        false,
                        Some(
                            "Cursor X in world-space (after camera transform, matches MapPosition)",
                        ),
                    ),
                    (
                        "mouse_world_y",
                        "number",
                        false,
                        Some(
                            "Cursor Y in world-space (after camera transform, matches MapPosition)",
                        ),
                    ),
                ],
            ),
            (
                "InputSnapshot",
                "Input state passed to callbacks",
                &[
                    ("digital", "DigitalInputs", false, None),
                    ("analog", "AnalogInputs", false, None),
                ],
            ),
            (
                "PhaseCallbacks",
                "Callbacks for a single phase",
                &[
                    (
                        "on_enter",
                        "string",
                        true,
                        Some("Function name called on phase enter"),
                    ),
                    (
                        "on_update",
                        "string",
                        true,
                        Some("Function name called each frame"),
                    ),
                    (
                        "on_exit",
                        "string",
                        true,
                        Some("Function name called on phase exit"),
                    ),
                ],
            ),
            (
                "PhaseDefinition",
                "Phase state machine definition",
                &[
                    ("initial", "string", false, Some("Initial phase name")),
                    (
                        "phases",
                        "{[string]: PhaseCallbacks}",
                        false,
                        Some("Map of phase name to callbacks"),
                    ),
                ],
            ),
            (
                "ParticleEmitterConfig",
                "Particle emitter configuration table",
                &[
                    (
                        "templates",
                        "string[]",
                        false,
                        Some("Entity template keys to emit"),
                    ),
                    (
                        "shape",
                        "string|table",
                        true,
                        Some("Emitter shape: 'point' or table {type='rect', width, height}"),
                    ),
                    (
                        "offset",
                        "table",
                        true,
                        Some("{x, y} offset from entity position"),
                    ),
                    ("particles_per_emission", "integer", true, None),
                    ("emissions_per_second", "number", true, None),
                    (
                        "emissions_remaining",
                        "integer",
                        true,
                        Some("nil = infinite"),
                    ),
                    ("arc", "table", true, Some("{min, max} angle in degrees")),
                    ("speed", "table", true, Some("{min, max} or single number")),
                    (
                        "ttl",
                        "number|table",
                        true,
                        Some("{min, max}, number, or 'none'"),
                    ),
                ],
            ),
            (
                "MenuItem",
                "Menu item definition",
                &[
                    ("id", "string", false, None),
                    ("label", "string", false, None),
                ],
            ),
            (
                "AnimationRuleCondition",
                "Animation rule condition (polymorphic)",
                &[
                    (
                        "type",
                        "string",
                        false,
                        Some(
                            "Condition type: has_flag, lacks_flag, scalar_cmp, scalar_range, integer_cmp, integer_range, all, any, not",
                        ),
                    ),
                    (
                        "key",
                        "string",
                        true,
                        Some("Signal key (for flag/scalar/integer conditions)"),
                    ),
                    (
                        "op",
                        "string",
                        true,
                        Some("Comparison operator (for cmp conditions)"),
                    ),
                    (
                        "value",
                        "number",
                        true,
                        Some("Comparison value (for cmp conditions)"),
                    ),
                    (
                        "min",
                        "number",
                        true,
                        Some("Range minimum (for range conditions)"),
                    ),
                    (
                        "max",
                        "number",
                        true,
                        Some("Range maximum (for range conditions)"),
                    ),
                    (
                        "conditions",
                        "AnimationRuleCondition[]",
                        true,
                        Some("Sub-conditions (for all/any/not)"),
                    ),
                ],
            ),
        ];

        for (name, description, fields_def) in type_defs {
            let tbl = self.lua.create_table()?;
            tbl.set("description", *description)?;
            let fields = self.lua.create_table()?;
            for (i, (fname, ftype, optional, fdesc)) in fields_def.iter().enumerate() {
                push_type_field(&self.lua, &fields, i, fname, ftype, *optional, *fdesc)?;
            }
            tbl.set("fields", fields)?;
            meta_types.set(*name, tbl)?;
        }

        Ok(())
    }

    /// Registers enum value sets in `engine.__meta.enums`.
    pub(super) fn register_enums_meta(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_enums: LuaTable = meta.get("enums")?;

        let enum_defs: &[(&str, &str, &[&str])] = &[
            (
                "Easing",
                "Tween easing function",
                &[
                    "linear",
                    "quad_in",
                    "quad_out",
                    "quad_in_out",
                    "cubic_in",
                    "cubic_out",
                    "cubic_in_out",
                ],
            ),
            (
                "LoopMode",
                "Tween loop mode",
                &["once", "loop", "ping_pong"],
            ),
            (
                "BoxSide",
                "Collision side",
                &["left", "right", "top", "bottom"],
            ),
            (
                "ComparisonOp",
                "Comparison operator for animation rules",
                &["lt", "le", "gt", "ge", "eq", "ne"],
            ),
            (
                "ConditionType",
                "Animation rule condition type",
                &[
                    "has_flag",
                    "lacks_flag",
                    "scalar_cmp",
                    "scalar_range",
                    "integer_cmp",
                    "integer_range",
                    "all",
                    "any",
                    "not",
                ],
            ),
            (
                "EmitterShape",
                "Particle emitter shape type",
                &["point", "rect"],
            ),
            (
                "TtlSpec",
                "Time-to-live specification (number, {min,max} table, or 'none')",
                &["none"],
            ),
            (
                "TextureFilter",
                "Texture sampling filter mode",
                &[
                    "nearest",
                    "bilinear",
                    "trilinear",
                    "anisotropic_4x",
                    "anisotropic_8x",
                    "anisotropic_16x",
                ],
            ),
            (
                "Category",
                "Function category",
                &[
                    "base",
                    "asset",
                    "spawn",
                    "audio",
                    "signal",
                    "phase",
                    "entity",
                    "group",
                    "camera",
                    "collision",
                    "animation",
                    "render",
                ],
            ),
        ];

        for (name, description, values) in enum_defs {
            let tbl = self.lua.create_table()?;
            tbl.set("description", *description)?;
            let vals = self.lua.create_table()?;
            for (i, val) in values.iter().enumerate() {
                vals.set(i + 1, *val)?;
            }
            tbl.set("values", vals)?;
            meta_enums.set(*name, tbl)?;
        }

        Ok(())
    }

    /// Registers well-known callback signatures in `engine.__meta.callbacks`.
    pub(super) fn register_callbacks_meta(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_callbacks: LuaTable = meta.get("callbacks")?;

        struct CbDef {
            name: &'static str,
            description: &'static str,
            params: &'static [(&'static str, &'static str)],
            returns: Option<&'static str>,
            context: Option<&'static str>,
            note: Option<&'static str>,
        }

        let callback_defs: &[CbDef] = &[
            CbDef {
                name: "on_setup",
                description: "Called once during game setup for asset loading",
                params: &[],
                returns: None,
                context: Some("setup"),
                note: None,
            },
            CbDef {
                name: "on_enter_play",
                description: "Called when entering Playing state",
                params: &[],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "on_switch_scene",
                description: "Called when switching scenes",
                params: &[("scene", "string")],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "on_update_<scene>",
                description: "Called each frame during a scene",
                params: &[("input", "InputSnapshot"), ("dt", "number")],
                returns: None,
                context: Some("play"),
                note: Some("Function name is dynamic: on_update_ + scene name"),
            },
            CbDef {
                name: "phase_on_enter",
                description: "Called when entering a phase",
                params: &[("ctx", "EntityContext"), ("input", "InputSnapshot")],
                returns: Some("string?"),
                context: Some("play"),
                note: Some("Return phase name to trigger transition"),
            },
            CbDef {
                name: "phase_on_update",
                description: "Called each frame during a phase",
                params: &[
                    ("ctx", "EntityContext"),
                    ("input", "InputSnapshot"),
                    ("dt", "number"),
                ],
                returns: Some("string?"),
                context: Some("play"),
                note: Some("Return phase name to trigger transition"),
            },
            CbDef {
                name: "phase_on_exit",
                description: "Called when exiting a phase",
                params: &[("ctx", "EntityContext")],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "timer_callback",
                description: "Called when a Lua timer fires",
                params: &[("ctx", "EntityContext"), ("input", "InputSnapshot")],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "collision_callback",
                description: "Called when two colliding groups overlap",
                params: &[("ctx", "CollisionContext")],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "menu_callback",
                description: "Called when a menu item is selected",
                params: &[
                    ("menu_id", "integer"),
                    ("item_id", "string"),
                    ("item_index", "integer"),
                ],
                returns: None,
                context: Some("play"),
                note: None,
            },
        ];

        for cb in callback_defs {
            let tbl = self.lua.create_table()?;
            tbl.set("description", cb.description)?;

            let params_tbl = self.lua.create_table()?;
            for (i, (pname, ptype)) in cb.params.iter().enumerate() {
                let p = self.lua.create_table()?;
                p.set("name", *pname)?;
                p.set("type", *ptype)?;
                params_tbl.set(i + 1, p)?;
            }
            tbl.set("params", params_tbl)?;

            if let Some(ret) = cb.returns {
                let r = self.lua.create_table()?;
                r.set("type", ret)?;
                tbl.set("returns", r)?;
            }
            if let Some(ctx) = cb.context {
                tbl.set("context", ctx)?;
            }
            if let Some(note) = cb.note {
                tbl.set("note", note)?;
            }

            meta_callbacks.set(cb.name, tbl)?;
        }

        Ok(())
    }
}
