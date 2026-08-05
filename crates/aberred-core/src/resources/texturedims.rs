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

    /// Move the dimensions entry for `old_key` to `new_key` in place (e.g.
    /// alongside queueing `RenderAssetCmd::RenameTexture`). No-op if
    /// `old_key` isn't tracked.
    pub fn rename(&mut self, old_key: &str, new_key: String) {
        if let Some(dims) = self.map.remove(old_key) {
            self.map.insert(new_key, dims);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_drops_a_previously_inserted_key() {
        let mut store = TextureDimsStore::default();
        store.insert("player", 32, 32);
        assert_eq!(store.get("player"), Some((32, 32)));
        store.remove("player");
        assert_eq!(store.get("player"), None);
        assert_eq!(store.width("player"), None);
    }

    #[test]
    fn remove_of_missing_key_is_a_no_op() {
        let mut store = TextureDimsStore::default();
        store.remove("does_not_exist");
        assert_eq!(store.get("does_not_exist"), None);
    }
}
