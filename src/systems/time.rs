//! Time update system.
//!
//! Updates the shared [`WorldTime`](crate::resources::worldtime::WorldTime)
//! resource once per frame, applying `time_scale` to the provided delta.
use bevy_ecs::prelude::*;

use crate::components::timer::Timer;
use crate::events::timer::TimerEvent;
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

/// Update all timer components and emit events when they expire.
///
/// Accumulates delta time on each [`Timer`](crate::components::timer::Timer)
/// and triggers a [`TimerEvent`](crate::events::timer::TimerEvent) when
/// `elapsed >= duration`. The timer resets after firing.
pub fn update_timers(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut Timer)>,
    mut commands: Commands,
) {
    for (entity, mut timer) in query.iter_mut() {
        timer.elapsed += world_time.delta;
        if timer.elapsed >= timer.duration {
            // Emit timer event
            commands.trigger(TimerEvent {
                entity,
                signal: timer.signal.clone(),
            });
            // Reset timer
            timer.elapsed -= timer.duration;
        }
    }
}
