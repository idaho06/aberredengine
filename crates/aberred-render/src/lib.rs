//! Render (main) thread implementation for Aberred Engine.
//!
//! This crate owns everything the render thread's `bevy_ecs::World`
//! exclusively touches: raylib's window/GL handles, Dear ImGui, and the
//! retained mirror entities reconciled write-only from each tick's
//! `DrawableSnapshot`. Nothing here is ever inserted into or read from the
//! sim/logic world -- crossing that boundary happens only through
//! `aberred_core::protocol` message/snapshot types.
//!
//! [`bootstrap`] builds the render `World` and its per-frame `Schedule`
//! (`setup_render_world`/`build_render_schedule`) plus the raylib window
//! itself (`setup_window`) -- the facade's `engine_app::run` calls these as
//! free functions rather than `EngineBuilder` methods, since a multi-file
//! `impl` block can't span a crate boundary.

pub mod bootstrap;
pub mod components;
pub mod events;
pub mod logging;
pub mod resources;
pub mod systems;
