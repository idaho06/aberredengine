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

#[test]
fn meta_table_has_functions_and_classes() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
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
    "#).exec().unwrap();
}

// =============================================================================
// Meta Schema Drift Protection Tests
// =============================================================================

#[test]
fn meta_types_table_is_populated() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
        local types = engine.__meta.types
        assert(types, "__meta.types missing")

        -- Key types must exist
        local required = {"EntityContext", "CollisionContext", "InputSnapshot", "Vector2",
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
        assert(ec_field_names.pos.type == "Vector2", "EntityContext.pos should be Vector2")
        assert(ec_field_names.signals, "EntityContext missing signals field")
        assert(ec_field_names.previous_phase, "EntityContext missing previous_phase")

        -- Spot-check Vector2
        local v2 = types.Vector2
        assert(#v2.fields == 2, "Vector2 should have 2 fields")
    "#).exec().unwrap();
}

#[test]
fn meta_enums_table_is_populated() {
    let rt = LuaRuntime::new().unwrap();
    let lua = rt.lua();
    lua.load(r#"
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
                               "entity", "group", "tilemap", "camera", "collision",
                               "animation", "render"}
        assert(#enums.Category.values == #expected_cats,
            "Category value count mismatch: expected " .. #expected_cats ..
            " got " .. #enums.Category.values)
    "#).exec().unwrap();
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
    lua.load(r#"
        local fns = engine.__meta.functions

        -- Expected function names (comprehensive list)
        local expected = {
            -- base
            "log", "log_info", "log_warn", "log_error",
            -- asset
            "load_texture", "load_font", "load_music", "load_sound", "load_tilemap",
            -- spawn
            "spawn", "clone",
            -- audio
            "play_music", "play_sound", "stop_all_music", "stop_all_sounds",
            -- signal reads
            "get_scalar", "get_integer", "get_string", "has_flag",
            "get_group_count", "get_entity",
            -- signal writes
            "set_scalar", "set_integer", "set_string", "set_flag", "clear_flag",
            "clear_scalar", "clear_integer", "clear_string",
            "set_entity", "remove_entity",
            -- phase
            "phase_transition",
            -- group
            "track_group", "untrack_group", "clear_tracked_groups", "has_tracked_group",
            -- tilemap
            "spawn_tiles",
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
            "collision_set_flag", "collision_clear_flag",
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
            "entity_signal_set_flag", "entity_signal_clear_flag",
            "entity_insert_lua_timer", "entity_remove_lua_timer",
            "entity_insert_ttl", "entity_set_rotation", "entity_set_scale",
            "entity_set_speed", "entity_set_friction", "entity_set_max_speed",
            "entity_insert_tween_position", "entity_insert_tween_rotation",
            "entity_insert_tween_scale", "entity_remove_tween_position",
            "entity_remove_tween_rotation", "entity_remove_tween_scale",
            "entity_signal_set_scalar", "entity_signal_set_string",
            "entity_signal_set_integer", "entity_add_force", "entity_remove_force",
            "entity_set_force_enabled", "entity_set_force_value",
            "release_stuckto", "entity_insert_stuckto",
            "entity_restart_animation", "entity_set_animation",
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
    "#).exec().unwrap();
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
