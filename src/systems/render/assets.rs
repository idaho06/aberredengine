//! Render-destined GL asset loading, fed by [`RenderAssetCmd`] messages.
//!
//! This is the only system permitted to perform GL texture/font/shader
//! loads and uploads originating from logic-side requests (Lua asset
//! commands, map spawning, menu label rasterization, tilemap atlas
//! uploads). Runs on the render world's schedule; `RenderAssetCmd` values
//! arrive via `RenderMsg::Asset(...)` over the `LogicBridge` channel.
//! `update_bevy_render_asset_cmds` (the queue-aging counterpart) is shared
//! with the logic world and lives in `crate::systems::render_assets`.

use bevy_ecs::prelude::*;
use log::{debug, error, warn};

use crate::events::render_assets::RenderAssetCmd;
use crate::protocol::endpoints::LogicTx;
use crate::protocol::render_logic::LogicMsg;
use crate::resources::fontmetrics::FontMetrics;
use crate::resources::render::fontstore::FontStore;
use crate::resources::render::shaderstore::ShaderStore;
use crate::resources::render::texturestore::{TextureStore, load_texture_from_text};
use crate::systems::render::RaylibAccess;

/// Drains queued [`RenderAssetCmd`]s and performs the corresponding GL
/// load/upload. The only system that touches `RaylibAccess`/`FontStore`/
/// `ShaderStore`/`TextureStore` writes on behalf of logic-originated
/// asset requests.
pub fn process_render_asset_cmds(
    mut reader: MessageReader<RenderAssetCmd>,
    mut raylib: RaylibAccess,
    mut tex_store: ResMut<TextureStore>,
    mut fonts: NonSendMut<FontStore>,
    mut shaders: NonSendMut<ShaderStore>,
    logic_tx: Res<LogicTx>,
    mut notifications: Local<Vec<LogicMsg>>,
) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    for cmd in reader.read() {
        apply_render_asset_cmd(
            rl,
            th,
            cmd.clone(),
            &mut tex_store,
            &mut fonts,
            &mut shaders,
            &mut notifications,
        );
    }
    // FontMetricsStore/TextureDimsStore are logic-world-owned (Phase 5e):
    // ship each load's metrics/dims across the channel instead of writing a
    // local resource. Send errors only occur during shutdown — ignored.
    for msg in notifications.drain(..) {
        let _ = logic_tx.0.send(msg);
    }
}

/// Performs the GL load/upload for a single [`RenderAssetCmd`].
///
/// Successful font/texture loads push a [`LogicMsg::FontLoaded`]/
/// [`LogicMsg::TextureLoaded`] notification into `notifications` instead of
/// writing `FontMetricsStore`/`TextureDimsStore` directly — those stores are
/// logic-owned, while this function runs on the render thread; the caller
/// ([`process_render_asset_cmds`]) ships each notification across the
/// `LogicTx` channel.
pub(crate) fn apply_render_asset_cmd(
    rl: &mut raylib::RaylibHandle,
    th: &raylib::RaylibThread,
    cmd: RenderAssetCmd,
    tex_store: &mut TextureStore,
    fonts: &mut FontStore,
    shaders: &mut ShaderStore,
    notifications: &mut Vec<LogicMsg>,
) {
    match cmd {
        RenderAssetCmd::Texture { id, path, filter } => match rl.load_texture(th, &path) {
            Ok(tex) => {
                debug!("Loaded texture '{}' from '{}'", id, path);
                let (width, height) = (tex.width, tex.height);
                tex_store.insert(&id, tex, filter, None);
                notifications.push(LogicMsg::TextureLoaded {
                    key: id,
                    width,
                    height,
                });
            }
            Err(e) => error!("Failed to load texture '{}': {}", path, e),
        },
        RenderAssetCmd::Font {
            id,
            path,
            size,
            skip_if_loaded,
        } => {
            if skip_if_loaded && fonts.meta.contains_key(&id) {
                debug!(
                    "process_render_asset_cmds: font '{}' already loaded, skipping",
                    id
                );
                return;
            }
            match load_font_with_mipmaps(rl, th, &path, size) {
                Ok(font) => {
                    debug!("Loaded font '{}' from '{}'", id, path);
                    let metrics = FontMetrics::extract(&font);
                    fonts.add(&id, font);
                    notifications.push(LogicMsg::FontLoaded { key: id, metrics });
                }
                Err(err) => error!("Failed to load font '{}' from '{}': {}", id, path, err),
            }
        }
        RenderAssetCmd::Shader {
            id,
            vs_path,
            fs_path,
        } => {
            let vs_path_c = vs_path.as_deref();
            let fs_path_c = fs_path.as_deref();
            match rl.load_shader(th, vs_path_c, fs_path_c) {
                Ok(shader) if shader.is_shader_valid() => {
                    debug!(
                        "Loaded shader '{}' (vs: {:?}, fs: {:?})",
                        id, vs_path, fs_path
                    );
                    shaders.add(&id, shader);
                }
                Ok(_) => error!(
                    "Shader '{}' loaded but is invalid (vs: {:?}, fs: {:?})",
                    id, vs_path, fs_path
                ),
                Err(e) => error!(
                    "Shader '{}' failed to load: {e} (vs: {:?}, fs: {:?})",
                    id, vs_path, fs_path
                ),
            }
        }
        RenderAssetCmd::RasterizeText {
            key,
            font_key,
            text,
            font_size,
            spacing,
            color,
        } => {
            let Some(font) = fonts.get(&font_key) else {
                warn!(
                    "process_render_asset_cmds: font '{}' missing for RasterizeText '{}'",
                    font_key, key
                );
                return;
            };
            match load_texture_from_text(rl, th, font, &text, font_size, spacing, color) {
                Some(tex) => {
                    let (width, height) = (tex.width, tex.height);
                    tex_store.insert(
                        &key,
                        tex,
                        crate::resources::texturefilter::TextureFilter::Nearest,
                        None,
                    );
                    notifications.push(LogicMsg::TextureLoaded {
                        key,
                        width,
                        height,
                    });
                }
                None => warn!(
                    "process_render_asset_cmds: failed to rasterize text for '{}'",
                    key
                ),
            }
        }
        RenderAssetCmd::TilemapTexture { key, png_path } => {
            if tex_store.get(&key).is_some() {
                return;
            }
            match rl.load_texture(th, &png_path) {
                Ok(tex) => {
                    let (width, height) = (tex.width, tex.height);
                    tex_store.insert(
                        &key,
                        tex,
                        crate::resources::texturefilter::TextureFilter::Nearest,
                        None,
                    );
                    notifications.push(LogicMsg::TextureLoaded {
                        key,
                        width,
                        height,
                    });
                }
                Err(e) => warn!(
                    "process_render_asset_cmds: failed to load tilemap texture '{}': {e}",
                    png_path
                ),
            }
        }
        RenderAssetCmd::RemoveTexture { key } => {
            tex_store.remove(&key);
        }
    }
}

/// Load a font with mipmaps and anisotropic filtering.
fn load_font_with_mipmaps(
    rl: &mut raylib::RaylibHandle,
    th: &raylib::RaylibThread,
    path: &str,
    size: i32,
) -> Result<raylib::prelude::Font, String> {
    let mut font = rl
        .load_font_ex(th, path, size, None)
        .map_err(|err| format!("Failed to load font '{path}': {err}"))?;
    unsafe {
        raylib::ffi::GenTextureMipmaps(&mut font.texture);
        raylib::ffi::SetTextureFilter(
            font.texture,
            raylib::ffi::TextureFilter::TEXTURE_FILTER_ANISOTROPIC_8X as i32,
        );
    }
    Ok(font)
}
