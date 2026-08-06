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
// `aberredengine::core::math::Vec2`, etc.
pub use aberred_core as core;
pub use core::EngineError;

// Module-style re-export of the render crate: `aberredengine::render::resources::...`,
// etc. Mirrors the `core` re-export above.
pub use aberred_render as render;

// Module-style re-export of the Lua crate: `aberredengine::lua::resources::...`,
// etc. Mirrors the `core`/`render` re-exports above. The four Lua-priority
// shadow systems (`systems::{menu,gui_interactable_click,collision_rule_index,
// mapspawn}`) are unaffected -- those paths never moved into `aberred-lua`.
#[cfg(feature = "lua")]
pub use aberred_lua as lua;

pub mod engine_app;
pub mod systems;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
