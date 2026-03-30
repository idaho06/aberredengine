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
    process_render_command, process_signal_command, process_tilemap_command,
};
pub use spawn_cmd::{process_clone_command, process_spawn_command};

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
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
use crate::resources::lua_runtime::TweenConfig;

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
