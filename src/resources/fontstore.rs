//! Font store resource.
//!
//! A non-send resource that stores loaded fonts keyed by string IDs.
//! Fonts are loaded during setup and referenced by key in
//! [`DynamicText`](crate::components::dynamictext::DynamicText) and
//! [`Menu`](crate::components::menu::Menu) components.
//!
//! Note: This is a non-send resource because Raylib fonts must be accessed
//! from the main thread only.

// use bevy_ecs::prelude::Resource; // NonSend resource: use NonSend<FontStore> in system parameters
use raylib::prelude::Font;
use rustc_hash::FxHashMap;

/// Editor-facing metadata for a loaded font entry.
#[derive(Debug)]
pub struct FontMeta {
    pub path: String,
    pub font_size: f32,
}

/// Map of font keys to loaded fonts.
///
/// This is a non-send resource; use `NonSend<FontStore>` in system parameters.
// NonSend resource: insert with insert_non_send_resource and access via NonSend/NonSendMut
pub struct FontStore {
    fonts: FxHashMap<String, Font>,
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
            meta: FxHashMap::default(),
        }
    }

    /// Add a font with the given key (no metadata — for engine-internal fonts).
    pub fn add(&mut self, id: impl Into<String>, font: Font) {
        self.fonts.insert(id.into(), font);
    }

    /// Add a font with editor metadata (path and font size).
    pub fn add_with_meta(&mut self, id: impl Into<String>, font: Font, path: String, font_size: f32) {
        let key = id.into();
        self.fonts.insert(key.clone(), font);
        self.meta.insert(key, FontMeta { path, font_size });
    }

    /// Get a font by its key.
    pub fn get(&self, id: impl AsRef<str>) -> Option<&Font> {
        self.fonts.get(id.as_ref())
    }

    /// Rename a font key, moving both the font and its metadata.
    pub fn rename(&mut self, old_id: impl AsRef<str>, new_id: impl Into<String>) {
        let old_key = old_id.as_ref();
        let new_key = new_id.into();
        if let Some(font) = self.fonts.remove(old_key) {
            self.fonts.insert(new_key.clone(), font);
        }
        if let Some(meta) = self.meta.remove(old_key) {
            self.meta.insert(new_key, meta);
        }
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
