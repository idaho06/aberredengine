//! Theme resource for GUI rendering.
//!
//! [`GuiTheme`] carries a `panel` nine-patch (used by `GuiWindow`) and an
//! optional `button` skin (used by `GuiButton`, one nine-patch per
//! [`GuiWidgetState`](crate::components::guibutton::GuiWidgetState)). See
//! `docs/gui-system-architecture.md` for the full design.
//!
//! `button` is `Option` because a v1 game that only themes panels (never
//! calls `engine.set_gui_theme_button`) shouldn't need to set it.

use std::sync::Arc;

use bevy_ecs::prelude::Resource;
use raylib::prelude::Rectangle;

/// Nine-patch metadata for one themed visual: a texture region plus border
/// offsets in pixels, mapping 1:1 onto raylib's `NPatchInfo`.
#[derive(Clone, Debug, Default)]
pub struct GuiNinePatch {
    pub tex_key: Arc<str>,
    pub source: Rectangle,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Per-state nine-patch skin for a `GuiButton`.
#[derive(Clone, Debug, Default)]
pub struct GuiButtonSkin {
    pub normal: GuiNinePatch,
    pub hover: GuiNinePatch,
    pub pressed: GuiNinePatch,
    pub disabled: GuiNinePatch,
}

/// Global theme for GUI rendering.
#[derive(Resource, Clone, Debug, Default)]
pub struct GuiTheme {
    pub panel: GuiNinePatch,
    pub button: Option<GuiButtonSkin>,
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
            button: None,
        };
        assert_eq!(theme.panel.left, 6);
        assert_eq!(&*theme.panel.tex_key, "gui_panel");
    }

    #[test]
    fn test_gui_nine_patch_default_empty_tex_key() {
        let patch = GuiNinePatch::default();
        assert_eq!(&*patch.tex_key, "");
        assert_eq!(patch.left, 0);
    }

    #[test]
    fn test_gui_theme_default_button_none() {
        let theme = GuiTheme::default();
        assert!(theme.button.is_none());
        assert_eq!(&*theme.panel.tex_key, "");
    }

    #[test]
    fn test_gui_button_skin_default_all_empty() {
        let skin = GuiButtonSkin::default();
        assert_eq!(&*skin.normal.tex_key, "");
        assert_eq!(&*skin.hover.tex_key, "");
        assert_eq!(&*skin.pressed.tex_key, "");
        assert_eq!(&*skin.disabled.tex_key, "");
    }
}
