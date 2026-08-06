//! Lua timer systems.
//!
//! This module provides systems for processing [`LuaTimer`](crate::components::luatimer::LuaTimer) components:
//!
//! - [`update_lua_timers`] – updates timer elapsed time and emits events when they expire
//! - [`lua_timer_observer`] – observer that calls Lua functions when timer events fire
//!
//! # System Flow
//!
//! Each frame:
//!
//! 1. `update_lua_timers` accumulates delta time on all LuaTimer components
//! 2. When `elapsed >= duration`, emits `LuaTimerEvent` and resets timer
//! 3. `lua_timer_observer` receives events and calls the named Lua function
//! 4. Lua callback executes with full engine API access
//! 5. Commands queued by Lua are processed (spawns, audio, signals, entity ops)
//!
//! # Lua Callback Signature
//!
//! ```lua
//! function callback_name(ctx, input)
//!     -- ctx is the entity context table with all component data
//!     -- input is the input table with digital and analog inputs
//!     -- Full access to engine API
//! end
//! ```
//!
//! # Performance
//!
//! Context tables are pooled and reused across callbacks to reduce Lua GC pressure.
//! See [`EntityCtxTables`](crate::resources::lua_runtime::EntityCtxTables) in runtime.rs.

use bevy_ecs::prelude::*;

use crate::components::luatimer::{LuaTimer, LuaTimerCallback};
use crate::events::luatimer::LuaTimerEvent;
use aberred_core::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{LuaDispatch, dispatch_and_drain};

use aberred_core::systems::timer_core::{TimerRunner, run_timer_update};

struct LuaTimerRunner<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
}

impl<'a, 'w, 's> TimerRunner<LuaTimerCallback> for LuaTimerRunner<'a, 'w, 's> {
    fn on_fire(&mut self, entity: Entity, callback: &LuaTimerCallback) {
        self.commands.trigger(LuaTimerEvent {
            entity,
            callback: callback.name.clone(),
        });
    }
}

/// Update all Lua timer components and emit events when they expire.
///
/// Accumulates delta time on each [`LuaTimer`](crate::components::luatimer::LuaTimer)
/// and triggers a [`LuaTimerEvent`](crate::events::luatimer::LuaTimerEvent) when
/// `elapsed >= duration`. The timer resets by subtracting duration, allowing for
/// consistent periodic timing.
pub fn update_lua_timers(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut LuaTimer)>,
    mut commands: Commands,
) {
    let delta = world_time.delta;
    let mut runner = LuaTimerRunner {
        commands: &mut commands,
    };
    run_timer_update(delta, &mut query, &mut runner);
}

pub fn lua_timer_observer(trigger: On<LuaTimerEvent>, mut p: LuaDispatch) {
    let event = trigger.event();
    dispatch_and_drain(&mut p, event.entity, &event.callback, "Timer");
}
