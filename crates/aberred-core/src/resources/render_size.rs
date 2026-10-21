//! Render size resource.
//!
//! The internal render resolution (`[render] width/height` in `config.ini`):
//! the size of the render target the game draws into, before it is scaled to
//! the OS window ([`WindowSize`](crate::resources::window_size::WindowSize)).

use bevy_ecs::prelude::Resource;

/// The internal render resolution in pixels.
///
/// Inserted independently in both `setup_logic_world` and
/// `setup_render_world` -- two separate instances of this same type, not a
/// `RenderX`-style wrapper mirroring one authoritative copy -- because both
/// threads read it (and the render side refreshes it on a config change).
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct RenderSize {
    /// Width in pixels.
    pub w: i32,
    /// Height in pixels.
    pub h: i32,
}
