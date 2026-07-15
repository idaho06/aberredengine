//! CPU-side texture dimension mirror for the logic thread.
//!
//! The logic world has no [`TextureStore`] (GPU handles live only in the
//! render world), but `animation` still needs the atlas width for multi-row
//! frame wrap (`vertical_displacement > 0`). The render side sends a
//! `LogicMsg::TextureLoaded { key, width, height }` after every texture
//! load/upload (`Texture`, `TilemapTexture`, `RasterizeText` arms of
//! `process_render_asset_cmds`), and the logic thread's message loop inserts
//! it here — the same one-owner mirror pattern as `FontMetricsStore`, just in
//! the opposite direction of ownership: render extracts-and-sends, logic owns
//! the store.
//!
//! [`TextureStore`]: crate::resources::render::texturestore::TextureStore

use bevy_ecs::prelude::*;
use rustc_hash::FxHashMap;

/// Pixel dimensions of render-side loaded textures, keyed like
/// `TextureStore`.
#[derive(Resource, Debug, Clone, Default)]
pub struct TextureDimsStore {
    map: FxHashMap<String, (i32, i32)>,
}

impl TextureDimsStore {
    /// Record (or overwrite) the dimensions for a texture key.
    pub fn insert(&mut self, key: impl Into<String>, width: i32, height: i32) {
        self.map.insert(key.into(), (width, height));
    }

    /// Dimensions as `(width, height)`, if the render side has reported the
    /// key. `None` also covers the 0-1 frame gap between requesting a load
    /// and the `TextureLoaded` message landing.
    pub fn get(&self, key: &str) -> Option<(i32, i32)> {
        self.map.get(key).copied()
    }

    /// Texture width, if known. Convenience for `animation`'s frame-wrap
    /// math, which only needs the width.
    pub fn width(&self, key: &str) -> Option<i32> {
        self.get(key).map(|(w, _)| w)
    }

    /// Drop a key (e.g. alongside queueing `RenderAssetCmd::RemoveTexture`).
    pub fn remove(&mut self, key: &str) {
        self.map.remove(key);
    }
}
