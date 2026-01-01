//! Window size resource.
//!
//! Tracks the actual window dimensions in pixels, which may differ from the
//! game's render resolution. Updated each frame to handle window resizing.

use bevy_ecs::prelude::Resource;
use raylib::prelude::*;

/// Current window size in pixels.
///
/// This represents the actual OS window dimensions, not the game's internal
/// render resolution. Use this for letterbox/pillarbox calculations when
/// scaling the render target to fit the window.
#[derive(Resource, Clone, Copy)]
pub struct WindowSize {
    /// Width in pixels.
    pub w: i32,
    /// Height in pixels.
    pub h: i32,
}

impl WindowSize {
    /// Calculate the destination rectangle for letterboxed rendering.
    ///
    /// Given the game's render resolution, returns a rectangle that:
    /// - Preserves the game's aspect ratio
    /// - Fits within the window bounds
    /// - Centers the content (letterbox/pillarbox as needed)
    pub fn calculate_letterbox(&self, game_width: u32, game_height: u32) -> Rectangle {
        let game_w = game_width as f32;
        let game_h = game_height as f32;
        let window_w = self.w as f32;
        let window_h = self.h as f32;

        /*         eprintln!(
                   "Calculating letterbox: game {}x{}, window {}x{}",
                   game_w, game_h, window_w, window_h
               );
        */
        let game_aspect = game_w / game_h;
        let window_aspect = window_w / window_h;

        if window_aspect > game_aspect {
            // Window is wider than game - pillarbox (black bars on sides)
            let scale = window_h / game_h;
            let scaled_w = game_w * scale;
            Rectangle {
                x: (window_w - scaled_w) / 2.0,
                y: 0.0,
                width: scaled_w,
                height: window_h,
            }
        } else {
            // Window is taller than game - letterbox (black bars top/bottom)
            let scale = window_w / game_w;
            let scaled_h = game_h * scale;
            Rectangle {
                x: 0.0,
                y: (window_h - scaled_h) / 2.0,
                width: window_w,
                height: scaled_h,
            }
        }
    }

    /// Transform a window-space position to game/render-target space.
    ///
    /// This accounts for letterboxing/pillarboxing. If the position is outside
    /// the game area (in the black bars), it will be clamped to the game bounds.
    ///
    /// # Arguments
    /// * `window_pos` - Position in window coordinates (e.g., from get_mouse_position)
    /// * `game_width` - Game's internal render width
    /// * `game_height` - Game's internal render height
    ///
    /// # Returns
    /// Position in game/render-target coordinates (0..game_width, 0..game_height)
    pub fn window_to_game_pos(
        &self,
        window_pos: Vector2,
        game_width: u32,
        game_height: u32,
    ) -> Vector2 {
        let letterbox = self.calculate_letterbox(game_width, game_height);

        // Transform from window space to game space
        // 1. Subtract letterbox offset to get position relative to game area
        // 2. Scale by the ratio of game size to letterbox size
        let game_w = game_width as f32;
        let game_h = game_height as f32;

        let relative_x = window_pos.x - letterbox.x;
        let relative_y = window_pos.y - letterbox.y;

        let scale_x = game_w / letterbox.width;
        let scale_y = game_h / letterbox.height;

        Vector2 {
            x: (relative_x * scale_x).clamp(0.0, game_w),
            y: (relative_y * scale_y).clamp(0.0, game_h),
        }
    }
}
