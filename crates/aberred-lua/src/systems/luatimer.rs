//! Lua timer systems.
//!
//! This module provides systems for processing [`LuaTimer`] components:
//!
//! - [`update_lua_timers`] – advances each timer's core `Timer` and emits events when they fire
//! - [`lua_timer_observer`] – observer that calls Lua functions when timer events fire
//!
//! # System Flow
//!
//! Each sim tick:
//!
//! 1. `update_lua_timers` advances each `LuaTimer`'s core `Timer` (same firing
//!    rules as `update_timers`) and emits `LuaTimerEvent` when it fires
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
use bevy_ecs::system::SystemChangeTick;

use crate::components::luatimer::LuaTimer;
use crate::events::luatimer::LuaTimerEvent;
use crate::systems::lua_commands::{LuaDispatch, dispatch_and_drain};
use aberred_core::resources::worldtime::WorldTime;

/// Update all Lua timer components and emit events when they expire.
///
/// Advances each [`LuaTimer`]'s core `Timer` with the same firing rules as
/// `update_timers`, triggering a [`LuaTimerEvent`] when it fires.
pub fn update_lua_timers(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut LuaTimer)>,
    mut commands: Commands,
    ticks: SystemChangeTick,
) {
    let fired_at = ticks.this_run();
    for (entity, mut lua_timer) in query.iter_mut() {
        if lua_timer.timer.advance(world_time.delta) {
            commands.trigger(LuaTimerEvent {
                entity,
                callback: lua_timer.callback.clone(),
            });
            lua_timer
                .timer
                .finish_fired::<LuaTimer>(&mut commands, entity, fired_at);
        }
    }
}

pub fn lua_timer_observer(trigger: On<LuaTimerEvent>, mut p: LuaDispatch) {
    let event = trigger.event();
    dispatch_and_drain(&mut p, event.entity, &event.callback, "Timer");
}

#[cfg(test)]
mod tests {
    //! The firing rules (accumulate, `>=`, once per tick, keep the overshoot,
    //! re-arm/zero-duration handling) are the core `Timer`'s and are tested in
    //! `aberred_core::systems::timer`; these tests cover the Lua wiring.
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    /// `(entity, callback)` of every [`LuaTimerEvent`], in firing order.
    #[derive(Resource, Default)]
    struct Fired(Vec<(Entity, String)>);

    fn record_fire(trigger: On<LuaTimerEvent>, mut fired: ResMut<Fired>) {
        let event = trigger.event();
        fired.0.push((event.entity, event.callback.to_string()));
    }

    /// World with a fixed per-tick delta, [`Fired`] and a global [`record_fire`].
    fn observed_world(delta: f32) -> World {
        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        world.init_resource::<Fired>();
        world.add_observer(record_fire);
        world.flush();
        world
    }

    fn tick_lua_timers(world: &mut World) {
        world
            .run_system_once(update_lua_timers)
            .expect("update_lua_timers should run");
    }

    fn fired(world: &World) -> &[(Entity, String)] {
        &world.resource::<Fired>().0
    }

    #[test]
    fn lua_timers_fire_with_their_own_callback_names() {
        let mut world = observed_world(1.0);
        let a = world.spawn(LuaTimer::new(0.5, "func_a")).id();
        let b = world.spawn(LuaTimer::new(0.5, "func_b")).id();
        world.spawn(LuaTimer::new(2.0, "not_yet"));

        tick_lua_timers(&mut world);

        let mut events = fired(&world).to_vec();
        events.sort();
        let mut expected = vec![(a, "func_a".to_string()), (b, "func_b".to_string())];
        expected.sort();
        assert_eq!(events, expected);
    }

    #[test]
    fn lua_repeat_timer_keeps_firing() {
        let mut world = observed_world(1.0);
        let entity = world.spawn(LuaTimer::new(0.5, "tick")).id();

        tick_lua_timers(&mut world);
        tick_lua_timers(&mut world);

        assert_eq!(fired(&world).len(), 2);
        assert!(world.get::<LuaTimer>(entity).is_some());
    }

    #[test]
    fn lua_once_timer_fires_once_then_loses_its_timer() {
        let mut world = observed_world(1.0);
        let entity = world.spawn(LuaTimer::once(0.5, "boom")).id();

        tick_lua_timers(&mut world);
        tick_lua_timers(&mut world);

        assert_eq!(fired(&world), [(entity, "boom".to_string())]);
        assert!(world.get::<LuaTimer>(entity).is_none());
        assert!(world.get_entity(entity).is_ok(), "the entity survives");
    }

    #[test]
    fn lua_once_timer_callback_may_despawn_its_entity() {
        let mut world = observed_world(1.0);
        world.add_observer(|trigger: On<LuaTimerEvent>, mut commands: Commands| {
            commands.entity(trigger.event().entity).despawn();
        });
        world.flush();
        let entity = world.spawn(LuaTimer::once(0.5, "boom")).id();

        tick_lua_timers(&mut world); // the removal after a despawn must not panic

        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn lua_once_timer_callback_can_rearm_a_new_timer() {
        let mut world = observed_world(1.0);
        world.add_observer(|trigger: On<LuaTimerEvent>, mut commands: Commands| {
            commands
                .entity(trigger.event().entity)
                .insert(LuaTimer::once(2.0, "again"));
        });
        world.flush();
        let entity = world.spawn(LuaTimer::once(0.5, "boom")).id();

        tick_lua_timers(&mut world);

        let lua_timer = world
            .get::<LuaTimer>(entity)
            .expect("the re-armed timer must survive the spent timer's removal");
        assert_eq!(&*lua_timer.callback, "again");
    }
}
