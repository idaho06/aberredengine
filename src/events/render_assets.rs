//! GL asset-load commands, produced by logic-destined systems and consumed
//! by `process_render_asset_cmds` (render-destined,
//! `crate::systems::render_assets`).
//!
//! Deliberately a *separate* enum from
//! [`AssetCmd`](crate::resources::lua_runtime::AssetCmd): `AssetCmd` is the
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
use raylib::prelude::Color;

use crate::resources::texturefilter::TextureFilter;

/// GL asset-load/upload commands. Consumed once per frame by
/// `process_render_asset_cmds`.
#[derive(Message, Debug, Clone)]
pub enum RenderAssetCmd {
    /// Load a texture from `path` and store it under `id`.
    Texture {
        id: String,
        path: String,
        filter: TextureFilter,
    },
    /// Load a font from `path` at `size` and store it under `id`, also
    /// populating `FontMetricsStore` under the same `id`.
    ///
    /// `skip_if_loaded`: when `true`, the load is skipped if `id` is
    /// already present in `FontStore`'s metadata (preserves `spawn_map`'s
    /// "don't reload a font shared across maps" behavior). Lua's
    /// `engine.load_font` always sets this `false` (always reloads).
    /// `spawn_map` deliberately has no `FontStore` access (that's the whole
    /// point of this seam), and the producer runs on the logic thread while
    /// `FontStore` lives on the render thread, so synchronously checking
    /// "already loaded" from the producer side isn't possible.
    Font {
        id: String,
        path: String,
        size: i32,
        skip_if_loaded: bool,
    },
    /// Load a shader from optional vertex/fragment paths, store under `id`.
    Shader {
        id: String,
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
    /// Remove the texture stored under `key` (drops the GPU handle).
    /// Logic-side cleanup (`menu_despawn`'s rasterized labels) cannot touch
    /// `TextureStore` directly — even its GL-free `remove()` — because the
    /// resource only exists in the render world.
    RemoveTexture { key: String },
}
