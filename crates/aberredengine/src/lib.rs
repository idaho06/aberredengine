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

// Module-style re-export of the render crate: `aberredengine::render::resources::...`,
// etc. Mirrors the `core` re-export above (docs/plans/workspaces-implementation.md's
// Phase 3 decision record) -- downstream code using the old
// `aberredengine::resources::render::...`/`aberredengine::systems::render::...` paths
// must migrate to `aberredengine::render::...`.
pub use aberred_render as render;

// Module-style re-export of the Lua crate: `aberredengine::lua::resources::...`,
// etc. Mirrors the `core`/`render` re-exports above
// (docs/plans/workspaces-implementation.md's Phase 5 decision record) --
// downstream code using the old `aberredengine::resources::lua_runtime::...`/
// `aberredengine::systems::lua_commands::...`/`aberredengine::lua_plugin`/etc.
// paths must migrate to `aberredengine::lua::...`. The four Lua-priority
// shadow systems (`systems::{menu,gui_interactable_click,collision_rule_index,
// mapspawn}`) are unaffected -- those paths never moved.
#[cfg(feature = "lua")]
pub use aberred_lua as lua;

pub mod engine_app;
pub mod systems;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
