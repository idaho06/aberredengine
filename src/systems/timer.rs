//! Rust timer systems.
//!
//! This module provides systems for processing [`Timer`](crate::components::timer::Timer) components:
//!
//! - [`update_timers`] – updates timer elapsed time and emits events when they expire
//! - [`timer_observer`] – observer that calls Rust callbacks when timer events fire
//!
//! Callbacks receive `&mut `[`GameCtx`](crate::systems::GameCtx) for full ECS access.
//!
//! # System Flow
//!
//! Each frame:
//!
//! 1. `update_timers` accumulates delta time on all Timer components
//! 2. When `elapsed >= duration`, emits `TimerEvent` and resets timer
//! 3. `timer_observer` receives events and calls the Rust callback
//! 4. Callback executes with full ECS access through `GameCtx`
//!
//! # Callback Signature
//!
//! ```ignore
//! fn my_callback(entity: Entity, ctx: &mut GameCtx, input: &InputState) {
//!     // Full ECS access: queries, commands, resources
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::components::timer::Timer`] – the timer component
//! - [`crate::events::timer::TimerEvent`] – event emitted on expiration
//! - [`crate::systems::luatimer`] – Lua equivalent

use bevy_ecs::prelude::*;

use crate::components::timer::{Timer, TimerCallback};
use crate::events::timer::TimerEvent;
use crate::resources::input::InputState;
use crate::resources::worldtime::WorldTime;
use crate::systems::GameCtx;

use super::timer_core::{TimerRunner, run_timer_update};

struct RustTimerRunner<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
}

impl<'a, 'w, 's> TimerRunner<TimerCallback> for RustTimerRunner<'a, 'w, 's> {
    fn on_fire(&mut self, entity: Entity, callback: &TimerCallback) {
        self.commands.trigger(TimerEvent {
            entity,
            callback: *callback,
        });
    }
}

/// Update all Rust timer components and emit events when they expire.
///
/// Accumulates delta time on each [`Timer`](crate::components::timer::Timer)
/// and triggers a [`TimerEvent`](crate::events::timer::TimerEvent) when
/// `elapsed >= duration`. The timer resets by subtracting duration, allowing for
/// consistent periodic timing.
pub fn update_timers(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut Timer)>,
    mut commands: Commands,
) {
    let delta = world_time.delta;
    let mut runner = RustTimerRunner { commands: &mut commands };
    run_timer_update(delta, &mut query, &mut runner);
}

/// Observer that handles Rust timer events by calling the callback function.
///
/// When a [`TimerEvent`](crate::events::timer::TimerEvent) is triggered:
///
/// 1. Extracts the entity and callback from the event
/// 2. Calls the callback with `(entity, &mut GameCtx, &InputState)`
/// 3. The callback can use [`GameCtx`] to interact with the ECS
pub fn timer_observer(trigger: On<TimerEvent>, input: Res<InputState>, mut ctx: GameCtx) {
    let event = trigger.event();
    (event.callback)(event.entity, &mut ctx, &input);
}
