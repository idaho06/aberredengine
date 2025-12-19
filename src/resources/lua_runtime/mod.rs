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
//! - [`runtime`] - Core Lua runtime implementation and `engine` table API
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

mod commands;
mod spawn_data;
mod entity_builder;
mod runtime;

// Re-export all public types for backwards compatibility
pub use commands::*;
pub use spawn_data::*;
pub use entity_builder::{LuaEntityBuilder, LuaCollisionEntityBuilder};
pub use runtime::LuaRuntime;
