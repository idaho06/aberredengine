//! Lua timer systems.
//!
//! This module provides systems for processing [`LuaTimer`] components:
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
//! See `EntityCtxTables` in runtime.rs.

use bevy_ecs::prelude::*;

use crate::components::luatimer::{LuaTimer, LuaTimerCallback};
use crate::events::luatimer::LuaTimerEvent;
use crate::systems::lua_commands::{LuaDispatch, dispatch_and_drain};
use aberred_core::resources::worldtime::WorldTime;

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
/// Accumulates delta time on each [`LuaTimer`]
/// and triggers a [`LuaTimerEvent`] when
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

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::testing::approx_eq;
    use bevy_ecs::system::RunSystemOnce;

    /// Minimal world for `update_lua_timers`: a fixed per-tick delta.
    fn world_with_delta(delta: f32) -> World {
        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        world
    }

    fn tick_lua_timers(world: &mut World) {
        world
            .run_system_once(update_lua_timers)
            .expect("update_lua_timers should run");
    }

    #[test]
    fn lua_timer_accumulates_time() {
        let mut world = world_with_delta(0.3);

        let entity = world
            .spawn((LuaTimer::new(
                1.0,
                LuaTimerCallback {
                    name: "my_callback".into(),
                },
            ),))
            .id();

        tick_lua_timers(&mut world);

        let timer = world.get::<LuaTimer>(entity).unwrap();
        assert!(approx_eq(timer.elapsed, 0.3));
    }

    #[test]
    fn lua_timer_fires_event_when_expired() {
        let mut world = world_with_delta(1.0);

        let entity = world
            .spawn((LuaTimer::new(
                0.5,
                LuaTimerCallback {
                    name: "on_timer".into(),
                },
            ),))
            .id();

        // Track if event was triggered
        let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
        let fired_entity = std::sync::Arc::new(std::sync::Mutex::new(None));
        let fired_clone = fired.clone();
        let entity_clone = fired_entity.clone();

        world.add_observer(move |trigger: On<LuaTimerEvent>| {
            *fired_clone.lock().unwrap() = true;
            *entity_clone.lock().unwrap() = Some(trigger.event().entity);
        });
        world.flush();

        tick_lua_timers(&mut world);

        assert!(*fired.lock().unwrap());
        assert_eq!(*fired_entity.lock().unwrap(), Some(entity));
    }

    #[test]
    fn lua_timer_resets_after_firing() {
        let mut world = world_with_delta(0.6);

        let entity = world
            .spawn((LuaTimer::new(
                0.5,
                LuaTimerCallback {
                    name: "callback".into(),
                },
            ),))
            .id();

        // Add dummy observer so events are processed
        world.add_observer(|_trigger: On<LuaTimerEvent>| {});
        world.flush();

        tick_lua_timers(&mut world);

        let timer = world.get::<LuaTimer>(entity).unwrap();
        // Timer should have reset: 0.6 - 0.5 = 0.1
        assert!(approx_eq(timer.elapsed, 0.1));
    }

    #[test]
    fn lua_timer_does_not_fire_before_duration() {
        let mut world = world_with_delta(0.3);

        world.spawn((LuaTimer::new(
            1.0,
            LuaTimerCallback {
                name: "callback".into(),
            },
        ),));

        let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
        let fired_clone = fired.clone();

        world.add_observer(move |_trigger: On<LuaTimerEvent>| {
            *fired_clone.lock().unwrap() = true;
        });
        world.flush();

        tick_lua_timers(&mut world);

        assert!(!*fired.lock().unwrap());
    }

    #[test]
    fn lua_timer_event_carries_correct_callback_name() {
        // Specific to the LuaTimerCallback refactor: LuaTimerCallback.name must
        // flow correctly into LuaTimerEvent.callback.
        let mut world = world_with_delta(1.0);

        world.spawn((LuaTimer::new(
            0.5,
            LuaTimerCallback {
                name: "my_func".into(),
            },
        ),));

        let received_name = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let name_clone = received_name.clone();

        world.add_observer(move |trigger: On<LuaTimerEvent>| {
            *name_clone.lock().unwrap() = trigger.event().callback.to_string();
        });
        world.flush();

        tick_lua_timers(&mut world);

        assert_eq!(*received_name.lock().unwrap(), "my_func");
    }

    #[test]
    fn lua_timer_multiple_entities_fire_with_correct_names() {
        // Each entity's LuaTimerCallback.name must appear in its own event — not swapped.
        let mut world = world_with_delta(1.0);

        let entity_a = world
            .spawn((LuaTimer::new(
                0.5,
                LuaTimerCallback {
                    name: "func_a".into(),
                },
            ),))
            .id();
        let entity_b = world
            .spawn((LuaTimer::new(
                0.5,
                LuaTimerCallback {
                    name: "func_b".into(),
                },
            ),))
            .id();

        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(Entity, String)>::new()));
        let events_clone = events.clone();

        world.add_observer(move |trigger: On<LuaTimerEvent>| {
            events_clone
                .lock()
                .unwrap()
                .push((trigger.event().entity, trigger.event().callback.to_string()));
        });
        world.flush();

        tick_lua_timers(&mut world);

        let events = events.lock().unwrap().clone();
        assert_eq!(events.len(), 2);

        let a_event = events.iter().find(|(e, _)| *e == entity_a).unwrap();
        let b_event = events.iter().find(|(e, _)| *e == entity_b).unwrap();
        assert_eq!(a_event.1, "func_a");
        assert_eq!(b_event.1, "func_b");
    }

    #[test]
    fn lua_timer_callback_name_preserved_after_reset() {
        // reset() only modifies elapsed — LuaTimerCallback.name must survive unchanged.
        let mut world = world_with_delta(1.0);

        let entity = world
            .spawn((LuaTimer::new(
                0.5,
                LuaTimerCallback {
                    name: "persist_cb".into(),
                },
            ),))
            .id();
        world.add_observer(|_trigger: On<LuaTimerEvent>| {});
        world.flush();

        tick_lua_timers(&mut world); // fires and resets

        let timer = world.get::<LuaTimer>(entity).unwrap();
        assert_eq!(&*timer.callback.name, "persist_cb");
    }

    #[test]
    fn lua_timer_fires_across_multiple_ticks() {
        // Verify elapsed accumulates correctly over multiple ticks before firing.
        // duration=0.8, delta=0.3 per tick: ticks 1+2 no fire, tick 3 fires.
        let fired_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        let fired_clone = fired_count.clone();

        let mut world = world_with_delta(0.3);
        world.spawn((LuaTimer::new(0.8, LuaTimerCallback { name: "cb".into() }),));

        world.add_observer(move |_trigger: On<LuaTimerEvent>| {
            *fired_clone.lock().unwrap() += 1;
        });
        world.flush();

        tick_lua_timers(&mut world); // elapsed=0.3
        assert_eq!(*fired_count.lock().unwrap(), 0);

        tick_lua_timers(&mut world); // elapsed=0.6
        assert_eq!(*fired_count.lock().unwrap(), 0);

        tick_lua_timers(&mut world); // elapsed=0.9 >= 0.8, fires, resets to 0.1
        assert_eq!(*fired_count.lock().unwrap(), 1);
    }
}
