//! Engine systems.
//!
//! This module groups all ECS systems that advance simulation, input, and
//! rendering.
//!
//! Submodules overview
//! - [`animation`] – advance sprite animations and select tracks via rules
//! - [`audio`] – bridge with the audio thread (poll/update message queues)
//! - [`collision`] – broad/simple overlap checks and event emission
//! - [`gamestate`] – check for pending state transitions and trigger events
//! - [`gridlayout`] – spawn entities from JSON-defined grid layouts
//! - [`group`] – count entities per tracked group and publish to [`WorldSignals`](crate::resources::worldsignals::WorldSignals)
//! - [`input`] – read hardware input and update [`crate::resources::input::InputState`]
//! - [`inputsimplecontroller`] – translate input state into velocity on entities
//! - [`lua_commands`] – shared command processing for Lua-Rust communication
//! - [`menu`] – menu spawning, input handling, and selection
//! - [`mousecontroller`] – update entity positions based on mouse position
//! - [`movement`] – integrate positions from rigid body velocities and time
//! - [`phase`] – process phase state machine transitions and callbacks
//! - [`render`] – draw world and debug overlays using Raylib
//! - [`signalbinding`] – update DynamicText components based on signal values
//! - [`stuckto`] – keep entities attached to other entities
//! - [`time`] – update simulation time and delta, process timers
//! - [`tween`] – animate position, rotation, and scale over time

pub mod animation;
pub mod audio;
pub mod collision;
pub mod dynamictext_size;
pub mod gamestate;
pub mod gridlayout;
pub mod group;
pub mod input;
pub mod inputsimplecontroller;
pub mod lua_commands;
pub mod luaphase;
pub mod luatimer;
pub mod menu;
pub mod mousecontroller;
pub mod movement;
pub mod phase;
pub mod render;
pub mod signalbinding;
pub mod stuckto;
pub mod time;
pub mod tween;
