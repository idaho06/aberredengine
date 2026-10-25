//! The engine's own loading screen: a dark-grey scene with the engine name
//! and a progress bar drawn with default-font glyphs.
//!
//! The layout functions are pure so the screen's text can be pinned without
//! a window or font metrics.

use crate::math::Color;

/// Name of the loading scene. Prefixed so it cannot clash with a game's own
/// scene names.
pub const LOADING_SCENE: &str = "aberred_loading";

/// Cells between the bar's two side borders.
pub const BAR_INNER_WIDTH: usize = 30;

/// Background shown while the loading scene is active.
pub const BACKGROUND: Color = Color::new(32, 32, 32, 255);

/// The bar's top border for `inner` cells between the corners.
pub fn bar_top(inner: usize) -> String {
    format!("┌{}┐", "─".repeat(inner))
}

/// The bar's bottom border for `inner` cells between the corners.
pub fn bar_bottom(inner: usize) -> String {
    format!("└{}┘", "─".repeat(inner))
}

/// The bar's middle row: `inner` cells filled left to right in proportion to
/// `progress` (clamped to `0..=1`, NaN counts as 0). The fill rounds down, so
/// the bar is full only when `progress` is complete.
pub fn bar_middle(progress: f32, inner: usize) -> String {
    let progress = if progress.is_nan() {
        0.0
    } else {
        progress.clamp(0.0, 1.0)
    };
    let filled = ((progress * inner as f32) as usize).min(inner);
    format!("│{}{}│", "█".repeat(filled), " ".repeat(inner - filled))
}

/// The x that centers content of `content_width` in `screen_width`, floored
/// so glyphs stay on the pixel grid.
pub fn centered_x(screen_width: f32, content_width: f32) -> f32 {
    ((screen_width - content_width) / 2.0).floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_top_and_bottom_rows_close_the_box() {
        assert_eq!(bar_top(3), "┌───┐");
        assert_eq!(bar_bottom(3), "└───┘");
        assert_eq!(bar_top(0), "┌┐");
    }

    #[test]
    fn the_middle_row_fills_from_the_left() {
        assert_eq!(bar_middle(0.0, 4), "│    │");
        assert_eq!(bar_middle(0.5, 4), "│██  │");
        assert_eq!(bar_middle(1.0, 4), "│████│");
    }

    #[test]
    fn every_row_is_inner_plus_two_cells_wide() {
        for progress in [0.0, 0.1, 0.5, 0.99, 1.0] {
            assert_eq!(bar_middle(progress, 7).chars().count(), 9);
        }
        assert_eq!(bar_top(7).chars().count(), 9);
        assert_eq!(bar_bottom(7).chars().count(), 9);
    }

    #[test]
    fn a_bar_is_full_only_when_progress_is_complete() {
        assert_eq!(bar_middle(0.99, 10), "│█████████ │");
        assert_eq!(bar_middle(0.01, 10), "│          │");
    }

    #[test]
    fn progress_outside_the_unit_interval_is_clamped() {
        assert_eq!(bar_middle(-1.0, 2), "│  │");
        assert_eq!(bar_middle(2.0, 2), "│██│");
        assert_eq!(bar_middle(f32::NAN, 2), "│  │");
    }

    #[test]
    fn centered_x_floors_to_a_whole_pixel() {
        assert_eq!(centered_x(100.0, 40.0), 30.0);
        assert_eq!(centered_x(101.0, 40.0), 30.0);
        assert_eq!(centered_x(100.0, 41.0), 29.0);
    }
}
