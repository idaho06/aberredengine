//! Lua phase callbacks on core `Phase`.
//!
//! Core `phase_system` drives every `Phase`; these observers call the entity's
//! Lua callbacks (`crate::components::lua_on_phase`) from its `PhaseEntered` /
//! `PhaseExited` triggers, so they run wherever `phase_system` runs, in every
//! game state. [`lua_phase_update_system`] calls the per-tick update callbacks.
//! All three are registered only in games that run a Lua script.

use bevy_ecs::prelude::*;
use log::{error, warn};
use mlua::prelude::LuaValue;

use crate::components::lua_on_phase::{LuaOnPhaseEntered, LuaOnPhaseExited, LuaOnPhaseUpdate};
use crate::resources::lua_runtime::LuaPhaseSnapshot;
use crate::systems::lua_commands::{
    CallShape, DrainScope, LuaDispatch, build_ctx_or_log, dispatch_and_drain_returning,
    drain_dispatch_commands, refresh_signal_cache, request_phase,
};
use aberred_core::components::phase::Phase;
use aberred_core::events::phase::{PhaseEntered, PhaseExited};

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
        CallShape::CtxAndInput,
        ev.previous.as_deref(),
    ) else {
        return;
    };
    if let Some(next) = phase_from_return(result, &ev.name, &on_entered.callback) {
        request_phase(&mut p.phase_query, ev.entity, next);
    }
}

/// Reacts to `PhaseExited` by calling the entity's [`LuaOnPhaseExited`]
/// callback with `(ctx)`. `ctx.phase` is already the new phase and
/// `ctx.previous_phase` is the exited one. The return value is ignored.
pub fn lua_phase_exited_observer(
    trigger: On<PhaseExited>,
    callbacks: Query<&LuaOnPhaseExited>,
    mut p: LuaDispatch,
) {
    let ev = trigger.event();
    let Ok(on_exited) = callbacks.get(ev.entity) else {
        return;
    };
    // Exit callbacks have no return sugar.
    let _ = dispatch_and_drain_returning(
        &mut p,
        ev.entity,
        &on_exited.callback,
        "PhaseExited",
        CallShape::CtxWithPhase,
        Some(&ev.name),
    );
}

/// Calls each core-`Phase` entity's [`LuaOnPhaseUpdate`] callback with
/// `(ctx, input, dt)`, once per run. Returned phase names are applied to
/// `Phase.next` after the queued commands drain (same precedence as
/// [`lua_phase_entered_observer`]); core `phase_system` applies them on the
/// next tick.
pub fn lua_phase_update_system(
    callbacks: Query<(Entity, &LuaOnPhaseUpdate), With<Phase>>,
    mut p: LuaDispatch,
    mut returns: Local<Vec<(Entity, String)>>,
) {
    aberred_core::tracy::tracy_span!("lua_phase_update");
    if callbacks.is_empty() {
        return;
    }
    refresh_signal_cache(&mut p);
    let input_table = match p
        .lua_runtime
        .resolve_input_table(&p.input, p.time.frame_count)
    {
        Ok(table) => table,
        Err(e) => {
            error!(target: "lua", "PhaseUpdate: error creating input table: {}", e);
            return;
        }
    };
    let dt = p.time.delta;

    for (entity, on_update) in &callbacks {
        let Ok(phase) = p.phase_query.get(entity) else {
            continue;
        };
        let Some(ctx_table) = build_ctx_or_log(
            &p,
            entity,
            "PhaseUpdate",
            Some(LuaPhaseSnapshot::from(phase)),
            None,
        ) else {
            continue;
        };
        let result = p
            .lua_runtime
            .call_named(&on_update.callback, "PhaseUpdate", |func| {
                func.call::<LuaValue>((ctx_table, &input_table, dt))
            })
            .unwrap_or(LuaValue::Nil);
        if let Some(next) = phase_from_return(result, &phase.current, &on_update.callback) {
            returns.push((entity, next));
        }
    }

    drain_dispatch_commands(&mut p, DrainScope::Regular);
    for (entity, next) in returns.drain(..) {
        request_phase(&mut p.phase_query, entity, next);
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
    use aberred_core::resources::worldtime::WorldTime;
    use aberred_core::systems::phase::phase_system;
    use bevy_ecs::system::RunSystemOnce;

    fn setup_world(script: &str) -> World {
        let mut world = World::new();
        init_dispatch_resources(&mut world);
        world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
        world.spawn(Observer::new(lua_phase_entered_observer));
        world.spawn(Observer::new(lua_phase_exited_observer));
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

    fn exit(world: &mut World, entity: Entity, name: &str, next: &str) {
        world.trigger(PhaseExited {
            entity,
            name: name.into(),
            next: next.into(),
        });
        world.flush();
    }

    #[test]
    fn exit_ctx_has_new_phase_and_exited_name() {
        let mut world = setup_world(
            "function on_exit(...) local ctx = ...; argc = select('#', ...); phase = ctx.phase; prev = ctx.previous_phase end",
        );
        // phase_system swaps before triggering, so `current` is already the new phase.
        let entity = world
            .spawn((Phase::new("run"), LuaOnPhaseExited::new("on_exit")))
            .id();

        exit(&mut world, entity, "idle", "run");

        assert_eq!(global::<i64>(&world, "argc"), 1, "called with (ctx) only");
        assert_eq!(global::<String>(&world, "phase"), "run");
        assert_eq!(global::<String>(&world, "prev"), "idle");
    }

    #[test]
    fn exit_return_value_is_ignored() {
        let mut world = setup_world("function on_exit(ctx) return \"jump\" end");
        let entity = world
            .spawn((Phase::new("run"), LuaOnPhaseExited::new("on_exit")))
            .id();

        exit(&mut world, entity, "idle", "run");

        assert_eq!(next(&world, entity), None);
    }

    fn spawn_with_update(world: &mut World, current: &str, callback: &str) -> Entity {
        world
            .spawn((Phase::new(current), LuaOnPhaseUpdate::new(callback)))
            .id()
    }

    fn run_update(world: &mut World) {
        world
            .run_system_once(lua_phase_update_system)
            .expect("lua_phase_update_system");
    }

    #[test]
    fn update_called_each_run_with_dt() {
        let mut world = setup_world(
            "calls = 0\nfunction on_update(ctx, input, dt) calls = calls + 1; seen_dt = dt; phase = ctx.phase; has_input = input ~= nil end",
        );
        world.resource_mut::<WorldTime>().delta = 0.25;
        spawn_with_update(&mut world, "idle", "on_update");
        // Without a core Phase the callback is never called.
        world.spawn(LuaOnPhaseUpdate::new("on_update"));

        run_update(&mut world);
        run_update(&mut world);

        assert_eq!(global::<i64>(&world, "calls"), 2);
        assert_eq!(global::<f32>(&world, "seen_dt"), 0.25);
        assert_eq!(global::<String>(&world, "phase"), "idle");
        assert!(global::<bool>(&world, "has_input"));
    }

    #[test]
    fn update_return_sets_next_applied_next_tick() {
        let mut world = setup_world(
            "function on_update(ctx) engine.phase_transition(ctx.id, \"jump\"); return \"run\" end",
        );
        let entity = spawn_with_update(&mut world, "idle", "on_update");

        run_update(&mut world);

        assert_eq!(next(&world, entity).as_deref(), Some("run"));
        assert_eq!(world.get::<Phase>(entity).unwrap().current, "idle");

        world.run_system_once(phase_system).expect("phase_system");

        assert_eq!(world.get::<Phase>(entity).unwrap().current, "run");
    }

    #[test]
    fn update_same_name_return_is_noop() {
        let mut world = setup_world("function on_update(ctx) return \"idle\" end");
        let entity = spawn_with_update(&mut world, "idle", "on_update");

        run_update(&mut world);

        assert_eq!(next(&world, entity), None);
    }
}
