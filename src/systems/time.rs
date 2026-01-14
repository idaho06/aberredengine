//! Time update system.
//!
//! Updates the shared [`WorldTime`](crate::resources::worldtime::WorldTime)
//! resource once per frame, applying `time_scale` to the provided delta.
use bevy_ecs::prelude::*;

use crate::resources::worldtime::WorldTime;

/// Update elapsed and delta seconds on the `WorldTime` resource.
///
/// `dt` is expected to be the unscaled frame delta in seconds. The system
/// applies the current `time_scale` and writes both `elapsed` and `delta`.
pub fn update_world_time(world: &mut World, dt: f32) {
    let mut wt = world.resource_mut::<WorldTime>();
    let scaled_dt = dt * wt.time_scale;
    wt.elapsed += scaled_dt;
    wt.delta = scaled_dt;
}
