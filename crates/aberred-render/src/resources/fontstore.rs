//! Font store resource.
//!
//! A non-send resource that stores loaded fonts keyed by string IDs.
//! Fonts are loaded during setup and referenced by key in
//! [`DynamicText`](aberred_core::components::dynamic_text::DynamicText) and
//! [`Menu`](aberred_core::components::menu::Menu) components.
//!
//! Note: This is a non-send resource because Raylib fonts must be accessed
//! from the main thread only.

// use bevy_ecs::prelude::Resource; // NonSend resource: use NonSend<FontStore> in system parameters
use aberred_core::resources::font_key::resolve_font_key;
use aberred_core::resources::font_metrics::TEXT_SPACING;
use raylib::prelude::Font;
use rustc_hash::FxHashMap;

/// Editor-facing metadata for a loaded font entry.
#[derive(Debug)]
pub struct FontMeta {
    pub path: String,
    pub font_size: f32,
}

/// A loaded font and the glyph spacing it was loaded with.
struct LoadedFont {
    font: Font,
    spacing: f32,
}

/// Map of font keys to loaded fonts, plus the embedded default font.
///
/// The default font lives in its own slot rather than the map, so `remove`,
/// `rename` and `clear` can never lose it. [`get`](Self::get) answers for
/// loaded keys only; [`get_or_default`](Self::get_or_default) is the lookup
/// for drawing, where an empty or unknown key means the default font.
///
/// This is a non-send resource; use `NonSend<FontStore>` in system parameters.
// NonSend resource: insert with insert_non_send and access via NonSend/NonSendMut
pub struct FontStore {
    fonts: FxHashMap<String, LoadedFont>,
    default: Option<LoadedFont>,
    pub meta: FxHashMap<String, FontMeta>,
}

impl Default for FontStore {
    fn default() -> Self {
        Self::new()
    }
}

impl FontStore {
    /// Create an empty font store.
    pub fn new() -> Self {
        Self {
            fonts: FxHashMap::default(),
            default: None,
            meta: FxHashMap::default(),
        }
    }

    /// Create a store whose default font is `font`, drawn with glyph spacing
    /// `spacing`.
    pub fn with_default(font: Font, spacing: f32) -> Self {
        Self {
            default: Some(LoadedFont { font, spacing }),
            ..Self::new()
        }
    }

    /// Add a font with the given key and glyph spacing (no metadata — for
    /// engine-internal fonts).
    pub fn add(&mut self, id: impl Into<String>, font: Font, spacing: f32) {
        self.fonts.insert(id.into(), LoadedFont { font, spacing });
    }

    /// Add a font with editor metadata (path and font size), with the
    /// standard glyph spacing.
    pub fn add_with_meta(
        &mut self,
        id: impl Into<String>,
        font: Font,
        path: String,
        font_size: f32,
    ) {
        let key = id.into();
        self.add(key.clone(), font, TEXT_SPACING);
        self.meta.insert(key, FontMeta { path, font_size });
    }

    /// Get a font by its key.
    pub fn get(&self, id: impl AsRef<str>) -> Option<&Font> {
        self.get_with_spacing(id).map(|(font, _)| font)
    }

    /// Get a font and the glyph spacing it was loaded with by its key.
    pub fn get_with_spacing(&self, id: impl AsRef<str>) -> Option<(&Font, f32)> {
        self.fonts.get(id.as_ref()).map(|f| (&f.font, f.spacing))
    }

    /// Get the font a draw call should use for `id`, with its glyph spacing:
    /// the loaded font under that key, else the default font (an empty or
    /// unknown key). `None` only when the key is unloaded and the store has no
    /// default font.
    pub fn get_or_default(&self, id: impl AsRef<str>) -> Option<(&Font, f32)> {
        self.fonts
            .get(self.resolve(id.as_ref()))
            .or(self.default.as_ref())
            .map(|f| (&f.font, f.spacing))
    }

    /// The loaded key `id` resolves to, or the default font's key.
    fn resolve<'a>(&self, id: &'a str) -> &'a str {
        resolve_font_key(id, |k| self.fonts.contains_key(k))
    }

    /// Rename a font key, moving both the font and its metadata.
    ///
    /// Returns `false` (no-op) if `old_id` is not loaded. Callers renaming a
    /// font must also rename/remove the matching `FontMetricsStore`
    /// (`font_metrics.rs`) entry, or a stale/aliased key will silently
    /// return wrong CPU-side metrics — worse than `dynamic_text_size_system`'s
    /// current "missing key, warn once" failure mode.
    pub fn rename(&mut self, old_id: impl AsRef<str>, new_id: impl Into<String>) -> bool {
        let old_key = old_id.as_ref();
        let Some(font) = self.fonts.remove(old_key) else {
            return false;
        };
        let new_key = new_id.into();
        self.fonts.insert(new_key.clone(), font);
        if let Some(meta) = self.meta.remove(old_key) {
            self.meta.insert(new_key, meta);
        }
        true
    }

    /// Remove a font and its metadata by key.
    pub fn remove(&mut self, id: impl AsRef<str>) {
        let key = id.as_ref();
        self.fonts.remove(key);
        self.meta.remove(key);
    }

    /// Remove all loaded fonts.
    pub fn clear(&mut self) {
        self.fonts.clear();
        self.meta.clear();
    }

    /// Get the number of loaded fonts.
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    /// Returns `true` if no fonts are loaded.
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }
}
