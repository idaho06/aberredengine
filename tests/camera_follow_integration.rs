//! Integration tests for the camera-follow system.
//!
//! Exercises `camera_follow_system` end-to-end in an ECS world: target
//! selection (priority, GlobalTransform2D preference), all four follow modes,
//! offset, and bounds clamping.

use bevy_ecs::prelude::*;
use raylib::prelude::{Camera2D, Rectangle, Vector2};

use aberredengine::components::cameratarget::CameraTarget;
use aberredengine::components::globaltransform2d::GlobalTransform2D;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::resources::camera2d::Camera2DRes;
use aberredengine::resources::camerafollowconfig::{CameraFollowConfig, EasingCurve, FollowMode};
use aberredengine::resources::screensize::ScreenSize;
use aberredengine::resources::worldtime::WorldTime;
use aberredengine::systems::camera_follow::camera_follow_system;

const EPSILON: f32 = 1e-4;

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < EPSILON
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_camera(target_x: f32, target_y: f32) -> Camera2DRes {
    Camera2DRes(Camera2D {
        target: Vector2 {
            x: target_x,
            y: target_y,
        },
        offset: Vector2 { x: 160.0, y: 120.0 },
        rotation: 0.0,
        zoom: 1.0,
    })
}

fn setup_world() -> World {
    let mut world = World::new();
    world.insert_resource(make_camera(0.0, 0.0));
    world.insert_resource(ScreenSize { w: 320, h: 240 });
    world.insert_resource(WorldTime::default().with_time_scale(1.0));
    // Set a 60fps-like delta
    {
        let mut wt = world.resource_mut::<WorldTime>();
        wt.delta = 1.0 / 60.0;
    }
    let mut cfg = CameraFollowConfig::default();
    cfg.enabled = true;
    world.insert_resource(cfg);
    world
}

fn tick(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(camera_follow_system);
    schedule.run(world);
}

fn camera_target(world: &World) -> Vector2 {
    world.resource::<Camera2DRes>().0.target
}

// ---------------------------------------------------------------------------
// Disabled / no targets
// ---------------------------------------------------------------------------

#[test]
fn disabled_config_does_not_move_camera() {
    let mut world = setup_world();
    world.resource_mut::<CameraFollowConfig>().enabled = false;
    world.spawn((MapPosition::new(500.0, 500.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 0.0));
    assert!(approx_eq(t.y, 0.0));
}

#[test]
fn no_targets_does_not_move_camera() {
    let mut world = setup_world();
    // No entities with CameraTarget
    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 0.0));
    assert!(approx_eq(t.y, 0.0));
}

// ---------------------------------------------------------------------------
// Instant mode
// ---------------------------------------------------------------------------

#[test]
fn instant_snaps_to_target() {
    let mut world = setup_world();
    world.resource_mut::<CameraFollowConfig>().mode = FollowMode::Instant;
    world.spawn((MapPosition::new(200.0, 300.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 200.0), "x: {}", t.x);
    assert!(approx_eq(t.y, 300.0), "y: {}", t.y);
}

#[test]
fn instant_with_offset() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Instant;
        cfg.offset = Vector2 { x: 10.0, y: -20.0 };
    }
    world.spawn((MapPosition::new(100.0, 100.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 110.0), "x: {}", t.x);
    assert!(approx_eq(t.y, 80.0), "y: {}", t.y);
}

// ---------------------------------------------------------------------------
// Priority selection
// ---------------------------------------------------------------------------

#[test]
fn highest_priority_wins() {
    let mut world = setup_world();
    world.resource_mut::<CameraFollowConfig>().mode = FollowMode::Instant;

    world.spawn((MapPosition::new(100.0, 100.0), CameraTarget::new(1)));
    world.spawn((MapPosition::new(500.0, 500.0), CameraTarget::new(10)));
    world.spawn((MapPosition::new(999.0, 999.0), CameraTarget::new(5)));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 500.0), "x: {}", t.x);
    assert!(approx_eq(t.y, 500.0), "y: {}", t.y);
}

#[test]
fn equal_priority_deterministic() {
    let mut world = setup_world();
    world.resource_mut::<CameraFollowConfig>().mode = FollowMode::Instant;

    // Same priority — the entity with the lower Entity id wins
    let e1 = world
        .spawn((MapPosition::new(100.0, 100.0), CameraTarget::new(5)))
        .id();
    let e2 = world
        .spawn((MapPosition::new(900.0, 900.0), CameraTarget::new(5)))
        .id();
    world.flush();

    // Figure out which entity has the lower id
    let (expected_x, expected_y) = if e1 < e2 {
        (100.0, 100.0)
    } else {
        (900.0, 900.0)
    };

    tick(&mut world);
    let t1 = camera_target(&world);

    // Reset camera and tick again — result must be identical
    world.insert_resource(make_camera(0.0, 0.0));
    tick(&mut world);
    let t2 = camera_target(&world);

    assert!(approx_eq(t1.x, expected_x), "x: {}", t1.x);
    assert!(approx_eq(t1.y, expected_y), "y: {}", t1.y);
    assert!(
        approx_eq(t1.x, t2.x),
        "deterministic x: {} vs {}",
        t1.x,
        t2.x
    );
    assert!(
        approx_eq(t1.y, t2.y),
        "deterministic y: {} vs {}",
        t1.y,
        t2.y
    );
}

// ---------------------------------------------------------------------------
// GlobalTransform2D preference
// ---------------------------------------------------------------------------

#[test]
fn prefers_global_transform_over_map_position() {
    let mut world = setup_world();
    world.resource_mut::<CameraFollowConfig>().mode = FollowMode::Instant;

    world.spawn((
        MapPosition::new(0.0, 0.0),
        CameraTarget::default(),
        GlobalTransform2D {
            position: Vector2 { x: 77.0, y: 88.0 },
            rotation_degrees: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
        },
    ));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 77.0), "x: {}", t.x);
    assert!(approx_eq(t.y, 88.0), "y: {}", t.y);
}

#[test]
fn falls_back_to_map_position_without_global_transform() {
    let mut world = setup_world();
    world.resource_mut::<CameraFollowConfig>().mode = FollowMode::Instant;

    world.spawn((MapPosition::new(42.0, 13.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 42.0), "x: {}", t.x);
    assert!(approx_eq(t.y, 13.0), "y: {}", t.y);
}

// ---------------------------------------------------------------------------
// Lerp mode
// ---------------------------------------------------------------------------

#[test]
fn lerp_moves_toward_target() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Lerp;
        cfg.easing = EasingCurve::EaseOut;
        cfg.lerp_speed = 5.0;
    }
    world.spawn((MapPosition::new(200.0, 0.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    // Should have moved toward 200 but not reached it in one frame
    assert!(t.x > 0.0, "should move: x={}", t.x);
    assert!(t.x < 200.0, "should not overshoot: x={}", t.x);
}

#[test]
fn lerp_converges_over_many_frames() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Lerp;
        cfg.easing = EasingCurve::EaseOut;
        cfg.lerp_speed = 10.0;
    }
    world.spawn((MapPosition::new(100.0, 100.0), CameraTarget::default()));
    world.flush();

    for _ in 0..300 {
        tick(&mut world);
    }

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 100.0), "x should converge: {}", t.x);
    assert!(approx_eq(t.y, 100.0), "y should converge: {}", t.y);
}

#[test]
fn lerp_linear_easing_moves_at_constant_rate() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Lerp;
        cfg.easing = EasingCurve::Linear;
        cfg.lerp_speed = 5.0;
    }
    world.spawn((MapPosition::new(100.0, 0.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);
    let x1 = camera_target(&world).x;

    tick(&mut world);
    let x2 = camera_target(&world).x;

    // With linear easing, each step moves a fraction of the remaining distance
    // step1: 0 + (100 - 0) * alpha = x1
    // step2: x1 + (100 - x1) * alpha = x2
    // The ratio (x2 - x1) / x1 should equal (100 - x1) / 100
    let ratio_actual = (x2 - x1) / x1;
    let ratio_expected = (100.0 - x1) / 100.0;
    assert!(
        approx_eq(ratio_actual, ratio_expected),
        "linear ratio: {} vs {}",
        ratio_actual,
        ratio_expected
    );
}

// ---------------------------------------------------------------------------
// SmoothDamp mode
// ---------------------------------------------------------------------------

#[test]
fn smooth_damp_moves_toward_target() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::SmoothDamp;
        cfg.spring_stiffness = 10.0;
        cfg.spring_damping = 5.0;
    }
    world.spawn((MapPosition::new(300.0, 0.0), CameraTarget::default()));
    world.flush();

    // A few ticks to build up velocity
    for _ in 0..5 {
        tick(&mut world);
    }

    let t = camera_target(&world);
    assert!(t.x > 0.0, "should move toward target: x={}", t.x);
    assert!(t.x < 300.0, "should not overshoot immediately: x={}", t.x);
}

#[test]
fn smooth_damp_converges() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::SmoothDamp;
        cfg.spring_stiffness = 15.0;
        cfg.spring_damping = 8.0;
    }
    world.spawn((MapPosition::new(50.0, 50.0), CameraTarget::default()));
    world.flush();

    for _ in 0..600 {
        tick(&mut world);
    }

    let t = camera_target(&world);
    assert!((t.x - 50.0).abs() < 0.1, "x should converge: {}", t.x);
    assert!((t.y - 50.0).abs() < 0.1, "y should converge: {}", t.y);
}

#[test]
fn smooth_damp_reset_velocity_prevents_jump() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::SmoothDamp;
        cfg.spring_stiffness = 20.0;
        cfg.spring_damping = 3.0;
    }
    world.spawn((MapPosition::new(500.0, 0.0), CameraTarget::default()));
    world.flush();

    // Build up velocity
    for _ in 0..30 {
        tick(&mut world);
    }

    // Reset velocity (as you'd do when switching targets)
    world.resource_mut::<CameraFollowConfig>().reset_velocity();

    let before = camera_target(&world);
    tick(&mut world);
    let after = camera_target(&world);

    // After reset, movement should be small (spring just starting from zero velocity)
    let jump = ((after.x - before.x).powi(2) + (after.y - before.y).powi(2)).sqrt();
    assert!(
        jump < 5.0,
        "movement after velocity reset should be small: {}",
        jump
    );
}

// ---------------------------------------------------------------------------
// Deadzone mode
// ---------------------------------------------------------------------------

#[test]
fn deadzone_holds_when_inside() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Deadzone {
            half_w: 50.0,
            half_h: 50.0,
        };
    }
    // Target within deadzone of camera (0,0)
    world.spawn((MapPosition::new(30.0, 20.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 0.0), "x should not move: {}", t.x);
    assert!(approx_eq(t.y, 0.0), "y should not move: {}", t.y);
}

#[test]
fn deadzone_moves_when_outside() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Deadzone {
            half_w: 50.0,
            half_h: 50.0,
        };
        cfg.lerp_speed = 10.0;
    }
    // Target well outside the deadzone
    world.spawn((MapPosition::new(200.0, 200.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(t.x > 0.0, "x should move toward target: {}", t.x);
    assert!(t.y > 0.0, "y should move toward target: {}", t.y);
}

#[test]
fn deadzone_per_axis_independence() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Deadzone {
            half_w: 50.0,
            half_h: 50.0,
        };
        cfg.lerp_speed = 10.0;
    }
    // x inside deadzone, y outside
    world.spawn((MapPosition::new(30.0, 200.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(
        approx_eq(t.x, 0.0),
        "x should hold (inside deadzone): {}",
        t.x
    );
    assert!(t.y > 0.0, "y should move (outside deadzone): {}", t.y);
}

// ---------------------------------------------------------------------------
// Bounds clamping
// ---------------------------------------------------------------------------

#[test]
fn bounds_clamp_prevents_camera_leaving_world() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Instant;
        cfg.bounds = Some(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 1000.0,
        });
    }
    // ScreenSize is 320x240, zoom 1.0 → half viewport = 160, 120
    // So camera target should clamp to [160..840, 120..880]
    // Place target at the origin corner
    world.spawn((MapPosition::new(0.0, 0.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, 160.0), "x clamped to half-viewport: {}", t.x);
    assert!(approx_eq(t.y, 120.0), "y clamped to half-viewport: {}", t.y);
}

#[test]
fn bounds_clamp_far_edge() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Instant;
        cfg.bounds = Some(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 1000.0,
        });
    }
    // Target past the far edge
    world.spawn((MapPosition::new(9999.0, 9999.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    // 1000 - 160 = 840, 1000 - 120 = 880
    assert!(approx_eq(t.x, 840.0), "x clamped to far edge: {}", t.x);
    assert!(approx_eq(t.y, 880.0), "y clamped to far edge: {}", t.y);
}

#[test]
fn bounds_clamp_respects_zoom() {
    let mut world = setup_world();
    // Set zoom = 2.0 → half viewport in world units = 160/2=80, 120/2=60
    world.insert_resource(Camera2DRes(Camera2D {
        target: Vector2 { x: 0.0, y: 0.0 },
        offset: Vector2 { x: 160.0, y: 120.0 },
        rotation: 0.0,
        zoom: 2.0,
    }));
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Instant;
        cfg.bounds = Some(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 500.0,
        });
    }
    // Target at origin
    world.spawn((MapPosition::new(0.0, 0.0), CameraTarget::default()));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    // half_vw = 160/2 = 80, half_vh = 120/2 = 60
    assert!(approx_eq(t.x, 80.0), "x with zoom: {}", t.x);
    assert!(approx_eq(t.y, 60.0), "y with zoom: {}", t.y);
}

#[test]
fn no_bounds_allows_any_position() {
    let mut world = setup_world();
    {
        let mut cfg = world.resource_mut::<CameraFollowConfig>();
        cfg.mode = FollowMode::Instant;
        cfg.bounds = None;
    }
    world.spawn((
        MapPosition::new(-99999.0, -99999.0),
        CameraTarget::default(),
    ));
    world.flush();

    tick(&mut world);

    let t = camera_target(&world);
    assert!(approx_eq(t.x, -99999.0), "x unclamped: {}", t.x);
    assert!(approx_eq(t.y, -99999.0), "y unclamped: {}", t.y);
}
