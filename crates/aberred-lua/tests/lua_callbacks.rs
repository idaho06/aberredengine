//! Integration tests for Lua callback side effects.
//!
//! Runs a Lua callback through its real dispatch path (collision observer,
//! timer observer, phase system) in an ECS world and checks that the commands
//! the callback queued are drained and applied.

use bevy_ecs::prelude::*;

use aberred_core::components::boxcollider::BoxCollider;
use aberred_core::components::collision::CollisionRule;
use aberred_core::components::group::Group;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::signals::Signals;
use aberred_core::components::ttl::Ttl;
use aberred_core::events::collision::CollisionEvent;
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::collision_rule_index::CollisionRuleIndex;
use aberred_core::resources::input::InputState;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_core::resources::worldtime::WorldTime;
use aberred_core::systems::collision_detector::collision_detector;
use aberred_lua::components::luacollision::LuaCollisionCallback;
use aberred_lua::resources::lua_runtime::LuaRuntime;
use aberred_lua::systems::lua_collision::lua_collision_observer;
use aberred_lua::systems::lua_collision_rule_index::rebuild_collision_rule_index;

/// World with every resource the Lua dispatch paths read, plus a fresh
/// `LuaRuntime`.
fn make_lua_callback_world(delta: f32) -> World {
    let mut world = World::new();
    world.insert_resource(WorldTime {
        delta,
        ..WorldTime::default()
    });
    world.insert_resource(InputState::default());
    world.insert_resource(WorldSignals::default());
    world.init_resource::<Messages<AudioCmd>>();
    world.insert_resource(SystemsStore::new());
    world.insert_resource(AnimationStore::default());
    world.insert_resource(CollisionRuleIndex::default());
    world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
    world
}

/// Lua-aware rule-index rebuild, then collision detection.
fn tick_collision_detector(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_collision_rule_index.before(collision_detector));
    schedule.add_systems(collision_detector);
    schedule.run(world);
}

#[test]
fn collision_pipeline_triggers_lua_side_effects() {
    let mut world = make_lua_callback_world(0.0);

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

#[test]
fn collision_callback_error_still_drains_queued_commands() {
    let mut world = make_lua_callback_world(0.0);

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
