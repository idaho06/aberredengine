//! Lua scripting runtime for Aberred Engine.
//!
//! This module provides the Lua integration layer, exposing game engine
//! functionality through the global `engine` table in Lua scripts.
//!
//! # Architecture
//!
//! The module is split into focused submodules:
//!
//! - [`commands`] - Command enums for Lua-Rust communication
//! - [`spawn_data`] - Component data structures for entity spawning
//! - [`entity_builder`] - Fluent builder interface for entity construction
//! - [`runtime`] - Core `LuaRuntime` struct, struct definitions, and utility methods
//! - [`engine_api`] - `engine` table API registration (all `register_*_api` methods)
//! - [`command_queues`] - Command queue draining and cache update methods
//! - [`stub_meta`] - `engine.__meta` stub metadata for IDE/tooling support
//!
//! # Example
//!
//! ```lua
//! -- From a Lua script
//! engine.log("Hello from Lua!")
//! engine.load_texture("ball", "assets/textures/ball.png")
//! engine.load_font("arcade", "assets/fonts/Arcade.ttf", 128)
//! engine.load_music("menu", "assets/audio/menu.xm")
//! engine.load_sound("ping", "assets/audio/ping.wav")
//!
//! -- Spawning entities
//! engine.spawn()
//!     :with_group("player")
//!     :with_position(400, 700)
//!     :with_sprite("vaus", 48, 12, 24, 6)
//!     :with_zindex(10)
//!     :build()
//! ```

mod command_queues;
mod commands;
mod context;
mod engine_api;
mod entity_builder;
mod input_snapshot;
mod runtime;
mod spawn_data;
mod stub_meta;

// Re-export all public types for backwards compatibility
pub use commands::*;
pub use context::{
    AnimationSnapshot, EntitySnapshot, LuaPhaseSnapshot, LuaTimerSnapshot, RigidBodySnapshot,
    SpriteSnapshot, build_entity_context_pooled,
};
pub(crate) use context::populate_entity_signals;
// pub use entity_builder::{LuaCollisionEntityBuilder, LuaEntityBuilder};
pub use input_snapshot::InputSnapshot;
pub use runtime::{LuaRuntime, action_from_str};
pub use spawn_data::*;
