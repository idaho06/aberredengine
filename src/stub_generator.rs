//! Lua stub generator for EmmyLua / lua-language-server.
//!
//! Reads `engine.__meta` from the Lua runtime and emits a deterministic
//! `engine.lua` stub file with `---@class`, `---@field`, `---@param`,
//! and `---@return` annotations.

use crate::resources::lua_runtime::LuaRuntime;
use mlua::prelude::*;
use std::fmt::Write as FmtWrite;
use std::path::Path;

/// Category display order for deterministic output.
const CATEGORY_ORDER: &[&str] = &[
    "base",
    "asset",
    "spawn",
    "audio",
    "signal",
    "phase",
    "entity",
    "group",
    "tilemap",
    "camera",
    "collision",
    "animation",
    "render",
];

/// Human-readable section titles for each category.
fn category_title(cat: &str) -> &str {
    match cat {
        "base" => "Logging Functions",
        "asset" => "Asset Loading",
        "spawn" => "Entity Spawning",
        "audio" => "Audio Playback",
        "signal" => "World Signals",
        "phase" => "Phase Control",
        "entity" => "Entity Commands",
        "group" => "Group Tracking",
        "tilemap" => "Tilemap",
        "camera" => "Camera",
        "collision" => "Collision Commands",
        "animation" => "Animation Registration",
        "render" => "Rendering & Shaders",
        _ => cat,
    }
}

/// Maps a meta type string to the EmmyLua annotation type.
fn lua_type_annotation(meta_type: &str) -> String {
    match meta_type {
        "number" => "number".into(),
        "integer" => "integer".into(),
        "string" => "string".into(),
        "boolean" => "boolean".into(),
        "table" => "table".into(),
        "number?" => "number|nil".into(),
        "integer?" => "integer|nil".into(),
        "string?" => "string|nil".into(),
        "boolean?" => "boolean|nil".into(),
        "table?" => "table|nil".into(),
        "string[]?" => "string[]|nil".into(),
        "string[]" => "string[]".into(),
        s if s.ends_with("?") => format!("{}|nil", &s[..s.len() - 1]),
        s if s.starts_with("{[") => s.replace("{[string]: ", "table<string, ").replace("}", ">"),
        other => other.into(),
    }
}

/// Extracted function metadata.
struct FnMeta {
    name: String,
    description: String,
    category: String,
    params: Vec<(String, String)>,
    returns: Option<String>,
}

/// Extracted class metadata.
struct ClassMeta {
    name: String,
    description: String,
    methods: Vec<MethodMeta>,
}

struct MethodMeta {
    name: String,
    description: String,
    params: Vec<ParamMeta>,
    returns: Option<String>,
}

struct ParamMeta {
    name: String,
    type_name: String,
    schema: Option<String>,
}

/// Extracted type metadata.
struct TypeMeta {
    name: String,
    description: String,
    fields: Vec<FieldMeta>,
}

struct FieldMeta {
    name: String,
    type_name: String,
    optional: bool,
    description: Option<String>,
}

/// Extracted enum metadata.
struct EnumMeta {
    name: String,
    description: String,
    values: Vec<String>,
}

/// Extracted callback metadata.
struct CallbackMeta {
    name: String,
    description: String,
    params: Vec<(String, String)>,
    returns: Option<String>,
    context: Option<String>,
    note: Option<String>,
}

/// Extract all metadata from `engine.__meta` and generate the stub file content.
pub fn generate_stubs(runtime: &LuaRuntime) -> Result<String, String> {
    let lua = runtime.lua();

    let engine: LuaTable = lua
        .globals()
        .get("engine")
        .map_err(|e| format!("Failed to get engine table: {e}"))?;
    let meta: LuaTable = engine
        .get("__meta")
        .map_err(|e| format!("Failed to get engine.__meta: {e}"))?;

    let functions = extract_functions(&meta).map_err(|e| format!("Functions: {e}"))?;
    let classes = extract_classes(&meta).map_err(|e| format!("Classes: {e}"))?;
    let types = extract_types(&meta).map_err(|e| format!("Types: {e}"))?;
    let enums = extract_enums(&meta).map_err(|e| format!("Enums: {e}"))?;
    let callbacks = extract_callbacks(&meta).map_err(|e| format!("Callbacks: {e}"))?;

    render_stubs(&functions, &classes, &types, &enums, &callbacks)
}

/// Write the generated stubs to a file.
pub fn write_stubs(path: &Path, content: &str) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

// --------------- Extraction ---------------

fn extract_functions(meta: &LuaTable) -> Result<Vec<FnMeta>, LuaError> {
    let fns_tbl: LuaTable = meta.get("functions")?;
    let mut result = Vec::new();
    for pair in fns_tbl.pairs::<String, LuaTable>() {
        let (name, tbl) = pair?;
        let description: String = tbl.get("description")?;
        let category: String = tbl.get("category")?;
        let params_tbl: LuaTable = tbl.get("params")?;
        let mut params = Vec::new();
        for p in params_tbl.sequence_values::<LuaTable>() {
            let p = p?;
            let pname: String = p.get("name")?;
            let ptype: String = p.get("type")?;
            params.push((pname, ptype));
        }
        let returns: Option<String> = tbl
            .get::<LuaTable>("returns")
            .ok()
            .and_then(|r| r.get::<String>("type").ok());
        result.push(FnMeta {
            name,
            description,
            category,
            params,
            returns,
        });
    }
    // Sort by category order, then alphabetically within category
    result.sort_by(|a, b| {
        let ca = CATEGORY_ORDER
            .iter()
            .position(|c| *c == a.category)
            .unwrap_or(99);
        let cb = CATEGORY_ORDER
            .iter()
            .position(|c| *c == b.category)
            .unwrap_or(99);
        ca.cmp(&cb).then_with(|| a.name.cmp(&b.name))
    });
    Ok(result)
}

fn extract_classes(meta: &LuaTable) -> Result<Vec<ClassMeta>, LuaError> {
    let classes_tbl: LuaTable = meta.get("classes")?;
    let mut result = Vec::new();
    for pair in classes_tbl.pairs::<String, LuaTable>() {
        let (name, tbl) = pair?;
        let description: String = tbl.get("description")?;
        let methods_tbl: LuaTable = tbl.get("methods")?;
        let mut methods = Vec::new();
        for mp in methods_tbl.pairs::<String, LuaTable>() {
            let (mname, mtbl) = mp?;
            let mdesc: String = mtbl.get("description")?;
            let params_tbl: LuaTable = mtbl.get("params")?;
            let mut params = Vec::new();
            for p in params_tbl.sequence_values::<LuaTable>() {
                let p = p?;
                let pname: String = p.get("name")?;
                let ptype: String = p.get("type")?;
                let schema: Option<String> = p.get::<String>("schema").ok();
                params.push(ParamMeta {
                    name: pname,
                    type_name: ptype,
                    schema,
                });
            }
            let returns: Option<String> = mtbl
                .get::<LuaTable>("returns")
                .ok()
                .and_then(|r| r.get::<String>("type").ok());
            methods.push(MethodMeta {
                name: mname,
                description: mdesc,
                params,
                returns,
            });
        }
        // Sort methods: with_* alphabetically, then register_as, then build
        methods.sort_by(|a, b| method_sort_key(&a.name).cmp(&method_sort_key(&b.name)));
        result.push(ClassMeta {
            name,
            description,
            methods,
        });
    }
    // EntityBuilder before CollisionEntityBuilder
    result.sort_by(|a, b| a.name.cmp(&b.name).reverse());
    // Actually: EntityBuilder < CollisionEntityBuilder alphabetically is wrong.
    // E comes after C. Let's explicitly order.
    result.sort_by_key(|c| match c.name.as_str() {
        "EntityBuilder" => 0,
        "CollisionEntityBuilder" => 1,
        _ => 2,
    });
    Ok(result)
}

fn method_sort_key(name: &str) -> (u8, &str) {
    match name {
        "build" => (2, name),
        "register_as" => (1, name),
        _ => (0, name),
    }
}

fn extract_types(meta: &LuaTable) -> Result<Vec<TypeMeta>, LuaError> {
    let types_tbl: LuaTable = meta.get("types")?;
    let mut result = Vec::new();
    for pair in types_tbl.pairs::<String, LuaTable>() {
        let (name, tbl) = pair?;
        let description: String = tbl.get("description")?;
        let fields_tbl: LuaTable = tbl.get("fields")?;
        let mut fields = Vec::new();
        for f in fields_tbl.sequence_values::<LuaTable>() {
            let f = f?;
            let fname: String = f.get("name")?;
            let ftype: String = f.get("type")?;
            let optional: bool = f.get("optional")?;
            let fdesc: Option<String> = f.get::<String>("description").ok();
            fields.push(FieldMeta {
                name: fname,
                type_name: ftype,
                optional,
                description: fdesc,
            });
        }
        result.push(TypeMeta {
            name,
            description,
            fields,
        });
    }
    // Stable sort: context types first, then alphabetical
    let type_order = [
        "Vector2",
        "Rect",
        "SpriteInfo",
        "AnimationInfo",
        "TimerInfo",
        "SignalSet",
        "EntityContext",
        "CollisionEntity",
        "CollisionSides",
        "CollisionContext",
        "DigitalButtonState",
        "DigitalInputs",
        "InputSnapshot",
        "PhaseCallbacks",
        "PhaseDefinition",
        "ParticleEmitterConfig",
        "MenuItem",
        "AnimationRuleCondition",
    ];
    result.sort_by_key(|t| type_order.iter().position(|n| *n == t.name).unwrap_or(99));
    Ok(result)
}

fn extract_enums(meta: &LuaTable) -> Result<Vec<EnumMeta>, LuaError> {
    let enums_tbl: LuaTable = meta.get("enums")?;
    let mut result = Vec::new();
    for pair in enums_tbl.pairs::<String, LuaTable>() {
        let (name, tbl) = pair?;
        let description: String = tbl.get("description")?;
        let vals_tbl: LuaTable = tbl.get("values")?;
        let mut values = Vec::new();
        for v in vals_tbl.sequence_values::<String>() {
            values.push(v?);
        }
        result.push(EnumMeta {
            name,
            description,
            values,
        });
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

fn extract_callbacks(meta: &LuaTable) -> Result<Vec<CallbackMeta>, LuaError> {
    let cb_tbl: LuaTable = meta.get("callbacks")?;
    let mut result = Vec::new();
    for pair in cb_tbl.pairs::<String, LuaTable>() {
        let (name, tbl) = pair?;
        let description: String = tbl.get("description")?;
        let params_tbl: LuaTable = tbl.get("params")?;
        let mut params = Vec::new();
        for p in params_tbl.sequence_values::<LuaTable>() {
            let p = p?;
            params.push((p.get::<String>("name")?, p.get::<String>("type")?));
        }
        let returns: Option<String> = tbl
            .get::<LuaTable>("returns")
            .ok()
            .and_then(|r| r.get::<String>("type").ok());
        let context: Option<String> = tbl.get::<String>("context").ok();
        let note: Option<String> = tbl.get::<String>("note").ok();
        result.push(CallbackMeta {
            name,
            description,
            params,
            returns,
            context,
            note,
        });
    }
    // Sort callbacks in a logical order
    let cb_order = [
        "on_setup",
        "on_enter_play",
        "on_switch_scene",
        "on_update_<scene>",
        "phase_on_enter",
        "phase_on_update",
        "phase_on_exit",
        "timer_callback",
        "collision_callback",
        "menu_callback",
    ];
    result.sort_by_key(|c| cb_order.iter().position(|n| *n == c.name).unwrap_or(99));
    Ok(result)
}

// --------------- Rendering ---------------

fn render_stubs(
    functions: &[FnMeta],
    classes: &[ClassMeta],
    types: &[TypeMeta],
    enums: &[EnumMeta],
    callbacks: &[CallbackMeta],
) -> Result<String, String> {
    let mut out = String::with_capacity(64 * 1024);

    // Header
    writeln!(out, "---@meta").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "-- THIS FILE IS AUTO-GENERATED by `aberredengine --create-lua-stubs`."
    )
    .unwrap();
    writeln!(
        out,
        "-- DO NOT EDIT MANUALLY. Regenerate from engine.__meta instead."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "---@class engine").unwrap();
    writeln!(out, "---Engine API provided by Aberred Engine (Rust)").unwrap();
    writeln!(
        out,
        "---All functions are available globally via the `engine` table"
    )
    .unwrap();
    writeln!(out, "engine = {{}}").unwrap();
    writeln!(out).unwrap();

    // Types section
    render_types(&mut out, types);

    // Enums section
    render_enums(&mut out, enums);

    // Callbacks section (as reference documentation)
    render_callbacks(&mut out, callbacks);

    // Functions grouped by category
    render_functions(&mut out, functions);

    // Builder classes
    for class in classes {
        render_class(&mut out, class);
    }

    Ok(out)
}

fn render_types(out: &mut String, types: &[TypeMeta]) {
    writeln!(out, "-- ==================== Types ====================").unwrap();
    writeln!(out).unwrap();

    for t in types {
        writeln!(out, "---{}", t.description).unwrap();
        writeln!(out, "---@class {}", t.name).unwrap();
        for f in &t.fields {
            let typ = lua_type_annotation(&f.type_name);
            let full_type = if f.optional {
                format!("{}|nil", typ)
            } else {
                typ
            };
            if let Some(ref desc) = f.description {
                writeln!(out, "---@field {} {} {}", f.name, full_type, desc).unwrap();
            } else {
                writeln!(out, "---@field {} {}", f.name, full_type).unwrap();
            }
        }
        writeln!(out).unwrap();
    }
}

fn render_enums(out: &mut String, enums: &[EnumMeta]) {
    writeln!(out, "-- ==================== Enums ====================").unwrap();
    writeln!(out).unwrap();

    for e in enums {
        writeln!(out, "---{}", e.description).unwrap();
        let values_str: Vec<String> = e.values.iter().map(|v| format!("\"{}\"", v)).collect();
        writeln!(out, "---@alias {} {}", e.name, values_str.join(" | ")).unwrap();
        writeln!(out).unwrap();
    }
}

fn render_callbacks(out: &mut String, callbacks: &[CallbackMeta]) {
    writeln!(
        out,
        "-- ==================== Callback Signatures ===================="
    )
    .unwrap();
    writeln!(
        out,
        "-- These are the callback functions your Lua scripts should define."
    )
    .unwrap();
    writeln!(
        out,
        "-- They are called by the engine at appropriate times."
    )
    .unwrap();
    writeln!(out).unwrap();

    for cb in callbacks {
        writeln!(out, "---{}", cb.description).unwrap();
        if let Some(ref note) = cb.note {
            writeln!(out, "---NOTE: {}", note).unwrap();
        }
        if let Some(ref ctx) = cb.context {
            writeln!(out, "---Context: {}", ctx).unwrap();
        }
        let param_names: Vec<&str> = cb.params.iter().map(|(n, _)| n.as_str()).collect();
        let is_dynamic = cb.name.contains('<');
        // For dynamic names (e.g., on_update_<scene>), emit params as comments to avoid LSP errors
        if is_dynamic {
            for (pname, ptype) in &cb.params {
                writeln!(out, "--- param: {} {}", pname, lua_type_annotation(ptype)).unwrap();
            }
            if let Some(ref ret) = cb.returns {
                writeln!(out, "--- returns: {}", lua_type_annotation(ret)).unwrap();
            }
            writeln!(
                out,
                "-- function {}({}) end",
                cb.name,
                param_names.join(", ")
            )
            .unwrap();
        } else {
            for (pname, ptype) in &cb.params {
                writeln!(out, "---@param {} {}", pname, lua_type_annotation(ptype)).unwrap();
            }
            if let Some(ref ret) = cb.returns {
                writeln!(out, "---@return {}", lua_type_annotation(ret)).unwrap();
            }
            writeln!(out, "function {}({}) end", cb.name, param_names.join(", ")).unwrap();
        }
        writeln!(out).unwrap();
    }
}

fn render_functions(out: &mut String, functions: &[FnMeta]) {
    let mut current_category = "";

    for f in functions {
        if f.category != current_category {
            current_category = &f.category;
            let title = category_title(current_category);
            writeln!(
                out,
                "-- ==================== {} ====================",
                title
            )
            .unwrap();
            writeln!(out).unwrap();
        }
        render_function(out, f);
    }
}

fn render_function(out: &mut String, f: &FnMeta) {
    write_description(out, &f.description);
    for (pname, ptype) in &f.params {
        writeln!(out, "---@param {} {}", pname, lua_type_annotation(ptype)).unwrap();
    }
    if let Some(ref ret) = f.returns {
        writeln!(out, "---@return {}", lua_type_annotation(ret)).unwrap();
    }
    let param_names: Vec<&str> = f.params.iter().map(|(n, _)| n.as_str()).collect();
    writeln!(
        out,
        "function engine.{}({}) end",
        f.name,
        param_names.join(", ")
    )
    .unwrap();
    writeln!(out).unwrap();
}

/// Writes a description as doc-comment lines, handling multi-line descriptions.
fn write_description(out: &mut String, description: &str) {
    for line in description.lines() {
        writeln!(out, "---{}", line).unwrap();
    }
}

fn render_class(out: &mut String, class: &ClassMeta) {
    let section_title = match class.name.as_str() {
        "EntityBuilder" => "Entity Builder",
        "CollisionEntityBuilder" => "Collision Entity Builder",
        _ => &class.name,
    };
    writeln!(
        out,
        "-- ==================== {} ====================",
        section_title
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "---@class {}", class.name).unwrap();
    writeln!(out, "---{}", class.description).unwrap();
    writeln!(out, "local {} = {{}}", class.name).unwrap();
    writeln!(out).unwrap();

    for m in &class.methods {
        write_description(out, &m.description);
        for p in &m.params {
            let ann = if let Some(ref schema) = p.schema {
                schema.clone()
            } else {
                lua_type_annotation(&p.type_name)
            };
            writeln!(out, "---@param {} {}", p.name, ann).unwrap();
        }
        if let Some(ref ret) = m.returns {
            writeln!(out, "---@return {}", lua_type_annotation(ret)).unwrap();
        }
        let param_names: Vec<&str> = m.params.iter().map(|p| p.name.as_str()).collect();
        writeln!(
            out,
            "function {}:{}({}) end",
            class.name,
            m.name,
            param_names.join(", ")
        )
        .unwrap();
        writeln!(out).unwrap();
    }
}
