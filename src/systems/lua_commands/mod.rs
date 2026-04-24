//! Shared command processing utilities for Lua-Rust communication.
//!
//! This module provides unified command processors used by various Lua callback
//! contexts (scene setup, phase callbacks, timer callbacks, collision callbacks, etc.).
//!
//! # Sub-modules
//!
//! - [`context`] – [`build_entity_context`]: entity context table construction
//! - [`entity_cmd`] – [`process_entity_commands`]: runtime entity manipulation
//! - [`processors`] – small per-command-domain `process_*` functions
//! - [`spawn_cmd`] – [`process_spawn_command`], [`process_clone_command`]: entity creation
//! - [`parse`] – animation condition conversion helpers
//!
//! # SystemParam bundles
//!
//! - [`EntityCmdQueries`] – mutable queries needed by `process_entity_commands`
//! - [`ContextQueries`] – read-only queries for building entity context tables

mod context;
mod entity_cmd;
mod parse;
mod processors;
mod spawn_cmd;

pub(crate) use context::build_entity_context;
pub use entity_cmd::process_entity_commands;
pub use processors::{
    process_animation_command, process_asset_command, process_audio_command,
    process_camera_command, process_camera_follow_command, process_gameconfig_command,
    process_group_command, process_input_command, process_phase_command,
    process_render_command, process_signal_command,
};
pub use spawn_cmd::{process_clone_command, process_spawn_command};

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::cameratarget::CameraTarget;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::luatimer::LuaTimer;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::components::tween::{Easing, LoopMode, Tween, TweenValue};
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::AnimationStore;
use crate::resources::lua_runtime::{LuaRuntime, TweenConfig};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;

/// Selects which set of command queues to drain from the Lua runtime.
pub(crate) enum DrainScope {
    /// Regular queues used by update, switch_scene, timer, and phase systems.
    Regular,
    /// Collision-scoped queues used by the collision observer.
    Collision,
}

/// Drain and process the 6 effect queues shared by all Lua callback contexts.
///
/// Canonical order: `signal → entity → spawn → clone → audio → camera`
///
/// Phase is intentionally excluded so callers can preserve their required
/// phase boundary (e.g. `apply_callback_transitions` in `lua_phase_system`)
/// before invoking this helper.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drain_and_process_effect_commands(
    lua_runtime: &LuaRuntime,
    scope: DrainScope,
    commands: &mut Commands,
    world_signals: &mut WorldSignals,
    cmd_queries: &mut EntityCmdQueries,
    audio: &mut MessageWriter<AudioCmd>,
    systems_store: &SystemsStore,
    animation_store: &AnimationStore,
) {
    let (signals, entities, spawns, clones, audios, cameras) = match scope {
        DrainScope::Regular => (
            lua_runtime.drain_signal_commands(),
            lua_runtime.drain_entity_commands(),
            lua_runtime.drain_spawn_commands(),
            lua_runtime.drain_clone_commands(),
            lua_runtime.drain_audio_commands(),
            lua_runtime.drain_camera_commands(),
        ),
        DrainScope::Collision => (
            lua_runtime.drain_collision_signal_commands(),
            lua_runtime.drain_collision_entity_commands(),
            lua_runtime.drain_collision_spawn_commands(),
            lua_runtime.drain_collision_clone_commands(),
            lua_runtime.drain_collision_audio_commands(),
            lua_runtime.drain_collision_camera_commands(),
        ),
    };

    for cmd in signals {
        process_signal_command(world_signals, cmd);
    }
    process_entity_commands(commands, entities, cmd_queries, systems_store, animation_store);
    for cmd in spawns {
        process_spawn_command(commands, cmd, world_signals);
    }
    for cmd in clones {
        process_clone_command(commands, cmd, world_signals);
    }
    for cmd in audios {
        process_audio_command(audio, cmd);
    }
    for cmd in cameras {
        process_camera_command(commands, cmd);
    }
}

/// Build a configured `Tween<T>` from component values and shared config.
pub(crate) fn build_tween<T: TweenValue>(from: T, to: T, config: &TweenConfig) -> Tween<T> {
    let easing = config.easing.parse::<Easing>().unwrap();
    let loop_mode = config.loop_mode.parse::<LoopMode>().unwrap();
    let mut tween = Tween::new(from, to, config.duration)
        .with_easing(easing)
        .with_loop_mode(loop_mode);
    if config.backwards {
        tween = tween.with_backwards();
    }
    tween
}

/// Mutable queries required by [`process_entity_commands`].
///
/// Embed this in any system or SystemParam that needs to call
/// `process_entity_commands`, and pass `&mut entity_cmd_queries` directly.
#[derive(SystemParam)]
pub struct EntityCmdQueries<'w, 's> {
    pub stuckto: Query<'w, 's, &'static StuckTo>,
    pub signals: Query<'w, 's, &'static mut Signals>,
    pub animation: Query<'w, 's, &'static mut Animation>,
    pub rigid_bodies: Query<'w, 's, &'static mut RigidBody>,
    pub positions: Query<'w, 's, &'static mut MapPosition>,
    pub screen_positions: Query<'w, 's, &'static mut ScreenPosition>,
    pub sprites: Query<'w, 's, &'static mut Sprite>,
    pub shaders: Query<'w, 's, &'static mut EntityShader>,
    pub global_transforms: Query<'w, 's, &'static GlobalTransform2D>,
    pub camera_targets: Query<'w, 's, &'static mut CameraTarget>,
}

/// Bundled read-only queries for building entity context tables.
///
/// This SystemParam includes read-only components that can be shared by systems
/// that also hold mutable command-processing queries.
#[derive(SystemParam)]
pub struct ContextQueries<'w, 's> {
    pub groups: Query<'w, 's, &'static crate::components::group::Group>,
    pub rotations: Query<'w, 's, &'static Rotation>,
    pub scales: Query<'w, 's, &'static Scale>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub lua_timers: Query<'w, 's, &'static LuaTimer>,
    pub global_transforms: Query<'w, 's, &'static GlobalTransform2D>,
    pub child_of: Query<'w, 's, &'static ChildOf>,
}
