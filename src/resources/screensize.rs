//! Screen size resource.
//!
//! Stores the current framebuffer dimensions in pixels. Rendering and UI
//! layout systems can read this to adapt to window resizes.

use bevy_ecs::prelude::Resource;

/// Current screen size in pixels.
///
/// Inserted independently in both `setup_logic_world` and
/// `setup_render_world` -- two separate instances of this same type, not a
/// `RenderX`-style wrapper mirroring one authoritative copy. That's why it
/// stays in flat `src/resources/` rather than `src/resources/render/`
/// alongside the render-exclusive resources: unlike those, it's read
/// (and, on the render side, refreshed each frame) by both threads.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScreenSize {
    /// Width in pixels.
    pub w: i32,
    /// Height in pixels.
    pub h: i32,
}
