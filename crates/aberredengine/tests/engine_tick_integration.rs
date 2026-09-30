//! Engine tick integration tests for collision, and other systems.

#![allow(dead_code, unused_imports)]

use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::system::SystemState;
use aberredengine::core::math::Vec2;

use aberredengine::core::components::boxcollider::BoxCollider;
use aberredengine::core::components::collision::{BoxSides, CollisionCallback, CollisionRule};
use aberredengine::core::components::group::Group;
#[cfg(feature = "lua")]
use aberredengine::lua::components::luacollision::{LuaCollisionCallback, LuaCollisionRule};
#[cfg(feature = "lua")]
use aberredengine::lua::components::luaphase::{LuaPhase, PhaseCallbacks};
#[cfg(feature = "lua")]
use aberredengine::lua::components::luatimer::{LuaTimer, LuaTimerCallback};
use aberredengine::core::components::mapposition::MapPosition;
use aberredengine::core::components::rigidbody::RigidBody;
use aberredengine::core::components::signals::Signals;
use aberredengine::core::components::ttl::Ttl;
use aberredengine::core::events::collision::CollisionEvent;
#[cfg(feature = "lua")]
use aberredengine::core::protocol::audio::AudioCmd;
use aberredengine::core::resources::animationstore::AnimationStore;
use aberredengine::core::resources::appstate::AppState;
use aberredengine::core::resources::camerafollowconfig::CameraFollowConfig;
use aberredengine::core::resources::gameconfig::GameConfig;
use aberredengine::core::resources::input::InputState;
use aberredengine::core::resources::input_bindings::InputBindings;
#[cfg(feature = "lua")]
use aberredengine::lua::resources::lua_runtime::LuaRuntime;
use aberredengine::core::resources::postprocessshader::PostProcessShader;
use aberredengine::render::resources::texturestore::TextureStore;
use aberredengine::core::resources::screensize::ScreenSize;
use aberredengine::core::resources::systemsstore::SystemsStore;
use aberredengine::core::resources::texturedims::TextureDimsStore;
use aberredengine::core::resources::worldsignals::WorldSignals;
use aberredengine::core::resources::worldtime::WorldTime;
use aberredengine::core::resources::collision_rule_index::CollisionRuleIndex;
use aberredengine::core::systems::collision_detector::collision_detector;
use aberredengine::core::systems::GameCtx;
use aberredengine::systems::collision_rule_index::rebuild_collision_rule_index;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::luatimer::{lua_timer_observer, update_lua_timers};

use aberredengine::core::testing::{approx_eq, insert_game_ctx_resources};

mod common;

fn make_world(delta: f32) -> World {
    let mut world = World::new();
    insert_game_ctx_resources(&mut world);
    world.insert_resource(WorldTime {
        elapsed: 0.0,
        delta,
        time_scale: 1.0,
        frame_count: 0,
    });
    world.insert_resource(ScreenSize { w: 800, h: 600 });
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });
    world.init_resource::<TextureStore>();
    world.init_resource::<TextureDimsStore>();
    world.insert_resource(CollisionRuleIndex::default());
    world
}

fn tick_collision_detector(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_collision_rule_index.before(collision_detector));
    schedule.add_systems(collision_detector);
    schedule.run(world);
}

#[cfg(feature = "lua")]
#[test]
fn collision_pipeline_triggers_lua_side_effects() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(SystemsStore::new());
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });
    world.init_resource::<Messages<AudioCmd>>();

    let lua_runtime = LuaRuntime::new().expect("Failed to init Lua runtime");
    world.insert_non_send(lua_runtime);

    {
        let lua_runtime = world.non_send::<LuaRuntime>();
        lua_runtime
            .lua()
            .load(
                r#"
                function on_player_enemy(ctx)
                    engine.collision_entity_signal_set_flag(ctx.a.id, "hit")
                    engine.collision_entity_insert_ttl(ctx.b.id, 1.5)
                end
                "#,
            )
            .exec()
            .expect("Failed to load collision Lua function");
    }

    let a = world
        .spawn((
            Group::new("player"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    let b = world
        .spawn((
            Group::new("enemy"),
            MapPosition::new(5.0, 0.0),
            BoxCollider::new(10.0, 10.0),
        ))
        .id();
    world.spawn((CollisionRule::new(
        "player",
        "enemy",
        LuaCollisionCallback {
            name: "on_player_enemy".into(),
        },
    ),));

    // Track if collision event was triggered
    let saw_collision = std::sync::Arc::new(std::sync::Mutex::new(false));
    let saw_collision_clone = saw_collision.clone();

    // Register the test observer to track collision events
    world.add_observer(move |_trigger: On<CollisionEvent>| {
        *saw_collision_clone.lock().unwrap() = true;
    });

    // Register the actual collision_observer that processes Lua callbacks
    world.add_observer(lua_collision_observer);

    world.flush();

    // Run collision detection - this will trigger CollisionEvent which fires both observers
    tick_collision_detector(&mut world);

    assert!(*saw_collision.lock().unwrap());

    let signals = world
        .get::<Signals>(a)
        .expect("Missing Signals on entity A");
    assert!(signals.has_flag("hit"));
    assert!(world.get::<Ttl>(b).is_some());
}

#[cfg(feature = "lua")]
#[test]
fn collision_callback_error_still_drains_queued_commands() {
    let mut world = make_lua_callback_world(0.0);
    world.init_resource::<Messages<AudioCmd>>();

    {
        let lua_runtime = world.non_send::<LuaRuntime>();
        lua_runtime
            .lua()
            .load(
                r#"
                function on_player_enemy_err(ctx)
                    engine.collision_entity_signal_set_flag(ctx.a.id, "hit")
                    local _ = ctx.nonexistent.field
                end
                "#,
            )
            .exec()
            .expect("Failed to load collision Lua function");
    }

    let a = world
        .spawn((
            Group::new("player"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((
        Group::new("enemy"),
        MapPosition::new(5.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    world.spawn((CollisionRule::new(
        "player",
        "enemy",
        LuaCollisionCallback {
            name: "on_player_enemy_err".into(),
        },
    ),));

    world.add_observer(lua_collision_observer);

    world.flush();

    tick_collision_detector(&mut world);

    let signals = world
        .get::<Signals>(a)
        .expect("Missing Signals on entity A");
    assert!(signals.has_flag("hit"));
}

#[cfg(feature = "lua")]
fn tick_lua_phases(world: &mut World) {
    world
        .run_system_once(lua_phase_system)
        .expect("lua_phase_system should run");
}

#[cfg(feature = "lua")]
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

// =============================================================================
// Meta Schema Drift Protection Tests
// =============================================================================

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
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

// =============================================================================
// Context Builder Snapshot String Tests
// =============================================================================

#[cfg(feature = "lua")]
#[test]
fn context_builder_passes_snapshot_strings_to_lua() {
    use aberredengine::lua::resources::lua_runtime::{
        AnimationSnapshot, EntitySnapshot, LuaPhaseSnapshot, LuaTimerSnapshot, SpriteSnapshot,
        build_entity_context_pooled,
    };
    use std::sync::Arc;

    let runtime = LuaRuntime::new().expect("LuaRuntime init");
    let tables = runtime.get_entity_ctx_pool();
    let lua = runtime.lua();

    // Source strings — simulate what the components hold
    let tex_key: Arc<str> = Arc::from("spaceship");
    let anim_key = String::from("propulsion");
    let phase = String::from("idle");
    let timer_cb = String::from("on_fire");

    // Borrow instead of clone (the new API)
    let sprite_snap = SpriteSnapshot {
        tex_key: tex_key.as_ref(),
        flip_h: false,
        flip_v: false,
    };
    let anim_snap = AnimationSnapshot {
        key: anim_key.as_str(),
        frame_index: 1,
        elapsed: 0.1,
    };
    let phase_snap = LuaPhaseSnapshot {
        current: phase.as_str(),
        time_in_phase: 2.5,
    };
    let timer_snap = LuaTimerSnapshot {
        duration: 3.0,
        elapsed: 1.0,
        callback: timer_cb.as_str(),
    };

    let snapshot = EntitySnapshot {
        entity_id: 99_u64,
        group: None,
        map_pos: None,
        screen_pos: None,
        rigid_body: None,
        rotation: None,
        scale: None,
        rect: None,
        sprite: Some(sprite_snap),
        animation: Some(anim_snap),
        signals: None,
        lua_phase: Some(phase_snap),
        lua_timer: Some(timer_snap),
        previous_phase: None,
        world_pos: None,
        world_rotation: None,
        world_scale: None,
        parent_id: None,
    };
    let ctx =
        build_entity_context_pooled(lua, &tables, &snapshot).expect("build_entity_context_pooled");

    lua.load(r#"
        local ctx = ...
        assert(ctx.sprite ~= nil,       "sprite is nil")
        assert(ctx.animation ~= nil,    "animation is nil")
        assert(ctx.timer ~= nil,        "timer is nil")
        assert(ctx.sprite.tex_key    == "spaceship",   "wrong tex_key: "         .. tostring(ctx.sprite.tex_key))
        assert(ctx.animation.key     == "propulsion",  "wrong animation.key: "   .. tostring(ctx.animation.key))
        assert(ctx.phase             == "idle",         "wrong phase: "           .. tostring(ctx.phase))
        assert(ctx.timer.callback    == "on_fire",     "wrong timer.callback: "  .. tostring(ctx.timer.callback))
    "#).call::<()>(ctx).expect("Lua context string assertions");
}

#[cfg(feature = "lua")]
#[test]
fn context_builder_nil_when_no_snapshots() {
    use aberredengine::lua::resources::lua_runtime::{EntitySnapshot, build_entity_context_pooled};

    let runtime = LuaRuntime::new().expect("LuaRuntime init");
    let tables = runtime.get_entity_ctx_pool();
    let lua = runtime.lua();

    let snapshot = EntitySnapshot {
        entity_id: 1_u64,
        group: None,
        map_pos: None,
        screen_pos: None,
        rigid_body: None,
        rotation: None,
        scale: None,
        rect: None,
        sprite: None,
        animation: None,
        signals: None,
        lua_phase: None,
        lua_timer: None,
        previous_phase: None,
        world_pos: None,
        world_rotation: None,
        world_scale: None,
        parent_id: None,
    };
    let ctx =
        build_entity_context_pooled(lua, &tables, &snapshot).expect("build_entity_context_pooled");

    lua.load(
        r#"
        local ctx = ...
        assert(ctx.sprite    == nil, "sprite should be nil")
        assert(ctx.animation == nil, "animation should be nil")
        assert(ctx.phase     == nil, "phase should be nil")
        assert(ctx.timer     == nil, "timer should be nil")
    "#,
    )
    .call::<()>(ctx)
    .expect("Lua nil assertions");
}

#[cfg(feature = "lua")]
#[test]
fn lua_phase_on_exit_sees_post_swap_phase_state() {
    let mut world = make_world(0.25);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(SystemsStore::new());
    world.insert_resource(InputState::default());
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });

    let lua_runtime = LuaRuntime::new().expect("Failed to init Lua runtime");
    world.insert_non_send(lua_runtime);

    {
        let lua_runtime = world.non_send::<LuaRuntime>();
        lua_runtime
            .lua()
            .load(
                r#"
                function moving_exit(ctx)
                    engine.set_string("exit_phase_seen", ctx.phase)
                    engine.set_scalar("exit_time_in_phase_seen", ctx.time_in_phase)
                end
                "#,
            )
            .exec()
            .expect("Failed to load Lua phase callback");
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert("idle".into(), PhaseCallbacks::default());
    phases.insert(
        "moving".into(),
        PhaseCallbacks {
            on_enter: None,
            on_update: None,
            on_exit: Some("moving_exit".into()),
        },
    );
    phases.insert("attacking".into(), PhaseCallbacks::default());

    let entity = world.spawn((LuaPhase::new("moving", phases),)).id();
    world.get_mut::<LuaPhase>(entity).unwrap().next = Some("attacking".into());

    tick_lua_phases(&mut world);

    let world_signals = world.resource::<WorldSignals>();
    assert_eq!(
        world_signals
            .get_string("exit_phase_seen")
            .map(|s| s.as_str()),
        Some("attacking")
    );
    assert!(approx_eq(
        world_signals
            .get_scalar("exit_time_in_phase_seen")
            .expect("exit time signal"),
        0.0
    ));
}

// =============================================================================
// CollisionRule<C> generic consistency — CollisionRule and LuaCollisionRule
// must produce identical match_and_order results for the same group inputs.
// =============================================================================

fn dummy_callback(_a: Entity, _b: Entity, _sa: &BoxSides, _sb: &BoxSides, _ctx: &mut GameCtx) {}

/// Build matching CollisionRule and LuaCollisionRule pairs with the same groups.
#[cfg(feature = "lua")]
fn make_matching_rules(ga: &str, gb: &str) -> (CollisionRule, LuaCollisionRule) {
    let rust_rule = CollisionRule::new(ga, gb, dummy_callback as CollisionCallback);
    let lua_rule = CollisionRule::new(ga, gb, LuaCollisionCallback { name: "cb".into() });
    (rust_rule, lua_rule)
}

#[cfg(feature = "lua")]
#[test]
fn collision_rule_and_lua_rule_match_direct_groups_consistently() {
    let (rust_rule, lua_rule) = make_matching_rules("ball", "brick");
    let ent_a = Entity::from_bits(1);
    let ent_b = Entity::from_bits(2);
    assert_eq!(
        rust_rule.match_and_order(ent_a, ent_b, "ball", "brick"),
        lua_rule.match_and_order(ent_a, ent_b, "ball", "brick"),
    );
    assert_eq!(
        lua_rule.match_and_order(ent_a, ent_b, "ball", "brick"),
        Some((ent_a, ent_b))
    );
}

#[cfg(feature = "lua")]
#[test]
fn collision_rule_and_lua_rule_reorder_entities_consistently_when_groups_swapped() {
    let (rust_rule, lua_rule) = make_matching_rules("ball", "brick");
    let ent_a = Entity::from_bits(1);
    let ent_b = Entity::from_bits(2);
    // Groups arrive swapped relative to the rule — both types must reorder identically.
    assert_eq!(
        rust_rule.match_and_order(ent_a, ent_b, "brick", "ball"),
        lua_rule.match_and_order(ent_a, ent_b, "brick", "ball"),
    );
    assert_eq!(
        lua_rule.match_and_order(ent_a, ent_b, "brick", "ball"),
        Some((ent_b, ent_a))
    );
}

#[cfg(feature = "lua")]
#[test]
fn collision_rule_and_lua_rule_both_return_none_for_non_matching_groups() {
    let (rust_rule, lua_rule) = make_matching_rules("ball", "brick");
    let ent_a = Entity::from_bits(1);
    let ent_b = Entity::from_bits(2);
    assert_eq!(
        rust_rule.match_and_order(ent_a, ent_b, "player", "enemy"),
        lua_rule.match_and_order(ent_a, ent_b, "player", "enemy"),
    );
    assert_eq!(
        lua_rule.match_and_order(ent_a, ent_b, "player", "enemy"),
        None
    );
}

// =============================================================================
// Command-drain ordering tests
//
// These tests lock in the canonical drain order introduced by the
// drain_and_process_effect_commands refactor.  They must pass BEFORE the
// refactor and continue to pass AFTER it.
// =============================================================================

/// Build a minimal world suitable for tests that exercise Lua callback side effects.
#[cfg(feature = "lua")]
fn make_lua_callback_world(delta: f32) -> World {
    let mut world = make_world(delta);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(SystemsStore::new());
    world.insert_resource(InputState::default());
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });
    let lua_runtime = LuaRuntime::new().expect("LuaRuntime::new");
    world.insert_non_send(lua_runtime);
    world
}

/// Register the lua timer observer, then run the update pass once, so the
/// LuaTimerEvent is both emitted and handled within one call.
#[cfg(feature = "lua")]
fn tick_lua_timers_with_observer(world: &mut World) {
    world.add_observer(lua_timer_observer);
    world.flush();
    world
        .run_system_once(update_lua_timers)
        .expect("update_lua_timers should run");
}

/// Test 1 — Regular path: spawn before clone (timer callback)
///
/// A timer callback registers a freshly spawned entity under a key, then
/// clones it.  The clone must succeed, which requires spawn to be processed
/// before clone inside the same drain pass.
#[cfg(feature = "lua")]
#[test]
fn timer_callback_spawn_then_clone_same_drain() {
    let mut world = make_lua_callback_world(1.0);

    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function spawn_and_clone_cb(ctx, input)
                    engine.spawn():with_group("template"):register_as("tpl"):build()
                    engine.clone("tpl"):with_group("copy"):build()
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }

    world.spawn((LuaTimer::new(
        0.5,
        LuaTimerCallback {
            name: "spawn_and_clone_cb".into(),
        },
    ),));

    tick_lua_timers_with_observer(&mut world);

    let copy_count = world
        .query::<&Group>()
        .iter(&world)
        .filter(|g| g.name() == "copy")
        .count();
    assert_eq!(
        copy_count, 1,
        "expected one cloned entity with group 'copy'"
    );
}

/// Test 2 — Collision path: spawn before clone (collision callback)
///
/// A collision callback registers a freshly spawned entity then clones it.
/// Both operations go through collision-scoped queues
/// (collision_spawn / collision_clone).  The clone must succeed, which
/// requires spawn to drain before clone.
#[cfg(feature = "lua")]
#[test]
fn collision_callback_spawn_then_clone_same_drain() {
    let mut world = make_lua_callback_world(0.0);

    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function on_coll_spawn_clone(ctx)
                    engine.collision_spawn():with_group("proj"):register_as("proj_src"):build()
                    engine.collision_clone("proj_src"):with_group("proj_copy"):build()
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }

    let _a = world
        .spawn((
            Group::new("shooter"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
        ))
        .id();
    let _b = world
        .spawn((
            Group::new("target"),
            MapPosition::new(5.0, 0.0),
            BoxCollider::new(10.0, 10.0),
        ))
        .id();
    world.spawn(LuaCollisionRule::new(
        "shooter",
        "target",
        LuaCollisionCallback {
            name: "on_coll_spawn_clone".into(),
        },
    ));

    world.add_observer(lua_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let copy_count = world
        .query::<&Group>()
        .iter(&world)
        .filter(|g| g.name() == "proj_copy")
        .count();
    assert_eq!(
        copy_count, 1,
        "expected one collision-cloned entity with group 'proj_copy'"
    );
}

/// Test 3 — Lua phase: return-value transition takes precedence over
/// engine.phase_transition() called in the same on_update.
///
/// If the on_update callback BOTH calls `engine.phase_transition(id, "via_cmd")`
/// AND returns `"return_winner"`, the return value must win because
/// apply_callback_transitions runs after the phase drain.
#[cfg(feature = "lua")]
#[test]
fn lua_phase_return_value_beats_phase_transition_cmd() {
    let mut world = make_lua_callback_world(0.016);

    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function idle_update(ctx, input, dt)
                    engine.phase_transition(ctx.id, "via_cmd")
                    return "return_winner"
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbacks {
            on_enter: None,
            on_update: Some("idle_update".into()),
            on_exit: None,
        },
    );
    phases.insert("via_cmd".into(), PhaseCallbacks::default());
    phases.insert("return_winner".into(), PhaseCallbacks::default());

    let entity = world.spawn((LuaPhase::new("idle", phases),)).id();

    // First tick: idle_update runs, queues both a PhaseCmd and a return transition.
    tick_lua_phases(&mut world);

    // The winning transition is stored in `next` after the first tick.
    let phase = world.get::<LuaPhase>(entity).unwrap();
    assert_eq!(
        phase.next.as_deref(),
        Some("return_winner"),
        "return value should override the phase_transition() cmd"
    );

    // Second tick: the pending transition is applied.
    tick_lua_phases(&mut world);

    let phase = world.get::<LuaPhase>(entity).unwrap();
    assert_eq!(phase.current, "return_winner");
}

/// Test 4 — Collision path: moving phase drain to front does not suppress
/// other queues.
///
/// A collision callback queues a phase transition, a world-signal mutation,
/// and a camera command.  All three must be observed after the observer runs.
/// This guards against the collision drain reorder causing any queue to be
/// silently dropped.
#[cfg(feature = "lua")]
#[test]
fn collision_callback_phase_plus_signal_all_processed() {
    let mut world = make_lua_callback_world(0.0);

    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function on_multi_effect(ctx)
                    engine.collision_phase_transition(ctx.a.id, "hit")
                    engine.collision_set_flag("was_hit")
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }

    let mut phases: rustc_hash::FxHashMap<String, PhaseCallbacks> =
        rustc_hash::FxHashMap::default();
    phases.insert("idle".into(), PhaseCallbacks::default());
    phases.insert("hit".into(), PhaseCallbacks::default());

    let a = world
        .spawn((
            Group::new("hero"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            LuaPhase::new("idle", phases),
        ))
        .id();
    world.spawn((
        Group::new("hazard"),
        MapPosition::new(5.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    world.spawn(LuaCollisionRule::new(
        "hero",
        "hazard",
        LuaCollisionCallback {
            name: "on_multi_effect".into(),
        },
    ));

    world.add_observer(lua_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    // Phase transition queued
    let phase = world.get::<LuaPhase>(a).unwrap();
    assert_eq!(
        phase.next.as_deref(),
        Some("hit"),
        "phase transition should be queued"
    );

    // World signal mutation applied
    let signals = world.resource::<WorldSignals>();
    assert!(
        signals.has_flag("was_hit"),
        "world signal flag should be set"
    );
}
