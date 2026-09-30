//! Window size resource.
//!
//! Tracks the actual window dimensions in pixels, which may differ from the
//! game's render resolution. Updated each frame to handle window resizing.

use bevy_ecs::prelude::Resource;
use crate::math::Rect;
use crate::math::Vec2;

/// Current window size in pixels.
///
/// This represents the actual OS window dimensions, not the game's internal
/// render resolution. Use this for letterbox/pillarbox calculations when
/// scaling the render target to fit the window.
///
/// Inserted independently in both `setup_logic_world` and
/// `setup_render_world` -- two separate instances of this same type, not a
/// `RenderX`-style wrapper mirroring one authoritative copy. That's why it
/// stays in flat `src/resources/` rather than `src/resources/render/`
/// alongside the render-exclusive resources.
#[derive(Resource, Clone, Copy, PartialEq)]
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
    pub fn calculate_letterbox(&self, game_width: u32, game_height: u32) -> Rect {
        let game_w = game_width as f32;
        let game_h = game_height as f32;
        let window_w = self.w as f32;
        let window_h = self.h as f32;

        let game_aspect = game_w / game_h;
        let window_aspect = window_w / window_h;

        if window_aspect > game_aspect {
            // Window is wider than game - pillarbox (black bars on sides)
            let scale = window_h / game_h;
            let scaled_w = game_w * scale;
            Rect {
                x: (window_w - scaled_w) / 2.0,
                y: 0.0,
                width: scaled_w,
                height: window_h,
            }
        } else {
            // Window is taller than game - letterbox (black bars top/bottom)
            let scale = window_w / game_w;
            let scaled_h = game_h * scale;
            Rect {
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
        window_pos: Vec2,
        game_width: u32,
        game_height: u32,
    ) -> Vec2 {
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

        Vec2 {
            x: (relative_x * scale_x).clamp(0.0, game_w),
            y: (relative_y * scale_y).clamp(0.0, game_h),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(w: i32, h: i32) -> WindowSize {
        WindowSize { w, h }
    }

    fn xywh(r: Rect) -> (f32, f32, f32, f32) {
        (r.x, r.y, r.width, r.height)
    }

    #[test]
    fn letterbox_fills_a_window_of_the_same_aspect() {
        assert_eq!(
            xywh(window(1280, 720).calculate_letterbox(640, 360)),
            (0.0, 0.0, 1280.0, 720.0)
        );
    }

    #[test]
    fn a_wider_window_is_pillarboxed_and_a_taller_one_letterboxed() {
        assert_eq!(
            xywh(window(1000, 360).calculate_letterbox(640, 360)),
            (180.0, 0.0, 640.0, 360.0),
            "bars left and right"
        );
        assert_eq!(
            xywh(window(640, 600).calculate_letterbox(640, 360)),
            (0.0, 120.0, 640.0, 360.0),
            "bars top and bottom"
        );
    }

    #[test]
    fn window_to_game_pos_undoes_the_letterbox_scale_and_offset() {
        let scaled = window(1280, 720);
        assert_eq!(
            scaled.window_to_game_pos(Vec2::new(640.0, 360.0), 640, 360),
            Vec2::new(320.0, 180.0)
        );

        let pillarboxed = window(1000, 360);
        assert_eq!(pillarboxed.window_to_game_pos(Vec2::new(180.0, 0.0), 640, 360), Vec2::ZERO);
        assert_eq!(
            pillarboxed.window_to_game_pos(Vec2::new(500.0, 100.0), 640, 360),
            Vec2::new(320.0, 100.0)
        );

        let letterboxed = window(640, 600);
        assert_eq!(
            letterboxed.window_to_game_pos(Vec2::new(320.0, 300.0), 640, 360),
            Vec2::new(320.0, 180.0)
        );
    }

    #[test]
    fn window_to_game_pos_clamps_positions_in_the_bars_to_the_game_edge() {
        let pillarboxed = window(1000, 360);
        assert_eq!(
            pillarboxed.window_to_game_pos(Vec2::new(10.0, 50.0), 640, 360),
            Vec2::new(0.0, 50.0)
        );
        assert_eq!(
            pillarboxed.window_to_game_pos(Vec2::new(990.0, 400.0), 640, 360),
            Vec2::new(640.0, 360.0)
        );
    }
}
