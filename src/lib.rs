//! Aberred Engine library.
//!
//! This module exposes the engine's ECS components, resources, systems, and events
//! for use in integration tests and as a reusable library.

pub mod components;
pub mod events;
#[cfg(feature = "lua")]
pub mod lua_plugin;
pub mod resources;
#[cfg(feature = "lua")]
pub mod luarc_generator;
#[cfg(feature = "lua")]
pub mod stub_generator;
pub mod systems;
