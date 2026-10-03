//! Aberred Engine library.
//!
//! This module exposes the engine's ECS components, resources, systems, and events
//! for use in integration tests and as a reusable library.
//!
//! [`render`] holds only the render-thread types that game callbacks receive:
//!
//! ```
//! use aberredengine::render::{FontStore, GuiCallback, GuiCtx, TextureStore};
//! ```
//!
//! Render-thread internals stay out of reach, so game code can't name a type
//! that only the render world holds:
//!
//! ```compile_fail
//! use aberredengine::render::systems::render_system;
//! ```

// Re-export engine dependencies so downstream crates need only list `aberredengine`.
pub use bevy_ecs;
pub use glam;
pub use imgui;
pub use raylib;

// Module-style re-export of the core crate: `aberredengine::core::components::...`,
// `aberredengine::core::math::Vec2`, etc.
pub use aberred_core as core;
pub use core::EngineError;

/// The render-thread types that game callbacks receive.
///
/// A GUI callback (`EngineBuilder::add_scene_gui`) gets a [`GuiCtx`](render::GuiCtx) whose `textures` and
/// `fonts` fields are a read-only [`TextureStore`](render::TextureStore) and
/// [`FontStore`](render::FontStore). Everything else in the render crate
/// belongs to the render world, which game code never touches: logic-side
/// systems load assets by writing `RenderAssetCmd`s instead.
pub mod render {
    pub use aberred_render::resources::fontstore::FontStore;
    pub use aberred_render::resources::scene_table::{GuiCallback, GuiCtx};
    pub use aberred_render::resources::texturestore::TextureStore;
}

// Module-style re-export of the Lua crate: `aberredengine::lua::resources::...`,
// etc. Mirrors the `core` re-export above. The four Lua-priority
// shadow systems (`systems::{menu,gui_interactable_click,collision_rule_index,
// mapspawn}`) are unaffected -- those paths never moved into `aberred-lua`.
#[cfg(feature = "lua")]
pub use aberred_lua as lua;

pub mod engine_app;
pub mod prelude;
pub mod systems;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
