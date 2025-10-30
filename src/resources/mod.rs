//! ECS resources made available to systems.
//!
//! This module groups the long-lived data injected into the ECS world and
//! accessed by systems during execution: input state, timing, rendering
//! handles, asset stores, and utilities. Each submodule documents the
//! semantics and intended usage of its resource(s).
//!
//! Overview
//! - `animationstore` – definitions for sprite animations reused across entities
//! - `audio` – bridge and channels for the background audio thread
//! - `camera2d` – shared 2D camera used for world/screen transforms
//! - `debugmode` – presence toggles optional debug overlays and logs
//! - `gamestate` – authoritative and pending high-level game state
//! - `input` – per-frame keyboard state of keys relevant to the game
//! - `screensize` – current framebuffer dimensions in pixels
//! - `systemsstore` – registry of dynamically-lookup-able systems by name
//! - `texturestore` – loaded textures keyed by string IDs
//! - `tilemapstore` – loaded tile maps and layers
//! - `worldtime` – simulation time and delta
pub mod animationstore;
pub mod audio;
pub mod camera2d;
pub mod debugmode;
pub mod fontstore;
pub mod gamestate;
pub mod input;
pub mod screensize;
pub mod systemsstore;
pub mod texturestore;
pub mod tilemapstore;
pub mod worldsignals;
pub mod worldtime;
