//! Lua observer for tween-finished events.
//!
//! When a [`TweenFinishedEvent<T>`] fires and the entity has a matching
//! [`LuaOnTweenFinished<T>`] component, this observer calls the named Lua
//! function with `(ctx, input)` — the same signature as timer, phase, and
//! `on_animation_end` callbacks.
//!
//! Entities without `LuaOnTweenFinished<T>` are silently skipped. Register
//! one monomorphized instance of this observer per tweened type
//! (`MapPosition`, `Rotation`, `Scale`, `ScreenPosition`).
//!
//! # Lua callback signature
//!
//! ```lua
//! function on_window_hidden(ctx, input)
//!     engine.entity_remove_screen_position(ctx.id)
//! end
//! ```

use bevy_ecs::prelude::*;

use crate::components::lua_on_tween_finished::LuaOnTweenFinished;
use crate::components::luaphase::LuaPhase;
use crate::components::tween::TweenValue;
use crate::events::audio::AudioCmd;
use crate::events::tween::TweenFinishedEvent;
use crate::resources::animationstore::AnimationStore;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::{InputSnapshot, LuaPhaseSnapshot, LuaRuntime, PhaseCmd};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    ContextQueries, EffectCmdBufs, EntityCmdQueries, build_entity_context,
    drain_phase_and_effects,
};
use log::error;

/// Observer that calls a Lua function when a `Tween<T>` finishes.
#[allow(clippy::too_many_arguments)]
pub fn lua_tween_finished_observer<T: TweenValue>(
    trigger: On<TweenFinishedEvent<T>>,
    mut commands: Commands,
    input: Res<InputState>,
    time: Res<WorldTime>,
    on_finished_query: Query<&LuaOnTweenFinished<T>>,
    ctx_queries: ContextQueries,
    mut cmd_queries: EntityCmdQueries,
    mut luaphase_query: Query<(Entity, &mut LuaPhase)>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    systems_store: Res<SystemsStore>,
    animation_store: Res<AnimationStore>,
    mut phase_buf: Local<Vec<PhaseCmd>>,
    mut effect_bufs: Local<EffectCmdBufs>,
) {
    let entity = trigger.event().entity;

    // Only proceed if the entity opted in with LuaOnTweenFinished<T>.
    let callback_name = match on_finished_query.get(entity) {
        Ok(c) => c.callback.clone(),
        Err(_) => return,
    };

    lua_runtime.update_signal_cache(world_signals.snapshot());

    let input_snapshot = InputSnapshot::from_input_state(&input);
    let input_table = match lua_runtime.update_input_table(&input_snapshot, time.frame_count) {
        Ok(t) => t,
        Err(e) => {
            error!(
                "Error creating input table for on_tween_finished callback: {}",
                e
            );
            return;
        }
    };

    let lua_phase_snapshot = luaphase_query
        .get(entity)
        .ok()
        .map(|(_, p)| LuaPhaseSnapshot::from(p));

    let ctx_table = match build_entity_context(
        &lua_runtime,
        entity,
        &ctx_queries,
        &cmd_queries,
        lua_phase_snapshot,
        None,
    ) {
        Ok(ctx) => ctx,
        Err(e) => {
            error!(
                "Error building context for on_tween_finished callback: {}",
                e
            );
            return;
        }
    };

    lua_runtime.call_named(&callback_name, "on_tween_finished", |func| {
        func.call::<()>((ctx_table, input_table))
    });

    drain_phase_and_effects(
        &lua_runtime,
        &mut phase_buf,
        &mut luaphase_query,
        &mut effect_bufs,
        &mut commands,
        &mut world_signals,
        &mut cmd_queries,
        &mut audio_cmd_writer,
        &systems_store,
        &animation_store,
    );
}
