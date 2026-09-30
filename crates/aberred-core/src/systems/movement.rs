//! Movement system with acceleration physics.
//!
//! Integrates entity positions from their current rigid body velocities and
//! the world's unscaled delta time. Supports multiple named acceleration forces
//! with individual enable/disable, friction damping, and optional speed clamping.
//!
//! Entities with `frozen = true` are skipped entirely, allowing external systems
//! to control their position directly.

use bevy_ecs::prelude::*;
use crate::math::Vec2;

use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::protocol::audio::AudioCmd;
use crate::resources::screensize::ScreenSize;
use crate::resources::signal_keys as sk;
use crate::resources::worldtime::WorldTime;

/// Apply acceleration forces and velocity to `MapPosition` using the frame's delta time.
///
/// This system performs physics integration in the following order:
/// 1. Skip if entity is frozen
/// 2. Sum all enabled acceleration forces
/// 3. Integrate acceleration into velocity: `velocity += total_acceleration * delta`
/// 4. Apply friction damping: `velocity *= (1 - friction * delta)`
/// 5. Clamp velocity to max_speed if configured
/// 6. Integrate velocity into position: `position += velocity * delta`
/// 7. Update movement signals for animation/audio systems
pub fn movement(
    mut query: Query<(
        Entity,
        &mut MapPosition,
        &mut RigidBody,
        Option<&mut Signals>,
    )>,
    time: Res<WorldTime>,
    _screensize: Res<ScreenSize>,
    mut _audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    crate::tracy::tracy_span!("movement");
    for (_entity, mut position, mut rigidbody, mut maybe_signals) in query.iter_mut() {
        // Step 1: Skip frozen entities
        if rigidbody.frozen {
            // Still update signals for frozen entities (they might still be "moving" via external control)
            if let Some(signals) = maybe_signals.as_mut() {
                signals.clear_flag(sk::MOVING);
                signals.update_scalar(sk::SPEED_SQ, 0.0);
            }
            continue;
        }

        let delta = time.delta;

        // Step 2: Calculate total acceleration from all enabled forces
        let total_acceleration = rigidbody.total_acceleration();

        // Step 3: Integrate acceleration into velocity
        rigidbody.velocity += total_acceleration * delta;

        // Step 4: Apply friction damping
        // Using linear damping: velocity *= (1 - friction * delta)
        // This is stable for typical friction values (0-10) and frame rates
        if rigidbody.friction > 0.0 {
            let damping = (1.0 - rigidbody.friction * delta).max(0.0);
            rigidbody.velocity *= damping;

            // Zero out very small velocities to prevent drift
            const VELOCITY_EPSILON_SQ: f32 = 0.01 * 0.01;
            if rigidbody.velocity.length_squared() < VELOCITY_EPSILON_SQ {
                rigidbody.velocity = Vec2 { x: 0.0, y: 0.0 };
            }
        }

        // Step 5: Clamp velocity to max_speed if configured
        if let Some(max_speed) = rigidbody.max_speed {
            let speed_sq = rigidbody.velocity.length_squared();
            if speed_sq > max_speed * max_speed {
                let speed = speed_sq.sqrt(); // single sqrt, only when clamping
                rigidbody.velocity = rigidbody.velocity / speed * max_speed;
            }
        }

        // Step 6: Integrate velocity into position
        position.pos += rigidbody.velocity * delta;

        // Step 7: Update movement signals
        if let Some(signals) = maybe_signals.as_mut() {
            let speed_sq = rigidbody.velocity.length_squared();
            if speed_sq > 0.0 {
                signals.ensure_flag(sk::MOVING);
            } else {
                signals.clear_flag(sk::MOVING);
            }
            signals.update_scalar(sk::SPEED_SQ, speed_sq);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::time::update_world_time;
    use crate::testing::approx_eq;
    use bevy_ecs::system::RunSystemOnce;

    /// Minimal world for `movement`: exactly the resources its params read.
    fn world_with_time(time: WorldTime) -> World {
        let mut world = World::new();
        world.insert_resource(time);
        world.insert_resource(ScreenSize { w: 800, h: 600 });
        world.init_resource::<Messages<AudioCmd>>();
        world
    }

    fn tick_movement(world: &mut World) {
        world
            .run_system_once(movement)
            .expect("movement should run");
    }

    #[test]
    fn movement_integrates_velocity_into_position() {
        let mut world = world_with_time(WorldTime::default());
        let mut rb = RigidBody::new();
        rb.velocity = Vec2 { x: 10.0, y: 0.0 };

        let entity = world.spawn((MapPosition::new(0.0, 0.0), rb)).id();

        update_world_time(&mut world, 0.5);
        tick_movement(&mut world);

        let pos = world.get::<MapPosition>(entity).unwrap();
        assert!(approx_eq(pos.pos.x, 5.0));
        assert!(approx_eq(pos.pos.y, 0.0));
    }

    #[test]
    fn movement_applies_acceleration_forces() {
        let mut world = world_with_time(WorldTime::default());
        let mut rb = RigidBody::new();
        rb.add_force("thrust", Vec2 { x: 2.0, y: 0.0 });

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
        let mut world = world_with_time(WorldTime::default());
        let mut rb = RigidBody::new();
        rb.velocity = Vec2 { x: 3.0, y: 4.0 };

        let entity = world
            .spawn((MapPosition::new(0.0, 0.0), rb, Signals::default()))
            .id();

        update_world_time(&mut world, 1.0);
        tick_movement(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(signals.has_flag(sk::MOVING));
        assert!(approx_eq(signals.get_scalar(sk::SPEED_SQ).unwrap(), 25.0));
    }

    #[test]
    fn movement_skips_frozen_but_clears_signals() {
        let mut world = world_with_time(WorldTime::default());
        let mut rb = RigidBody::new();
        rb.velocity = Vec2 { x: 5.0, y: 0.0 };
        rb.freeze();

        let mut signals = Signals::default().with_flag(sk::MOVING);
        signals.set_scalar(sk::SPEED_SQ, 123.0);

        let entity = world.spawn((MapPosition::new(1.0, 1.0), rb, signals)).id();

        update_world_time(&mut world, 1.0);
        tick_movement(&mut world);

        let pos = world.get::<MapPosition>(entity).unwrap();
        let signals = world.get::<Signals>(entity).unwrap();
        assert!(approx_eq(pos.pos.x, 1.0));
        assert!(approx_eq(pos.pos.y, 1.0));
        assert!(!signals.has_flag(sk::MOVING));
        assert!(approx_eq(signals.get_scalar(sk::SPEED_SQ).unwrap(), 0.0));
    }

    #[test]
    fn time_scale_zero_freezes_movement() {
        let mut world = world_with_time(WorldTime::default().with_time_scale(0.0));
        let mut rb = RigidBody::new();
        rb.velocity = Vec2 { x: 100.0, y: 0.0 };

        let entity = world.spawn((MapPosition::new(0.0, 0.0), rb)).id();

        update_world_time(&mut world, 1.0);
        tick_movement(&mut world);

        let pos = world.get::<MapPosition>(entity).unwrap();
        assert!(approx_eq(pos.pos.x, 0.0));
        assert!(approx_eq(pos.pos.y, 0.0));
    }

    #[test]
    fn time_scale_doubles_effective_movement() {
        // time_scale 2.0 with a base dt of 0.5 gives an effective delta of 1.0.
        let mut world = world_with_time(WorldTime::default().with_time_scale(2.0));
        let mut rb = RigidBody::new();
        rb.velocity = Vec2 { x: 10.0, y: 0.0 };

        let entity = world.spawn((MapPosition::new(0.0, 0.0), rb)).id();

        update_world_time(&mut world, 0.5);
        tick_movement(&mut world);

        let pos = world.get::<MapPosition>(entity).unwrap();
        assert!(approx_eq(pos.pos.x, 10.0));
    }
}
