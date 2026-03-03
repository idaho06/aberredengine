//! ECS resources made available to systems.
//!
//! This module groups the long-lived data injected into the ECS world and
//! accessed by systems during execution: input state, timing, rendering
//! handles, asset stores, and utilities. Each submodule documents the
//! semantics and intended usage of its resource(s).
//!
//! Overview
//! - [`animationstore`] – definitions for sprite animations reused across entities
//! - [`audio`] – bridge and channels for the background audio thread
//! - [`camera2d`] – shared 2D camera used for world/screen transforms
//! - [`camerafollowconfig`] – configuration for the camera-follow system
//! - [`debugmode`] – presence toggles optional debug overlays and logs
//! - [`debugoverlayconfig`] – per-overlay toggles for the imgui debug HUD
//! - [`fontstore`] – loaded fonts keyed by string IDs
//! - [`fullscreen`] – presence toggles fullscreen mode
//! - [`gamestate`] – authoritative and pending high-level game state
//! - [`group`] – set of group names tracked for entity counting
//! - [`input`] – per-frame keyboard state of keys relevant to the game
//! - [`rendertarget`] – render texture for fixed-resolution rendering with scaling
//! - [`screensize`] – game's internal render resolution in pixels
//! - [`scenemanager`] – scene registry for `SceneManager`-based Rust games
//! - [`systemsstore`] – registry of dynamically-lookup-able systems by name
//! - [`texturestore`] – loaded textures keyed by string IDs
//! - [`tilemapstore`] – loaded tile maps and layers
//! - [`windowsize`] – actual window dimensions for letterbox calculations
//! - [`worldsignals`] – global signal storage for cross-system communication
//! - [`worldtime`] – simulation time and delta

pub mod animationstore;
pub mod audio;
pub mod camera2d;
pub mod camerafollowconfig;
pub mod debugmode;
pub mod debugoverlayconfig;
pub mod fontstore;
pub mod fullscreen;
pub mod gameconfig;
pub mod gamestate;
pub mod group;
pub mod input;
#[cfg(feature = "lua")]
pub mod lua_runtime;
pub mod postprocessshader;
pub mod rendertarget;
pub mod screensize;
pub mod shaderstore;
pub mod scenemanager;
pub mod systemsstore;
pub mod texturestore;
pub mod tilemapstore;
pub mod uniformvalue;
pub mod windowsize;
pub mod worldsignals;
pub mod worldtime;
