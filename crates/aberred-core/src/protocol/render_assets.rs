//! GL asset-load commands, produced by logic-destined systems and consumed
//! by `process_render_asset_cmds` (render-destined,
//! `crate::systems::render_assets`).
//!
//! Deliberately a *separate* enum from
//! `AssetCmd` (`aberred_lua::resources::lua_runtime`): `AssetCmd` is the
//! raw Lua-facing queue type and includes `Music`/`Sound`, which must keep
//! routing to `MessageWriter<AudioCmd>` and never pass through this seam.
//! `RenderAssetCmd` additionally carries requests that never existed as
//! `AssetCmd` variants: menu label rasterization and tilemap atlas uploads.
//!
//! Part of the render/logic thread split: producers on the logic thread
//! queue commands into `Messages<RenderAssetCmd>`, which cross the
//! `LogicBridge` as `RenderMsg::Asset(...)` to the render thread for
//! consumption.

use bevy_ecs::message::Message;

use crate::math::Color;
use crate::protocol::asset_kind::AssetKind;
use crate::resources::texturefilter::TextureFilter;

/// GL asset-load/upload commands. Consumed once per frame by
/// `process_render_asset_cmds`.
#[derive(Message, Debug, Clone)]
pub enum RenderAssetCmd {
    /// Load a texture from `path` and store it under `key`.
    Texture {
        key: String,
        path: String,
        filter: TextureFilter,
    },
    /// Load a font from `path` at `size` and store it under `key`, also
    /// populating `FontMetricsStore` under the same `key`.
    ///
    /// `skip_if_loaded`: when `true`, the load is skipped if `key` is
    /// already present in `FontStore`'s metadata (preserves `spawn_map`'s
    /// "don't reload a font shared across maps" behavior). Lua's
    /// `engine.load_font` always sets this `false` (always reloads).
    /// `spawn_map` deliberately has no `FontStore` access (that's the whole
    /// point of this seam), and the producer runs on the logic thread while
    /// `FontStore` lives on the render thread, so synchronously checking
    /// "already loaded" from the producer side isn't possible.
    Font {
        key: String,
        path: String,
        size: i32,
        skip_if_loaded: bool,
    },
    /// Load a shader from optional vertex/fragment paths, store under `key`.
    Shader {
        key: String,
        vs_path: Option<String>,
        fs_path: Option<String>,
    },
    /// Rasterize `text` using the already-loaded font `font_key` into a new
    /// texture stored under `key` (menu static labels).
    RasterizeText {
        key: String,
        font_key: String,
        text: String,
        font_size: f32,
        spacing: f32,
        color: Color,
    },
    /// Upload an already-located tilemap atlas PNG (`png_path`) and store
    /// it under `key`, skipped if `key` is already loaded. The JSON tile
    /// data was already parsed CPU-side by the caller before this command
    /// was queued.
    TilemapTexture { key: String, png_path: String },
    /// Load a texture from an in-memory-encoded image buffer (e.g. an
    /// embedded PNG) and store it under `key`. `ext` is the file-type hint
    /// raylib's decoder needs, e.g. ".png" (leading dot, matches
    /// `LoadImageFromMemory`'s `fileType` convention).
    TextureFromMemory {
        key: String,
        ext: String,
        bytes: Vec<u8>,
        filter: TextureFilter,
    },
    /// Load a shader from optional in-memory vertex/fragment source
    /// strings, store under `key`. Mirrors `Shader`'s `Option`-per-stage
    /// shape (a shader can supply just one stage's source and let raylib
    /// use its default for the other).
    ShaderFromMemory {
        key: String,
        vs_src: Option<String>,
        fs_src: Option<String>,
    },
    /// Remove the texture stored under `key` (drops the GPU handle).
    /// Logic-side cleanup (`menu_despawn`'s rasterized labels) cannot touch
    /// `TextureStore` directly — even its GL-free `remove()` — because the
    /// resource only exists in the render world.
    RemoveTexture { key: String },
    /// Remove the font stored under `key` (drops the GPU handle + its
    /// `FontStore` metadata). Same rationale as `RemoveTexture`: logic-side
    /// code cannot touch `FontStore` directly.
    RemoveFont { key: String },
    /// Rename an already-loaded texture's key in place: no disk re-read or
    /// GPU re-upload, just a `TextureStore` key move (preserves the same
    /// GPU handle, filter, and path metadata). No-ops with a warning if
    /// `old_key` isn't loaded.
    RenameTexture { old_key: String, new_key: String },
    /// Update the sampling filter of an already-loaded texture in place
    /// (a single `SetTextureFilter` GL call) — no reload. No-ops with a
    /// warning if `key` isn't loaded.
    SetTextureFilter { key: String, filter: TextureFilter },
    /// Rename an already-loaded font's key in place: no disk re-read or
    /// glyph-atlas regeneration, just a `FontStore` key move. No-ops with a
    /// warning if `old_key` isn't loaded.
    RenameFont { old_key: String, new_key: String },
}

impl RenderAssetCmd {
    /// The kind and key of the asset this command loads, or `None` for a
    /// command that changes an already-loaded asset (removal, rename,
    /// filter). The render thread answers every load command with exactly
    /// one reply for this key: a success message or
    /// `LogicMsg::AssetLoadFailed`.
    pub fn load_target(&self) -> Option<(AssetKind, &str)> {
        match self {
            Self::Texture { key, .. }
            | Self::TextureFromMemory { key, .. }
            | Self::TilemapTexture { key, .. }
            | Self::RasterizeText { key, .. } => Some((AssetKind::Texture, key)),
            Self::Font { key, .. } => Some((AssetKind::Font, key)),
            Self::Shader { key, .. } | Self::ShaderFromMemory { key, .. } => {
                Some((AssetKind::Shader, key))
            }
            Self::RemoveTexture { .. }
            | Self::RemoveFont { .. }
            | Self::RenameTexture { .. }
            | Self::SetTextureFilter { .. }
            | Self::RenameFont { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_owned()
    }

    #[test]
    fn load_commands_name_their_kind_and_key() {
        let cases = [
            (
                RenderAssetCmd::Texture {
                    key: s("t"),
                    path: s("t.png"),
                    filter: TextureFilter::Nearest,
                },
                AssetKind::Texture,
                "t",
            ),
            (
                RenderAssetCmd::TextureFromMemory {
                    key: s("m"),
                    ext: s(".png"),
                    bytes: Vec::new(),
                    filter: TextureFilter::Nearest,
                },
                AssetKind::Texture,
                "m",
            ),
            (
                RenderAssetCmd::TilemapTexture {
                    key: s("tm"),
                    png_path: s("tm.png"),
                },
                AssetKind::Texture,
                "tm",
            ),
            (
                RenderAssetCmd::RasterizeText {
                    key: s("label"),
                    font_key: s("f"),
                    text: s("hi"),
                    font_size: 8.0,
                    spacing: 1.0,
                    color: Color::new(255, 255, 255, 255),
                },
                AssetKind::Texture,
                "label",
            ),
            (
                RenderAssetCmd::Font {
                    key: s("f"),
                    path: s("f.ttf"),
                    size: 8,
                    skip_if_loaded: true,
                },
                AssetKind::Font,
                "f",
            ),
            (
                RenderAssetCmd::Shader {
                    key: s("sh"),
                    vs_path: None,
                    fs_path: None,
                },
                AssetKind::Shader,
                "sh",
            ),
            (
                RenderAssetCmd::ShaderFromMemory {
                    key: s("shm"),
                    vs_src: None,
                    fs_src: None,
                },
                AssetKind::Shader,
                "shm",
            ),
        ];
        for (cmd, kind, key) in &cases {
            assert_eq!(cmd.load_target(), Some((*kind, *key)), "{cmd:?}");
        }
    }

    #[test]
    fn non_load_commands_have_no_load_target() {
        let cases = [
            RenderAssetCmd::RemoveTexture { key: s("t") },
            RenderAssetCmd::RemoveFont { key: s("f") },
            RenderAssetCmd::RenameTexture {
                old_key: s("a"),
                new_key: s("b"),
            },
            RenderAssetCmd::RenameFont {
                old_key: s("a"),
                new_key: s("b"),
            },
            RenderAssetCmd::SetTextureFilter {
                key: s("t"),
                filter: TextureFilter::Bilinear,
            },
        ];
        for cmd in &cases {
            assert_eq!(cmd.load_target(), None, "{cmd:?}");
        }
    }
}
