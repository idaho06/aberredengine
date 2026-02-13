use aberredengine::resources::lua_runtime::LuaRuntime;
use aberredengine::stub_generator;

#[test]
fn generate_stubs_produces_valid_output() {
    let rt = LuaRuntime::new().unwrap();
    let content = stub_generator::generate_stubs(&rt).unwrap();

    // Must start with @meta annotation
    assert!(content.starts_with("---@meta"), "Should start with ---@meta");

    // Must contain the engine table declaration
    assert!(content.contains("engine = {}"), "Should declare engine table");
}

#[test]
fn generated_stubs_contain_representative_signatures() {
    let rt = LuaRuntime::new().unwrap();
    let content = stub_generator::generate_stubs(&rt).unwrap();

    // Core functions
    assert!(content.contains("function engine.spawn()"), "Missing engine.spawn()");
    assert!(content.contains("function engine.clone(source_key)"), "Missing engine.clone()");
    assert!(content.contains("function engine.log(message)"), "Missing engine.log()");
    assert!(content.contains("function engine.load_texture(id, path)"), "Missing engine.load_texture()");
    assert!(content.contains("function engine.play_sound(id)"), "Missing engine.play_sound()");

    // Signal functions
    assert!(content.contains("function engine.set_flag(key)"), "Missing engine.set_flag()");
    assert!(content.contains("function engine.get_scalar(key)"), "Missing engine.get_scalar()");

    // Entity commands
    assert!(content.contains("function engine.entity_despawn(entity_id)"), "Missing engine.entity_despawn()");
    assert!(content.contains("function engine.entity_set_position(entity_id, x, y)"), "Missing engine.entity_set_position()");

    // Collision commands
    assert!(content.contains("function engine.collision_spawn()"), "Missing engine.collision_spawn()");
    assert!(content.contains("function engine.collision_entity_despawn(entity_id)"), "Missing engine.collision_entity_despawn()");
    assert!(content.contains("function engine.collision_clone(source_key)"), "Missing engine.collision_clone()");

    // Builder classes
    assert!(content.contains("---@class EntityBuilder"), "Missing EntityBuilder class");
    assert!(content.contains("---@class CollisionEntityBuilder"), "Missing CollisionEntityBuilder class");

    // Builder methods with return types
    assert!(content.contains("---@return EntityBuilder\nfunction EntityBuilder:with_position(x, y)"), "Missing EntityBuilder:with_position");
    assert!(content.contains("function EntityBuilder:build()"), "Missing EntityBuilder:build()");
    assert!(content.contains("---@return CollisionEntityBuilder\nfunction CollisionEntityBuilder:with_position(x, y)"), "Missing CollisionEntityBuilder:with_position");

    // Types
    assert!(content.contains("---@class EntityContext"), "Missing EntityContext type");
    assert!(content.contains("---@class CollisionContext"), "Missing CollisionContext type");
    assert!(content.contains("---@class InputSnapshot"), "Missing InputSnapshot type");
    assert!(content.contains("---@class Vector2"), "Missing Vector2 type");

    // Enums
    assert!(content.contains("---@alias Easing"), "Missing Easing enum");
    assert!(content.contains("---@alias LoopMode"), "Missing LoopMode enum");

    // Callbacks
    assert!(content.contains("function on_setup()"), "Missing on_setup callback");
    assert!(content.contains("function collision_callback(ctx)"), "Missing collision_callback");
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
    assert_eq!(written, content, "Written content should match generated content");

    // Cleanup
    std::fs::remove_dir_all(&dir).ok();
}
