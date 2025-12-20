//! Font store resource.
//!
//! A non-send resource that stores loaded fonts keyed by string IDs.
//! Fonts are loaded during setup and referenced by key in
//! [`DynamicText`](crate::components::dynamictext::DynamicText) and
//! [`Menu`](crate::components::menu::Menu) components.
//!
//! Note: This is a non-send resource because Raylib fonts must be accessed
//! from the main thread only.

use bevy_ecs::prelude::Resource;
use raylib::prelude::Font;
use rustc_hash::FxHashMap;

/// Map of font keys to loaded fonts.
///
/// This is a non-send resource; use `NonSend<FontStore>` in system parameters.
// NonSend resource: insert with insert_non_send_resource and access via NonSend/NonSendMut
pub struct FontStore {
    fonts: FxHashMap<String, Font>,
}

impl FontStore {
    /// Create an empty font store.
    pub fn new() -> Self {
        Self {
            fonts: FxHashMap::default(),
        }
    }

    /// Add a font with the given key.
    pub fn add(&mut self, id: impl Into<String>, font: Font) {
        self.fonts.insert(id.into(), font);
    }

    /// Get a font by its key.
    pub fn get(&self, id: impl AsRef<str>) -> Option<&Font> {
        self.fonts.get(id.as_ref())
    }

    /// Remove all loaded fonts.
    pub fn clear(&mut self) {
        self.fonts.clear();
    }

    /// Get the number of loaded fonts.
    pub fn len(&self) -> usize {
        self.fonts.len()
    }
}
