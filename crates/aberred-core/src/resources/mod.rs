//! ECS resources made available to systems.
//!
//! This module groups the long-lived data injected into the ECS world and
//! accessed by systems during execution: input state, timing, rendering
//! handles, asset stores, and utilities. Each submodule documents the
//! semantics and intended usage of its resource(s).
//!
//! Overview
//! - [`animation_store`] – definitions for sprite animations reused across entities
//! - [`app_state`] – typed state store passed to `GuiCallback`; one slot per Rust type
//! - [`camera2d`] – shared 2D camera used for world/screen transforms
//! - [`camera_follow_config`] – configuration for the camera-follow system
//! - [`collision_contacts`] – ruled collision pairs touching last tick, for started/ended contacts
//! - [`collision_rule_index`] – pre-filters collision rule entities by group pair, avoiding a per-event linear scan
//! - [`debug_mode`] – presence toggles optional debug overlays and logs
//! - [`debug_overlay_config`] – per-overlay toggles for the imgui debug HUD
//! - [`deterministic_mode`] – marker resource of a `.deterministic()` game
//! - [`font_key`] – reserved default font key and key resolution
//! - [`font_metrics`] – CPU-side glyph metrics for text measurement without a GL context
//! - [`game_state`] – authoritative and pending high-level game state
//! - [`group`] – set of group names tracked for entity counting
//! - [`gui_input_state`] – per-frame scratch state for GUI click consumption
//! - [`gui_theme`] – theme resource for GUI rendering (nine-patch window/button skins)
//! - [`input`] – per-tick resolved input ([`input::InputState`]) and the bindable [`input::InputAction`]s
//! - [`next_scene`] – pending scene switch request
//! - [`render_size`] – game's internal render resolution in pixels
//! - [`scene_manager`] – scene registry and active-scene tracking
//! - [`signal_intents`] – deferred `WorldSignals` writes queued by render-side scene callbacks
//! - [`sim_rng`] – shared, deterministic RNG for sim-schedule systems and observers
//! - [`systems_store`] – registry of dynamically-lookup-able systems by name
//! - [`texture_filter`] – texture sampling filter mode shared by render target and texture store
//! - [`texture_dims`] – CPU-side texture dimensions mirror for the logic thread
//! - [`window_size`] – actual window dimensions for letterbox calculations
//! - [`world_signals`] – global signal storage for cross-system communication
//! - [`world_time`] – simulation time and delta

pub mod animation_store;
pub mod app_state;
pub mod camera2d;
pub mod camera_follow_config;
pub mod collision_contacts;
pub mod collision_rule_index;
pub mod debug_mode;
pub mod debug_overlay_config;
pub mod deterministic_mode;
pub mod drawable_snapshot;
pub mod font_key;
pub mod font_metrics;
pub mod game_config;
pub mod game_state;
pub mod group;
pub mod gui_input_state;
pub mod gui_theme;
pub mod input;
pub mod input_bindings;
pub mod loaded_assets;
pub mod loading_hold;
pub mod map_data;
pub mod next_scene;
pub mod pending_assets;
pub mod post_process_shader;
pub mod raw_input;
pub mod render_size;
pub mod scene_manager;
pub mod signal_intents;
pub mod signal_keys;
pub mod sim_rng;
pub mod systems_store;
pub mod texture_dims;
pub mod texture_filter;
pub mod thread_stats;
pub mod uniform_value;
pub mod warn_once;
pub mod window_size;
pub mod world_signals;
pub mod world_time;
