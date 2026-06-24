//! Theme resource for GUI rendering.
//!
//! [`GuiTheme`] carries a `panel` nine-patch (used by `GuiWindow`), an
//! optional `button` skin (used by `GuiButton`, one nine-patch per
//! [`GuiWidgetState`](crate::components::guibutton::GuiWidgetState)), and an
//! optional `label` nine-patch (used by
//! [`GuiLabel`](crate::components::guilabel::GuiLabel)). See
//! `docs/gui-system-architecture.md` for the full design.
//!
//! `button`/`label` are `Option` because a v1 game that only themes panels
//! (never calls `engine.set_gui_theme_button`/`set_gui_theme_label`)
//! shouldn't need to set them.

use std::sync::Arc;

use bevy_ecs::prelude::Resource;
use raylib::prelude::{Color, Rectangle};
use rustc_hash::{FxHashMap, FxHashSet};

/// Default theme key used by widgets that never call `:with_gui_theme_key`.
pub const DEFAULT_GUI_THEME_KEY: &str = "default";

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

impl GuiNinePatch {
    /// True if this patch has never been set (still `GuiNinePatch::default()`'s empty `tex_key`).
    pub fn is_unset(&self) -> bool {
        self.tex_key.is_empty()
    }
}

/// Per-state nine-patch skin for a `GuiButton`. `normal` is the only
/// required patch — `hover`/`pressed`/`disabled` fall back to `normal` when
/// unset, so a game that wants a flat look doesn't have to call
/// `engine.set_gui_theme_button` once per state.
#[derive(Clone, Debug, Default)]
pub struct GuiButtonSkin {
    pub normal: GuiNinePatch,
    pub hover: Option<GuiNinePatch>,
    pub pressed: Option<GuiNinePatch>,
    pub disabled: Option<GuiNinePatch>,
}

/// One named theme's worth of GUI styling. `label` is `None` until
/// `engine.set_gui_theme_label` is called — a `GuiLabel` renders its caption
/// with no background panel until then. `font`/`font_size`/`text_color`
/// configure every widget caption's `DynamicText`; an unset `font` (empty
/// `Arc<str>`, the default) renders no glyphs — `FontStore::get` already
/// returns `None` for an unset key and silently skips the draw, so this is
/// the same "unconfigured = skip" idiom `button`/`label` already use.
///
/// Not a `Resource` itself — stored by name inside [`GuiThemeStore`], which
/// is the actual resource. See `docs/gui-system-architecture.md` Roadmap #2.
#[derive(Clone, Debug)]
pub struct GuiTheme {
    pub panel: GuiNinePatch,
    pub button: Option<GuiButtonSkin>,
    pub label: Option<GuiNinePatch>,
    pub font: Arc<str>,
    pub font_size: f32,
    pub text_color: Color,
}

impl Default for GuiTheme {
    fn default() -> Self {
        Self {
            panel: GuiNinePatch::default(),
            button: None,
            label: None,
            font: Arc::from(""),
            font_size: 16.0,
            text_color: Color::WHITE,
        }
    }
}

impl GuiTheme {
    /// Clears `button` if its skin's `normal` patch was never configured —
    /// a developer mistake (`button` is `Some` but its one required patch is
    /// still unset), not a fallback case. Returns `false` when this happened,
    /// so callers can log a warning; centralizes the "is this theme
    /// renderable" check next to the type instead of duplicating it in
    /// render/Lua-integration code.
    pub fn drop_invalid_button_skin(&mut self) -> bool {
        if let Some(skin) = &self.button
            && skin.normal.is_unset()
        {
            self.button = None;
            return false;
        }
        true
    }
}

/// Named, persistent store of GUI themes. Replaces the old single global
/// `GuiTheme` resource so a scene can mix multiple themed `GuiWindow`s/
/// `GuiButton`s/`GuiLabel`s (different `theme_key` per widget, see
/// `components.md`). Always inserted at startup (see `engine_app.rs`) so
/// consumers take a plain `Res<GuiThemeStore>` rather than
/// `Option<Res<_>>` — only individual *keys* may be missing, not the store
/// itself.
///
/// Resources aren't touched by `clear_all_commands()`, so a theme registered
/// under a name persists across scene switches exactly like the old single
/// `GuiTheme` resource did — only the `RenderCmd` queue used to set it is
/// scene-scoped.
#[derive(Resource, Clone, Debug, Default)]
pub struct GuiThemeStore {
    pub themes: FxHashMap<Arc<str>, GuiTheme>,
}

impl GuiThemeStore {
    pub fn get(&self, key: &str) -> Option<&GuiTheme> {
        self.themes.get(key)
    }
}

/// Dedupes GUI theme warnings so each one logs once instead of every frame
/// for every widget that triggers it (a real `warn!`/`error!` per widget
/// per frame would flood the log at 60Hz). Scoped to process lifetime, not
/// per-scene — revisit if a typo fixed mid-session needs to be re-flagged
/// after a scene switch.
///
/// Two independent domains, two independent sets: "widget references an
/// unregistered theme_key" (`warn_once`) and "theme's button skin has no
/// `normal` patch, dropped" (`warn_once_invalid_button_skin`) are unrelated
/// conditions that both key off the same `theme_key` string. Sharing one set
/// between them (e.g. via a hand-rolled `"{key}::button"` composite key)
/// would let a theme literally named `"foo::button"` collide with the
/// warning key for theme `"foo"`'s button-skin case — kept as two sets
/// instead, so each domain's key is just the plain `theme_key`.
#[derive(Resource, Default)]
pub struct GuiThemeWarnCache {
    warned_keys: FxHashSet<Arc<str>>,
    warned_invalid_button_skins: FxHashSet<Arc<str>>,
}

impl GuiThemeWarnCache {
    /// Returns `true` the first time `key` is reported missing, `false` on
    /// every subsequent call for the same key.
    pub fn warn_once(&mut self, key: &str) -> bool {
        if self.warned_keys.contains(key) {
            false
        } else {
            self.warned_keys.insert(Arc::from(key));
            true
        }
    }

    /// Returns `true` the first time `key`'s button skin is reported
    /// invalid (no `normal` patch set), `false` on every subsequent call for
    /// the same key.
    pub fn warn_once_invalid_button_skin(&mut self, key: &str) -> bool {
        if self.warned_invalid_button_skins.contains(key) {
            false
        } else {
            self.warned_invalid_button_skins.insert(Arc::from(key));
            true
        }
    }
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
            label: None,
            ..GuiTheme::default()
        };
        assert_eq!(theme.panel.left, 6);
        assert_eq!(&*theme.panel.tex_key, "gui_panel");
    }

    #[test]
    fn test_gui_nine_patch_default_empty_tex_key() {
        let patch = GuiNinePatch::default();
        assert_eq!(&*patch.tex_key, "");
        assert_eq!(patch.left, 0);
        assert!(patch.is_unset());
    }

    #[test]
    fn test_gui_nine_patch_is_unset_false_when_tex_key_set() {
        let patch = GuiNinePatch {
            tex_key: Arc::from("some_tex"),
            ..GuiNinePatch::default()
        };
        assert!(!patch.is_unset());
    }

    #[test]
    fn test_gui_theme_default_button_none() {
        let theme = GuiTheme::default();
        assert!(theme.button.is_none());
        assert_eq!(&*theme.panel.tex_key, "");
    }

    #[test]
    fn test_gui_theme_default_label_none() {
        let theme = GuiTheme::default();
        assert!(theme.label.is_none());
    }

    #[test]
    fn test_gui_theme_default_font_config() {
        let theme = GuiTheme::default();
        assert_eq!(&*theme.font, "");
        assert_eq!(theme.font_size, 16.0);
        assert_eq!(theme.text_color, Color::WHITE);
    }


    #[test]
    fn drop_invalid_button_skin_clears_button_when_normal_unset() {
        let mut theme = GuiTheme {
            button: Some(GuiButtonSkin::default()),
            ..GuiTheme::default()
        };
        assert!(!theme.drop_invalid_button_skin());
        assert!(theme.button.is_none());
    }

    #[test]
    fn drop_invalid_button_skin_keeps_button_when_normal_set() {
        let mut theme = GuiTheme {
            button: Some(GuiButtonSkin {
                normal: GuiNinePatch {
                    tex_key: Arc::from("tex"),
                    ..GuiNinePatch::default()
                },
                ..GuiButtonSkin::default()
            }),
            ..GuiTheme::default()
        };
        assert!(theme.drop_invalid_button_skin());
        assert!(theme.button.is_some());
    }

    #[test]
    fn test_gui_button_skin_default_all_empty() {
        let skin = GuiButtonSkin::default();
        assert_eq!(&*skin.normal.tex_key, "");
        assert!(skin.hover.is_none());
        assert!(skin.pressed.is_none());
        assert!(skin.disabled.is_none());
    }
}
