//! Loaded texture store.
//!
//! A thin wrapper around a hash map that stores `raylib::prelude::Texture2D`
//! objects keyed by string IDs. Insert textures during setup and read them in
//! render systems.
use crate::resources::texturefilter::TextureFilter;
use bevy_ecs::prelude::Resource;
use raylib::ffi;
use raylib::prelude::Texture2D;
use raylib::prelude::{Color, Font, Image, RaylibHandle, RaylibThread};
use rustc_hash::FxHashMap;
use std::ffi::CString;

#[derive(Resource)]
/// Map of texture keys to loaded textures.
///
/// The `paths` map stores the source file path (relative to the working
/// directory) for each editor-managed texture, set via the `path` argument to
/// `insert()`. Engine-internal textures (loaded by mapspawn, Lua, etc.) pass
/// `None` and are never added to `paths`; absence of an entry means the
/// texture was not loaded from a user-visible file.
///
/// The `filters` map stores the sampling filter each texture was last
/// `insert()`ed with. Absence of an entry means [`TextureFilter::default`]
/// (`Nearest`).
pub struct TextureStore {
    pub map: FxHashMap<String, Texture2D>,
    pub paths: FxHashMap<String, String>,
    pub filters: FxHashMap<String, TextureFilter>,
}

impl Default for TextureStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureStore {
    pub fn new() -> Self {
        TextureStore {
            map: FxHashMap::default(),
            paths: FxHashMap::default(),
            filters: FxHashMap::default(),
        }
    }
    /// Get a texture by its key.
    pub fn get(&self, key: impl AsRef<str>) -> Option<&Texture2D> {
        self.map.get(key.as_ref())
    }
    /// Sampling filter the texture at `key` was last inserted with, or
    /// [`TextureFilter::default`] (`Nearest`) if `key` is not tracked.
    pub fn filter(&self, key: impl AsRef<str>) -> TextureFilter {
        self.filters.get(key.as_ref()).copied().unwrap_or_default()
    }
    /// Insert or replace a texture with a specific key, applying the given sampling filter.
    ///
    /// `TextureFilter::Nearest` (the default) avoids atlas tiles bleeding into
    /// adjacent tiles due to sub-pixel sampling -- the right choice for pixel
    /// art. Use `Bilinear`/`Trilinear`/`Anisotropic*` for smoothly
    /// scaled/rotated sprites.
    ///
    /// `path` records the source file path in `paths` (see struct docs). Pass `None` for
    /// engine-internal textures not loaded from a user-visible file.
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        texture: Texture2D,
        filter: TextureFilter,
        path: Option<String>,
    ) {
        unsafe {
            ffi::SetTextureFilter(*texture, filter.to_ffi());
        }
        let key = key.into();
        self.filters.insert(key.clone(), filter);
        if let Some(path) = path {
            self.paths.insert(key.clone(), path);
        }
        self.map.insert(key, texture);
    }
    /// Remove a texture by its key, returning it if it existed.
    pub fn remove(&mut self, key: impl AsRef<str>) -> Option<Texture2D> {
        self.filters.remove(key.as_ref());
        self.paths.remove(key.as_ref());
        self.map.remove(key.as_ref())
    }
    /// Update the sampling filter of an already-loaded texture in place.
    ///
    /// Returns `false` (no-op) if `key` is not loaded.
    pub fn set_filter(&mut self, key: impl AsRef<str>, filter: TextureFilter) -> bool {
        let Some(texture) = self.map.get(key.as_ref()) else {
            return false;
        };
        unsafe {
            ffi::SetTextureFilter(**texture, filter.to_ffi());
        }
        self.filters.insert(key.as_ref().to_string(), filter);
        true
    }
}

/// Render text into a new [`Texture2D`] using the given font.
pub fn load_texture_from_text(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    font: &Font,
    text: &str,
    font_size: f32,
    spacing: f32,
    color: Color,
) -> Option<Texture2D> {
    let c_text = CString::new(text).ok()?;
    let image = unsafe {
        let raw = ffi::ImageTextEx(**font, c_text.as_ptr(), font_size, spacing, color.into());
        Image::from_raw(raw)
    };
    let texture = rl.load_texture_from_image(thread, &image).ok()?;
    Some(texture)
}
