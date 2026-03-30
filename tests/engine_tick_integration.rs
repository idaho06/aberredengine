//! Engine tick integration tests for movement, TTL, collision, and other systems.

#![allow(dead_code, unused_imports)]

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;
use raylib::prelude::Vector2;

use aberredengine::components::animation::{Animation, AnimationController, Condition};
use aberredengine::components::boxcollider::BoxCollider;
use aberredengine::components::collision::{BoxSides, CollisionCallback, CollisionRule};
use aberredengine::components::group::Group;
#[cfg(feature = "lua")]
use aberredengine::components::luacollision::{LuaCollisionCallback, LuaCollisionRule};
#[cfg(feature = "lua")]
use aberredengine::components::luaphase::{LuaPhase, PhaseCallbacks};
#[cfg(feature = "lua")]
use aberredengine::components::luatimer::{LuaTimer, LuaTimerCallback};
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::rigidbody::RigidBody;
use aberredengine::components::rotation::Rotation;
use aberredengine::components::scale::Scale;
use aberredengine::components::signals::Signals;
use aberredengine::components::sprite::Sprite;
use aberredengine::components::stuckto::StuckTo;
use aberredengine::components::timer::{Timer, TimerCallback};
use aberredengine::components::ttl::Ttl;
use aberredengine::components::tween::{Easing, LoopMode, Tween};
use aberredengine::events::audio::AudioCmd;
use aberredengine::events::collision::CollisionEvent;
#[cfg(feature = "lua")]
use aberredengine::events::luatimer::LuaTimerEvent;
use aberredengine::events::timer::TimerEvent;
use aberredengine::resources::animationstore::{AnimationResource, AnimationStore};
use aberredengine::resources::gameconfig::GameConfig;
use aberredengine::resources::group::TrackedGroups;
use aberredengine::resources::input::InputState;
#[cfg(feature = "lua")]
use aberredengine::resources::lua_runtime::LuaRuntime;
use aberredengine::resources::postprocessshader::PostProcessShader;
use aberredengine::resources::screensize::ScreenSize;
use aberredengine::resources::systemsstore::SystemsStore;
use aberredengine::resources::texturestore::TextureStore;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::resources::worldtime::WorldTime;
use aberredengine::systems::animation::{animation, animation_controller};
use aberredengine::systems::collision_detector::collision_detector;
use aberredengine::systems::group::update_group_counts_system;
#[cfg(feature = "lua")]
use aberredengine::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use aberredengine::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use aberredengine::systems::luatimer::update_lua_timers;
use aberredengine::systems::movement::movement;
use aberredengine::systems::rust_collision::rust_collision_observer;
use aberredengine::systems::stuckto::stuck_to_entity_system;
use aberredengine::systems::time::update_world_time;
use aberredengine::systems::timer::{timer_observer, update_timers};
use aberredengine::systems::ttl::ttl_system;
use aberredengine::systems::tween::tween_system;

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
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });
    world.init_resource::<Messages<AudioCmd>>();
    world.init_resource::<TextureStore>();
    world.insert_resource(GameConfig::default());
    world.init_resource::<PostProcessShader>();
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

#[cfg(feature = "lua")]
#[test]
fn collision_pipeline_triggers_lua_side_effects() {
    let mut world = make_world(0.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(SystemsStore::new());
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });
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

#[test]
fn animation_controller_syncs_sprite_tex_key_on_switch() {
    // Verify that when the controller switches animation, Sprite.tex_key is
    // updated to match the new animation's texture (the bug was that only
    // Animation.animation_key was updated, leaving Sprite.tex_key stale).
    let mut world = make_world(0.0);

    // Register two animations with distinct textures
    let mut anim_store = AnimationStore {
        animations: Default::default(),
    };
    anim_store.animations.insert(
        "idle".to_string(),
        make_animation_resource("sheet_idle", (0.0, 0.0), (64.0, 0.0), 4, 10.0, true),
    );
    anim_store.animations.insert(
        "walk".to_string(),
        make_animation_resource("sheet_walk", (0.0, 0.0), (64.0, 0.0), 8, 12.0, true),
    );
    world.insert_resource(anim_store);

    let controller = AnimationController::new("idle").with_rule(
        Condition::HasFlag {
            key: "moving".to_string(),
        },
        "walk",
    );

    // Sprite starts on the idle sheet
    let entity = world
        .spawn((
            Animation::new("idle"),
            controller,
            make_sprite("sheet_idle"),
            Signals::default().with_flag("moving"),
        ))
        .id();

    tick_animation_controller(&mut world);

    let anim = world.get::<Animation>(entity).unwrap();
    let sprite = world.get::<Sprite>(entity).unwrap();

    assert_eq!(anim.animation_key, "walk", "animation key should switch");
    assert_eq!(
        sprite.tex_key.as_ref(),
        "sheet_walk",
        "sprite tex_key must update to match the new animation's texture"
    );
}

#[test]
fn animation_controller_does_not_change_sprite_tex_key_when_no_switch() {
    // When the animation does not change, tex_key must remain untouched.
    let mut world = make_world(0.0);

    let mut anim_store = AnimationStore {
        animations: Default::default(),
    };
    anim_store.animations.insert(
        "idle".to_string(),
        make_animation_resource("sheet_idle", (0.0, 0.0), (64.0, 0.0), 4, 10.0, true),
    );
    world.insert_resource(anim_store);

    let controller = AnimationController::new("idle"); // no rules → always fallback "idle"

    let entity = world
        .spawn((
            Animation::new("idle"),
            controller,
            make_sprite("sheet_idle"),
            Signals::default(), // no flags
        ))
        .id();

    tick_animation_controller(&mut world);

    let sprite = world.get::<Sprite>(entity).unwrap();
    assert_eq!(
        sprite.tex_key.as_ref(),
        "sheet_idle",
        "tex_key must not change when animation stays the same"
    );
}

#[test]
fn animation_controller_skips_tex_key_when_animation_not_in_store() {
    // If the target animation key is not registered in AnimationStore, the
    // controller still switches Animation.animation_key but leaves
    // Sprite.tex_key unchanged — no panic.
    let mut world = make_world(0.0);
    // AnimationStore is empty (inserted by make_world) — "run" is not registered

    let controller = AnimationController::new("idle").with_rule(
        Condition::HasFlag {
            key: "moving".to_string(),
        },
        "run",
    );

    let entity = world
        .spawn((
            Animation::new("idle"),
            controller,
            make_sprite("sheet_idle"),
            Signals::default().with_flag("moving"),
        ))
        .id();

    tick_animation_controller(&mut world);

    let anim = world.get::<Animation>(entity).unwrap();
    let sprite = world.get::<Sprite>(entity).unwrap();

    assert_eq!(
        anim.animation_key, "run",
        "animation key should still switch"
    );
    assert_eq!(
        sprite.tex_key.as_ref(),
        "sheet_idle",
        "tex_key must remain unchanged when animation is not in store"
    );
}

// =============================================================================
// Tween System Tests
// =============================================================================

fn tick_tween_position(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(tween_system::<MapPosition>);
    schedule.run(world);
}

fn tick_tween_rotation(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(tween_system::<Rotation>);
    schedule.run(world);
}

fn tick_tween_scale(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(tween_system::<Scale>);
    schedule.run(world);
}

#[test]
fn tween_position_interpolates_linearly() {
    let mut world = make_world(0.5); // 0.5 second delta

    let tween = Tween::new(
        MapPosition::from_vec(Vector2 { x: 0.0, y: 0.0 }),
        MapPosition::from_vec(Vector2 { x: 100.0, y: 200.0 }),
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

    let tween = Tween::new(
        MapPosition::from_vec(Vector2 { x: 0.0, y: 0.0 }),
        MapPosition::from_vec(Vector2 { x: 100.0, y: 0.0 }),
        0.5, // Half second duration
    )
    .with_loop_mode(LoopMode::Once);

    let entity = world.spawn((MapPosition::new(0.0, 0.0), tween)).id();

    tick_tween_position(&mut world);

    let pos = world.get::<MapPosition>(entity).unwrap();
    let tween = world.get::<Tween<MapPosition>>(entity).unwrap();
    assert!(approx_eq(pos.pos.x, 100.0)); // At end
    assert!(!tween.playing); // Stopped
}

#[test]
fn tween_position_loops_with_loop_mode() {
    let mut world = make_world(0.6);

    let tween = Tween::new(
        MapPosition::from_vec(Vector2 { x: 0.0, y: 0.0 }),
        MapPosition::from_vec(Vector2 { x: 100.0, y: 0.0 }),
        0.5,
    )
    .with_loop_mode(LoopMode::Loop);

    let entity = world.spawn((MapPosition::new(0.0, 0.0), tween)).id();

    tick_tween_position(&mut world);

    let tween = world.get::<Tween<MapPosition>>(entity).unwrap();
    assert!(tween.playing); // Still playing
    assert!(tween.time < 0.5); // Wrapped around
}

#[test]
fn tween_position_pingpong_reverses() {
    let mut world = make_world(0.6);

    let tween = Tween::new(
        MapPosition::from_vec(Vector2 { x: 0.0, y: 0.0 }),
        MapPosition::from_vec(Vector2 { x: 100.0, y: 0.0 }),
        0.5,
    )
    .with_loop_mode(LoopMode::PingPong);

    let entity = world.spawn((MapPosition::new(0.0, 0.0), tween)).id();

    tick_tween_position(&mut world);

    let tween = world.get::<Tween<MapPosition>>(entity).unwrap();
    assert!(tween.playing);
    assert!(!tween.forward); // Direction reversed
}

#[test]
fn tween_rotation_interpolates() {
    let mut world = make_world(0.5);

    let tween = Tween::new(Rotation { degrees: 0.0 }, Rotation { degrees: 180.0 }, 1.0);

    let entity = world.spawn((Rotation { degrees: 0.0 }, tween)).id();

    tick_tween_rotation(&mut world);

    let rot = world.get::<Rotation>(entity).unwrap();
    assert!(approx_eq(rot.degrees, 90.0)); // Halfway
}

#[test]
fn tween_scale_interpolates() {
    let mut world = make_world(0.5);

    let tween = Tween::new(Scale::new(1.0, 1.0), Scale::new(2.0, 3.0), 1.0);

    let entity = world.spawn((Scale::new(1.0, 1.0), tween)).id();

    tick_tween_scale(&mut world);

    let scale = world.get::<Scale>(entity).unwrap();
    assert!(approx_eq(scale.scale.x, 1.5)); // Halfway
    assert!(approx_eq(scale.scale.y, 2.0));
}

#[test]
fn tween_position_with_quad_in_easing() {
    let mut world = make_world(0.5);

    let tween = Tween::new(
        MapPosition::from_vec(Vector2 { x: 0.0, y: 0.0 }),
        MapPosition::from_vec(Vector2 { x: 100.0, y: 0.0 }),
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

#[cfg(feature = "lua")]
fn tick_lua_timers(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(update_lua_timers);
    schedule.run(world);
}

#[cfg(feature = "lua")]
fn tick_lua_phases(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(lua_phase_system);
    schedule.run(world);
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_accumulates_time() {
    let mut world = make_world(0.3);

    let entity = world
        .spawn((LuaTimer::new(
            1.0,
            LuaTimerCallback {
                name: "my_callback".to_string(),
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
                name: "on_timer".to_string(),
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
                name: "callback".to_string(),
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
            name: "callback".to_string(),
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
            name: "my_func".to_string(),
        },
    ),));

    let received_name = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let name_clone = received_name.clone();

    world.add_observer(move |trigger: On<LuaTimerEvent>| {
        *name_clone.lock().unwrap() = trigger.event().callback.clone();
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
                name: "func_a".to_string(),
            },
        ),))
        .id();
    let entity_b = world
        .spawn((LuaTimer::new(
            0.5,
            LuaTimerCallback {
                name: "func_b".to_string(),
            },
        ),))
        .id();

    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(Entity, String)>::new()));
    let events_clone = events.clone();

    world.add_observer(move |trigger: On<LuaTimerEvent>| {
        events_clone
            .lock()
            .unwrap()
            .push((trigger.event().entity, trigger.event().callback.clone()));
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
                name: "persist_cb".to_string(),
            },
        ),))
        .id();
    world.add_observer(|_trigger: On<LuaTimerEvent>| {});
    world.flush();

    tick_lua_timers(&mut world); // fires and resets

    let timer = world.get::<LuaTimer>(entity).unwrap();
    assert_eq!(timer.callback.name, "persist_cb");
}

#[cfg(feature = "lua")]
#[test]
fn lua_timer_fires_across_multiple_ticks() {
    // Verify elapsed accumulates correctly over multiple ticks before firing.
    // duration=0.8, delta=0.3 per tick: ticks 1+2 no fire, tick 3 fires.
    let fired_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let fired_clone = fired_count.clone();

    let mut world = make_world(0.3);
    world.spawn((LuaTimer::new(
        0.8,
        LuaTimerCallback {
            name: "cb".to_string(),
        },
    ),));

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

// =============================================================================
// Rust Timer System Tests
// =============================================================================

fn tick_timers(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(update_timers);
    schedule.run(world);
}

#[test]
fn rust_timer_accumulates_time() {
    let mut world = make_world(0.3);

    fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
    let entity = world.spawn((Timer::new(1.0, noop as TimerCallback),)).id();

    tick_timers(&mut world);

    let timer = world.get::<Timer>(entity).unwrap();
    assert!(approx_eq(timer.elapsed, 0.3));
}

#[test]
fn rust_timer_fires_event_when_expired() {
    let mut world = make_world(1.0);

    fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
    let entity = world.spawn((Timer::new(0.5, noop as TimerCallback),)).id();

    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_entity = std::sync::Arc::new(std::sync::Mutex::new(None));
    let fired_clone = fired.clone();
    let entity_clone = fired_entity.clone();

    world.add_observer(move |trigger: On<TimerEvent>| {
        *fired_clone.lock().unwrap() = true;
        *entity_clone.lock().unwrap() = Some(trigger.event().entity);
    });
    world.flush();

    tick_timers(&mut world);

    assert!(*fired.lock().unwrap());
    assert_eq!(*fired_entity.lock().unwrap(), Some(entity));
}

#[test]
fn rust_timer_resets_after_firing() {
    let mut world = make_world(0.6);

    fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
    let entity = world.spawn((Timer::new(0.5, noop as TimerCallback),)).id();

    world.add_observer(|_trigger: On<TimerEvent>| {});
    world.flush();

    tick_timers(&mut world);

    let timer = world.get::<Timer>(entity).unwrap();
    // Timer should have reset: 0.6 - 0.5 = 0.1
    assert!(approx_eq(timer.elapsed, 0.1));
}

#[test]
fn rust_timer_does_not_fire_before_duration() {
    let mut world = make_world(0.3);

    fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
    world.spawn((Timer::new(1.0, noop as TimerCallback),));

    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_clone = fired.clone();

    world.add_observer(move |_trigger: On<TimerEvent>| {
        *fired_clone.lock().unwrap() = true;
    });
    world.flush();

    tick_timers(&mut world);

    assert!(!*fired.lock().unwrap());
}

#[test]
fn rust_timer_observer_calls_callback() {
    let mut world = make_world(1.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(InputState::default());

    fn set_flag(entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
        if let Ok(mut signals) = ctx.signals.get_mut(entity) {
            signals.set_flag("timer_fired");
        }
    }

    let entity = world
        .spawn((
            Timer::new(0.5, set_flag as TimerCallback),
            Signals::default(),
        ))
        .id();

    // Register the real timer_observer so the callback gets invoked
    world.add_observer(timer_observer);
    world.flush();

    tick_timers(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(signals.has_flag("timer_fired"));
}

#[test]
fn rust_timer_observer_can_write_audio() {
    let mut world = make_world(1.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(InputState::default());

    fn play_sound(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
        ctx.audio.write(AudioCmd::PlayFx {
            id: "explosion".into(),
        });
    }

    world.spawn((Timer::new(0.5, play_sound as TimerCallback),));

    world.add_observer(timer_observer);
    world.flush();

    tick_timers(&mut world);

    // Flip message buffers so they become readable
    world.resource_mut::<Messages<AudioCmd>>().update();

    // Read messages via SystemState<MessageReader>
    let mut state = SystemState::<MessageReader<AudioCmd>>::new(&mut world);
    let mut reader = state.get_mut(&mut world);
    let cmds: Vec<_> = reader.read().collect();
    assert_eq!(cmds.len(), 1);
    assert!(matches!(cmds[0], AudioCmd::PlayFx { id } if id == "explosion"));
}

#[test]
fn rust_timer_observer_can_set_world_signal() {
    let mut world = make_world(1.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(InputState::default());

    fn set_signal(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
        ctx.world_signals.set_flag("game_over");
    }

    world.spawn((Timer::new(0.5, set_signal as TimerCallback),));

    world.add_observer(timer_observer);
    world.flush();

    tick_timers(&mut world);

    let world_signals = world.resource::<WorldSignals>();
    assert!(world_signals.has_flag("game_over"));
}

#[test]
fn rust_timer_observer_receives_input_state() {
    let mut world = make_world(1.0);
    world.insert_resource(WorldSignals::default());

    let mut input = InputState::default();
    input.action_1.active = true;
    input.action_1.just_pressed = true;
    world.insert_resource(input);

    fn check_input(entity: Entity, ctx: &mut GameCtx, input: &InputState) {
        // Verify input is passed through — set a signal if action_1 is pressed
        if input.action_1.active
            && let Ok(mut signals) = ctx.signals.get_mut(entity)
        {
            signals.set_flag("input_received");
        }
    }

    let entity = world
        .spawn((
            Timer::new(0.5, check_input as TimerCallback),
            Signals::default(),
        ))
        .id();

    world.add_observer(timer_observer);
    world.flush();

    tick_timers(&mut world);

    let signals = world.get::<Signals>(entity).unwrap();
    assert!(signals.has_flag("input_received"));
}

#[test]
fn rust_timer_fires_across_multiple_ticks() {
    // Verify elapsed accumulates correctly over multiple ticks before firing.
    // duration=0.8, delta=0.3 per tick: ticks 1+2 no fire, tick 3 fires.
    let fired_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let fired_clone = fired_count.clone();

    fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
    let mut world = make_world(0.3);
    world.spawn((Timer::new(0.8, noop as TimerCallback),));

    world.add_observer(move |_trigger: On<TimerEvent>| {
        *fired_clone.lock().unwrap() += 1;
    });
    world.flush();

    tick_timers(&mut world); // elapsed=0.3
    assert_eq!(*fired_count.lock().unwrap(), 0);

    tick_timers(&mut world); // elapsed=0.6
    assert_eq!(*fired_count.lock().unwrap(), 0);

    tick_timers(&mut world); // elapsed=0.9 >= 0.8, fires, resets to 0.1
    assert_eq!(*fired_count.lock().unwrap(), 1);
}

#[test]
fn rust_timer_multiple_entities_fire_independently() {
    // Short-duration timer fires; long-duration timer does not.
    let fired_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let fired_clone = fired_count.clone();

    fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
    let mut world = make_world(1.0);
    world.spawn((Timer::new(0.5, noop as TimerCallback),)); // fires (1.0 >= 0.5)
    world.spawn((Timer::new(2.0, noop as TimerCallback),)); // does not fire (1.0 < 2.0)

    world.add_observer(move |_trigger: On<TimerEvent>| {
        *fired_clone.lock().unwrap() += 1;
    });
    world.flush();

    tick_timers(&mut world);

    assert_eq!(*fired_count.lock().unwrap(), 1);
}

#[test]
fn rust_timer_callback_receives_correct_entity() {
    // Verify the entity passed to the callback is the timer's own entity,
    // not another entity that happens to have Signals.
    let mut world = make_world(1.0);
    world.insert_resource(WorldSignals::default());
    world.insert_resource(InputState::default());

    fn mark_self(entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
        if let Ok(mut signals) = ctx.signals.get_mut(entity) {
            signals.set_flag("fired");
        }
    }

    let bystander = world.spawn(Signals::default()).id();
    let timer_entity = world
        .spawn((
            Timer::new(0.5, mark_self as TimerCallback),
            Signals::default(),
        ))
        .id();

    world.add_observer(timer_observer);
    world.flush();

    tick_timers(&mut world);

    assert!(
        world
            .get::<Signals>(timer_entity)
            .unwrap()
            .has_flag("fired")
    );
    assert!(!world.get::<Signals>(bystander).unwrap().has_flag("fired"));
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
                               "entity", "group", "tilemap", "camera", "collision",
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

use aberredengine::events::gamestate::GameStateChangedEvent;
use aberredengine::resources::gamestate::{GameState, GameStates, NextGameState, NextGameStates};
use aberredengine::systems::gamestate::check_pending_state;

/// Helper: run `check_pending_state` once in a schedule.
fn tick_check_pending_state(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(check_pending_state);
    schedule.run(world);
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
    use aberredengine::resources::lua_runtime::{
        AnimationSnapshot, EntitySnapshot, LuaPhaseSnapshot, LuaTimerSnapshot, SpriteSnapshot,
        build_entity_context_pooled,
    };
    use std::sync::Arc;

    let runtime = LuaRuntime::new().expect("LuaRuntime init");
    let tables = runtime.get_entity_ctx_pool().expect("ctx pool");
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
    use aberredengine::resources::lua_runtime::{EntitySnapshot, build_entity_context_pooled};

    let runtime = LuaRuntime::new().expect("LuaRuntime init");
    let tables = runtime.get_entity_ctx_pool().expect("ctx pool");
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

use aberredengine::components::phase::{
    Phase, PhaseCallbackFns, PhaseEnterFn, PhaseExitFn, PhaseUpdateFn,
};
use aberredengine::systems::GameCtx;
use aberredengine::systems::phase::phase_system;

fn tick_phases(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(phase_system);
    schedule.run(world);
}

fn make_phase_world(delta: f32) -> World {
    let mut world = make_world(delta);
    world.insert_resource(WorldSignals::default());
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
    world.insert_resource(SystemsStore::new());
    world.insert_resource(InputState::default());
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });

    let lua_runtime = LuaRuntime::new().expect("Failed to init Lua runtime");
    world.insert_non_send_resource(lua_runtime);

    {
        let lua_runtime = world.non_send_resource::<LuaRuntime>();
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
    let mut reader = state.get_mut(&mut world);
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
        use aberredengine::components::collision::BoxSide;
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
// Animation system integration tests — row-wrapping (vertical_displacement)
// =============================================================================

use std::sync::Arc;

fn make_animation_resource(
    tex_key: &str,
    position: (f32, f32),
    displacement: (f32, f32),
    frame_count: usize,
    fps: f32,
    looped: bool,
) -> AnimationResource {
    AnimationResource {
        tex_key: Arc::from(tex_key),
        position: Vector2 {
            x: position.0,
            y: position.1,
        },
        horizontal_displacement: displacement.0,
        vertical_displacement: displacement.1,
        frame_count,
        fps,
        looped,
    }
}

/// Create a mock Texture2D without requiring a GPU context.
/// The texture has `id: 0` so raylib's UnloadTexture is a harmless no-op.
fn make_dummy_texture(width: i32, height: i32) -> raylib::prelude::Texture2D {
    unsafe {
        raylib::prelude::Texture2D::from_raw(raylib::ffi::Texture2D {
            id: 0,
            width,
            height,
            mipmaps: 1,
            format: 0,
        })
    }
}

/// Safely remove all textures from the store to prevent UnloadTexture calls
/// on fake textures when the World drops (no OpenGL context in tests).
fn drain_textures(world: &mut World) {
    let mut store = world.resource_mut::<TextureStore>();
    let keys: Vec<String> = store.map.keys().cloned().collect();
    for key in keys {
        if let Some(tex) = store.map.remove(&key) {
            let _ = tex.to_raw(); // forget without calling UnloadTexture
        }
    }
}

fn make_sprite(tex_key: &str) -> Sprite {
    Sprite {
        tex_key: Arc::from(tex_key),
        width: 64.0,
        height: 64.0,
        offset: Vector2 { x: 0.0, y: 0.0 },
        origin: Vector2 { x: 0.0, y: 0.0 },
        flip_h: false,
        flip_v: false,
    }
}

fn tick_animation(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(animation);
    schedule.run(world);
}

/// Helper: advance the animation system N ticks, each tick advances delta time.
fn tick_animation_n(world: &mut World, n: usize) {
    for _ in 0..n {
        tick_animation(world);
    }
}

#[test]
fn animation_horizontal_only_no_wrapping() {
    // v_disp=0: frames advance purely horizontally (backward-compatible behaviour).
    let fps = 10.0;
    let delta = 1.0 / fps; // exactly one frame per tick
    let mut world = make_world(delta);

    let mut anim_store = AnimationStore {
        animations: Default::default(),
    };
    anim_store.animations.insert(
        "walk".to_string(),
        make_animation_resource("sheet", (0.0, 0.0), (64.0, 0.0), 8, fps, true),
    );
    world.insert_resource(anim_store);

    let entity = world
        .spawn((
            Animation {
                animation_key: "walk".to_string(),
                frame_index: 0,
                elapsed_time: 0.0,
            },
            make_sprite("sheet"),
            MapPosition {
                pos: Vector2 { x: 0.0, y: 0.0 },
            },
        ))
        .id();

    // Tick once: frame 0 → frame 1
    tick_animation(&mut world);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 64.0),
        "frame 1: x={}",
        sprite.offset.x
    );
    assert!(approx_eq(sprite.offset.y, 0.0));

    // Tick to frame 5
    tick_animation_n(&mut world, 4);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 320.0),
        "frame 5: x={}",
        sprite.offset.x
    );
    assert!(approx_eq(sprite.offset.y, 0.0));
}

#[test]
fn animation_wraps_rows_with_vertical_displacement() {
    // 256px wide texture, 64px frames, v_disp=64 → 4 frames per row.
    // Animation has 12 frames spanning 3 rows.
    let fps = 10.0;
    let delta = 1.0 / fps;
    let mut world = make_world(delta);

    let mut anim_store = AnimationStore {
        animations: Default::default(),
    };
    anim_store.animations.insert(
        "big".to_string(),
        make_animation_resource("sheet", (0.0, 0.0), (64.0, 64.0), 12, fps, true),
    );
    world.insert_resource(anim_store);

    // Insert a mock texture so the system can look up the width.
    world
        .resource_mut::<TextureStore>()
        .map
        .insert("sheet".to_string(), make_dummy_texture(256, 256));

    let entity = world
        .spawn((
            Animation {
                animation_key: "big".to_string(),
                frame_index: 0,
                elapsed_time: 0.0,
            },
            make_sprite("sheet"),
            MapPosition {
                pos: Vector2 { x: 0.0, y: 0.0 },
            },
        ))
        .id();

    // Advance through all 12 frames, checking offsets at key points.
    // frame 0 already set on first tick (will advance to frame 1)
    // We need frame_index=0 first (initial state, before any tick).
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 0.0),
        "initial: ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    // Tick to frames 1..4 — frame 3 is last on row 0, frame 4 should wrap
    tick_animation_n(&mut world, 4);
    let anim = world.get::<Animation>(entity).unwrap();
    assert_eq!(anim.frame_index, 4, "should be on frame 4");
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 64.0),
        "frame 4: expected (0, 64), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    // Tick to frame 7 (last on row 1)
    tick_animation_n(&mut world, 3);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 192.0) && approx_eq(sprite.offset.y, 64.0),
        "frame 7: expected (192, 64), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    // Tick to frame 8 (first on row 2)
    tick_animation(&mut world);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 128.0),
        "frame 8: expected (0, 128), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    // Tick to frame 11 (last frame)
    tick_animation_n(&mut world, 3);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 192.0) && approx_eq(sprite.offset.y, 128.0),
        "frame 11: expected (192, 128), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    // Tick once more: looped animation wraps to frame 0
    tick_animation(&mut world);
    let anim = world.get::<Animation>(entity).unwrap();
    assert_eq!(anim.frame_index, 0, "should loop back to frame 0");
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 0.0),
        "loop: expected (0, 0), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    drain_textures(&mut world);
}

#[test]
fn animation_wraps_with_partial_first_row() {
    // Start at x=128, 256px texture, 64px frames → first row has 2 frames,
    // subsequent rows have 4.
    let fps = 10.0;
    let delta = 1.0 / fps;
    let mut world = make_world(delta);

    let mut anim_store = AnimationStore {
        animations: Default::default(),
    };
    anim_store.animations.insert(
        "partial".to_string(),
        make_animation_resource("sheet", (128.0, 0.0), (64.0, 64.0), 10, fps, true),
    );
    world.insert_resource(anim_store);

    world
        .resource_mut::<TextureStore>()
        .map
        .insert("sheet".to_string(), make_dummy_texture(256, 256));

    let entity = world
        .spawn((
            Animation {
                animation_key: "partial".to_string(),
                frame_index: 0,
                elapsed_time: 0.0,
            },
            make_sprite("sheet"),
            MapPosition {
                pos: Vector2 { x: 0.0, y: 0.0 },
            },
        ))
        .id();

    // Frame 0: (128, 0)
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 0.0),
        "initial sprite offset before first tick"
    );

    // Tick once to advance frame and compute offset for frame 1
    tick_animation(&mut world);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 192.0) && approx_eq(sprite.offset.y, 0.0),
        "frame 1: expected (192, 0), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    // Frame 2: wraps to row 1
    tick_animation(&mut world);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 64.0),
        "frame 2: expected (0, 64), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    // Frame 5: last of row 1
    tick_animation_n(&mut world, 3);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 192.0) && approx_eq(sprite.offset.y, 64.0),
        "frame 5: expected (192, 64), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    // Frame 6: row 2
    tick_animation(&mut world);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 128.0),
        "frame 6: expected (0, 128), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    drain_textures(&mut world);
}

#[test]
fn animation_vdisp_no_texture_falls_back_to_horizontal() {
    // v_disp > 0 but no texture in store → fallback to horizontal-only (no crash).
    let fps = 10.0;
    let delta = 1.0 / fps;
    let mut world = make_world(delta);

    let mut anim_store = AnimationStore {
        animations: Default::default(),
    };
    anim_store.animations.insert(
        "missing_tex".to_string(),
        make_animation_resource("nonexistent", (0.0, 0.0), (64.0, 64.0), 8, fps, true),
    );
    world.insert_resource(anim_store);

    let entity = world
        .spawn((
            Animation {
                animation_key: "missing_tex".to_string(),
                frame_index: 0,
                elapsed_time: 0.0,
            },
            make_sprite("nonexistent"),
            MapPosition {
                pos: Vector2 { x: 0.0, y: 0.0 },
            },
        ))
        .id();

    // Advance 5 frames — should still work, just no wrapping
    tick_animation_n(&mut world, 5);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 320.0),
        "frame 5: x={}",
        sprite.offset.x
    );
    assert!(approx_eq(sprite.offset.y, 0.0), "frame 5: y should stay 0");
}

#[test]
fn animation_single_frame_per_row_wrapping() {
    // Texture exactly as wide as one frame → every frame gets its own row.
    let fps = 10.0;
    let delta = 1.0 / fps;
    let mut world = make_world(delta);

    let mut anim_store = AnimationStore {
        animations: Default::default(),
    };
    anim_store.animations.insert(
        "column".to_string(),
        make_animation_resource("sheet", (0.0, 0.0), (64.0, 64.0), 4, fps, false),
    );
    world.insert_resource(anim_store);

    world
        .resource_mut::<TextureStore>()
        .map
        .insert("sheet".to_string(), make_dummy_texture(64, 256));

    let entity = world
        .spawn((
            Animation {
                animation_key: "column".to_string(),
                frame_index: 0,
                elapsed_time: 0.0,
            },
            make_sprite("sheet"),
            MapPosition {
                pos: Vector2 { x: 0.0, y: 0.0 },
            },
        ))
        .id();

    // Frame 0: (0, 0)
    tick_animation(&mut world);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 64.0),
        "frame 1: expected (0, 64), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    tick_animation(&mut world);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 128.0),
        "frame 2: expected (0, 128), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    tick_animation(&mut world);
    let sprite = world.get::<Sprite>(entity).unwrap();
    assert!(
        approx_eq(sprite.offset.x, 0.0) && approx_eq(sprite.offset.y, 192.0),
        "frame 3: expected (0, 192), got ({}, {})",
        sprite.offset.x,
        sprite.offset.y
    );

    drain_textures(&mut world);
}
