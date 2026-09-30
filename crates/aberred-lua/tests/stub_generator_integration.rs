
use aberred_lua::resources::lua_runtime::LuaRuntime;
use aberred_lua::stub_generator;

#[test]
fn generate_stubs_produces_valid_output() {
    let rt = LuaRuntime::new().unwrap();
    let content = stub_generator::generate_stubs(&rt).unwrap();

    // Must start with @meta annotation
    assert!(
        content.starts_with("---@meta"),
        "Should start with ---@meta"
    );

    // Must contain the engine table declaration
    assert!(
        content.contains("engine = {}"),
        "Should declare engine table"
    );
}

#[test]
fn generated_stubs_contain_representative_signatures() {
    let rt = LuaRuntime::new().unwrap();
    let content = stub_generator::generate_stubs(&rt).unwrap();

    // Core functions
    assert!(
        content.contains("function engine.spawn()"),
        "Missing engine.spawn()"
    );
    assert!(
        content.contains("function engine.clone(source_key)"),
        "Missing engine.clone()"
    );
    assert!(
        content.contains("function engine.log(message)"),
        "Missing engine.log()"
    );
    assert!(
        content.contains("function engine.load_texture(id, path, filter)"),
        "Missing engine.load_texture()"
    );
    assert!(
        content.contains("function engine.play_sound(id)"),
        "Missing engine.play_sound()"
    );

    // Signal functions
    assert!(
        content.contains("function engine.set_flag(key)"),
        "Missing engine.set_flag()"
    );
    assert!(
        content.contains("function engine.get_scalar(key)"),
        "Missing engine.get_scalar()"
    );

    // Entity commands
    assert!(
        content.contains("function engine.entity_despawn(entity_id)"),
        "Missing engine.entity_despawn()"
    );
    assert!(
        content.contains("function engine.entity_set_position(entity_id, x, y)"),
        "Missing engine.entity_set_position()"
    );

    // Collision commands
    assert!(
        content.contains("function engine.collision_spawn()"),
        "Missing engine.collision_spawn()"
    );
    assert!(
        content.contains("function engine.collision_entity_despawn(entity_id)"),
        "Missing engine.collision_entity_despawn()"
    );
    assert!(
        content.contains("function engine.collision_clone(source_key)"),
        "Missing engine.collision_clone()"
    );

    // Builder classes
    assert!(
        content.contains("---@class EntityBuilder"),
        "Missing EntityBuilder class"
    );
    assert!(
        content.contains("---@class CollisionEntityBuilder"),
        "Missing CollisionEntityBuilder class"
    );

    // Builder methods with return types
    assert!(
        content.contains("---@return EntityBuilder\nfunction EntityBuilder:with_position(x, y)"),
        "Missing EntityBuilder:with_position"
    );
    assert!(
        content.contains("function EntityBuilder:build()"),
        "Missing EntityBuilder:build()"
    );
    assert!(
        content.contains(
            "---@return CollisionEntityBuilder\nfunction CollisionEntityBuilder:with_position(x, y)"
        ),
        "Missing CollisionEntityBuilder:with_position"
    );

    // Types
    assert!(
        content.contains("---@class EntityContext"),
        "Missing EntityContext type"
    );
    assert!(
        content.contains("---@class CollisionContext"),
        "Missing CollisionContext type"
    );
    assert!(
        content.contains("---@class InputSnapshot"),
        "Missing InputSnapshot type"
    );
    assert!(content.contains("---@class Vec2"), "Missing Vec2 type");

    // Enums
    assert!(content.contains("---@alias Easing"), "Missing Easing enum");
    assert!(
        content.contains("---@alias LoopMode"),
        "Missing LoopMode enum"
    );

    // Callbacks
    assert!(
        content.contains("function on_setup()"),
        "Missing on_setup callback"
    );
    assert!(
        content.contains("function collision_callback(ctx)"),
        "Missing collision_callback"
    );
}

#[test]
fn generated_function_set_matches_meta() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();

    // Collect all function names from __meta
    let meta_fn_names: Vec<String> = lua
        .load(
            r#"
        local names = {}
        for name, _ in pairs(engine.__meta.functions) do
            table.insert(names, name)
        end
        table.sort(names)
        return names
    "#,
        )
        .eval::<Vec<String>>()
        .unwrap();

    let content = stub_generator::generate_stubs(&rt).unwrap();

    // Every function in __meta must appear in the generated stubs
    for name in &meta_fn_names {
        let pattern = format!("function engine.{}(", name);
        assert!(
            content.contains(&pattern),
            "Meta function '{}' not found in generated stubs (looked for '{}')",
            name,
            pattern
        );
    }
}

#[test]
fn generated_builder_methods_match_meta() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();

    // Collect all EntityBuilder method names from __meta
    let method_names: Vec<String> = lua
        .load(
            r#"
        local names = {}
        for name, _ in pairs(engine.__meta.classes.EntityBuilder.methods) do
            table.insert(names, name)
        end
        table.sort(names)
        return names
    "#,
        )
        .eval::<Vec<String>>()
        .unwrap();

    let content = stub_generator::generate_stubs(&rt).unwrap();

    for name in &method_names {
        let pattern = format!("function EntityBuilder:{}(", name);
        assert!(
            content.contains(&pattern),
            "Builder method '{}' not found in generated stubs",
            name
        );
    }
}

#[test]
fn write_stubs_creates_file() {
    let dir = std::env::temp_dir().join("aberred_stub_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("engine.lua");

    let rt = LuaRuntime::new().unwrap();
    let content = stub_generator::generate_stubs(&rt).unwrap();
    stub_generator::write_stubs(&path, &content).unwrap();

    assert!(path.exists(), "Stub file should be created");
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        written, content,
        "Written content should match generated content"
    );

    // Cleanup
    std::fs::remove_dir_all(&dir).ok();
}

// Checked-in generated files must match what the generators produce today.

fn scripts_path(file: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/scripts")
        .join(file)
}

/// Panics naming the first differing line instead of dumping both files.
fn assert_matches_checked_in(file: &str, generated: &str, regen_cmd: &str) {
    let path = scripts_path(file);
    let checked_in = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    if checked_in == generated {
        return;
    }
    let first_diff = checked_in
        .lines()
        .zip(generated.lines())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| checked_in.lines().count().min(generated.lines().count()));
    panic!(
        "assets/scripts/{file} is out of date (first difference at line {}). \
         Regenerate it with `{regen_cmd}`.",
        first_diff + 1
    );
}

#[test]
fn engine_lua_stub_is_up_to_date() {
    let rt = LuaRuntime::new().unwrap();
    let content = stub_generator::generate_stubs(&rt).unwrap();
    assert_matches_checked_in("engine.lua", &content, "cargo run -- --create-lua-stubs");
}

#[test]
fn luarc_json_is_up_to_date() {
    let rt = LuaRuntime::new().unwrap();
    let content = aberred_lua::luarc_generator::generate_luarc(&rt, "engine.lua").unwrap();
    assert_matches_checked_in(".luarc.json", &content, "cargo run -- --create-luarc");
}

// engine.__meta drift protection: the metadata tables that stub generation
// reads must stay populated and schema-complete.

#[test]
fn meta_table_has_functions_and_classes() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(
        r#"
        assert(engine.__meta, "__meta table missing")
        assert(engine.__meta.functions, "__meta.functions missing")
        assert(engine.__meta.classes, "__meta.classes missing")

        local fn_count = 0
        for k, v in pairs(engine.__meta.functions) do
            fn_count = fn_count + 1
            assert(v.description, "missing description for " .. k)
            assert(v.category, "missing category for " .. k)
            assert(v.params, "missing params for " .. k)
        end
        assert(fn_count > 50, "expected >50 functions, got " .. fn_count)

        assert(engine.__meta.functions.spawn.returns.type == "EntityBuilder",
            "spawn should return EntityBuilder")
        assert(engine.__meta.classes.EntityBuilder.methods.with_position,
            "missing with_position on EntityBuilder")

        local method_count = 0
        for _ in pairs(engine.__meta.classes.EntityBuilder.methods) do
            method_count = method_count + 1
        end
        assert(method_count > 50, "expected >50 builder methods, got " .. method_count)
    "#,
    )
    .exec()
    .unwrap();
}

#[test]
fn meta_types_table_is_populated() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(
        r#"
        local types = engine.__meta.types
        assert(types, "__meta.types missing")

        -- Key types must exist
        local required = {"EntityContext", "CollisionContext", "InputSnapshot", "Vec2",
                          "Rect", "SpriteInfo", "AnimationInfo", "TimerInfo", "SignalSet",
                          "CollisionEntity", "CollisionSides", "DigitalButtonState",
                          "DigitalInputs", "PhaseDefinition", "PhaseCallbacks",
                          "ParticleEmitterConfig", "MenuItem", "AnimationRuleCondition"}
        for _, name in ipairs(required) do
            assert(types[name], "missing type: " .. name)
            assert(types[name].description, "missing description for type " .. name)
            assert(types[name].fields, "missing fields for type " .. name)

            -- Each field must have name, type, optional
            for i, field in ipairs(types[name].fields) do
                assert(field.name, name .. " field #" .. i .. " missing name")
                assert(field.type, name .. " field #" .. i .. " missing type")
                assert(field.optional ~= nil, name .. "." .. field.name .. " missing optional")
            end
        end

        -- Spot-check EntityContext fields
        local ec = types.EntityContext
        local ec_field_names = {}
        for _, f in ipairs(ec.fields) do ec_field_names[f.name] = f end
        assert(ec_field_names.id, "EntityContext missing id field")
        assert(ec_field_names.id.type == "integer", "EntityContext.id should be integer")
        assert(ec_field_names.id.optional == false, "EntityContext.id should not be optional")
        assert(ec_field_names.pos, "EntityContext missing pos field")
        assert(ec_field_names.pos.type == "Vec2", "EntityContext.pos should be Vec2")
        assert(ec_field_names.signals, "EntityContext missing signals field")
        assert(ec_field_names.previous_phase, "EntityContext missing previous_phase")

        -- Spot-check Vec2
        local v2 = types.Vec2
        assert(#v2.fields == 2, "Vec2 should have 2 fields")
    "#,
    )
    .exec()
    .unwrap();
}

#[test]
fn meta_enums_table_is_populated() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(
        r#"
        local enums = engine.__meta.enums
        assert(enums, "__meta.enums missing")

        -- Key enums must exist
        local required = {"Easing", "LoopMode", "BoxSide", "ComparisonOp",
                          "ConditionType", "EmitterShape", "TtlSpec", "Category"}
        for _, name in ipairs(required) do
            assert(enums[name], "missing enum: " .. name)
            assert(enums[name].description, "missing description for enum " .. name)
            assert(enums[name].values, "missing values for enum " .. name)
            assert(#enums[name].values > 0, "empty values for enum " .. name)
        end

        -- Hard-code expected Easing values for drift detection
        local expected_easings = {"linear", "quad_in", "quad_out", "quad_in_out",
                                  "cubic_in", "cubic_out", "cubic_in_out"}
        local actual_easings = {}
        for _, v in ipairs(enums.Easing.values) do actual_easings[v] = true end
        for _, e in ipairs(expected_easings) do
            assert(actual_easings[e], "Easing missing value: " .. e)
        end
        assert(#enums.Easing.values == #expected_easings,
            "Easing value count mismatch: expected " .. #expected_easings ..
            " got " .. #enums.Easing.values)

        -- Hard-code expected LoopMode values
        local expected_loops = {"once", "loop", "ping_pong"}
        assert(#enums.LoopMode.values == #expected_loops,
            "LoopMode value count mismatch")

        -- Hard-code expected BoxSide values
        local expected_sides = {"left", "right", "top", "bottom"}
        assert(#enums.BoxSide.values == #expected_sides,
            "BoxSide value count mismatch")

        -- Hard-code expected Category values
        local expected_cats = {"base", "asset", "spawn", "audio", "signal", "phase",
                               "entity", "group", "camera", "collision",
                               "animation", "render"}
        assert(#enums.Category.values == #expected_cats,
            "Category value count mismatch: expected " .. #expected_cats ..
            " got " .. #enums.Category.values)
    "#,
    )
    .exec()
    .unwrap();
}

#[test]
fn meta_callbacks_table_is_populated() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
        local cbs = engine.__meta.callbacks
        assert(cbs, "__meta.callbacks missing")

        -- Key callbacks must exist
        local required = {"on_setup", "on_enter_play", "on_switch_scene",
                          "on_update_<scene>", "phase_on_enter", "phase_on_update",
                          "phase_on_exit", "timer_callback", "collision_callback",
                          "menu_callback"}
        for _, name in ipairs(required) do
            assert(cbs[name], "missing callback: " .. name)
            assert(cbs[name].description, "missing description for callback " .. name)
            assert(cbs[name].params, "missing params for callback " .. name)
        end

        -- Spot-check param shapes
        local pe = cbs.phase_on_enter
        assert(#pe.params == 2, "phase_on_enter should have 2 params, got " .. #pe.params)
        assert(pe.params[1].name == "ctx", "phase_on_enter param 1 should be ctx")
        assert(pe.params[1].type == "EntityContext", "phase_on_enter ctx should be EntityContext")
        assert(pe.returns and pe.returns.type == "string?", "phase_on_enter should return string?")

        local cc = cbs.collision_callback
        assert(#cc.params == 1, "collision_callback should have 1 param")
        assert(cc.params[1].type == "CollisionContext", "collision_callback param should be CollisionContext")

        local mc = cbs.menu_callback
        assert(#mc.params == 3, "menu_callback should have 3 params")

        -- on_setup has no params
        assert(#cbs.on_setup.params == 0, "on_setup should have 0 params")
    "#).exec().unwrap();
}

#[test]
fn meta_functions_complete() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(
        r#"
        local fns = engine.__meta.functions

        -- Expected function names (comprehensive list)
        local expected = {
            -- base
            "log", "log_info", "log_warn", "log_error",
            -- asset
            "load_texture", "load_font", "load_music", "load_sound",
            -- spawn
            "spawn", "clone",
            -- audio
            "play_music", "play_sound", "stop_all_music", "stop_all_sounds",
            -- signal reads
            "get_scalar", "get_integer", "get_string", "has_flag",
            "get_scalars", "get_integers", "get_strings", "get_flags",
            "get_group_count", "get_entity",
            -- signal writes
            "set_scalar", "set_integer", "set_string", "set_flag", "clear_flag",
            "toggle_flag",
            "clear_scalar", "clear_integer", "clear_string",
            "set_entity", "remove_entity",
            -- phase
            "phase_transition",
            -- group
            "track_group", "untrack_group", "clear_tracked_groups", "has_tracked_group",
            -- camera
            "set_camera",
            -- render
            "load_shader", "post_process_shader",
            "post_process_set_float", "post_process_set_int",
            "post_process_set_vec2", "post_process_set_vec4",
            "post_process_clear_uniform", "post_process_clear_uniforms",
            -- animation
            "register_animation",
            -- collision context
            "collision_spawn", "collision_clone",
            "collision_play_sound",
            "collision_set_scalar", "collision_set_integer", "collision_set_string",
            "collision_set_flag", "collision_clear_flag", "collision_toggle_flag",
            "collision_clear_scalar", "collision_clear_integer", "collision_clear_string",
            "collision_phase_transition", "collision_set_camera",
        }

        local missing = {}
        for _, name in ipairs(expected) do
            if not fns[name] then
                table.insert(missing, name)
            end
        end
        assert(#missing == 0,
            "Missing functions in __meta: " .. table.concat(missing, ", "))

        -- Entity commands should exist for both regular and collision prefix
        local entity_cmds = {
            "entity_despawn", "entity_menu_despawn", "entity_set_velocity",
            "entity_set_position", "entity_freeze", "entity_unfreeze",
            "entity_signal_set_flag", "entity_signal_clear_flag", "entity_signal_toggle_flag",
            "entity_insert_lua_timer", "entity_remove_lua_timer",
            "entity_insert_ttl", "entity_set_rotation", "entity_set_scale",
            "entity_set_speed", "entity_set_friction", "entity_set_max_speed",
            "entity_insert_tween_position", "entity_insert_tween_rotation",
            "entity_insert_tween_scale", "entity_remove_tween_position",
            "entity_remove_tween_rotation", "entity_remove_tween_scale",
            "entity_signal_set_scalar", "entity_signal_clear_scalar",
            "entity_signal_set_string", "entity_signal_clear_string",
            "entity_signal_set_integer", "entity_signal_clear_integer",
            "entity_add_force", "entity_remove_force",
            "entity_set_force_enabled", "entity_set_force_value",
            "release_stuckto", "entity_insert_stuckto",
            "entity_restart_animation", "entity_set_animation", "entity_set_sprite_flip",
            "entity_set_shader", "entity_remove_shader",
            "entity_set_tint", "entity_remove_tint",
            "entity_shader_set_float", "entity_shader_set_int",
            "entity_shader_set_vec2", "entity_shader_set_vec4",
            "entity_shader_clear_uniform", "entity_shader_clear_uniforms",
        }

        -- Check regular entity commands exist
        local missing_entity = {}
        for _, name in ipairs(entity_cmds) do
            if not fns[name] then
                table.insert(missing_entity, name)
            end
        end
        assert(#missing_entity == 0,
            "Missing entity functions: " .. table.concat(missing_entity, ", "))

        -- Check collision-prefixed entity commands have parity
        local missing_collision = {}
        for _, name in ipairs(entity_cmds) do
            local collision_name = "collision_" .. name
            if not fns[collision_name] then
                table.insert(missing_collision, collision_name)
            end
        end
        assert(#missing_collision == 0,
            "Missing collision entity functions (parity check): " ..
            table.concat(missing_collision, ", "))
    "#,
    )
    .exec()
    .unwrap();
}

#[test]
fn meta_builder_methods_have_schema_refs() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
        local types = engine.__meta.types
        local classes = engine.__meta.classes
        local builder = classes.EntityBuilder.methods

        -- Helper to find a param by name in a method
        local function find_param(method_name, param_name)
            local method = builder[method_name]
            assert(method, "missing builder method: " .. method_name)
            for _, p in ipairs(method.params) do
                if p.name == param_name then return p end
            end
            error("missing param " .. param_name .. " in " .. method_name)
        end

        -- Check schema references
        local p1 = find_param("with_phase", "table")
        assert(p1.schema == "PhaseDefinition",
            "with_phase.table should have schema PhaseDefinition, got " .. tostring(p1.schema))

        local p2 = find_param("with_particle_emitter", "table")
        assert(p2.schema == "ParticleEmitterConfig",
            "with_particle_emitter.table should have schema ParticleEmitterConfig")

        local p3 = find_param("with_animation_rule", "condition_table")
        assert(p3.schema == "AnimationRuleCondition",
            "with_animation_rule.condition_table should have schema AnimationRuleCondition")

        local p4 = find_param("with_menu", "items")
        assert(p4.schema == "MenuItem[]",
            "with_menu.items should have schema MenuItem[]")

        -- Verify referenced schemas exist in types (strip [] suffix)
        local schemas = {"PhaseDefinition", "ParticleEmitterConfig", "AnimationRuleCondition", "MenuItem"}
        for _, s in ipairs(schemas) do
            assert(types[s], "schema type " .. s .. " not found in __meta.types")
        end
    "#).exec().unwrap();
}

// Hierarchy API metadata: parent commands, `with_parent`, and the
// world-transform fields on EntityContext.

#[test]
fn meta_entity_cmds_include_parent_commands() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(
        r#"
        local fns = engine.__meta.functions
        assert(fns.entity_set_parent, "entity_set_parent missing from __meta.functions")
        assert(fns.entity_set_parent.description, "entity_set_parent missing description")
        assert(fns.entity_remove_parent, "entity_remove_parent missing from __meta.functions")
        assert(fns.entity_remove_parent.description, "entity_remove_parent missing description")
        -- Also check collision_ variants (auto-generated by define_entity_cmds!)
        assert(fns.collision_entity_set_parent, "collision_entity_set_parent missing")
        assert(fns.collision_entity_remove_parent, "collision_entity_remove_parent missing")
    "#,
    )
    .exec()
    .expect("Lua meta parent commands assertions");
}

#[test]
fn meta_builder_includes_with_parent() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
        local builder = engine.__meta.classes.EntityBuilder
        assert(builder, "EntityBuilder class missing from __meta.classes")
        local method = builder.methods.with_parent
        assert(method, "with_parent method missing from EntityBuilder")
        assert(method.description, "with_parent missing description")
        assert(method.params, "with_parent missing params")
        -- Verify param
        local p1 = method.params[1]
        assert(p1.name == "parent_id", "first param should be parent_id, got: " .. tostring(p1.name))
        assert(p1.type == "integer", "parent_id type should be integer, got: " .. tostring(p1.type))
        -- Also check CollisionEntityBuilder
        local collision_builder = engine.__meta.classes.CollisionEntityBuilder
        assert(collision_builder.methods.with_parent, "with_parent missing from CollisionEntityBuilder")
    "#).exec().expect("Lua meta builder with_parent assertions");
}

#[test]
fn meta_entity_context_includes_world_fields() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(
        r#"
        local types = engine.__meta.types
        local ctx_type = types.EntityContext
        assert(ctx_type, "EntityContext type missing from __meta.types")
        -- Find world_pos, world_rotation, world_scale, parent_id fields
        local found_world_pos = false
        local found_world_rotation = false
        local found_world_scale = false
        local found_parent_id = false
        for _, field in ipairs(ctx_type.fields) do
            if field.name == "world_pos" then found_world_pos = true end
            if field.name == "world_rotation" then found_world_rotation = true end
            if field.name == "world_scale" then found_world_scale = true end
            if field.name == "parent_id" then found_parent_id = true end
        end
        assert(found_world_pos, "world_pos field missing from EntityContext type")
        assert(found_world_rotation, "world_rotation field missing from EntityContext type")
        assert(found_world_scale, "world_scale field missing from EntityContext type")
        assert(found_parent_id, "parent_id field missing from EntityContext type")
    "#,
    )
    .exec()
    .expect("Lua meta EntityContext world fields assertions");
}
