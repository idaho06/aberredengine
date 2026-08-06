//! Resources used only by the render (main) thread's `bevy_ecs::World`.
//!
//! Each of these is constructed/inserted exactly once, in `setup_render_world`
//! -- nothing here is ever inserted into or read from the logic/sim world.
//!
//! Submodules
//! - [`fontstore`] – loaded fonts keyed by string IDs (NonSend)
//! - [`fullscreen`] – presence toggles fullscreen mode
//! - [`imgui_bridge`] – internal Dear ImGui backend (NonSend)
//! - [`mirrors`] – render-world mirrors of `DrawableSnapshot`'s global fields
//! - [`pending_imgui_capture`] – one-frame-lag imgui capture state pending send to the logic thread
//! - [`quit_requested`] – render-loop quit flag read by the loop's `while` condition
//! - [`rendertarget`] – render texture for fixed-resolution rendering with scaling (NonSend)
//! - [`scene_table`] – render-side clone of the scene-descriptor table
//! - [`shaderstore`] – loaded shaders keyed by string IDs (NonSend)
//! - [`sim_id_map`] – sim-entity-id -> mirror-entity lookup backing mirror-entity reconciliation
//! - [`texturestore`] – loaded textures keyed by string IDs

pub mod fontstore;
pub mod fullscreen;
pub mod imgui_bridge;
pub mod mirrors;
pub mod pending_imgui_capture;
pub mod quit_requested;
pub mod rendertarget;
pub mod scene_table;
pub mod shaderstore;
pub mod sim_id_map;
pub mod texturestore;
pub mod thread_stats;
