//! Aberred Engine library.
//!
//! This module exposes the engine's ECS components, resources, systems, and events
//! for use in integration tests and as a reusable library.

// Re-export engine dependencies so downstream crates need only list `aberredengine`.
pub use bevy_ecs;
pub use glam;
pub use imgui;
pub use raylib;

// Module-style re-export of the core crate: `aberredengine::core::components::...`,
// `aberredengine::core::math::Vec2`, etc. See
// docs/plans/workspaces-implementation.md's Phase 2 decision record --
// downstream code using the old `aberredengine::systems::...`/
// `aberredengine::components::...` paths for now-core items must migrate to
// `aberredengine::core::...`.
pub use aberred_core as core;
pub use core::EngineError;

pub mod components;
pub mod engine_app;
pub mod events;
#[cfg(feature = "lua")]
pub mod lua_plugin;
#[cfg(feature = "lua")]
pub mod luarc_generator;
pub mod resources;
#[cfg(feature = "lua")]
pub mod stub_generator;
pub mod systems;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
