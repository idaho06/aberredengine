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

use aberred_core::protocol::endpoints::LogicTx;
use aberred_core::protocol::render_assets::RenderAssetCmd;
use aberred_core::protocol::render_logic::LogicMsg;
use aberred_core::resources::fontmetrics::{FontMetrics, GlyphMetrics};
use crate::resources::fontstore::FontStore;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::texturestore::{TextureStore, load_texture_from_text};
use crate::systems::RaylibAccess;
use crate::systems::math::color_to_raylib;
use raylib::ffi;
use raylib::prelude::Image;
use rustc_hash::FxHashMap;

/// Extract CPU-side [`FontMetrics`] from a loaded `ffi::Font`. Must be called
/// while the owning [`raylib::prelude::Font`] wrapper (or its `ffi::Font`
/// resource) is still alive — `recs`/`glyphs` are raw pointers that
/// `UnloadFont` frees on drop.
///
/// Lives here (render) rather than on `FontMetrics` itself (core) because
/// `ffi::Font` is a raylib type — `aberred-core` cannot depend on raylib.
/// `FontMetrics` the struct and `measure_text` stay core-side as a
/// pure-Rust port of raylib's `MeasureTextEx`/`GetGlyphIndex`.
pub(crate) fn extract_font_metrics(font: &ffi::Font) -> FontMetrics {
    let glyph_count = font.glyphCount.max(0) as usize;
    // SAFETY: `font.glyphs`/`font.recs` are raylib-owned arrays of
    // `glyphCount` entries, valid as long as the font hasn't been
    // unloaded (guaranteed by the caller while extracting immediately
    // after load).
    let (glyph_infos, recs) = unsafe {
        (
            std::slice::from_raw_parts(font.glyphs, glyph_count),
            std::slice::from_raw_parts(font.recs, glyph_count),
        )
    };

    let mut glyphs = FxHashMap::default();
    let mut first_glyph = None;

    for (i, glyph) in glyph_infos.iter().enumerate() {
        let metrics = GlyphMetrics {
            advance_x: glyph.advanceX,
            offset_x: glyph.offsetX,
            rec_width: recs[i].width,
        };
        if i == 0 {
            first_glyph = Some(metrics);
        }
        glyphs.insert(glyph.value, metrics);
    }

    FontMetrics {
        base_size: font.baseSize,
        glyphs,
        first_glyph,
    }
}

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
    // FontMetricsStore/TextureDimsStore are logic-world-owned:
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
                tex_store.insert(&id, tex, filter, Some(path));
                notifications.push(LogicMsg::TextureLoaded {
                    key: id,
                    width,
                    height,
                });
            }
            Err(e) => error!("Failed to load texture '{}': {}", path, e),
        },
        RenderAssetCmd::TextureFromMemory {
            id,
            ext,
            bytes,
            filter,
        } => match Image::load_image_from_mem(&ext, &bytes) {
            Ok(image) => match rl.load_texture_from_image(th, &image) {
                Ok(tex) => {
                    debug!(
                        "Loaded texture '{}' from {} in-memory bytes (ext '{}')",
                        id,
                        bytes.len(),
                        ext
                    );
                    let (width, height) = (tex.width, tex.height);
                    tex_store.insert(&id, tex, filter, None);
                    notifications.push(LogicMsg::TextureLoaded {
                        key: id,
                        width,
                        height,
                    });
                }
                Err(e) => error!("Failed to upload in-memory texture '{}': {}", id, e),
            },
            Err(e) => error!(
                "Failed to decode in-memory texture '{}' (ext '{}'): {}",
                id, ext, e
            ),
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
                    let metrics = extract_font_metrics(&font);
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
        RenderAssetCmd::ShaderFromMemory { id, vs_src, fs_src } => {
            match rl.load_shader_from_memory(th, vs_src.as_deref(), fs_src.as_deref()) {
                Ok(shader) => {
                    debug!("Loaded shader '{}' from memory", id);
                    shaders.add(&id, shader);
                }
                Err(e) => error!("Shader '{}' failed to load from memory: {e}", id),
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
            match load_texture_from_text(rl, th, font, &text, font_size, spacing, color_to_raylib(color)) {
                Some(tex) => {
                    let (width, height) = (tex.width, tex.height);
                    tex_store.insert(
                        &key,
                        tex,
                        aberred_core::resources::texturefilter::TextureFilter::Nearest,
                        None,
                    );
                    notifications.push(LogicMsg::TextureLoaded { key, width, height });
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
                        aberred_core::resources::texturefilter::TextureFilter::Nearest,
                        Some(png_path),
                    );
                    notifications.push(LogicMsg::TextureLoaded { key, width, height });
                }
                Err(e) => warn!(
                    "process_render_asset_cmds: failed to load tilemap texture '{}': {e}",
                    png_path
                ),
            }
        }
        RenderAssetCmd::RemoveTexture { key } => {
            tex_store.remove(&key);
            notifications.push(LogicMsg::TextureRemoved { key });
        }
        RenderAssetCmd::RemoveFont { key } => {
            fonts.remove(&key);
            notifications.push(LogicMsg::FontRemoved { key });
        }
        RenderAssetCmd::RenameTexture { old_key, new_key } => {
            if tex_store.rename(&old_key, new_key.clone()) {
                debug!("Renamed texture '{}' -> '{}'", old_key, new_key);
                notifications.push(LogicMsg::TextureRenamed { old_key, new_key });
            } else {
                warn!(
                    "process_render_asset_cmds: RenameTexture '{}' -> '{}': '{}' not loaded",
                    old_key, new_key, old_key
                );
            }
        }
        RenderAssetCmd::SetTextureFilter { key, filter } => {
            if !tex_store.set_filter(&key, filter) {
                warn!(
                    "process_render_asset_cmds: SetTextureFilter '{}': not loaded",
                    key
                );
            }
        }
        RenderAssetCmd::RenameFont { old_key, new_key } => {
            if fonts.rename(&old_key, new_key.clone()) {
                debug!("Renamed font '{}' -> '{}'", old_key, new_key);
                notifications.push(LogicMsg::FontRenamed { old_key, new_key });
            } else {
                warn!(
                    "process_render_asset_cmds: RenameFont '{}' -> '{}': '{}' not loaded",
                    old_key, new_key, old_key
                );
            }
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

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Windowed parity check: `extract_font_metrics(...).measure_text(...)`
    /// must match raylib's real `ffi::MeasureTextEx` for a real loaded font.
    /// Opens an actual window (needs a GL context), so this does NOT run in
    /// CI — run manually (`cargo test --features lua -- --ignored
    /// font_metrics_matches_raylib_measure_text_ex`) before landing any
    /// change to `measure_text`/`extract_font_metrics`.
    #[test]
    #[ignore]
    fn font_metrics_matches_raylib_measure_text_ex() {
        let (mut rl, thread) = raylib::init()
            .size(64, 64)
            .title("fontmetrics parity test")
            .build();

        let font = rl
            .load_font_ex(&thread, "assets/fonts/Arcade_Cabinet.ttf", 32, None)
            .expect("failed to load test font");

        let metrics = extract_font_metrics(&font);

        let corpus = [
            "Hello, world!",
            "The quick brown fox jumps over the lazy dog.",
            "multi\nline\ntext",
            "",
            "caf\u{e9} r\u{e9}sum\u{e9}", // café résumé — accented multibyte UTF-8
            "1234567890",
        ];

        for text in corpus {
            let text_c = std::ffi::CString::new(text).unwrap();
            for font_size in [16.0_f32, 32.0, 48.0] {
                for spacing in [0.0_f32, 1.0, 2.5] {
                    let expected = unsafe {
                        raylib::ffi::MeasureTextEx(*font, text_c.as_ptr(), font_size, spacing)
                    };
                    let actual = metrics.measure_text(text, font_size, spacing);
                    assert!(
                        (actual.x - expected.x).abs() < 0.01
                            && (actual.y - expected.y).abs() < 0.01,
                        "mismatch for {text:?} @ font_size={font_size} spacing={spacing}: \
                         got {actual:?}, raylib says {expected:?}"
                    );
                }
            }
        }
    }
}
