//! Aberred Engine library.
//!
//! This module exposes the engine's ECS components, resources, systems, and events
//! for use in integration tests and as a reusable library.

// Re-export engine dependencies so downstream crates need only list `aberredengine`.
pub use bevy_ecs;
pub use imgui;
pub use raylib;

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
