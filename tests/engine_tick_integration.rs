//! Engine tick integration tests for movement, TTL, collision, and other systems.

#![allow(dead_code, unused_imports)]

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use aberredengine::components::animation::{Animation, AnimationController, Condition};
use aberredengine::components::boxcollider::BoxCollider;
use aberredengine::components::group::Group;
use aberredengine::components::luacollision::LuaCollisionRule;
use aberredengine::components::luatimer::LuaTimer;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::rigidbody::RigidBody;
use aberredengine::components::rotation::Rotation;
use aberredengine::components::scale::Scale;
use aberredengine::components::signals::Signals;
use aberredengine::components::stuckto::StuckTo;
use aberredengine::components::ttl::Ttl;
use aberredengine::components::tween::{
    Easing, LoopMode, TweenPosition, TweenRotation, TweenScale,
};
use aberredengine::events::audio::AudioCmd;
use aberredengine::events::collision::CollisionEvent;
use aberredengine::events::luatimer::LuaTimerEvent;
use aberredengine::resources::group::TrackedGroups;
use aberredengine::resources::lua_runtime::LuaRuntime;
use aberredengine::resources::screensize::ScreenSize;
use aberredengine::resources::systemsstore::SystemsStore;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::resources::worldtime::WorldTime;
use aberredengine::systems::animation::animation_controller;
use aberredengine::systems::collision::{collision_detector, collision_observer};
use aberredengine::systems::group::update_group_counts_system;
use aberredengine::systems::luatimer::update_lua_timers;
use aberredengine::systems::movement::movement;
use aberredengine::systems::stuckto::stuck_to_entity_system;
use aberredengine::systems::time::update_world_time;
use aberredengine::systems::ttl::ttl_system;
use aberredengine::systems::tween::{
    tween_mapposition_system, tween_rotation_system, tween_scale_system,
};

const EPSILON: f32 = 1e-6;

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < EPSILON
}

fn make_world(delta: f32) -> World {
    let mut world = World::new();
    world.insert_resource(WorldTime {
        elapsed: 0.0,
        delta,
        time_scale: 1.0,
        frame_count: 0,
    });
    world.insert_resource(ScreenSize { w: 800, h: 600 });
    world.init_resource::<Messages<AudioCmd>>();
    world
}

fn tick_movement(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(movement);
    schedule.run(world);
}

fn tick_ttl(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(ttl_system);
    schedule.run(world);
}

fn tick_collision_detector(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(collision_detector);
    schedule.run(world);
}

#[test]
fn movement_integrates_velocity_into_position() {
    let mut world = make_world(0.0);
    let mut rb = RigidBody::new();
    rb.velocity = Vector2 { x: 10.0, y: 0.0 };

    let entity = world.spawn((MapPosition::new(0.0, 0.0), rb)).id();

    update_world_time(&mut world, 0.5);
    tick_movement(&mut world);

    let pos = world.get::<MapPosition>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 5.0));
    assert!(approx_eq(pos.pos.y, 0.0));
}

#[test]
fn movement_applies_acceleration_forces() {
    let mut world = make_world(0.0);
    let mut rb = RigidBody::new();
    rb.add_force("thrust", Vector2 { x: 2.0, y: 0.0 });

    let entity = world.spawn((MapPosition::new(0.0, 0.0), rb)).id();

    update_world_time(&mut world, 1.0);
    tick_movement(&mut world);

    let rb = world.get::<RigidBody>(entity).unwrap();
    let pos = world.get::<MapPosition>(entity).unwrap();
    assert!(approx_eq(rb.velocity.x, 2.0));
    assert!(approx_eq(rb.velocity.y, 0.0));
    assert!(approx_eq(pos.pos.x, 2.0));
    assert!(approx_eq(pos.pos.y, 0.0));
}

#[test]
fn movement_sets_signals_moving_and_speed_sq() {
    let mut world = make_world(0.0);
    let mut rb = RigidBody::new();
    rb.velocity = Vector2 { x: 3.0, y: 4.0 };

    let entity = world
        .spawn((MapPosition::new(0.0, 0.0), rb, Signals::default()))
        .id();

    update_world_time(&mut world, 1.0);
    tick_movement(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(signals.has_flag("moving"));
    assert!(approx_eq(signals.get_scalar("speed_sq").unwrap(), 25.0));
}

#[test]
fn movement_skips_frozen_but_clears_signals() {
    let mut world = make_world(0.0);
    let mut rb = RigidBody::new();
    rb.velocity = Vector2 { x: 5.0, y: 0.0 };
    rb.freeze();

    let mut signals = Signals::default().with_flag("moving");
    signals.set_scalar("speed_sq", 123.0);

    let entity = world.spawn((MapPosition::new(1.0, 1.0), rb, signals)).id();

    update_world_time(&mut world, 1.0);
    tick_movement(&mut world);

    let pos = world.get::<MapPosition>(entity).unwrap();
    let signals = world.get::<Signals>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 1.0));
    assert!(approx_eq(pos.pos.y, 1.0));
    assert!(!signals.has_flag("moving"));
    assert!(approx_eq(signals.get_scalar("speed_sq").unwrap(), 0.0));
}

#[test]
fn ttl_decrements_and_despawns() {
    let mut world = make_world(0.5);
    let entity = world.spawn((Ttl::new(1.0),)).id();

    tick_ttl(&mut world);

    assert!(world.get_entity(entity).is_ok());
    let ttl = world.get::<Ttl>(entity).unwrap();
    assert!(approx_eq(ttl.remaining, 0.5));

    tick_ttl(&mut world);

    assert!(world.get_entity(entity).is_err());
}

#[test]
fn ttl_does_not_despawn_before_zero() {
    let mut world = make_world(0.25);
    let entity = world.spawn((Ttl::new(0.3),)).id();

    tick_ttl(&mut world);

    assert!(world.get_entity(entity).is_ok());
    let ttl = world.get::<Ttl>(entity).unwrap();
    assert!(ttl.remaining > 0.0);
}

#[test]
fn collision_pipeline_triggers_lua_side_effects() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(SystemsStore::new());
    world.init_resource::<Messages<AudioCmd>>();

    let lua_runtime = LuaRuntime::new().expect("Failed to init Lua runtime");
    world.insert_non_send_resource(lua_runtime);

    {
        let lua_runtime = world.non_send_resource::<LuaRuntime>();
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
    world.spawn((LuaCollisionRule::new("player", "enemy", "on_player_enemy"),));

    // Track if collision event was triggered
    let saw_collision = std::sync::Arc::new(std::sync::Mutex::new(false));
    let saw_collision_clone = saw_collision.clone();

    // Register the test observer to track collision events
    world.add_observer(move |_trigger: On<CollisionEvent>| {
        *saw_collision_clone.lock().unwrap() = true;
    });

    // Register the actual collision_observer that processes Lua callbacks
    world.add_observer(collision_observer);

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

// =============================================================================
// StuckTo System Tests
// =============================================================================

fn tick_stuckto(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(stuck_to_entity_system);
    schedule.run(world);
}

#[test]
fn stuckto_follows_target_both_axes() {
    let mut world = make_world(0.0);

    let target = world.spawn((MapPosition::new(100.0, 50.0),)).id();
    let follower = world
        .spawn((MapPosition::new(0.0, 0.0), StuckTo::new(target)))
        .id();

    tick_stuckto(&mut world);

    let pos = world.get::<MapPosition>(follower).unwrap();
    assert!(approx_eq(pos.pos.x, 100.0));
    assert!(approx_eq(pos.pos.y, 50.0));
}

#[test]
fn stuckto_follows_target_x_only() {
    let mut world = make_world(0.0);

    let target = world.spawn((MapPosition::new(100.0, 50.0),)).id();
    let follower = world
        .spawn((MapPosition::new(0.0, 25.0), StuckTo::follow_x_only(target)))
        .id();

    tick_stuckto(&mut world);

    let pos = world.get::<MapPosition>(follower).unwrap();
    assert!(approx_eq(pos.pos.x, 100.0));
    assert!(approx_eq(pos.pos.y, 25.0)); // Y unchanged
}

#[test]
fn stuckto_follows_target_y_only() {
    let mut world = make_world(0.0);

    let target = world.spawn((MapPosition::new(100.0, 50.0),)).id();
    let follower = world
        .spawn((MapPosition::new(30.0, 0.0), StuckTo::follow_y_only(target)))
        .id();

    tick_stuckto(&mut world);

    let pos = world.get::<MapPosition>(follower).unwrap();
    assert!(approx_eq(pos.pos.x, 30.0)); // X unchanged
    assert!(approx_eq(pos.pos.y, 50.0));
}

#[test]
fn stuckto_applies_offset() {
    let mut world = make_world(0.0);

    let target = world.spawn((MapPosition::new(100.0, 100.0),)).id();
    let follower = world
        .spawn((
            MapPosition::new(0.0, 0.0),
            StuckTo::new(target).with_offset(Vector2 { x: 10.0, y: -20.0 }),
        ))
        .id();

    tick_stuckto(&mut world);

    let pos = world.get::<MapPosition>(follower).unwrap();
    assert!(approx_eq(pos.pos.x, 110.0));
    assert!(approx_eq(pos.pos.y, 80.0));
}

#[test]
fn stuckto_does_not_move_if_target_missing() {
    let mut world = make_world(0.0);

    // Create a fake entity ID that doesn't exist
    let fake_target = Entity::from_bits(99999);
    let follower = world
        .spawn((MapPosition::new(50.0, 50.0), StuckTo::new(fake_target)))
        .id();

    tick_stuckto(&mut world);

    let pos = world.get::<MapPosition>(follower).unwrap();
    assert!(approx_eq(pos.pos.x, 50.0)); // Unchanged
    assert!(approx_eq(pos.pos.y, 50.0));
}

// =============================================================================
// Group Counting System Tests
// =============================================================================

fn tick_group_counts(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(update_group_counts_system);
    schedule.run(world);
}

#[test]
fn group_counts_are_published_to_world_signals() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());

    let mut tracked = TrackedGroups::default();
    tracked.add_group("enemy");
    world.insert_resource(tracked);

    world.spawn((Group::new("enemy"),));
    world.spawn((Group::new("enemy"),));
    world.spawn((Group::new("enemy"),));

    tick_group_counts(&mut world);

    let signals = world.resource::<WorldSignals>();
    assert_eq!(signals.get_group_count("enemy"), Some(3));
}

#[test]
fn group_counts_update_when_entities_despawn() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());

    let mut tracked = TrackedGroups::default();
    tracked.add_group("ball");
    world.insert_resource(tracked);

    let ball1 = world.spawn((Group::new("ball"),)).id();
    let ball2 = world.spawn((Group::new("ball"),)).id();

    tick_group_counts(&mut world);

    let signals = world.resource::<WorldSignals>();
    assert_eq!(signals.get_group_count("ball"), Some(2));

    // Despawn one ball
    world.despawn(ball1);

    tick_group_counts(&mut world);

    let signals = world.resource::<WorldSignals>();
    assert_eq!(signals.get_group_count("ball"), Some(1));

    // Despawn the other
    world.despawn(ball2);

    tick_group_counts(&mut world);

    let signals = world.resource::<WorldSignals>();
    assert_eq!(signals.get_group_count("ball"), Some(0));
}

#[test]
fn group_counts_zero_for_empty_tracked_groups() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());

    let mut tracked = TrackedGroups::default();
    tracked.add_group("brick");
    world.insert_resource(tracked);

    // No bricks spawned

    tick_group_counts(&mut world);

    let signals = world.resource::<WorldSignals>();
    assert_eq!(signals.get_group_count("brick"), Some(0));
}

#[test]
fn group_counts_ignores_untracked_groups() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());

    let mut tracked = TrackedGroups::default();
    tracked.add_group("player");
    world.insert_resource(tracked);

    world.spawn((Group::new("player"),));
    world.spawn((Group::new("enemy"),)); // Not tracked
    world.spawn((Group::new("bullet"),)); // Not tracked

    tick_group_counts(&mut world);

    let signals = world.resource::<WorldSignals>();
    assert_eq!(signals.get_group_count("player"), Some(1));
    assert_eq!(signals.get_group_count("enemy"), None); // Not tracked
    assert_eq!(signals.get_group_count("bullet"), None); // Not tracked
}

// =============================================================================
// Animation Controller System Tests
// =============================================================================

fn tick_animation_controller(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(animation_controller);
    schedule.run(world);
}

#[test]
fn animation_controller_switches_on_flag() {
    let mut world = make_world(0.0);

    let controller = AnimationController::new("idle").with_rule(
        Condition::HasFlag {
            key: "moving".to_string(),
        },
        "walk",
    );

    let entity = world
        .spawn((
            Animation::new("idle"),
            controller,
            Signals::default().with_flag("moving"),
        ))
        .id();

    tick_animation_controller(&mut world);

    let anim = world.get::<Animation>(entity).unwrap();
    assert_eq!(anim.animation_key, "walk");
}

#[test]
fn animation_controller_uses_fallback_when_no_match() {
    let mut world = make_world(0.0);

    let controller = AnimationController::new("idle").with_rule(
        Condition::HasFlag {
            key: "running".to_string(),
        },
        "run",
    );

    let entity = world
        .spawn((
            Animation::new("idle"),
            controller,
            Signals::default(), // No "running" flag
        ))
        .id();

    tick_animation_controller(&mut world);

    let anim = world.get::<Animation>(entity).unwrap();
    assert_eq!(anim.animation_key, "idle"); // Fallback
}

#[test]
fn animation_controller_resets_animation_on_switch() {
    let mut world = make_world(0.0);

    let controller = AnimationController::new("idle").with_rule(
        Condition::HasFlag {
            key: "attack".to_string(),
        },
        "attack",
    );

    // Start with animation already advanced
    let mut anim = Animation::new("idle");
    anim.frame_index = 5;
    anim.elapsed_time = 0.5;

    let entity = world
        .spawn((anim, controller, Signals::default().with_flag("attack")))
        .id();

    tick_animation_controller(&mut world);

    let anim = world.get::<Animation>(entity).unwrap();
    assert_eq!(anim.animation_key, "attack");
    assert_eq!(anim.frame_index, 0); // Reset
    assert!(approx_eq(anim.elapsed_time, 0.0)); // Reset
}

#[test]
fn animation_controller_first_matching_rule_wins() {
    let mut world = make_world(0.0);

    let controller = AnimationController::new("idle")
        .with_rule(
            Condition::HasFlag {
                key: "dead".to_string(),
            },
            "death",
        )
        .with_rule(
            Condition::HasFlag {
                key: "moving".to_string(),
            },
            "walk",
        );

    // Both flags set, but "dead" rule comes first
    let mut signals = Signals::default();
    signals.set_flag("dead");
    signals.set_flag("moving");

    let entity = world
        .spawn((Animation::new("idle"), controller, signals))
        .id();

    tick_animation_controller(&mut world);

    let anim = world.get::<Animation>(entity).unwrap();
    assert_eq!(anim.animation_key, "death"); // First match wins
}

// =============================================================================
// Tween System Tests
// =============================================================================

fn tick_tween_position(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(tween_mapposition_system);
    schedule.run(world);
}

fn tick_tween_rotation(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(tween_rotation_system);
    schedule.run(world);
}

fn tick_tween_scale(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(tween_scale_system);
    schedule.run(world);
}

#[test]
fn tween_position_interpolates_linearly() {
    let mut world = make_world(0.5); // 0.5 second delta

    let tween = TweenPosition::new(
        Vector2 { x: 0.0, y: 0.0 },
        Vector2 { x: 100.0, y: 200.0 },
        1.0, // 1 second duration
    );

    let entity = world.spawn((MapPosition::new(0.0, 0.0), tween)).id();

    tick_tween_position(&mut world);

    let pos = world.get::<MapPosition>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 50.0)); // Halfway
    assert!(approx_eq(pos.pos.y, 100.0));
}

#[test]
fn tween_position_stops_at_end_with_once_mode() {
    let mut world = make_world(1.0);

    let tween = TweenPosition::new(
        Vector2 { x: 0.0, y: 0.0 },
        Vector2 { x: 100.0, y: 0.0 },
        0.5, // Half second duration
    )
    .with_loop_mode(LoopMode::Once);

    let entity = world.spawn((MapPosition::new(0.0, 0.0), tween)).id();

    tick_tween_position(&mut world);

    let pos = world.get::<MapPosition>(entity).unwrap();
    let tween = world.get::<TweenPosition>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 100.0)); // At end
    assert!(!tween.playing); // Stopped
}

#[test]
fn tween_position_loops_with_loop_mode() {
    let mut world = make_world(0.6);

    let tween = TweenPosition::new(
        Vector2 { x: 0.0, y: 0.0 },
        Vector2 { x: 100.0, y: 0.0 },
        0.5,
    )
    .with_loop_mode(LoopMode::Loop);

    let entity = world.spawn((MapPosition::new(0.0, 0.0), tween)).id();

    tick_tween_position(&mut world);

    let tween = world.get::<TweenPosition>(entity).unwrap();
    assert!(tween.playing); // Still playing
    assert!(tween.time < 0.5); // Wrapped around
}

#[test]
fn tween_position_pingpong_reverses() {
    let mut world = make_world(0.6);

    let tween = TweenPosition::new(
        Vector2 { x: 0.0, y: 0.0 },
        Vector2 { x: 100.0, y: 0.0 },
        0.5,
    )
    .with_loop_mode(LoopMode::PingPong);

    let entity = world.spawn((MapPosition::new(0.0, 0.0), tween)).id();

    tick_tween_position(&mut world);

    let tween = world.get::<TweenPosition>(entity).unwrap();
    assert!(tween.playing);
    assert!(!tween.forward); // Direction reversed
}

#[test]
fn tween_rotation_interpolates() {
    let mut world = make_world(0.5);

    let tween = TweenRotation::new(0.0, 180.0, 1.0);

    let entity = world.spawn((Rotation { degrees: 0.0 }, tween)).id();

    tick_tween_rotation(&mut world);

    let rot = world.get::<Rotation>(entity).unwrap();
    assert!(approx_eq(rot.degrees, 90.0)); // Halfway
}

#[test]
fn tween_scale_interpolates() {
    let mut world = make_world(0.5);

    let tween = TweenScale::new(Vector2 { x: 1.0, y: 1.0 }, Vector2 { x: 2.0, y: 3.0 }, 1.0);

    let entity = world.spawn((Scale::new(1.0, 1.0), tween)).id();

    tick_tween_scale(&mut world);

    let scale = world.get::<Scale>(entity).unwrap();
    assert!(approx_eq(scale.scale.x, 1.5)); // Halfway
    assert!(approx_eq(scale.scale.y, 2.0));
}

#[test]
fn tween_position_with_quad_in_easing() {
    let mut world = make_world(0.5);

    let tween = TweenPosition::new(
        Vector2 { x: 0.0, y: 0.0 },
        Vector2 { x: 100.0, y: 0.0 },
        1.0,
    )
    .with_easing(Easing::QuadIn);

    let entity = world.spawn((MapPosition::new(0.0, 0.0), tween)).id();

    tick_tween_position(&mut world);

    let pos = world.get::<MapPosition>(entity).unwrap();
    // QuadIn at t=0.5 gives 0.5^2 = 0.25
    assert!(approx_eq(pos.pos.x, 25.0));
}

// =============================================================================
// Lua Timer System Tests
// =============================================================================

fn tick_lua_timers(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(update_lua_timers);
    schedule.run(world);
}

#[test]
fn lua_timer_accumulates_time() {
    let mut world = make_world(0.3);

    let entity = world.spawn((LuaTimer::new(1.0, "my_callback"),)).id();

    tick_lua_timers(&mut world);

    let timer = world.get::<LuaTimer>(entity).unwrap();
    assert!(approx_eq(timer.elapsed, 0.3));
}

#[test]
fn lua_timer_fires_event_when_expired() {
    let mut world = make_world(1.0);

    let entity = world.spawn((LuaTimer::new(0.5, "on_timer"),)).id();

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

#[test]
fn lua_timer_resets_after_firing() {
    let mut world = make_world(0.6);

    let entity = world.spawn((LuaTimer::new(0.5, "callback"),)).id();

    // Add dummy observer so events are processed
    world.add_observer(|_trigger: On<LuaTimerEvent>| {});
    world.flush();

    tick_lua_timers(&mut world);

    let timer = world.get::<LuaTimer>(entity).unwrap();
    // Timer should have reset: 0.6 - 0.5 = 0.1
    assert!(approx_eq(timer.elapsed, 0.1));
}

#[test]
fn lua_timer_does_not_fire_before_duration() {
    let mut world = make_world(0.3);

    world.spawn((LuaTimer::new(1.0, "callback"),));

    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_clone = fired.clone();

    world.add_observer(move |_trigger: On<LuaTimerEvent>| {
        *fired_clone.lock().unwrap() = true;
    });
    world.flush();

    tick_lua_timers(&mut world);

    assert!(!*fired.lock().unwrap());
}

// =============================================================================
// Time Scaling Tests
// =============================================================================

#[test]
fn time_scale_zero_freezes_movement() {
    let mut world = World::new();
    let worldtime = WorldTime::default().with_time_scale(0.0);

    world.insert_resource(worldtime);
    world.insert_resource(ScreenSize { w: 800, h: 600 });
    world.init_resource::<Messages<AudioCmd>>();

    let mut rb = RigidBody::new();
    rb.velocity = Vector2 { x: 100.0, y: 0.0 };

    let entity = world.spawn((MapPosition::new(0.0, 0.0), rb)).id();

    update_world_time(&mut world, 1.0); // Update WorldTime with delta=1.0 should apply time_scale
    tick_movement(&mut world);

    let pos = world.get::<MapPosition>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 0.0)); // No movement
    assert!(approx_eq(pos.pos.y, 0.0));
}

#[test]
fn time_scale_doubles_effective_movement() {
    let mut world = World::new();
    // Simulating time_scale=2.0 with base delta of 0.5 => effective delta = 1.0

    let worldtime = WorldTime::default().with_time_scale(2.0);

    world.insert_resource(worldtime);
    world.insert_resource(ScreenSize { w: 800, h: 600 });
    world.init_resource::<Messages<AudioCmd>>();

    let mut rb = RigidBody::new();
    rb.velocity = Vector2 { x: 10.0, y: 0.0 };

    let entity = world.spawn((MapPosition::new(0.0, 0.0), rb)).id();

    update_world_time(&mut world, 0.5); // Update WorldTime with delta=0.5 should apply time_scale
    tick_movement(&mut world);

    let pos = world.get::<MapPosition>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 10.0)); // vel * delta = 10 * 1.0
}
