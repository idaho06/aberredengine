//! Lua timer dispatch.
//!
//! Lua timers are core `Timer`s ticked by `update_timers`; these observers are
//! spawned only in games that run a Lua script.

use bevy_ecs::prelude::*;

use crate::components::lua_on_timer_fired::LuaOnTimerFired;
use crate::systems::lua_commands::{LuaDispatch, dispatch_and_drain};
use aberred_core::components::timer::Timer;
use aberred_core::events::timer::TimerFired;

/// Reacts to `TimerFired` by calling the entity's [`LuaOnTimerFired`] callback
/// with `(ctx, input)`. An entity without one (a Rust timer) is skipped.
pub fn lua_timer_fired_observer(
    trigger: On<TimerFired>,
    callbacks: Query<&LuaOnTimerFired>,
    mut p: LuaDispatch,
) {
    let entity = trigger.event().entity;
    let Ok(on_fired) = callbacks.get(entity) else {
        return;
    };
    dispatch_and_drain(&mut p, entity, &on_fired.callback, "Timer");
}

/// Removes [`LuaOnTimerFired`] when its entity loses its `Timer` (a fired
/// one-shot, or an explicit removal), so the callback never outlives its timer.
/// Replacing the `Timer` keeps the callback.
pub fn lua_timer_removed_observer(trigger: On<Remove, Timer>, mut commands: Commands) {
    commands
        .entity(trigger.event().entity)
        .try_remove::<LuaOnTimerFired>();
}

#[cfg(test)]
mod tests {
    //! The firing rules (accumulate, `>=`, once per tick, keep the overshoot,
    //! re-arm/zero-duration handling) are the core `Timer`'s and are tested in
    //! `aberred_core::systems::timer`; these tests cover the Lua wiring.
    use super::*;
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::init_dispatch_resources;
    use aberred_core::components::timer::TimerMode;

    fn setup_world() -> World {
        let mut world = World::new();
        init_dispatch_resources(&mut world);
        world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
        world.spawn(Observer::new(lua_timer_fired_observer));
        world.spawn(Observer::new(lua_timer_removed_observer));
        world.flush();
        world
            .non_send::<LuaRuntime>()
            .lua()
            .load("calls = 0\nfunction on_fire(ctx) calls = calls + 1 end")
            .exec()
            .expect("lua load");
        world
    }

    fn spawn_lua_timer(world: &mut World) -> Entity {
        world
            .spawn(LuaOnTimerFired::timer(1.0, "on_fire", TimerMode::Repeat))
            .id()
    }

    fn calls(world: &World) -> i64 {
        world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .get("calls")
            .expect("calls")
    }

    #[test]
    fn timer_fired_without_a_lua_callback_is_skipped() {
        let mut world = setup_world();
        let entity = world.spawn(Timer::new(1.0)).id();

        world.trigger(TimerFired { entity });
        world.flush();

        assert_eq!(calls(&world), 0);
    }

    #[test]
    fn removing_the_timer_removes_its_lua_callback() {
        let mut world = setup_world();
        let entity = spawn_lua_timer(&mut world);

        world.entity_mut(entity).remove::<Timer>();
        world.flush();

        assert!(world.get::<LuaOnTimerFired>(entity).is_none());
        assert!(world.get_entity(entity).is_ok(), "the entity survives");
    }

    #[test]
    fn despawning_a_lua_timer_entity_is_clean() {
        let mut world = setup_world();
        let entity = spawn_lua_timer(&mut world);

        world.despawn(entity);
        world.flush();

        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn replacing_the_timer_keeps_its_lua_callback() {
        let mut world = setup_world();
        let entity = spawn_lua_timer(&mut world);

        world.entity_mut(entity).insert(Timer::once(2.0));
        world.flush();

        assert!(world.get::<LuaOnTimerFired>(entity).is_some());
    }
}
