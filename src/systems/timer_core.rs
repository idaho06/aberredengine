//! Shared timer tick loop used by both the Rust and Lua timer systems.
//!
//! [`run_timer_update`] contains the backend-agnostic logic for advancing
//! [`Timer<C>`](crate::components::timer::Timer) values. The concrete timer systems
//! provide a [`TimerRunner`] implementation that bridges the callback payload into
//! the appropriate dispatch path: the Rust timer path uses `RustTimerRunner` to
//! trigger a `TimerEvent`, while the Lua timer path uses `LuaTimerRunner` to
//! trigger a `LuaTimerEvent` for later script dispatch.

use bevy_ecs::prelude::*;

use crate::components::timer::Timer;

/// Backend-specific callback dispatcher for the shared timer update loop.
///
/// `C` is the callback payload stored in [`Timer<C>`]. In the Rust timer path
/// this is [`TimerCallback`](crate::components::timer::TimerCallback); in the Lua
/// timer path it is [`LuaTimerCallback`](crate::components::luatimer::LuaTimerCallback).
/// [`on_fire`](Self::on_fire) is called once for each timer that elapses and is
/// responsible for invoking or scheduling callback dispatch in whatever way that
/// backend requires.
pub(crate) trait TimerRunner<C> {
    /// Dispatch the callback for a timer that has just elapsed.
    fn on_fire(&mut self, entity: Entity, callback: &C);
}

/// Tick every [`Timer<C>`] in `query` by `delta`, fire elapsed timers, and reset them.
///
/// The shared loop is responsible only for time accumulation and expiry detection.
/// Whenever a timer reaches its duration, `runner` is called exactly once for that
/// fired timer to perform the backend-specific callback dispatch.
pub(crate) fn run_timer_update<C, R>(
    delta: f32,
    query: &mut Query<(Entity, &mut Timer<C>)>,
    runner: &mut R,
) where
    C: Send + Sync + 'static,
    R: TimerRunner<C>,
{
    for (entity, mut timer) in query.iter_mut() {
        timer.elapsed += delta;
        if timer.elapsed >= timer.duration {
            runner.on_fire(entity, &timer.callback);
            timer.reset();
        }
    }
}
