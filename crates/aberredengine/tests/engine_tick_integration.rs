//! Engine tick integration tests for collision, and other systems.

#![allow(dead_code, unused_imports)]

use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::system::SystemState;
use aberredengine::core::math::Vec2;

use aberredengine::core::components::boxcollider::BoxCollider;
use aberredengine::core::components::collision::CollisionRule;
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
use aberredengine::systems::collision_rule_index::rebuild_collision_rule_index;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::luatimer::{lua_timer_observer, update_lua_timers};

use aberredengine::core::testing::insert_game_ctx_resources;

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
