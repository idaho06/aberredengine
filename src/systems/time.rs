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
/// Called once per sim tick (Phase 7b: one `Pacer`-driven wakeup at
/// `[simulation] hz`, no more fixed-substep accumulator) to advance the
/// simulation clock before that tick's `sim` schedule runs. The `present`
/// schedule (snapshot build/ship) sees the same `WorldTime.delta` this call
/// set — there is no separate render-frame delta override anymore.
pub fn update_world_time(world: &mut World, dt: f32) {
    let mut wt = world.resource_mut::<WorldTime>();
    let scaled_dt = dt * wt.time_scale;
    wt.elapsed += scaled_dt;
    wt.delta = scaled_dt;
    wt.frame_count += 1;
}
