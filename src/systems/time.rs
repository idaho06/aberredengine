//! Time update system.
//!
//! Updates the shared [`WorldTime`](crate::resources::worldtime::WorldTime)
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
/// Called once per logic-thread wakeup to advance the simulation clock.
/// During the fixed-schedule accumulator loop, `delta` is temporarily
/// overridden to `FIXED_DT * time_scale` for each fixed substep. Before the
/// `PRESENT` schedule runs (`VARIABLE` through Phase 6c), the main loop
/// overwrites `delta` again with the latest render-frame delta carried by
/// `LogicMsg::Input`, so `build_drawable_snapshot`'s captured `WorldTime`
/// reflects render-frame time rather than the last 240 Hz wakeup.
pub fn update_world_time(world: &mut World, dt: f32) {
    let mut wt = world.resource_mut::<WorldTime>();
    let scaled_dt = dt * wt.time_scale;
    wt.elapsed += scaled_dt;
    wt.delta = scaled_dt;
    wt.frame_count += 1;
}
