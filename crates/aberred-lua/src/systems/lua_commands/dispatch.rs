//! Shared callback dispatch flow.
//!
//! Bundles the params and steps shared by `lua_timer_fired_observer`,
//! `lua_setup_entity_system`, `lua_animation_finished_observer`,
//! `lua_tween_finished_observer`, `lua_gui_interactable_click_observer`,
//! `lua_menu_selection_observer`, the Lua phase enter/exit observers, and the
//! Lua collision observers: sync the signal cache, build the callback's
//! arguments (an entity context and optional input table, or a custom context
//! table), call the named Lua function, then drain the commands it queued (the
//! collision observers drain the collision-scoped queues).
//!
//! `lua_phase_update_system` (`crate::systems::lua_phase`) batches like
//! `lua_setup_entity_system` (one signal refresh, one drain per run) but
//! passes one shared input table and `dt`, so it builds ctx with
//! [`build_ctx_or_log`] and calls Lua itself instead of going through a
//! [`CallShape`].

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use log::error;
use mlua::prelude::{Lua, LuaResult, LuaTable, LuaValue};

use crate::resources::lua_runtime::{LuaPhaseSnapshot, LuaRuntime, PhaseCmd};
use aberred_core::components::phase::Phase;
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::input::InputState;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_core::resources::worldtime::WorldTime;

use super::{
    ContextQueries, DrainScope, EffectCmdBufs, EntityCmdQueries, build_entity_context,
    drain_and_process_effect_commands, drain_and_process_phase_commands,
};

/// Everything needed to dispatch one Lua entity callback and drain the
/// commands it queued.
#[derive(SystemParam)]
pub struct LuaDispatch<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub input: Res<'w, InputState>,
    pub time: Res<'w, WorldTime>,
    pub ctx_queries: ContextQueries<'w, 's>,
    pub cmd_queries: EntityCmdQueries<'w, 's>,
    pub phase_query: Query<'w, 's, &'static mut Phase>,
    pub world_signals: ResMut<'w, WorldSignals>,
    pub lua_runtime: NonSend<'w, LuaRuntime>,
    pub audio_cmd_writer: MessageWriter<'w, AudioCmd>,
    pub systems_store: Res<'w, SystemsStore>,
    pub animation_store: Res<'w, AnimationStore>,
    pub phase_buf: Local<'s, Vec<PhaseCmd>>,
    pub effect_bufs: Local<'s, EffectCmdBufs>,
}

/// The call shapes the dispatch sites need: whether the ctx carries the
/// phase fields ([`CallShape::wants_phase`]) and whether an input table is
/// passed ([`CallShape::wants_input`]) are independent choices.
#[derive(Clone, Copy)]
pub enum CallShape {
    /// Resolve the phase snapshot, build the input table, call `(ctx, input)`
    /// — timer/anim/tween/phase-enter path.
    CtxAndInput,
    /// Resolve the phase snapshot, no input table; call `(ctx)` — phase-exit path.
    CtxWithPhase,
    /// Skip the phase snapshot, no input table; call `(ctx)` — `lua_setup` path.
    CtxOnly,
}

impl CallShape {
    /// Whether the callback gets an input table as its second argument.
    pub fn wants_input(self) -> bool {
        matches!(self, CallShape::CtxAndInput)
    }

    /// Whether the ctx carries `phase` / `time_in_phase`.
    pub fn wants_phase(self) -> bool {
        !matches!(self, CallShape::CtxOnly)
    }
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
    call_entity_callback_value(p, entity, callback_name, label, call_shape, None).is_some()
}

/// [`call_entity_callback`] with a `ctx.previous_phase`, returning the
/// callback's return value: `None` if input-table or ctx construction failed
/// (already logged), `Some(Nil)` if the function is missing or errored
/// (also logged).
fn call_entity_callback_value(
    p: &mut LuaDispatch,
    entity: Entity,
    callback_name: &str,
    label: &str,
    call_shape: CallShape,
    previous_phase: Option<&str>,
) -> Option<LuaValue> {
    let input_table = if call_shape.wants_input() {
        match p
            .lua_runtime
            .resolve_input_table(&p.input, p.time.frame_count)
        {
            Ok(table) => Some(table),
            Err(e) => {
                error!(
                    target: "lua",
                    "{label}: error creating input table for {:?}: {}",
                    entity, e
                );
                return None;
            }
        }
    } else {
        None
    };

    let lua_phase_snapshot = if call_shape.wants_phase() {
        p.phase_query.get(entity).ok().map(LuaPhaseSnapshot::from)
    } else {
        None
    };

    let ctx_table = build_ctx_or_log(p, entity, label, lua_phase_snapshot, previous_phase)?;

    let result = match input_table {
        Some(input_table) => p.lua_runtime.call_named(callback_name, label, |func| {
            func.call::<LuaValue>((ctx_table, input_table))
        }),
        None => p.lua_runtime.call_named(callback_name, label, |func| {
            func.call::<LuaValue>(ctx_table)
        }),
    };

    Some(result.unwrap_or(LuaValue::Nil))
}

/// Refresh the signal cache, call the named entity callback with
/// `(ctx, input)`, and drain its queued commands if the call happened.
/// Covers the shape shared by every single-entity-per-invocation observer
/// (timer, on_animation_end, on_tween_finished) — `lua_setup_entity_system`
/// still calls `refresh_signal_cache`/`call_entity_callback`/
/// `drain_dispatch_commands` directly, since it batches one refresh and one
/// drain across many entities per invocation instead of one each.
pub fn dispatch_and_drain(p: &mut LuaDispatch, entity: Entity, callback_name: &str, label: &str) {
    dispatch_and_drain_returning(
        p,
        entity,
        callback_name,
        label,
        CallShape::CtxAndInput,
        None,
    );
}

/// Build `entity`'s ctx table from the dispatch queries, logging (with
/// `label`) and returning `None` on failure.
pub(crate) fn build_ctx_or_log(
    p: &LuaDispatch,
    entity: Entity,
    label: &str,
    lua_phase: Option<LuaPhaseSnapshot<'_>>,
    previous_phase: Option<&str>,
) -> Option<LuaTable> {
    match build_entity_context(
        &p.lua_runtime,
        entity,
        &p.ctx_queries,
        &p.cmd_queries,
        lua_phase,
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

/// [`dispatch_and_drain`] with a chosen [`CallShape`] and `ctx.previous_phase`
/// (phase callbacks). Hands the callback's return value back after the drain,
/// so a phase it returns can be applied after (and so beat) any
/// `engine.phase_transition` the callback queued. `None` if the call did not
/// happen (ctx or input construction failed).
pub fn dispatch_and_drain_returning(
    p: &mut LuaDispatch,
    entity: Entity,
    callback_name: &str,
    label: &str,
    call_shape: CallShape,
    previous_phase: Option<&str>,
) -> Option<LuaValue> {
    refresh_signal_cache(p);
    let result =
        call_entity_callback_value(p, entity, callback_name, label, call_shape, previous_phase)?;
    drain_dispatch_commands(p, DrainScope::Regular);
    Some(result)
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

/// Drain the `scope`'s phase queue, then its 6 effect queues, in the
/// canonical order.
pub(crate) fn drain_dispatch_commands(p: &mut LuaDispatch, scope: DrainScope) {
    drain_and_process_phase_commands(&p.lua_runtime, scope, &mut p.phase_buf, &mut p.phase_query);
    drain_and_process_effect_commands(
        &p.lua_runtime,
        scope,
        &mut p.effect_bufs,
        &mut p.commands,
        &mut p.world_signals,
        &mut p.cmd_queries,
        &mut p.audio_cmd_writer,
        &p.systems_store,
        &p.animation_store,
    );
}

/// Inserts every resource a [`LuaDispatch`] system param reads, so unit tests
/// can run a dispatching observer in a bare `World`.
#[cfg(test)]
pub(crate) fn init_dispatch_resources(world: &mut World) {
    world.init_resource::<WorldSignals>();
    world.init_resource::<Messages<AudioCmd>>();
    world.init_resource::<InputState>();
    world.init_resource::<WorldTime>();
    world.init_resource::<AnimationStore>();
    world.insert_resource(SystemsStore::default());
}
