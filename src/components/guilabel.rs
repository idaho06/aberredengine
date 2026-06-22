//! Static themed GUI label.
//!
//! [`GuiLabel`] carries its own size and caption text — its caption is a
//! separate child [`DynamicText`](super::dynamictext::DynamicText) entity
//! (`ChildOf` + `GuiOffset`), spawned by `gui_label_spawn_system`
//! (`systems/gui_spawn.rs`) reacting on `Added<GuiLabel>` — the same
//! composition pattern [`GuiButton`](super::guibutton::GuiButton)'s caption
//! uses, minus any interaction state: a label is never hit-tested. See
//! `docs/gui-system-architecture.md`.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Static themed label panel, rendered via the global `GuiTheme`'s optional
/// `label` nine-patch (skipped entirely if unset, same gating as
/// `GuiTheme.button`). Carries its own caption text; `gui_label_spawn_system`
/// reacts on `Added<GuiLabel>` to spawn the caption `DynamicText` child.
#[derive(Component, Clone, Debug)]
pub struct GuiLabel {
    pub size: Vector2,
    /// Empty string = captionless label, no caption child spawned.
    pub caption: String,
}

impl GuiLabel {
    pub fn new(width: f32, height: f32, caption: impl Into<String>) -> Self {
        Self {
            size: Vector2::new(width, height),
            caption: caption.into(),
        }
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
    }
}
