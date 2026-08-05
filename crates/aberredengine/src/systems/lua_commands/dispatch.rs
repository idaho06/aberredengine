//! Shared entity-callback dispatch flow.
//!
//! Bundles the params and steps previously duplicated across
//! `lua_timer_observer`, `lua_setup_entity_system`,
//! `lua_animation_finished_observer`, and `lua_tween_finished_observer`: sync
//! the signal cache, build the input table (if the call shape wants one),
//! resolve the entity context (optionally including phase state), call the
//! named Lua function, then drain the commands it queued.
//!
//! `lua_phase_system` (`crate::systems::luaphase`) does NOT use this helper —
//! it must interleave `apply_callback_transitions` between the phase drain
//! and the effect drain and builds its input table once for many entities,
//! so it keeps its own `drain_and_process_phase_commands` /
//! `drain_and_process_effect_commands` calls.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use log::error;

use crate::components::luaphase::LuaPhase;
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::input::InputState;
use crate::resources::lua_runtime::{LuaPhaseSnapshot, LuaRuntime, PhaseCmd};
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
    pub luaphase_query: Query<'w, 's, (Entity, &'static mut LuaPhase)>,
    pub world_signals: ResMut<'w, WorldSignals>,
    pub lua_runtime: NonSend<'w, LuaRuntime>,
    pub audio_cmd_writer: MessageWriter<'w, AudioCmd>,
    pub systems_store: Res<'w, SystemsStore>,
    pub animation_store: Res<'w, AnimationStore>,
    pub phase_buf: Local<'s, Vec<PhaseCmd>>,
    pub effect_bufs: Local<'s, EffectCmdBufs>,
}

/// The two call shapes every dispatch site needs. The phase snapshot and the
/// input table are always resolved together — no call site has ever needed
/// them independently, so this is one axis, not two.
#[derive(Clone, Copy)]
pub enum CallShape {
    /// Resolve `LuaPhaseSnapshot`, build the input table, call `(ctx, input)`
    /// — timer/anim/tween path.
    CtxAndInput,
    /// Skip the phase snapshot, no input table; call `(ctx)` — `lua_setup` path.
    CtxOnly,
}

/// Sync the Lua signal cache from `WorldSignals`. Call once per system
/// invocation (not per entity — see `lua_setup_entity_system`, which loops
/// many entities per invocation but only wants this done once, up front).
pub fn refresh_signal_cache(p: &mut LuaDispatch) {
    p.lua_runtime
        .update_signal_cache(p.world_signals.snapshot());
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
    let input_table = match call_shape {
        CallShape::CtxAndInput => {
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
                    return false;
                }
            }
        }
        CallShape::CtxOnly => None,
    };

    let lua_phase_snapshot = match call_shape {
        CallShape::CtxAndInput => p
            .luaphase_query
            .get(entity)
            .ok()
            .map(|(_, phase)| LuaPhaseSnapshot::from(phase)),
        CallShape::CtxOnly => None,
    };

    let ctx_table = match build_entity_context(
        &p.lua_runtime,
        entity,
        &p.ctx_queries,
        &p.cmd_queries,
        lua_phase_snapshot,
        None,
    ) {
        Ok(ctx) => ctx,
        Err(e) => {
            error!(
                target: "lua",
                "{label}: error building context for {:?}: {}",
                entity, e
            );
            return false;
        }
    };

    match input_table {
        Some(input_table) => {
            p.lua_runtime.call_named(callback_name, label, |func| {
                func.call::<()>((ctx_table, input_table))
            });
        }
        None => {
            p.lua_runtime
                .call_named(callback_name, label, |func| func.call::<()>(ctx_table));
        }
    }

    true
}

/// Refresh the signal cache, call the named entity callback with
/// `(ctx, input)`, and drain its queued commands if the call happened.
/// Covers the shape shared by every single-entity-per-invocation observer
/// (timer, on_animation_end, on_tween_finished) — `lua_setup_entity_system`
/// still calls `refresh_signal_cache`/`call_entity_callback`/
/// `drain_dispatch_commands` directly, since it batches one refresh and one
/// drain across many entities per invocation instead of one each.
pub fn dispatch_and_drain(p: &mut LuaDispatch, entity: Entity, callback_name: &str, label: &str) {
    refresh_signal_cache(p);
    if call_entity_callback(p, entity, callback_name, label, CallShape::CtxAndInput) {
        drain_dispatch_commands(p);
    }
}

/// Drain the phase queue, then the 6 regular effect queues, in the canonical
/// order. The one-and-only surviving caller of the flow previously exposed
/// as `drain_phase_and_effects`.
pub fn drain_dispatch_commands(p: &mut LuaDispatch) {
    drain_and_process_phase_commands(&p.lua_runtime, &mut p.phase_buf, &mut p.luaphase_query);
    drain_and_process_effect_commands(
        &p.lua_runtime,
        DrainScope::Regular,
        &mut p.effect_bufs,
        &mut p.commands,
        &mut p.world_signals,
        &mut p.cmd_queries,
        &mut p.audio_cmd_writer,
        &p.systems_store,
        &p.animation_store,
    );
}
