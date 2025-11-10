//! Engine systems.
//!
//! This module groups all ECS systems that advance simulation, input, and
//! rendering.
//!
//! Submodules overview
//! - `animation` – advance sprite animations and select tracks via rules
//! - `audio` – bridge with the audio thread (poll/update message queues)
//! - `collision` – broad/simple overlap checks and event emission
//! - `gamestate` – check for pending state transitions and trigger events
//! - `input` – read hardware input and update [`crate::resources::input::InputState`]
//! - `inputsimplecontroller` – translate input state into velocity on entities
//! - `movement` – integrate positions from rigid body velocities and time
//! - `render` – draw world and debug overlays using Raylib
//! - `time` – update simulation time and delta
pub mod animation;
pub mod audio;
pub mod collision;
pub mod gamestate;
pub mod input;
pub mod inputsimplecontroller;
pub mod menu;
pub mod movement;
pub mod render;
pub mod time;
pub mod tween;
