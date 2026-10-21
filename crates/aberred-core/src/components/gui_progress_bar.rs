//! Themed progress bar widget.
//!
//! [`GuiProgressBar`] renders a nine-patch track (optional background) and a
//! nine-patch fill scaled proportionally to `value / max`. Direction controls
//! which edge the fill grows from. Signal binding keeps `value` in sync with a
//! `WorldSignals` key without Lua polling.

use std::sync::Arc;

use crate::math::Vec2;
use bevy_ecs::prelude::Component;

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
#[derive(Component, Clone, Debug, PartialEq)]
pub struct GuiProgressBar {
    pub size: Vec2,
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
            size: Vec2::new(width, height),
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
    fn new_clamps_value_into_zero_max() {
        for (value, expected) in [(150.0, 100.0), (-10.0, 0.0), (40.0, 40.0)] {
            let bar = GuiProgressBar::new(200.0, 16.0, value, 100.0);
            assert_eq!(bar.value, expected, "value {value}");
        }
    }

    #[test]
    fn new_uses_default_theme_key() {
        let bar = GuiProgressBar::new(200.0, 16.0, 0.0, 1.0);
        assert_eq!(&*bar.theme_key, DEFAULT_GUI_THEME_KEY);
    }
}
