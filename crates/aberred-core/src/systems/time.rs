//! Time update system.
//!
//! Updates the shared [`WorldTime`]
//! resource once per frame, applying `time_scale` to the provided delta.
use bevy_ecs::prelude::*;

use crate::resources::worldtime::WorldTime;

/// Update elapsed and delta seconds on the `WorldTime` resource.
///
/// `dt` is expected to be the unscaled real-time delta in seconds since the
/// last logic-thread clock advance. The system applies the current
/// `time_scale` and writes both `elapsed` and `delta`. Also increments the
/// frame counter.
///
/// Called once per sim tick (one `Pacer`-driven wakeup at `[simulation] hz`)
/// to advance the simulation clock before that tick's `sim` schedule runs.
/// The `present` schedule (snapshot build/ship) sees the same
/// `WorldTime.delta` this call set — there is no separate render-frame delta.
pub fn update_world_time(world: &mut World, dt: f32) {
    let mut wt = world.resource_mut::<WorldTime>();
    let scaled_dt = dt * wt.time_scale;
    wt.elapsed += scaled_dt;
    wt.delta = scaled_dt;
    wt.frame_count += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::approx_eq;

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
}
