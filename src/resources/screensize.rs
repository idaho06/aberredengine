//! Screen size resource.
//!
//! Stores the current framebuffer dimensions in pixels. Rendering and UI
//! layout systems can read this to adapt to window resizes.

use bevy_ecs::prelude::Resource;

/// Current screen size in pixels.
#[derive(Resource, Clone, Copy)]
pub struct ScreenSize {
    /// Width in pixels.
    pub w: i32,
    /// Height in pixels.
    pub h: i32,
}
