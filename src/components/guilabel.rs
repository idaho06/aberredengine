//! Static themed GUI label.
//!
//! [`GuiLabel`] is a dedicated component (just a size) — its text is a
//! separate child [`DynamicText`](super::dynamictext::DynamicText) entity
//! (`ChildOf` + `GuiOffset`), spawned the same frame via
//! `spawn_gui_caption` — the same composition pattern
//! [`GuiButton`](super::guibutton::GuiButton)'s caption uses, minus any
//! interaction state: a label is never hit-tested. See
//! `docs/gui-system-architecture.md`.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Static themed label panel, rendered via the global `GuiTheme`'s optional
/// `label` nine-patch (skipped entirely if unset, same gating as
/// `GuiTheme.button`).
#[derive(Component, Clone, Copy, Debug)]
pub struct GuiLabel {
    pub size: Vector2,
}

impl GuiLabel {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vector2::new(width, height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guilabel_new() {
        let l = GuiLabel::new(160.0, 24.0);
        assert!((l.size.x - 160.0).abs() < f32::EPSILON);
        assert!((l.size.y - 24.0).abs() < f32::EPSILON);
    }
}
