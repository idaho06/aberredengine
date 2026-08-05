//! Audio thread implementation for Aberred Engine.
//!
//! This crate hosts the background audio thread and its own `bevy_ecs::World`
//! (see [`systems`]), plus the components/resources that world uses
//! internally ([`components`], [`resources`]). Nothing here is ever
//! inserted into or read from the sim/logic world -- the only contract
//! between the audio thread and the rest of the engine is
//! `AudioCmd`/`AudioMessage` (`aberred_core::protocol::audio`).
//!
//! Depends on `aberred-core` for the wire protocol, pacing, and shutdown
//! signaling, and on Raylib for the audio device itself -- `aberred-core`
//! cannot depend on Raylib, which is why this is a separate leaf crate
//! rather than living in core.

pub mod components;
pub mod resources;
pub mod systems;
