//! Aberred Engine core library.
//!
//! Thread-agnostic, render/audio/Lua-free gameplay logic: ECS components,
//! resources, systems, events, and the cross-thread wire-format contract
//! (`protocol`). No `raylib`, `mlua`, or `imgui` anywhere in this crate's
//! dependency tree — that is this crate's whole reason for existing (see
//! `docs/plans/workspaces-implementation.md`). Render-only, audio-only, and
//! Lua-only code lives in sibling crates / the `aberredengine` facade.

pub use bevy_ecs;
pub use glam;

pub mod components;
pub mod error;
pub use error::EngineError;
pub mod events;
pub mod math;
pub mod pacing;
pub mod protocol;
pub mod resources;
pub mod systems;
pub mod tracy;
