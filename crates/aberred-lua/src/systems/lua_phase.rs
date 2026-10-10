//! Lua phase callbacks on core `Phase`.
//!
//! Core `phase_system` drives every `Phase`; these observers call the entity's
//! Lua callbacks (`crate::components::lua_on_phase`) from its `PhaseEntered` /
//! `PhaseExited` triggers, so they run wherever `phase_system` runs, in every
//! game state. They are spawned only in games that run a Lua script.

use bevy_ecs::prelude::*;
use log::{error, warn};
use mlua::prelude::LuaValue;

use crate::components::lua_on_phase::LuaOnPhaseEntered;
use crate::systems::lua_commands::{LuaDispatch, dispatch_and_drain_returning};
use aberred_core::events::phase::PhaseEntered;

/// Reacts to `PhaseEntered` by calling the entity's [`LuaOnPhaseEntered`]
/// callback with `(ctx, input)`; `ctx.previous_phase` is the event's
/// `previous`. A returned phase name sets `Phase.next` after the callback's
/// queued commands drain, so it beats an `engine.phase_transition` from the
/// same callback; returning the entered phase is a no-op.
pub fn lua_phase_entered_observer(
    trigger: On<PhaseEntered>,
    callbacks: Query<&LuaOnPhaseEntered>,
    mut p: LuaDispatch,
) {
    let ev = trigger.event();
    let Ok(on_entered) = callbacks.get(ev.entity) else {
        return;
    };
    let Some(result) = dispatch_and_drain_returning(
        &mut p,
        ev.entity,
        &on_entered.callback,
        "PhaseEntered",
        ev.previous.as_deref(),
    ) else {
        return;
    };
    if let Some(next) = phase_from_return(result, &ev.name, &on_entered.callback)
        && let Ok(mut phase) = p.phase_query.get_mut(ev.entity)
    {
        phase.next = Some(next);
    }
}

/// A phase callback's return value as a requested phase: a string naming a
/// phase other than `current`. `nil` and `current` request nothing; any other
/// value is warned about and ignored.
pub(crate) fn phase_from_return(
    result: LuaValue,
    current_phase: &str,
    fn_name: &str,
) -> Option<String> {
    match result {
        LuaValue::String(s) => match s.to_str() {
            Ok(phase) => (*phase != *current_phase).then(|| phase.to_string()),
            Err(e) => {
                error!(target: "lua", "Error converting return value in {}(): {}", fn_name, e);
                None
            }
        },
        LuaValue::Nil => None,
        _ => {
            warn!(target: "lua", "Phase callback '{}' returned non-string, non-nil value", fn_name);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::init_dispatch_resources;
    use aberred_core::components::phase::Phase;

    fn setup_world(script: &str) -> World {
        let mut world = World::new();
        init_dispatch_resources(&mut world);
        world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
        world.spawn(Observer::new(lua_phase_entered_observer));
        world.flush();
        world
            .non_send::<LuaRuntime>()
            .lua()
            .load(script)
            .exec()
            .expect("lua load");
        world
    }

    fn global<T: mlua::FromLua>(world: &World, name: &str) -> T {
        world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .get(name)
            .expect(name)
    }

    /// Spawns a phase entity whose core `Phase` is already in `current`, as
    /// `phase_system` leaves it before triggering `PhaseEntered`.
    fn spawn_in_phase(world: &mut World, current: &str, callback: &str) -> Entity {
        world
            .spawn((Phase::new(current), LuaOnPhaseEntered::new(callback)))
            .id()
    }

    fn enter(world: &mut World, entity: Entity, name: &str, previous: Option<&str>) {
        world.trigger(PhaseEntered {
            entity,
            name: name.into(),
            previous: previous.map(Into::into),
        });
        world.flush();
    }

    fn next(world: &World, entity: Entity) -> Option<String> {
        world.get::<Phase>(entity).expect("Phase").next.clone()
    }

    #[test]
    fn fires_on_phase_entered() {
        let mut world = setup_world(
            "calls = 0\nfunction on_enter(ctx, input) calls = calls + 1; seen = ctx.phase; has_input = input ~= nil end",
        );
        let entity = spawn_in_phase(&mut world, "idle", "on_enter");

        enter(&mut world, entity, "idle", None);

        assert_eq!(global::<i64>(&world, "calls"), 1);
        assert_eq!(global::<String>(&world, "seen"), "idle");
        assert!(global::<bool>(&world, "has_input"));
    }

    #[test]
    fn skips_entity_without_callback() {
        let mut world = setup_world("calls = 0\nfunction on_enter(ctx) calls = calls + 1 end");
        let entity = world.spawn(Phase::new("idle")).id();

        enter(&mut world, entity, "idle", None);

        assert_eq!(global::<i64>(&world, "calls"), 0);
    }

    #[test]
    fn return_value_sets_next() {
        let mut world = setup_world("function on_enter(ctx) return \"run\" end");
        let entity = spawn_in_phase(&mut world, "idle", "on_enter");

        enter(&mut world, entity, "idle", None);

        assert_eq!(next(&world, entity).as_deref(), Some("run"));
    }

    #[test]
    fn return_value_beats_phase_transition_in_same_callback() {
        let mut world = setup_world(
            "function on_enter(ctx) engine.phase_transition(ctx.id, \"jump\"); return \"run\" end",
        );
        let entity = spawn_in_phase(&mut world, "idle", "on_enter");

        enter(&mut world, entity, "idle", None);

        assert_eq!(next(&world, entity).as_deref(), Some("run"));
    }

    #[test]
    fn same_name_return_is_noop() {
        let mut world = setup_world("function on_enter(ctx) return \"idle\" end");
        let entity = spawn_in_phase(&mut world, "idle", "on_enter");

        enter(&mut world, entity, "idle", None);

        assert_eq!(next(&world, entity), None);
    }

    #[test]
    fn ctx_has_previous_phase() {
        let mut world = setup_world("function on_enter(ctx) prev = ctx.previous_phase end");
        let entity = spawn_in_phase(&mut world, "run", "on_enter");

        enter(&mut world, entity, "run", Some("idle"));

        assert_eq!(global::<String>(&world, "prev"), "idle");
    }
}
