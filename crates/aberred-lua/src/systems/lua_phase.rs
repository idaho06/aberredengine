//! Lua phase callbacks on core `Phase`.
//!
//! Core `phase_system` drives every `Phase` at the start of the tick. The Lua
//! observers only queue its `PhaseEntered` / `PhaseExited` triggers into
//! [`LuaPhaseEvents`]; [`lua_phase_callbacks_system`] calls the entity's Lua
//! callbacks (`crate::components::lua_on_phase`) later in the tick, after
//! collision, in Playing only. All three are registered only in games that run
//! a Lua script.

use bevy_ecs::prelude::*;
use log::{error, warn};
use mlua::prelude::{LuaFunction, LuaResult, LuaTable, LuaValue};

use crate::components::lua_on_phase::{LuaOnPhaseEntered, LuaOnPhaseExited, LuaOnPhaseUpdate};
use crate::resources::lua_phase_events::{LuaPhaseEvent, LuaPhaseEvents};
use crate::resources::lua_runtime::PhaseSnapshot;
use crate::systems::lua_commands::{
    DrainScope, LuaDispatch, build_ctx_or_log, drain_dispatch_commands, refresh_signal_cache,
    request_phase,
};
use aberred_core::components::phase::Phase;
use aberred_core::events::phase::{PhaseEntered, PhaseExited};

/// Queues `PhaseEntered` for [`lua_phase_callbacks_system`], which calls the
/// entity's [`LuaOnPhaseEntered`] callback.
pub fn lua_phase_entered_observer(trigger: On<PhaseEntered>, mut events: ResMut<LuaPhaseEvents>) {
    let ev = trigger.event();
    events.0.push(LuaPhaseEvent::Entered {
        entity: ev.entity,
        name: ev.name.clone(),
        previous: ev.previous.clone(),
    });
}

/// Queues `PhaseExited` for [`lua_phase_callbacks_system`], which calls the
/// entity's [`LuaOnPhaseExited`] callback.
pub fn lua_phase_exited_observer(trigger: On<PhaseExited>, mut events: ResMut<LuaPhaseEvents>) {
    let ev = trigger.event();
    events.0.push(LuaPhaseEvent::Exited {
        entity: ev.entity,
        name: ev.name.clone(),
    });
}

/// Runs the Lua phase callbacks of one tick:
///
/// 1. The queued [`LuaPhaseEvents`], in trigger order: [`LuaOnPhaseExited`]
///    with `(ctx)`, where `ctx.phase` is already the new phase and
///    `ctx.previous_phase` the exited one; [`LuaOnPhaseEntered`] with
///    `(ctx, input)`, where `ctx.phase` is the entered phase and
///    `ctx.previous_phase` the one left. An entity that lost its callback or
///    its `Phase` since the trigger is skipped.
/// 2. Each core-`Phase` entity's [`LuaOnPhaseUpdate`] with `(ctx, input, dt)`.
///
/// The queued commands drain once, after every callback. Then the returned
/// phase names (enter and update; exit returns are ignored) set `Phase.next`
/// in call order, so they beat an `engine.phase_transition` from the same
/// tick and an update's return beats its entity's enter return. Returning the
/// phase the entity is in is a no-op. Core `phase_system` applies `next` on
/// the next tick.
pub fn lua_phase_callbacks_system(
    entered: Query<&LuaOnPhaseEntered>,
    exited: Query<&LuaOnPhaseExited>,
    updates: Query<(Entity, &LuaOnPhaseUpdate), With<Phase>>,
    mut events: ResMut<LuaPhaseEvents>,
    mut p: LuaDispatch,
    mut returns: Local<Vec<(Entity, String)>>,
) {
    aberred_core::tracy::tracy_span!("lua_phase_callbacks");
    if updates.is_empty() && entered.is_empty() && exited.is_empty() {
        // No Lua phase callbacks: the queued events are all Rust-only.
        events.0.clear();
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

    for event in events.0.drain(..) {
        match event {
            LuaPhaseEvent::Entered {
                entity,
                name,
                previous,
            } => {
                let (Ok(on_entered), Ok(phase)) = (entered.get(entity), p.phase_query.get(entity))
                else {
                    continue;
                };
                let result = call_phase_callback(
                    &p,
                    entity,
                    &on_entered.callback,
                    "PhaseEntered",
                    phase_snapshot(phase, &name),
                    previous.as_deref(),
                    |func, ctx| func.call((ctx, &input_table)),
                );
                if let Some(next) = phase_from_return(result, &phase.current, &on_entered.callback)
                {
                    returns.push((entity, next));
                }
            }
            LuaPhaseEvent::Exited { entity, name } => {
                let (Ok(on_exited), Ok(phase)) = (exited.get(entity), p.phase_query.get(entity))
                else {
                    continue;
                };
                // Exit callbacks have no return sugar.
                call_phase_callback(
                    &p,
                    entity,
                    &on_exited.callback,
                    "PhaseExited",
                    PhaseSnapshot::from(phase),
                    Some(&name),
                    |func, ctx| func.call(ctx),
                );
            }
        }
    }

    for (entity, on_update) in &updates {
        let Ok(phase) = p.phase_query.get(entity) else {
            continue;
        };
        let result = call_phase_callback(
            &p,
            entity,
            &on_update.callback,
            "PhaseUpdate",
            PhaseSnapshot::from(phase),
            None,
            |func, ctx| func.call((ctx, &input_table, dt)),
        );
        if let Some(next) = phase_from_return(result, &phase.current, &on_update.callback) {
            returns.push((entity, next));
        }
    }

    drain_dispatch_commands(&mut p, DrainScope::Regular);
    for (entity, next) in returns.drain(..) {
        request_phase(&mut p.commands, entity, next);
    }
}

/// Builds `entity`'s ctx with the `phase` fields and calls `callback` through
/// `call`. `Nil` when the ctx cannot be built, the function is missing or the
/// call errors (each logged).
fn call_phase_callback(
    p: &LuaDispatch,
    entity: Entity,
    callback: &str,
    label: &str,
    phase: PhaseSnapshot<'_>,
    previous: Option<&str>,
    call: impl FnOnce(LuaFunction, LuaTable) -> LuaResult<LuaValue>,
) -> LuaValue {
    let Some(ctx_table) = build_ctx_or_log(p, entity, label, Some(phase), previous) else {
        return LuaValue::Nil;
    };
    p.lua_runtime
        .call_named(callback, label, |func| call(func, ctx_table))
        .unwrap_or(LuaValue::Nil)
}

/// The enter ctx phase fields for `live`, naming the entered phase `name`.
/// `phase_system` can trigger an initial enter and a transition in one run,
/// and both are called after the swap, so the first must not report the
/// phase it is not in. A phase other than the live one was left this tick,
/// so its time is 0.
fn phase_snapshot<'a>(live: &'a Phase, name: &'a str) -> PhaseSnapshot<'a> {
    if name == live.current {
        PhaseSnapshot::from(live)
    } else {
        PhaseSnapshot {
            current: name,
            time_in_phase: 0.0,
        }
    }
}

/// A phase callback's return value as a requested phase: a string naming a
/// phase other than `current`. `nil` and `current` request nothing; any other
/// value is warned about and ignored.
fn phase_from_return(result: LuaValue, current_phase: &str, fn_name: &str) -> Option<String> {
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
    use crate::lua_plugin::{lua_exec, lua_global};
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::init_dispatch_resources;
    use aberred_core::resources::world_time::WorldTime;
    use aberred_core::systems::phase::phase_system;
    use bevy_ecs::system::RunSystemOnce;

    fn setup_world(script: &str) -> World {
        let mut world = World::new();
        init_dispatch_resources(&mut world);
        world.init_resource::<LuaPhaseEvents>();
        world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
        world.spawn(Observer::new(lua_phase_entered_observer));
        world.spawn(Observer::new(lua_phase_exited_observer));
        world.flush();
        lua_exec(&world, script);
        world
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
        run_update(world);
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

        assert_eq!(lua_global::<i64>(&world, "calls"), 1);
        assert_eq!(lua_global::<String>(&world, "seen"), "idle");
        assert!(lua_global::<bool>(&world, "has_input"));
    }

    #[test]
    fn skips_entity_without_callback() {
        let mut world = setup_world("calls = 0\nfunction on_enter(ctx) calls = calls + 1 end");
        let entity = world.spawn(Phase::new("idle")).id();

        enter(&mut world, entity, "idle", None);

        assert_eq!(lua_global::<i64>(&world, "calls"), 0);
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
    fn phase_transition_after_entity_insert_phase_in_same_callback() {
        let mut world = setup_world(
            "function on_enter(ctx) engine.entity_insert_phase(target, \"a\"); engine.phase_transition(target, \"b\") end",
        );
        let target = world.spawn_empty().id();
        world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .set("target", target.to_bits())
            .expect("set target");
        let entity = spawn_in_phase(&mut world, "idle", "on_enter");

        enter(&mut world, entity, "idle", None);

        let phase = world.get::<Phase>(target).expect("Phase inserted");
        assert_eq!(phase.current, "a");
        assert_eq!(phase.next.as_deref(), Some("b"));
    }

    /// An entity whose initial phase `idle` already has `next = "run"`, so
    /// one `phase_system` run fires Entered(idle), Exited(idle), Entered(run).
    fn spawn_with_preset_next(world: &mut World, callback: &str) -> Entity {
        let entity = spawn_in_phase(world, "idle", callback);
        world.get_mut::<Phase>(entity).expect("Phase").next = Some("run".into());
        entity
    }

    fn run_phase_system(world: &mut World) {
        world
            .run_system_once(phase_system)
            .expect("phase_system should run");
        world.flush();
        run_update(world);
    }

    #[test]
    fn initial_enter_and_transition_in_one_run_see_their_own_phase() {
        let mut world = setup_world(
            "seen = \"\"\nfunction on_enter(ctx) seen = seen .. ctx.phase .. \",\" end",
        );
        spawn_with_preset_next(&mut world, "on_enter");

        run_phase_system(&mut world);

        assert_eq!(lua_global::<String>(&world, "seen"), "idle,run,");
    }

    #[test]
    fn returning_the_phase_already_moved_to_is_noop() {
        let mut world = setup_world(
            "function on_enter(ctx) if ctx.phase == \"idle\" then return \"run\" end end",
        );
        let entity = spawn_with_preset_next(&mut world, "on_enter");

        run_phase_system(&mut world);

        assert_eq!(world.get::<Phase>(entity).expect("Phase").current, "run");
        assert_eq!(next(&world, entity), None);
    }

    #[test]
    fn ctx_has_previous_phase() {
        let mut world = setup_world("function on_enter(ctx) prev = ctx.previous_phase end");
        let entity = spawn_in_phase(&mut world, "run", "on_enter");

        enter(&mut world, entity, "run", Some("idle"));

        assert_eq!(lua_global::<String>(&world, "prev"), "idle");
    }

    /// An entity already in `current` (phase_system swaps before triggering
    /// `PhaseExited`) with a `LuaOnPhaseExited` calling `callback`.
    fn spawn_with_exit(world: &mut World, current: &str, callback: &str) -> Entity {
        world
            .spawn((Phase::new(current), LuaOnPhaseExited::new(callback)))
            .id()
    }

    fn exit(world: &mut World, entity: Entity, name: &str, next: &str) {
        world.trigger(PhaseExited {
            entity,
            name: name.into(),
            next: next.into(),
        });
        world.flush();
        run_update(world);
    }

    #[test]
    fn exit_ctx_has_new_phase_and_exited_name() {
        let mut world = setup_world(
            "function on_exit(...) local ctx = ...; argc = select('#', ...); phase = ctx.phase; prev = ctx.previous_phase end",
        );
        let entity = spawn_with_exit(&mut world, "run", "on_exit");

        exit(&mut world, entity, "idle", "run");

        assert_eq!(
            lua_global::<i64>(&world, "argc"),
            1,
            "called with (ctx) only"
        );
        assert_eq!(lua_global::<String>(&world, "phase"), "run");
        assert_eq!(lua_global::<String>(&world, "prev"), "idle");
    }

    #[test]
    fn exit_return_value_is_ignored() {
        let mut world = setup_world("function on_exit(ctx) return \"jump\" end");
        let entity = spawn_with_exit(&mut world, "run", "on_exit");

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
            .run_system_once(lua_phase_callbacks_system)
            .expect("lua_phase_callbacks_system");
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

        assert_eq!(lua_global::<i64>(&world, "calls"), 2);
        assert_eq!(lua_global::<f32>(&world, "seen_dt"), 0.25);
        assert_eq!(lua_global::<String>(&world, "phase"), "idle");
        assert!(lua_global::<bool>(&world, "has_input"));
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

        run_phase_system(&mut world);

        assert_eq!(world.get::<Phase>(entity).unwrap().current, "run");
    }

    #[test]
    fn enter_and_update_returns_in_one_tick_update_wins() {
        let mut world = setup_world(
            "function on_enter(ctx) return \"a\" end\nfunction on_update(ctx) return \"b\" end",
        );
        let entity = world
            .spawn((
                Phase::new("idle"),
                LuaOnPhaseEntered::new("on_enter"),
                LuaOnPhaseUpdate::new("on_update"),
            ))
            .id();

        enter(&mut world, entity, "idle", None);

        assert_eq!(next(&world, entity).as_deref(), Some("b"));
    }

    #[test]
    fn enter_and_first_update_see_zero_time() {
        let mut world = setup_world(
            "seen = \"\"\n\
             function on_enter(ctx) seen = seen .. \"enter:\" .. ctx.time_in_phase .. \",\" end\n\
             function on_update(ctx) seen = seen .. \"update:\" .. ctx.time_in_phase .. \",\" end",
        );
        world.resource_mut::<WorldTime>().delta = 0.25;
        world.spawn((
            Phase::new("idle"),
            LuaOnPhaseEntered::new("on_enter"),
            LuaOnPhaseUpdate::new("on_update"),
        ));

        run_phase_system(&mut world);
        run_phase_system(&mut world);

        assert_eq!(
            lua_global::<String>(&world, "seen"),
            "enter:0,update:0,update:0.25,"
        );
    }

    #[test]
    fn queued_enter_skips_despawned_entity() {
        let mut world = setup_world("calls = 0\nfunction on_enter(ctx) calls = calls + 1 end");
        let entity = spawn_in_phase(&mut world, "idle", "on_enter");
        world.trigger(PhaseEntered {
            entity,
            name: "idle".into(),
            previous: None,
        });
        world.flush();
        world.despawn(entity);

        run_update(&mut world);

        assert_eq!(lua_global::<i64>(&world, "calls"), 0);
    }

    #[test]
    fn update_same_name_return_is_noop() {
        let mut world = setup_world("function on_update(ctx) return \"idle\" end");
        let entity = spawn_with_update(&mut world, "idle", "on_update");

        run_update(&mut world);

        assert_eq!(next(&world, entity), None);
    }
}
