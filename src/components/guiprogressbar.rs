//! Themed progress bar widget.
//!
//! [`GuiProgressBar`] renders a nine-patch track (optional background) and a
//! nine-patch fill scaled proportionally to `value / max`. Direction controls
//! which edge the fill grows from. Signal binding keeps `value` in sync with a
//! `WorldSignals` key without Lua polling.
//!
//! See `docs/gui-system-architecture.md`.

use std::sync::Arc;

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

use crate::components::gui_themed::Themed;
use crate::resources::guitheme::DEFAULT_GUI_THEME_KEY;

/// Fill direction for a [`GuiProgressBar`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProgressBarDirection {
    /// Fill grows left → right (default).
    #[default]
    Horizontal,
    /// Fill grows right → left.
    HorizontalReversed,
    /// Fill grows bottom → top.
    Vertical,
    /// Fill grows top → bottom.
    VerticalReversed,
}

/// Themed progress bar rendered as a track nine-patch (optional background)
/// plus a fill nine-patch scaled to `value / max`. Rendered directly by
/// `render_system` — no spawn system or companion components are needed.
///
/// `signal_binding`, when set, causes `gui_progressbar_signal_update_system`
/// to read `value` from `WorldSignals` every frame (integer preferred, scalar
/// as fallback), so the bar stays in sync without Lua polling.
#[derive(Component, Clone, Debug)]
pub struct GuiProgressBar {
    pub size: Vector2,
    /// Current fill level. Clamped to `[0, max]` at construction and by
    /// the entity command handlers — not re-clamped at render time.
    pub value: f32,
    pub max: f32,
    pub direction: ProgressBarDirection,
    /// Selects which named theme in `GuiThemeStore` provides the
    /// `GuiProgressBarSkin`. Default `"default"`.
    pub theme_key: Arc<str>,
    /// When `Some(key)`, `gui_progressbar_signal_update_system` writes the
    /// `WorldSignals` value at `key` into `self.value` every frame.
    pub signal_binding: Option<String>,
}

impl GuiProgressBar {
    pub fn new(width: f32, height: f32, value: f32, max: f32) -> Self {
        let max = max.max(0.0);
        Self {
            size: Vector2::new(width, height),
            value: value.clamp(0.0, max),
            max,
            direction: ProgressBarDirection::default(),
            theme_key: Arc::from(DEFAULT_GUI_THEME_KEY),
            signal_binding: None,
        }
    }

    pub fn with_direction(mut self, dir: ProgressBarDirection) -> Self {
        self.direction = dir;
        self
    }

    pub fn with_signal_binding(mut self, key: impl Into<String>) -> Self {
        self.signal_binding = Some(key.into());
        self
    }

    pub fn with_theme_key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.theme_key = key.into();
        self
    }
}

impl Themed for GuiProgressBar {
    fn theme_key_mut(&mut self) -> &mut Arc<str> {
        &mut self.theme_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let bar = GuiProgressBar::new(200.0, 16.0, 50.0, 100.0);
        assert!((bar.size.x - 200.0).abs() < f32::EPSILON);
        assert!((bar.size.y - 16.0).abs() < f32::EPSILON);
        assert!((bar.value - 50.0).abs() < f32::EPSILON);
        assert!((bar.max - 100.0).abs() < f32::EPSILON);
        assert_eq!(bar.direction, ProgressBarDirection::Horizontal);
        assert_eq!(&*bar.theme_key, "default");
        assert!(bar.signal_binding.is_none());
    }

    #[test]
    fn value_clamped_to_max() {
        let bar = GuiProgressBar::new(200.0, 16.0, 150.0, 100.0);
        assert!((bar.value - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn value_clamped_to_zero() {
        let bar = GuiProgressBar::new(200.0, 16.0, -10.0, 100.0);
        assert!((bar.value - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn with_direction() {
        let bar = GuiProgressBar::new(16.0, 100.0, 0.0, 1.0)
            .with_direction(ProgressBarDirection::Vertical);
        assert_eq!(bar.direction, ProgressBarDirection::Vertical);
    }

    #[test]
    fn with_signal_binding() {
        let bar = GuiProgressBar::new(200.0, 16.0, 0.0, 100.0).with_signal_binding("player_hp");
        assert_eq!(bar.signal_binding.as_deref(), Some("player_hp"));
    }

    #[test]
    fn with_theme_key() {
        let bar = GuiProgressBar::new(200.0, 16.0, 0.0, 100.0).with_theme_key("danger");
        assert_eq!(&*bar.theme_key, "danger");
    }

    #[test]
    fn direction_variants_all_distinct() {
        assert_ne!(ProgressBarDirection::Horizontal, ProgressBarDirection::HorizontalReversed);
        assert_ne!(ProgressBarDirection::Vertical, ProgressBarDirection::VerticalReversed);
        assert_ne!(ProgressBarDirection::Horizontal, ProgressBarDirection::Vertical);
    }
}
