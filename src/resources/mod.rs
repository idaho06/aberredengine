//! ECS resources made available to systems.
//!
//! This module groups the long-lived data injected into the ECS world and
//! accessed by systems during execution: input state, timing, rendering
//! handles, asset stores, and utilities. Each submodule documents the
//! semantics and intended usage of its resource(s).
//!
//! Overview
//! - [`animationstore`] – definitions for sprite animations reused across entities
//! - [`appstate`] – typed state store passed to `GuiCallback`; one slot per Rust type
//! - [`audio`] – bridge and channels for the background audio thread
//! - [`camera2d`] – shared 2D camera used for world/screen transforms
//! - [`camerafollowconfig`] – configuration for the camera-follow system
//! - [`debugmode`] – presence toggles optional debug overlays and logs
//! - [`debugoverlayconfig`] – per-overlay toggles for the imgui debug HUD
//! - [`fontstore`] – loaded fonts keyed by string IDs
//! - [`fullscreen`] – presence toggles fullscreen mode
//! - [`gamestate`] – authoritative and pending high-level game state
//! - [`group`] – set of group names tracked for entity counting
//! - [`guiinputstate`] – per-frame scratch state for GUI click consumption
//! - [`guitheme`] – theme resource for GUI rendering (nine-patch window/button skins)
//! - [`imgui_bridge`] – internal Dear ImGui backend that replaces raylib's removed feature
//! - [`input`] – per-frame keyboard state of keys relevant to the game
//! - [`rendertarget`] – render texture for fixed-resolution rendering with scaling
//! - [`screensize`] – game's internal render resolution in pixels
//! - [`scenemanager`] – scene registry for `SceneManager`-based Rust games
//! - [`systemsstore`] – registry of dynamically-lookup-able systems by name
//! - [`texturefilter`] – texture sampling filter mode shared by render target and texture store
//! - [`texturestore`] – loaded textures keyed by string IDs
//! - [`windowsize`] – actual window dimensions for letterbox calculations
//! - [`worldsignals`] – global signal storage for cross-system communication
//! - [`worldtime`] – simulation time and delta

pub mod animationstore;
pub mod appstate;
pub mod audio;
pub mod camera2d;
pub mod camerafollowconfig;
pub mod debugmode;
pub mod debugoverlayconfig;
pub mod drawable_snapshot;
pub mod fontstore;
pub mod fullscreen;
pub mod gameconfig;
pub mod gamestate;
pub mod group;
pub mod guiinputstate;
pub mod guitheme;
pub mod imgui_bridge;
pub mod input;
pub mod input_bindings;
#[cfg(feature = "lua")]
pub mod lua_runtime;
pub mod mapdata;
pub mod postprocessshader;
pub mod rendertarget;
pub mod scenemanager;
pub mod screensize;
pub mod shaderstore;
pub mod signal_keys;
pub mod systemsstore;
pub mod texturefilter;
pub mod texturestore;
pub mod uniformvalue;
pub mod windowsize;
pub mod worldsignals;
pub mod worldtime;
