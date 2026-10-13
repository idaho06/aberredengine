//! Shared command processing utilities for Lua-Rust communication.
//!
//! This module provides unified command processors used by various Lua callback
//! contexts (scene setup, phase callbacks, timer callbacks, collision callbacks, etc.).
//!
//! # Sub-modules
//!
//! - `context` – `build_entity_context`: entity context table construction
//! - `dispatch` – [`LuaDispatch`]: shared entity-callback dispatch flow
//! - `entity_cmd` – [`process_entity_commands`]: runtime entity manipulation
//! - `processors` – small per-command-domain `process_*` functions
//! - `spawn_cmd` – [`process_spawn_command`], [`process_clone_command`]: entity creation
//! - `parse` – animation condition conversion helpers
//!
//! # SystemParam bundles
//!
//! - [`ContextQueries`] – read-only queries for building entity context tables
//! - [`LuaDispatch`] – bundles [`ContextQueries`] plus everything needed to
//!   call one Lua entity callback and drain its queued commands

mod context;
mod dispatch;
mod entity_cmd;
mod parse;
mod processors;
mod spawn_cmd;

pub(crate) use context::build_entity_context;
#[cfg(test)]
pub(crate) use dispatch::init_dispatch_resources;
pub use dispatch::{
    CallShape, LuaDispatch, call_entity_callback, dispatch_and_drain, dispatch_custom_and_drain,
    refresh_signal_cache,
};
pub(crate) use dispatch::{build_ctx_or_log, drain_dispatch_commands};
pub use entity_cmd::process_entity_commands;
pub(crate) use processors::request_phase;
pub use processors::{
    asset_cmd_to_audio_cmd, asset_cmd_to_render_asset_cmd, process_animation_command,
    process_audio_command, process_camera_command, process_camera_follow_command,
    process_gameconfig_command, process_group_command, process_input_command,
    process_phase_command, process_render_command, process_signal_command, translate_asset_command,
};
pub use spawn_cmd::{process_clone_command, process_spawn_command};

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::lua_on_timer_fired::LuaOnTimerFired;
use crate::components::lua_on_tween_finished::LuaOnTweenFinished;
use crate::resources::lua_runtime::{
    AudioLuaCmd, CameraCmd, CloneCmd, EntityCmd, LuaRuntime, PhaseCmd, SignalCmd, SpawnCmd,
    TweenConfig,
};
use aberred_core::components::animation::Animation;
use aberred_core::components::boxcollider::BoxCollider;
use aberred_core::components::globaltransform2d::GlobalTransform2D;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::rigidbody::RigidBody;
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::components::screenposition::ScreenPosition;
use aberred_core::components::signals::Signals;
use aberred_core::components::sprite::Sprite;
use aberred_core::components::timer::Timer;
use aberred_core::components::tween::{Tween, TweenValue};
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;

/// Persistent per-frame buffers for the 6 effect command queues drained by
/// `drain_and_process_effect_commands`.
///
/// Hold one of these in a `Local<EffectCmdBufs>` on each Bevy system that
/// calls the helper. The Vecs retain their heap capacity across frames so
/// no allocation occurs after the first warm-up frame.
#[derive(Default)]
pub struct EffectCmdBufs {
    pub(crate) signals: Vec<SignalCmd>,
    pub(crate) entities: Vec<EntityCmd>,
    // SpawnCmd is ~2KB; boxing keeps this Vec's per-element push/drain/realloc cost
    // at 8 bytes instead of a full-struct memcpy.
    #[allow(clippy::vec_box)]
    pub(crate) spawns: Vec<Box<SpawnCmd>>,
    pub(crate) clones: Vec<CloneCmd>,
    pub(crate) audios: Vec<AudioLuaCmd>,
    pub(crate) cameras: Vec<CameraCmd>,
    pub(crate) phases: Vec<PhaseCmd>,
}

/// Selects which set of command queues to drain from the Lua runtime.
#[derive(Clone, Copy)]
pub(crate) enum DrainScope {
    /// Regular queues, drained after every non-collision callback.
    Regular,
    /// Collision-scoped queues, drained by the Lua collision observers right
    /// after each callback.
    Collision,
}

/// Drain and process the 7 command queues shared by all Lua callback contexts.
///
/// Canonical order: `signal → entity → spawn → clone → audio → camera → phase`
///
/// Phase requests are deferred commands queued last, so an
/// `engine.phase_transition` lands after an `engine.entity_insert_phase` from
/// the same callback, and a phase name the callback returns, requested after
/// this drain, wins over the `phase_transition`.
///
/// `bufs` must be a caller-owned [`EffectCmdBufs`] (typically `Local<EffectCmdBufs>`).
/// The Vecs retain capacity across frames to avoid repeated allocation.
pub(crate) fn drain_and_process_effect_commands(
    lua_runtime: &LuaRuntime,
    scope: DrainScope,
    bufs: &mut EffectCmdBufs,
    commands: &mut Commands,
    world_signals: &mut WorldSignals,
    audio: &mut MessageWriter<AudioCmd>,
    systems_store: &SystemsStore,
) {
    match scope {
        DrainScope::Regular => {
            lua_runtime.drain_signal_commands_into(&mut bufs.signals);
            lua_runtime.drain_entity_commands_into(&mut bufs.entities);
            lua_runtime.drain_spawn_commands_into(&mut bufs.spawns);
            lua_runtime.drain_clone_commands_into(&mut bufs.clones);
            lua_runtime.drain_audio_commands_into(&mut bufs.audios);
            lua_runtime.drain_camera_commands_into(&mut bufs.cameras);
            lua_runtime.drain_phase_commands_into(&mut bufs.phases);
        }
        DrainScope::Collision => {
            lua_runtime.drain_collision_signal_commands_into(&mut bufs.signals);
            lua_runtime.drain_collision_entity_commands_into(&mut bufs.entities);
            lua_runtime.drain_collision_spawn_commands_into(&mut bufs.spawns);
            lua_runtime.drain_collision_clone_commands_into(&mut bufs.clones);
            lua_runtime.drain_collision_audio_commands_into(&mut bufs.audios);
            lua_runtime.drain_collision_camera_commands_into(&mut bufs.cameras);
            lua_runtime.drain_collision_phase_commands_into(&mut bufs.phases);
        }
    }

    for cmd in bufs.signals.drain(..) {
        process_signal_command(world_signals, cmd);
    }
    process_entity_commands(
        commands,
        bufs.entities.drain(..),
        world_signals,
        systems_store,
    );
    for cmd in bufs.spawns.drain(..) {
        process_spawn_command(commands, cmd, world_signals);
    }
    for cmd in bufs.clones.drain(..) {
        process_clone_command(commands, cmd, world_signals);
    }
    for cmd in bufs.audios.drain(..) {
        process_audio_command(audio, cmd);
    }
    for cmd in bufs.cameras.drain(..) {
        process_camera_command(commands, cmd);
    }
    for cmd in bufs.phases.drain(..) {
        process_phase_command(commands, cmd);
    }
}

/// Build a configured `Tween<T>` from component values and shared config.
pub(crate) fn build_tween<T: TweenValue>(from: T, to: T, config: &TweenConfig) -> Tween<T> {
    let mut tween = Tween::new(from, to, config.duration)
        .with_easing(config.easing)
        .with_loop_mode(config.loop_mode);
    if config.backwards {
        tween = tween.with_backwards();
    }
    tween
}

/// An entity command that applies a tween's "on finished" callback: it inserts
/// `LuaOnTweenFinished<T>` if `config.callback` is set, or removes it otherwise.
///
/// The removal half matters as much as the insert half: re-inserting a
/// `Tween<T>` on an entity that already has a stale `LuaOnTweenFinished<T>`
/// from a *previous* tween (e.g. a hide-tween's callback, followed later by
/// a callback-less show-tween) must clear it — otherwise the old callback
/// fires again when the new, unrelated tween finishes. Owning both halves
/// here means every call site gets this for free instead of re-deriving the
/// `Some`/`None` branch itself.
pub(crate) fn tween_finished_callback<T: TweenValue>(
    config: &TweenConfig,
) -> impl FnOnce(EntityWorldMut) + Send + 'static {
    let callback = (!config.callback.is_empty())
        .then(|| LuaOnTweenFinished::<T>::new(config.callback.as_str()));
    move |mut e| match callback {
        Some(callback) => {
            e.insert(callback);
        }
        None => {
            e.remove::<LuaOnTweenFinished<T>>();
        }
    }
}

/// Bundled read-only queries for building entity context tables.
///
/// Entity commands never write through queries (they queue deferred
/// `EntityWorldMut` work), so every component a ctx reads lives here.
#[derive(SystemParam)]
pub struct ContextQueries<'w, 's> {
    pub positions: Query<'w, 's, &'static MapPosition>,
    pub screen_positions: Query<'w, 's, &'static ScreenPosition>,
    pub rigid_bodies: Query<'w, 's, &'static RigidBody>,
    pub sprites: Query<'w, 's, &'static Sprite>,
    pub animation: Query<'w, 's, &'static Animation>,
    pub signals: Query<'w, 's, &'static Signals>,
    pub groups: Query<'w, 's, &'static aberred_core::components::group::Group>,
    pub rotations: Query<'w, 's, &'static Rotation>,
    pub scales: Query<'w, 's, &'static Scale>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub lua_timers: Query<'w, 's, (&'static Timer, &'static LuaOnTimerFired)>,
    pub global_transforms: Query<'w, 's, &'static GlobalTransform2D>,
    pub child_of: Query<'w, 's, &'static ChildOf>,
}
