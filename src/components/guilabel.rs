//! Static themed GUI label.
//!
//! [`GuiLabel`] carries its own size and caption text — its caption is a
//! separate child [`DynamicText`](super::dynamictext::DynamicText) entity
//! (`ChildOf` + `GuiOffset`), spawned by `gui_label_spawn_system`
//! (`systems/gui_spawn.rs`) reacting on `Added<GuiLabel>` — the same
//! composition pattern [`GuiButton`](super::guibutton::GuiButton)'s caption
//! uses, minus any interaction state: a label is never hit-tested. See
//! `docs/gui-system-architecture.md`.

use std::sync::Arc;

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

use crate::components::gui_themed::Themed;
use crate::resources::guitheme::DEFAULT_GUI_THEME_KEY;

/// Static themed label panel, rendered via the named theme (`theme_key`,
/// looked up in `GuiThemeStore`)'s optional `label` nine-patch (skipped
/// entirely if unset, same gating as `GuiTheme.button`). Carries its own
/// caption text; `gui_label_spawn_system` reacts on `Added<GuiLabel>` to
/// spawn the caption `DynamicText` child.
#[derive(Component, Clone, Debug)]
pub struct GuiLabel {
    pub size: Vector2,
    /// Empty string = captionless label, no caption child spawned.
    pub caption: String,
    /// Selects which named theme in `GuiThemeStore` to render this label
    /// (and its caption) with. Default `"default"`.
    pub theme_key: Arc<str>,
    /// Optional (signal_key, format) attached to the caption `DynamicText`
    /// child as a `SignalBinding` by `gui_label_spawn_system`, so the
    /// caption auto-updates from `WorldSignals` via
    /// `update_world_signals_binding_system` instead of needing a Lua
    /// script to poll and call `entity_set_text` every frame. `caption` is
    /// still the text shown until the signal key first resolves (the
    /// binding system leaves `DynamicText.text` untouched when the key
    /// isn't found yet), so an empty `caption` with a binding set still
    /// spawns no caption at all (see "captionless label" above) -- pass a
    /// placeholder string (e.g. `"0"`) if you want a binding.
    pub signal_binding: Option<(String, Option<String>)>,
}

impl GuiLabel {
    pub fn new(width: f32, height: f32, caption: impl Into<String>) -> Self {
        Self {
            size: Vector2::new(width, height),
            caption: caption.into(),
            theme_key: Arc::from(DEFAULT_GUI_THEME_KEY),
            signal_binding: None,
        }
    }

    pub fn with_theme_key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.theme_key = key.into();
        self
    }

    pub fn with_signal_binding(mut self, key: impl Into<String>) -> Self {
        self.signal_binding = Some((key.into(), None));
        self
    }

    /// Sets the format string (use `{}` as the value placeholder) on an
    /// already-set signal binding. No-op if `with_signal_binding` wasn't
    /// called first -- the Lua API enforces this ordering with a runtime
    /// error instead (see `entity_builder.rs`'s
    /// `with_gui_label_signal_binding_format`).
    pub fn with_signal_binding_format(mut self, format: impl Into<String>) -> Self {
        if let Some((_, fmt)) = self.signal_binding.as_mut() {
            *fmt = Some(format.into());
        }
        self
    }
}

impl Themed for GuiLabel {
    fn theme_key_mut(&mut self) -> &mut Arc<str> {
        &mut self.theme_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guilabel_new() {
        let l = GuiLabel::new(160.0, 24.0, "Inventory");
        assert!((l.size.x - 160.0).abs() < f32::EPSILON);
        assert!((l.size.y - 24.0).abs() < f32::EPSILON);
        assert_eq!(l.caption, "Inventory");
        assert!(l.signal_binding.is_none());
    }

    #[test]
    fn test_guilabel_with_signal_binding() {
        let l = GuiLabel::new(80.0, 24.0, "0").with_signal_binding("score");
        assert_eq!(l.signal_binding, Some(("score".to_string(), None)));
    }

    #[test]
    fn test_guilabel_with_signal_binding_format() {
        let l = GuiLabel::new(80.0, 24.0, "0")
            .with_signal_binding("score")
            .with_signal_binding_format("Score: {}");
        assert_eq!(
            l.signal_binding,
            Some(("score".to_string(), Some("Score: {}".to_string())))
        );
    }

    #[test]
    fn test_guilabel_with_signal_binding_format_without_binding_is_noop() {
        let l = GuiLabel::new(80.0, 24.0, "0").with_signal_binding_format("Score: {}");
        assert!(l.signal_binding.is_none());
    }
}
