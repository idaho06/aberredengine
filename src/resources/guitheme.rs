//! Theme resource for the static GUI window slice.
//!
//! v1 ships a single global [`GuiTheme`] resource with just a `panel`
//! nine-patch — no per-widget theme keys, no button/label skins yet. See
//! `docs/gui-system-architecture.md` for the full design; this resource only
//! covers what the current slice (`GuiWindow`) renders.

use std::sync::Arc;

use bevy_ecs::prelude::Resource;
use raylib::prelude::Rectangle;

/// Nine-patch metadata for one themed visual: a texture region plus border
/// offsets in pixels, mapping 1:1 onto raylib's `NPatchInfo`.
#[derive(Clone, Debug)]
pub struct GuiNinePatch {
    pub tex_key: Arc<str>,
    pub source: Rectangle,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Global theme for GUI rendering. v1 scope: window panel background only.
#[derive(Resource, Clone, Debug)]
pub struct GuiTheme {
    pub panel: GuiNinePatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guitheme_construction() {
        let theme = GuiTheme {
            panel: GuiNinePatch {
                tex_key: Arc::from("gui_panel"),
                source: Rectangle::new(0.0, 0.0, 64.0, 64.0),
                left: 6,
                top: 6,
                right: 6,
                bottom: 6,
            },
        };
        assert_eq!(theme.panel.left, 6);
        assert_eq!(&*theme.panel.tex_key, "gui_panel");
    }
}
