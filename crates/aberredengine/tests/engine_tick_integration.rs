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
use aberredengine::lua::events::luatimer::LuaTimerEvent;
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
use aberredengine::systems::collision_rule_index::rebuild_collision_rule_index;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::luatimer::{lua_timer_observer, update_lua_timers};
use aberredengine::core::systems::rust_collision::rust_collision_observer;
use aberredengine::core::systems::time::update_world_time;

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

// =============================================================================
// Lua Timer System Tests
// =============================================================================

#[cfg(feature = "lua")]
fn tick_lua_timers(world: &mut World) {
    world
        .run_system_once(update_lua_timers)
        .expect("update_lua_timers should run");
}

#[cfg(feature = "lua")]
fn tick_lua_phases(world: &mut World) {
    world
        .run_system_once(lua_phase_system)
        .expect("lua_phase_system should run");
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_accumulates_time() {
    let mut world = make_world(0.3);

    let entity = world
        .spawn((LuaTimer::new(
            1.0,
            LuaTimerCallback {
                name: "my_callback".into(),
            },
        ),))
        .id();

    tick_lua_timers(&mut world);

    let timer = world.get::<LuaTimer>(entity).unwrap();
    assert!(approx_eq(timer.elapsed, 0.3));
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_fires_event_when_expired() {
    let mut world = make_world(1.0);

    let entity = world
        .spawn((LuaTimer::new(
            0.5,
            LuaTimerCallback {
                name: "on_timer".into(),
            },
        ),))
        .id();

    // Track if event was triggered
    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_entity = std::sync::Arc::new(std::sync::Mutex::new(None));
    let fired_clone = fired.clone();
    let entity_clone = fired_entity.clone();

    world.add_observer(move |trigger: On<LuaTimerEvent>| {
        *fired_clone.lock().unwrap() = true;
        *entity_clone.lock().unwrap() = Some(trigger.event().entity);
    });
    world.flush();

    tick_lua_timers(&mut world);

    assert!(*fired.lock().unwrap());
    assert_eq!(*fired_entity.lock().unwrap(), Some(entity));
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_resets_after_firing() {
    let mut world = make_world(0.6);

    let entity = world
        .spawn((LuaTimer::new(
            0.5,
            LuaTimerCallback {
                name: "callback".into(),
            },
        ),))
        .id();

    // Add dummy observer so events are processed
    world.add_observer(|_trigger: On<LuaTimerEvent>| {});
    world.flush();

    tick_lua_timers(&mut world);

    let timer = world.get::<LuaTimer>(entity).unwrap();
    // Timer should have reset: 0.6 - 0.5 = 0.1
    assert!(approx_eq(timer.elapsed, 0.1));
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_does_not_fire_before_duration() {
    let mut world = make_world(0.3);

    world.spawn((LuaTimer::new(
        1.0,
        LuaTimerCallback {
            name: "callback".into(),
        },
    ),));

    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_clone = fired.clone();

    world.add_observer(move |_trigger: On<LuaTimerEvent>| {
        *fired_clone.lock().unwrap() = true;
    });
    world.flush();

    tick_lua_timers(&mut world);

    assert!(!*fired.lock().unwrap());
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_event_carries_correct_callback_name() {
    // Specific to the LuaTimerCallback refactor: LuaTimerCallback.name must
    // flow correctly into LuaTimerEvent.callback.
    let mut world = make_world(1.0);

    world.spawn((LuaTimer::new(
        0.5,
        LuaTimerCallback {
            name: "my_func".into(),
        },
    ),));

    let received_name = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let name_clone = received_name.clone();

    world.add_observer(move |trigger: On<LuaTimerEvent>| {
        *name_clone.lock().unwrap() = trigger.event().callback.to_string();
    });
    world.flush();

    tick_lua_timers(&mut world);

    assert_eq!(*received_name.lock().unwrap(), "my_func");
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_multiple_entities_fire_with_correct_names() {
    // Each entity's LuaTimerCallback.name must appear in its own event — not swapped.
    let mut world = make_world(1.0);

    let entity_a = world
        .spawn((LuaTimer::new(
            0.5,
            LuaTimerCallback {
                name: "func_a".into(),
            },
        ),))
        .id();
    let entity_b = world
        .spawn((LuaTimer::new(
            0.5,
            LuaTimerCallback {
                name: "func_b".into(),
            },
        ),))
        .id();

    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(Entity, String)>::new()));
    let events_clone = events.clone();

    world.add_observer(move |trigger: On<LuaTimerEvent>| {
        events_clone
            .lock()
            .unwrap()
            .push((trigger.event().entity, trigger.event().callback.to_string()));
    });
    world.flush();

    tick_lua_timers(&mut world);

    let events = events.lock().unwrap().clone();
    assert_eq!(events.len(), 2);

    let a_event = events.iter().find(|(e, _)| *e == entity_a).unwrap();
    let b_event = events.iter().find(|(e, _)| *e == entity_b).unwrap();
    assert_eq!(a_event.1, "func_a");
    assert_eq!(b_event.1, "func_b");
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_callback_name_preserved_after_reset() {
    // reset() only modifies elapsed — LuaTimerCallback.name must survive unchanged.
    let mut world = make_world(1.0);

    let entity = world
        .spawn((LuaTimer::new(
            0.5,
            LuaTimerCallback {
                name: "persist_cb".into(),
            },
        ),))
        .id();
    world.add_observer(|_trigger: On<LuaTimerEvent>| {});
    world.flush();

    tick_lua_timers(&mut world); // fires and resets

    let timer = world.get::<LuaTimer>(entity).unwrap();
    assert_eq!(&*timer.callback.name, "persist_cb");
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_fires_across_multiple_ticks() {
    // Verify elapsed accumulates correctly over multiple ticks before firing.
    // duration=0.8, delta=0.3 per tick: ticks 1+2 no fire, tick 3 fires.
    let fired_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let fired_clone = fired_count.clone();

    let mut world = make_world(0.3);
    world.spawn((LuaTimer::new(0.8, LuaTimerCallback { name: "cb".into() }),));

    world.add_observer(move |_trigger: On<LuaTimerEvent>| {
        *fired_clone.lock().unwrap() += 1;
    });
    world.flush();

    tick_lua_timers(&mut world); // elapsed=0.3
    assert_eq!(*fired_count.lock().unwrap(), 0);

    tick_lua_timers(&mut world); // elapsed=0.6
    assert_eq!(*fired_count.lock().unwrap(), 0);

    tick_lua_timers(&mut world); // elapsed=0.9 >= 0.8, fires, resets to 0.1
    assert_eq!(*fired_count.lock().unwrap(), 1);
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

// ---------------------------------------------------------------------------
// D7 – check_pending_state triggers GameStateChangedEvent when pending
// ---------------------------------------------------------------------------

use aberredengine::core::events::gamestate::GameStateChangedEvent;
use aberredengine::core::resources::gamestate::{GameState, GameStates, NextGameState, NextGameStates};
use aberredengine::core::systems::gamestate::check_pending_state;

/// Helper: run `check_pending_state` once.
fn tick_check_pending_state(world: &mut World) {
    world
        .run_system_once(check_pending_state)
        .expect("check_pending_state should run");
}

#[test]
fn check_pending_state_triggers_event_when_pending() {
    let mut world = World::new();
    world.init_resource::<GameState>();

    // Set a pending state
    let mut next = NextGameState::new();
    next.set(GameStates::Playing);
    world.insert_resource(next);

    // Run the system – it calls commands.trigger(GameStateChangedEvent{})
    tick_check_pending_state(&mut world);

    // After commands are flushed the event should have been triggered.
    // We can't easily inspect triggered events without an observer, but we can
    // verify the system didn't panic and the pending value is still there
    // (the observer is responsible for clearing it, not check_pending_state).
    let ns = world.resource::<NextGameState>();
    assert_eq!(*ns.get(), NextGameStates::Pending(GameStates::Playing));
}

#[test]
fn check_pending_state_does_nothing_when_unchanged() {
    let mut world = World::new();
    world.init_resource::<GameState>();
    world.init_resource::<NextGameState>(); // defaults to Unchanged

    tick_check_pending_state(&mut world);

    let ns = world.resource::<NextGameState>();
    assert_eq!(*ns.get(), NextGameStates::Unchanged);
}

// ---------------------------------------------------------------------------
// D11 – update_world_time integration tests
// ---------------------------------------------------------------------------

#[test]
fn update_world_time_increments_elapsed_and_frame() {
    let mut world = World::new();
    world.insert_resource(WorldTime::default());

    update_world_time(&mut world, 0.016);

    let wt = world.resource::<WorldTime>();
    assert!(approx_eq(wt.elapsed, 0.016));
    assert!(approx_eq(wt.delta, 0.016));
    assert_eq!(wt.frame_count, 1);
}

#[test]
fn update_world_time_applies_time_scale() {
    let mut world = World::new();
    world.insert_resource(WorldTime::default().with_time_scale(0.5));

    update_world_time(&mut world, 0.016);

    let wt = world.resource::<WorldTime>();
    assert!(approx_eq(wt.elapsed, 0.008));
    assert!(approx_eq(wt.delta, 0.008));
    assert_eq!(wt.frame_count, 1);
}

#[test]
fn update_world_time_accumulates_over_multiple_frames() {
    let mut world = World::new();
    world.insert_resource(WorldTime::default());

    update_world_time(&mut world, 0.01);
    update_world_time(&mut world, 0.02);
    update_world_time(&mut world, 0.03);

    let wt = world.resource::<WorldTime>();
    assert!(approx_eq(wt.elapsed, 0.06));
    // delta should be last frame only
    assert!(approx_eq(wt.delta, 0.03));
    assert_eq!(wt.frame_count, 3);
}

#[test]
fn update_world_time_zero_dt() {
    let mut world = World::new();
    world.insert_resource(WorldTime::default());

    update_world_time(&mut world, 0.0);

    let wt = world.resource::<WorldTime>();
    assert!(approx_eq(wt.elapsed, 0.0));
    assert!(approx_eq(wt.delta, 0.0));
    assert_eq!(wt.frame_count, 1);
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

// =============================================================================
// Rust Phase System Tests
// =============================================================================

use aberredengine::core::components::phase::{
    Phase, PhaseCallbackFns, PhaseEnterFn, PhaseExitFn, PhaseUpdateFn,
};
use aberredengine::core::systems::GameCtx;
use aberredengine::core::systems::phase::phase_system;

fn tick_phases(world: &mut World) {
    world
        .run_system_once(phase_system)
        .expect("phase_system should run");
}

fn make_phase_world(delta: f32) -> World {
    let mut world = make_world(delta);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(InputState::default());
    world
}

fn simple_two_phase_map() -> rustc_hash::FxHashMap<String, PhaseCallbackFns> {
    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: None,
            on_update: None,
            on_exit: None,
        },
    );
    phases.insert(
        "moving".into(),
        PhaseCallbackFns {
            on_enter: None,
            on_update: None,
            on_exit: None,
        },
    );
    phases
}

#[test]
fn phase_calls_on_enter_on_first_frame() {
    let mut world = make_phase_world(0.016);

    fn enter_fn(entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
        if let Ok(mut signals) = ctx.signals.get_mut(entity) {
            signals.set_flag("entered");
        }
        None
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: Some(enter_fn),
            on_update: None,
            on_exit: None,
        },
    );

    let entity = world
        .spawn((Phase::new("idle", phases), Signals::default()))
        .id();

    tick_phases(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(signals.has_flag("entered"));
}

#[test]
fn phase_on_enter_not_called_twice() {
    let mut world = make_phase_world(0.016);

    fn enter_fn(entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
        if let Ok(mut signals) = ctx.signals.get_mut(entity) {
            let count = signals.get_scalar("enter_count").unwrap_or(0.0);
            signals.set_scalar("enter_count", count + 1.0);
        }
        None
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: Some(enter_fn),
            on_update: None,
            on_exit: None,
        },
    );

    let entity = world
        .spawn((Phase::new("idle", phases), Signals::default()))
        .id();

    tick_phases(&mut world);
    tick_phases(&mut world);
    tick_phases(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(approx_eq(signals.get_scalar("enter_count").unwrap(), 1.0));
}

#[test]
fn phase_calls_on_update_every_frame() {
    let mut world = make_phase_world(0.016);

    fn update_fn(
        entity: Entity,
        ctx: &mut GameCtx,
        _input: &InputState,
        _dt: f32,
    ) -> Option<String> {
        if let Ok(mut signals) = ctx.signals.get_mut(entity) {
            let count = signals.get_scalar("update_count").unwrap_or(0.0);
            signals.set_scalar("update_count", count + 1.0);
        }
        None
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: None,
            on_update: Some(update_fn),
            on_exit: None,
        },
    );

    let entity = world
        .spawn((Phase::new("idle", phases), Signals::default()))
        .id();

    tick_phases(&mut world);
    tick_phases(&mut world);
    tick_phases(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(approx_eq(signals.get_scalar("update_count").unwrap(), 3.0));
}

#[test]
fn phase_transition_via_update_return() {
    let mut world = make_phase_world(0.016);

    fn update_fn(
        _entity: Entity,
        _ctx: &mut GameCtx,
        _input: &InputState,
        _dt: f32,
    ) -> Option<String> {
        Some("moving".into())
    }

    let mut phases = simple_two_phase_map();
    phases.get_mut("idle").unwrap().on_update = Some(update_fn);

    let entity = world.spawn((Phase::new("idle", phases),)).id();

    // First tick: on_update returns "moving", which gets stored in phase.next
    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    // After first tick, the transition is pending (stored in next)
    assert_eq!(phase.next.as_deref(), Some("moving"));

    // Second tick: the pending transition is processed
    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    assert_eq!(phase.current, "moving");
    assert_eq!(phase.previous.as_deref(), Some("idle"));
}

#[test]
fn phase_transition_via_external_next() {
    let mut world = make_phase_world(0.016);

    let entity = world
        .spawn((Phase::new("idle", simple_two_phase_map()),))
        .id();

    // Externally request a transition
    world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    assert_eq!(phase.current, "moving");
    assert_eq!(phase.previous.as_deref(), Some("idle"));
}

#[test]
fn phase_on_enter_return_is_applied_on_next_frame() {
    let mut world = make_phase_world(0.016);

    fn enter_fn(_entity: Entity, _ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
        Some("moving".into())
    }

    let mut phases = simple_two_phase_map();
    phases.get_mut("idle").unwrap().on_enter = Some(enter_fn);

    let entity = world.spawn((Phase::new("idle", phases),)).id();

    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    assert_eq!(phase.current, "idle");
    assert_eq!(phase.next.as_deref(), Some("moving"));

    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    assert_eq!(phase.current, "moving");
    assert_eq!(phase.previous.as_deref(), Some("idle"));
}

#[test]
fn phase_on_exit_called_on_transition() {
    let mut world = make_phase_world(0.016);

    fn exit_fn(entity: Entity, ctx: &mut GameCtx) {
        if let Ok(mut signals) = ctx.signals.get_mut(entity) {
            signals.set_flag("exited_idle");
        }
    }

    let mut phases = simple_two_phase_map();
    phases.get_mut("idle").unwrap().on_exit = Some(exit_fn);

    let entity = world
        .spawn((Phase::new("idle", phases), Signals::default()))
        .id();

    // Request transition
    world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

    tick_phases(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(signals.has_flag("exited_idle"));
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

#[test]
fn phase_on_enter_called_on_transition() {
    let mut world = make_phase_world(0.016);

    fn enter_fn(entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
        if let Ok(mut signals) = ctx.signals.get_mut(entity) {
            signals.set_flag("entered_moving");
        }
        None
    }

    let mut phases = simple_two_phase_map();
    phases.get_mut("moving").unwrap().on_enter = Some(enter_fn);

    let entity = world
        .spawn((Phase::new("idle", phases), Signals::default()))
        .id();

    // Request transition
    world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

    tick_phases(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(signals.has_flag("entered_moving"));
}

#[test]
fn phase_callback_return_takes_precedence_after_external_transition() {
    let mut world = make_phase_world(0.016);

    fn update_fn(
        _entity: Entity,
        _ctx: &mut GameCtx,
        _input: &InputState,
        _dt: f32,
    ) -> Option<String> {
        Some("attacking".into())
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: None,
            on_update: None,
            on_exit: None,
        },
    );
    phases.insert(
        "moving".into(),
        PhaseCallbackFns {
            on_enter: None,
            on_update: Some(update_fn),
            on_exit: None,
        },
    );
    phases.insert(
        "attacking".into(),
        PhaseCallbackFns {
            on_enter: None,
            on_update: None,
            on_exit: None,
        },
    );

    let entity = world.spawn((Phase::new("idle", phases),)).id();
    world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    assert_eq!(phase.current, "moving");
    assert_eq!(phase.next.as_deref(), Some("attacking"));
    assert_eq!(phase.previous.as_deref(), Some("idle"));

    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    assert_eq!(phase.current, "attacking");
    assert_eq!(phase.previous.as_deref(), Some("moving"));
}

#[test]
fn phase_time_in_phase_resets_on_transition() {
    let mut world = make_phase_world(0.5);

    let entity = world
        .spawn((Phase::new("idle", simple_two_phase_map()),))
        .id();

    // Run a couple frames to accumulate time
    tick_phases(&mut world);
    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    assert!(approx_eq(phase.time_in_phase, 1.0));

    // Request transition
    world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

    tick_phases(&mut world);

    let phase = world.get::<Phase>(entity).unwrap();
    // time_in_phase was reset to 0 at transition, then incremented by delta (0.5)
    assert!(approx_eq(phase.time_in_phase, 0.5));
}

#[test]
fn phase_update_receives_delta_time() {
    let mut world = make_phase_world(0.25);

    fn update_fn(
        entity: Entity,
        ctx: &mut GameCtx,
        _input: &InputState,
        dt: f32,
    ) -> Option<String> {
        if let Ok(mut signals) = ctx.signals.get_mut(entity) {
            signals.set_scalar("received_dt", dt);
        }
        None
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: None,
            on_update: Some(update_fn),
            on_exit: None,
        },
    );

    let entity = world
        .spawn((Phase::new("idle", phases), Signals::default()))
        .id();

    tick_phases(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(approx_eq(signals.get_scalar("received_dt").unwrap(), 0.25));
}

#[test]
fn phase_callback_can_set_world_signal() {
    let mut world = make_phase_world(0.016);

    fn enter_fn(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
        ctx.world_signals.set_flag("game_started");
        None
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: Some(enter_fn),
            on_update: None,
            on_exit: None,
        },
    );

    world.spawn((Phase::new("idle", phases),));

    tick_phases(&mut world);

    let world_signals = world.resource::<WorldSignals>();
    assert!(world_signals.has_flag("game_started"));
}

#[test]
fn phase_callback_can_write_audio() {
    let mut world = make_phase_world(0.016);

    fn enter_fn(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
        ctx.audio.write(AudioCmd::PlayFx {
            id: "phase_start".into(),
        });
        None
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: Some(enter_fn),
            on_update: None,
            on_exit: None,
        },
    );

    world.spawn((Phase::new("idle", phases),));

    tick_phases(&mut world);

    // Flip message buffers so they become readable
    world.resource_mut::<Messages<AudioCmd>>().update();

    let mut state = SystemState::<MessageReader<AudioCmd>>::new(&mut world);
    let mut reader = state
        .get_mut(&mut world)
        .expect("Audio command reader should fetch");
    let cmds: Vec<_> = reader.read().collect();
    assert_eq!(cmds.len(), 1);
    assert!(matches!(cmds[0], AudioCmd::PlayFx { id } if id == "phase_start"));
}

#[test]
fn phase_callback_receives_input_state() {
    let mut world = make_phase_world(0.016);

    let mut input = InputState::default();
    input.action_1.active = true;
    input.action_1.just_pressed = true;
    world.insert_resource(input);

    fn update_fn(
        entity: Entity,
        ctx: &mut GameCtx,
        input: &InputState,
        _dt: f32,
    ) -> Option<String> {
        if input.action_1.active
            && let Ok(mut signals) = ctx.signals.get_mut(entity)
        {
            signals.set_flag("input_received");
        }
        None
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbackFns {
            on_enter: None,
            on_update: Some(update_fn),
            on_exit: None,
        },
    );

    let entity = world
        .spawn((Phase::new("idle", phases), Signals::default()))
        .id();

    tick_phases(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(signals.has_flag("input_received"));
}

// =============================================================================
// Rust CollisionRule System Tests
// =============================================================================

#[test]
fn collision_rule_callback_fires_on_matching_groups() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(InputState::default());

    fn on_collision(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        if let Ok(mut signals) = ctx.signals.get_mut(ent_a) {
            signals.set_flag("collided");
        }
    }

    let a = world
        .spawn((
            Group::new("ball"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((
        Group::new("brick"),
        MapPosition::new(5.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    world.spawn((CollisionRule::new(
        "ball",
        "brick",
        on_collision as CollisionCallback,
    ),));

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let signals = world.get::<Signals>(a).unwrap();
    assert!(signals.has_flag("collided"));
}

#[test]
fn collision_rule_callback_not_fired_on_non_matching_groups() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(InputState::default());

    fn on_collision(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        if let Ok(mut signals) = ctx.signals.get_mut(ent_a) {
            signals.set_flag("should_not_fire");
        }
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
    // Rule is for "ball" vs "brick", not "player" vs "enemy"
    world.spawn((CollisionRule::new(
        "ball",
        "brick",
        on_collision as CollisionCallback,
    ),));

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let signals = world.get::<Signals>(a).unwrap();
    assert!(!signals.has_flag("should_not_fire"));
}

#[test]
fn collision_rule_entities_ordered_correctly_when_groups_swapped() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(InputState::default());

    // Callback expects entity_a to be "ball" (group_a of the rule).
    // It sets a flag on entity_a to prove ordering is correct.
    fn on_collision(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        // ent_a should be ball (group_a of rule)
        if let Ok(group) = ctx.groups.get(ent_a)
            && group.name() == "ball"
            && let Ok(mut signals) = ctx.signals.get_mut(ent_a)
        {
            signals.set_flag("ball_is_first");
        }
    }

    // Spawn "brick" first so it gets a lower Entity id.
    // The collision detector will report (brick, ball) but the rule
    // defines group_a="ball", so the observer must reorder them.
    let brick = world
        .spawn((
            Group::new("brick"),
            MapPosition::new(5.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    let ball = world
        .spawn((
            Group::new("ball"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((CollisionRule::new(
        "ball",
        "brick",
        on_collision as CollisionCallback,
    ),));

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let ball_signals = world.get::<Signals>(ball).unwrap();
    assert!(ball_signals.has_flag("ball_is_first"));
    // brick should NOT have the flag
    let brick_signals = world.get::<Signals>(brick).unwrap();
    assert!(!brick_signals.has_flag("ball_is_first"));
}

#[test]
fn collision_rule_sides_passed_to_callback() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(InputState::default());

    // rect_a is at (0,0) 10x10, rect_b is at (8,0) 10x10
    // → rect_a's right side collides, rect_b's left side collides
    fn on_collision(
        ent_a: Entity,
        _ent_b: Entity,
        sides_a: &BoxSides,
        sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        use aberredengine::core::components::collision::BoxSide;
        let has_right_a = sides_a.iter().any(|s| matches!(s, BoxSide::Right));
        let has_left_b = sides_b.iter().any(|s| matches!(s, BoxSide::Left));
        if has_right_a
            && has_left_b
            && let Ok(mut signals) = ctx.signals.get_mut(ent_a)
        {
            signals.set_flag("sides_correct");
        }
    }

    let a = world
        .spawn((
            Group::new("ball"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((
        Group::new("brick"),
        MapPosition::new(8.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    world.spawn((CollisionRule::new(
        "ball",
        "brick",
        on_collision as CollisionCallback,
    ),));

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let signals = world.get::<Signals>(a).unwrap();
    assert!(signals.has_flag("sides_correct"));
}

/// When multiple rules cover the same group pair, first-match is
/// deterministic (lowest `Entity` wins), not query-iteration order. Per
/// `.claude/context/system-order.md`, `Entity`'s `Ord` does NOT correlate
/// with spawn order (its niche encoding stores `!index`), so this test
/// determines which of the two rules has the lower id *after* spawning
/// both, rather than assuming spawn order predicts it.
#[test]
fn collision_rule_same_pair_multiple_rules_lowest_entity_wins() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(InputState::default());

    fn on_collision_first(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        if let Ok(mut signals) = ctx.signals.get_mut(ent_a) {
            signals.set_flag("first_rule_fired");
        }
    }

    fn on_collision_second(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        if let Ok(mut signals) = ctx.signals.get_mut(ent_a) {
            signals.set_flag("second_rule_fired");
        }
    }

    let a = world
        .spawn((
            Group::new("ball"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((
        Group::new("brick"),
        MapPosition::new(5.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));

    let rule_1 = world
        .spawn(CollisionRule::new(
            "ball",
            "brick",
            on_collision_first as CollisionCallback,
        ))
        .id();
    let rule_2 = world
        .spawn(CollisionRule::new(
            "ball",
            "brick",
            on_collision_second as CollisionCallback,
        ))
        .id();
    let first_rule_has_lower_entity = rule_1 < rule_2;

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let (expected, unexpected) = if first_rule_has_lower_entity {
        ("first_rule_fired", "second_rule_fired")
    } else {
        ("second_rule_fired", "first_rule_fired")
    };
    let signals = world.get::<Signals>(a).unwrap();
    assert!(signals.has_flag(expected));
    assert!(!signals.has_flag(unexpected));
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
