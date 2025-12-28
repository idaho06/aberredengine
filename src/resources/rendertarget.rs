//! Render target resource for fixed-resolution rendering.
//!
//! Provides a framebuffer texture at the game's internal resolution, which is
//! then scaled to fit the actual window size. This enables resolution-independent
//! rendering with proper aspect ratio preservation.

use raylib::ffi::{self, TextureFilter};
use raylib::prelude::*;

/// Texture filtering mode for scaling the render target.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum RenderFilter {
    /// Point/nearest-neighbor filtering - sharp pixels, no blur.
    /// Best for pixel art games.
    #[default]
    Nearest,
    /// Bilinear filtering - smooth scaling with interpolation.
    /// Best for high-resolution or vector-style graphics.
    Bilinear,
}

/// Render target for fixed-resolution rendering with scaling.
///
/// This resource holds a `RenderTexture2D` at the game's internal resolution.
/// The render system draws all game content to this texture, then scales it
/// to fit the window with letterboxing/pillarboxing as needed.
///
/// # Note
/// This is a NonSend resource because `RenderTexture2D` contains GPU resources
/// that must be accessed from the main thread.
pub struct RenderTarget {
    /// The underlying raylib render texture.
    pub texture: RenderTexture2D,
    /// Game's internal render width in pixels.
    pub game_width: u32,
    /// Game's internal render height in pixels.
    pub game_height: u32,
    /// Current texture filtering mode.
    pub filter: RenderFilter,
}

impl RenderTarget {
    /// Create a new render target at the specified game resolution.
    ///
    /// Initializes with nearest-neighbor filtering by default.
    pub fn new(
        rl: &mut RaylibHandle,
        th: &RaylibThread,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let texture = rl
            .load_render_texture(th, width, height)
            .map_err(|e| format!("Failed to create render texture: {}", e))?;

        let mut target = Self {
            texture,
            game_width: width,
            game_height: height,
            filter: RenderFilter::default(),
        };

        // Apply default filter
        target.apply_filter();

        Ok(target)
    }

    /// Set the texture filtering mode.
    ///
    /// Changes take effect immediately.
    pub fn set_filter(&mut self, filter: RenderFilter) {
        self.filter = filter;
        self.apply_filter();
    }

    /// Apply the current filter setting to the texture via FFI.
    fn apply_filter(&mut self) {
        let filter_value = match self.filter {
            RenderFilter::Nearest => TextureFilter::TEXTURE_FILTER_POINT as i32,
            RenderFilter::Bilinear => TextureFilter::TEXTURE_FILTER_BILINEAR as i32,
        };
        unsafe {
            ffi::SetTextureFilter(self.texture.texture, filter_value);
        }
    }

    /// Get the aspect ratio of the game resolution.
    pub fn aspect_ratio(&self) -> f32 {
        self.game_width as f32 / self.game_height as f32
    }

    /// Recreate the render texture at a new resolution.
    ///
    /// Useful for changing the game's internal resolution at runtime.
    pub fn recreate(
        &mut self,
        rl: &mut RaylibHandle,
        th: &RaylibThread,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let texture = rl
            .load_render_texture(th, width, height)
            .map_err(|e| format!("Failed to recreate render texture: {}", e))?;

        self.texture = texture;
        self.game_width = width;
        self.game_height = height;
        self.apply_filter();

        Ok(())
    }

    /// Get the source rectangle for drawing this texture.
    ///
    /// Returns a rectangle with negative height to flip the Y axis,
    /// compensating for OpenGL's inverted texture coordinates.
    pub fn source_rect(&self) -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: self.game_width as f32,
            height: -(self.game_height as f32), // Negative to flip Y
        }
    }
}