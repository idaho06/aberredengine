//! Systems used only by the render (main) thread's `bevy_ecs::World`.
//!
//! Submodules
//! - [`render`] – main render pass ([`render_system`]): draws sprites, optional
//!   debug overlays, and basic diagnostics each frame
//! - [`mirror`] – retained render-world mirror-entity reconciliation
//! - [`geometry`] – sprite/text geometry and view-bounds helpers
//! - [`math`] – `aberred_core::math::Color`/`Rect` <-> raylib `Color`/`Rectangle` conversions
//! - [`window`] – refreshes `WindowSize` from the OS each render frame
//! - [`input`] – samples raw device input each render frame
//! - [`messages`] – drains logic->render messages once per render frame
//! - [`snapshot`] – reads the newest `DrawableSnapshot` and reconciles mirror entities
//! - [`output`] – ships render-owned mirror diffs back to the logic thread
//! - [`gameconfig`] – applies `GameConfig` changes to the live raylib window
//! - [`assets`] – the render-thread GL asset loader ([`process_render_asset_cmds`](assets::process_render_asset_cmds))
//! - [`raylib_access`] – bundled [`RaylibHandle`](raylib::RaylibHandle)/[`RaylibThread`](raylib::RaylibThread) `SystemParam`

mod assets;
mod debug_overlay;
mod gameconfig;
pub mod geometry;
mod gui_panel;
pub mod input;
pub(crate) mod math;
pub mod messages;
pub mod mirror;
pub mod output;
mod postprocess;
pub mod raylib_access;
#[allow(clippy::module_inception)]
mod render;
pub mod snapshot;
mod sprite;
mod text;
pub mod window;

pub use assets::process_render_asset_cmds;
pub use gameconfig::apply_gameconfig_changes;
pub use raylib_access::RaylibAccess;
pub use render::render_system;
