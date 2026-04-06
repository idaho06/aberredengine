//! Engine systems.
//!
//! This module groups all ECS systems that advance simulation, input, and
//! rendering.
//!
//! Submodules overview
//! - [`animation`] – advance sprite animations and select tracks via rules
//! - [`camera_follow`] – move the camera to track entities with `CameraTarget`
//! - [`audio`] – bridge with the audio thread (poll/update message queues)
//! - [`collision_detector`] – broad/simple overlap checks and event emission
//! - [`lua_collision`] – *(feature = "lua")* Lua-based collision observer and callback dispatch
//! - [`gamestate`] – check for pending state transitions and trigger events
//! - [`gridlayout`] – spawn entities from JSON-defined grid layouts
//! - [`group`] – count entities per tracked group and publish to [`WorldSignals`](crate::resources::worldsignals::WorldSignals)
//! - [`input`] – read hardware input and update [`crate::resources::input::InputState`]
//! - [`inputsimplecontroller`] – translate input state into velocity on entities
//! - [`inputaccelerationcontroller`] – translate input state into acceleration on entities
//! - [`lua_commands`] – *(feature = "lua")* shared command processing for Lua-Rust communication
//! - [`menu`] – menu spawning, input handling, and selection
//! - [`mousecontroller`] – update entity positions based on mouse position
//! - [`movement`] – integrate positions from rigid body velocities and time
//! - [`luaphase`] – *(feature = "lua")* process Lua phase state machine transitions and callbacks
//! - [`phase`] – process Rust phase state machine transitions and callbacks
//! - [`rust_collision`] – Rust-native collision observer and callback dispatch
//! - [`scene_dispatch`] – scene switch and update systems for `SceneManager`-based games
//! - [`render`] – draw world and debug overlays using Raylib
//! - [`signalbinding`] – update DynamicText components based on signal values
//! - [`stuckto`] – keep entities attached to other entities
//! - [`time`] – update simulation time and delta
//! - [`tween`] – animate position, rotation, and scale over time

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

pub use game_ctx::GameCtx;

/// Bundled Raylib handle + thread to reduce system parameter count.
#[derive(SystemParam)]
pub struct RaylibAccess<'w> {
    pub rl: NonSendMut<'w, raylib::RaylibHandle>,
    pub th: NonSend<'w, raylib::RaylibThread>,
}

pub mod animation;
pub mod audio;
pub mod camera_follow;
pub mod collision;
pub mod collision_detector;
pub mod dynamictext_size;
pub mod game_ctx;
pub mod gameconfig;
pub mod gamestate;
pub mod gridlayout;
pub mod group;
pub mod input;
pub mod inputaccelerationcontroller;
pub mod inputsimplecontroller;
#[cfg(feature = "lua")]
pub mod lua_collision;
#[cfg(feature = "lua")]
pub mod lua_commands;
#[cfg(feature = "lua")]
pub mod luaphase;
#[cfg(feature = "lua")]
pub mod luatimer;
pub mod mapspawn;
pub mod menu;
pub mod mousecontroller;
pub mod movement;
pub mod particleemitter;
pub mod phase;
mod phase_core;
pub mod propagate_transforms;
pub mod render;
pub mod rust_collision;
pub mod scene_dispatch;
pub mod signalbinding;
pub mod stuckto;
pub mod tilemap;
pub mod time;
pub mod timer;
mod timer_core;
pub mod ttl;
pub mod tween;
