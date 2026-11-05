//! Shared callback dispatch flow.
//!
//! Bundles the params and steps shared by `lua_timer_fired_observer`,
//! `lua_setup_entity_system`, `lua_animation_finished_observer`,
//! `lua_tween_finished_observer`, `lua_gui_clicked_observer`,
//! `lua_menu_selected_observer`, the Lua collision observers, and the
//! dialogue observers (`crate::systems::lua_dialogue`): sync the signal
//! cache, build the callback's arguments (an entity context and optional
//! input table, or a custom context table), call the named Lua function,
//! then drain the commands it queued (the collision observers drain the
//! collision-scoped queues). The dialogue observers additionally pass a
//! second, event-built table through [`dispatch_ctx_call_and_drain`].
//!
//! `lua_phase_callbacks_system` (`crate::systems::lua_phase`) batches like
//! `lua_setup_entity_system` (one signal refresh, one drain per run) but
//! passes one shared input table, the event's phase fields and `dt`, so it
//! builds ctx with [`build_ctx_or_log`] and calls Lua itself instead of going
//! through a [`CallShape`].

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use log::error;
use mlua::prelude::{Lua, LuaResult, LuaTable};

use crate::resources::lua_runtime::{LuaRuntime, PhaseSnapshot};
use aberred_core::components::phase::Phase;
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::resources::game_state::NextGameState;
use aberred_core::resources::input::InputState;
use aberred_core::resources::next_scene::NextScene;
use aberred_core::resources::systems_store::SystemsStore;
use aberred_core::resources::world_signals::WorldSignals;
use aberred_core::resources::world_time::WorldTime;

use super::{
    ContextQueries, DrainScope, EffectCmdBufs, SignalTargets, build_entity_context,
    drain_and_process_effect_commands,
};

/// Everything needed to dispatch one Lua entity callback and drain the
/// commands it queued.
#[derive(SystemParam)]
pub struct LuaDispatch<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub input: Res<'w, InputState>,
    pub time: Res<'w, WorldTime>,
    pub ctx_queries: ContextQueries<'w, 's>,
    pub phase_query: Query<'w, 's, &'static Phase>,
    pub world_signals: ResMut<'w, WorldSignals>,
    pub next_scene: ResMut<'w, NextScene>,
    pub next_game_state: ResMut<'w, NextGameState>,
    pub lua_runtime: NonSend<'w, LuaRuntime>,
    pub audio_cmd_writer: MessageWriter<'w, AudioCmd>,
    pub systems_store: Res<'w, SystemsStore>,
    pub effect_bufs: Local<'s, EffectCmdBufs>,
}

/// The call shapes the dispatch sites need.
#[derive(Clone, Copy)]
pub enum CallShape {
    /// Resolve the phase snapshot, build the input table, call `(ctx, input)`
    /// — timer/anim/tween path.
    CtxAndInput,
    /// Skip the phase snapshot, no input table; call `(ctx)` — `lua_setup` path.
    CtxOnly,
}

/// Sync the Lua signal cache from `WorldSignals`. Call once per system
/// invocation (not per entity — see `lua_setup_entity_system`, which loops
/// many entities per invocation but only wants this done once, up front).
pub fn refresh_signal_cache(p: &mut LuaDispatch) {
    p.lua_runtime.sync_signals(&mut p.world_signals);
}

/// Build the entity context (and input table, if `call_shape` wants one) and
/// call the named Lua function. Returns `false` if input-table or ctx
/// construction failed (already logged), so callers can early-return/continue.
/// Does not refresh the signal cache — callers do that once, up front (see
/// [`refresh_signal_cache`]), since the right granularity (per-call vs.
/// per-invocation) differs between single-entity observers and
/// `lua_setup_entity_system`'s per-frame batch.
pub fn call_entity_callback(
    p: &mut LuaDispatch,
    entity: Entity,
    callback_name: &str,
    label: &str,
    call_shape: CallShape,
) -> bool {
    match call_shape {
        CallShape::CtxAndInput => {
            let input_table = match p
                .lua_runtime
                .resolve_input_table(&p.input, p.time.frame_count)
            {
                Ok(table) => table,
                Err(e) => {
                    error!(
                        target: "lua",
                        "{label}: error creating input table for {:?}: {}",
                        entity, e
                    );
                    return false;
                }
            };
            let phase = p.phase_query.get(entity).ok().map(PhaseSnapshot::from);
            let Some(ctx_table) = build_ctx_or_log(p, entity, label, phase, None) else {
                return false;
            };
            p.lua_runtime.call_named(callback_name, label, |func| {
                func.call::<()>((ctx_table, input_table))
            });
        }
        CallShape::CtxOnly => {
            let Some(ctx_table) = build_ctx_or_log(p, entity, label, None, None) else {
                return false;
            };
            p.lua_runtime
                .call_named(callback_name, label, |func| func.call::<()>(ctx_table));
        }
    }
    true
}

/// Refresh the signal cache, call the named entity callback with
/// `(ctx, input)`, and drain its queued commands if the call happened.
/// Covers the shape shared by every single-entity-per-invocation observer
/// (timer, on_animation_finished, on_tween_finished) — `lua_setup_entity_system`
/// still calls `refresh_signal_cache`/`call_entity_callback`/
/// `drain_dispatch_commands` directly, since it batches one refresh and one
/// drain across many entities per invocation instead of one each.
pub fn dispatch_and_drain(p: &mut LuaDispatch, entity: Entity, callback_name: &str, label: &str) {
    refresh_signal_cache(p);
    if call_entity_callback(p, entity, callback_name, label, CallShape::CtxAndInput) {
        drain_dispatch_commands(p, DrainScope::Regular);
    }
}

/// Refresh the signal cache, call the named entity callback as
/// `fn(ctx, extra)` — `ctx` built by `build_ctx_or_log`, `extra` built by
/// `build_extra` — then drain the commands it queued, also after a Lua
/// error, which may have queued some first. For the dialogue observers,
/// where `extra` is the event's dialogue table.
pub fn dispatch_ctx_call_and_drain(
    p: &mut LuaDispatch,
    entity: Entity,
    callback_name: &str,
    label: &str,
    build_extra: impl FnOnce(&Lua) -> LuaResult<LuaTable>,
) {
    refresh_signal_cache(p);
    let Some(ctx_table) = build_ctx_or_log(p, entity, label, None, None) else {
        return;
    };
    p.lua_runtime.call_named(callback_name, label, |func| {
        let extra = build_extra(p.lua_runtime.lua())?;
        func.call::<()>((ctx_table, extra))
    });
    drain_dispatch_commands(p, DrainScope::Regular);
}

/// Build `entity`'s ctx table from the dispatch queries, logging (with
/// `label`) and returning `None` on failure.
pub(crate) fn build_ctx_or_log(
    p: &LuaDispatch,
    entity: Entity,
    label: &str,
    phase: Option<PhaseSnapshot<'_>>,
    previous_phase: Option<&str>,
) -> Option<LuaTable> {
    match build_entity_context(
        &p.lua_runtime,
        entity,
        &p.ctx_queries,
        phase,
        previous_phase,
    ) {
        Ok(ctx) => Some(ctx),
        Err(e) => {
            error!(
                target: "lua",
                "{label}: error building context for {:?}: {}",
                entity, e
            );
            None
        }
    }
}

/// Refresh the signal cache, call the named Lua function with a single
/// context table built by `build`, then drain the commands it queued (also
/// after a Lua error, which may have queued some first). For callbacks whose
/// ctx is not an entity ctx: GUI click and menu selection.
pub fn dispatch_custom_and_drain(
    p: &mut LuaDispatch,
    callback_name: &str,
    label: &str,
    build: impl FnOnce(&Lua) -> LuaResult<LuaTable>,
) {
    refresh_signal_cache(p);
    p.lua_runtime.call_named(callback_name, label, |func| {
        func.call::<()>(build(p.lua_runtime.lua())?)
    });
    drain_dispatch_commands(p, DrainScope::Regular);
}

/// Drain and process the `scope`'s command queues (see
/// `drain_and_process_effect_commands` for the order).
pub(crate) fn drain_dispatch_commands(p: &mut LuaDispatch, scope: DrainScope) {
    drain_and_process_effect_commands(
        &p.lua_runtime,
        scope,
        &mut p.effect_bufs,
        &mut p.commands,
        &mut SignalTargets {
            world_signals: &mut p.world_signals,
            next_scene: &mut p.next_scene,
            next_game_state: &mut p.next_game_state,
        },
        &mut p.audio_cmd_writer,
        &p.systems_store,
    );
}

/// Inserts every resource a [`LuaDispatch`] system param reads, so unit tests
/// can run a dispatching observer in a bare `World`.
#[cfg(test)]
pub(crate) fn init_dispatch_resources(world: &mut World) {
    world.init_resource::<WorldSignals>();
    world.init_resource::<NextScene>();
    world.init_resource::<NextGameState>();
    world.init_resource::<Messages<AudioCmd>>();
    world.init_resource::<InputState>();
    world.init_resource::<WorldTime>();
    world.insert_resource(SystemsStore::default());
}

/// A `World` with the dispatch resources and a `LuaRuntime`, so unit tests
/// can run a dispatching observer in a bare world.
#[cfg(test)]
pub(crate) fn init_test_world() -> World {
    let mut world = World::new();
    init_dispatch_resources(&mut world);
    world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
    world
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::lua_runtime::LuaRuntime;
    use bevy_ecs::system::RunSystemOnce;

    /// A Lua error raised inside the callback still drains the commands the
    /// callback queued before raising.
    #[test]
    fn ctx_call_and_drain_still_drains_after_a_lua_error() {
        let mut world = init_test_world();
        world
            .non_send::<LuaRuntime>()
            .lua()
            .load("function boom(ctx, extra) engine.set_flag('seen'); error('boom') end")
            .exec()
            .expect("lua load");
        let entity = world.spawn_empty().id();

        world
            .run_system_once(move |mut p: LuaDispatch| {
                dispatch_ctx_call_and_drain(&mut p, entity, "boom", "Test", |lua| {
                    lua.create_table()
                });
            })
            .expect("system runs");

        assert!(world.resource::<WorldSignals>().has_flag("seen"));
    }
}
