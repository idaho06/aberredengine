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

use crate::resources::fontstore::FontStore;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::texturestore::{TextureStore, load_texture_from_text};
use crate::systems::RaylibAccess;
use crate::systems::math::color_to_raylib;
use aberred_core::protocol::asset_kind::AssetKind;
use aberred_core::protocol::endpoints::LogicTx;
use aberred_core::protocol::render_assets::RenderAssetCmd;
use aberred_core::protocol::render_logic::LogicMsg;
use aberred_core::resources::font_metrics::{FontMetrics, GlyphMetrics};
use aberred_core::resources::texture_filter::TextureFilter;
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
pub(crate) fn extract_font_metrics(font: &ffi::Font, spacing: f32) -> FontMetrics {
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
        spacing,
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

/// Rejects a command that loads, removes or renames onto a reserved font key
/// (the embedded default font), returning whether it did. A rejected load is
/// answered with [`LogicMsg::AssetLoadFailed`]; a rejected remove or rename
/// has no pending load behind it, so it is only logged. Touches no store.
fn reject_reserved_font_key(cmd: &RenderAssetCmd, notifications: &mut Vec<LogicMsg>) -> bool {
    let Some(reserved) = cmd.reserved_font_key() else {
        return false;
    };
    let error = format!("font key '{reserved}' is reserved for the embedded default font");
    match cmd.load_target() {
        Some((kind, key)) => notifications.push(load_reply(kind, key.to_owned(), Err(error))),
        None => error!("Rejected {cmd:?}: {error}"),
    }
    true
}

/// Performs the GL load/upload for a single [`RenderAssetCmd`].
///
/// Every load command ([`RenderAssetCmd::load_target`] is `Some`) pushes
/// exactly one reply into `notifications`: [`LogicMsg::TextureLoaded`],
/// [`LogicMsg::FontLoaded`] or [`LogicMsg::ShaderLoaded`] on success, also
/// when the load is skipped because the key is already loaded, and
/// [`LogicMsg::AssetLoadFailed`] on failure. `FontMetricsStore`/
/// `TextureDimsStore` are logic-owned, while this function runs on the
/// render thread; the caller ([`process_render_asset_cmds`]) ships each
/// reply across the `LogicTx` channel.
pub(crate) fn apply_render_asset_cmd(
    rl: &mut raylib::RaylibHandle,
    th: &raylib::RaylibThread,
    cmd: RenderAssetCmd,
    tex_store: &mut TextureStore,
    fonts: &mut FontStore,
    shaders: &mut ShaderStore,
    notifications: &mut Vec<LogicMsg>,
) {
    if reject_reserved_font_key(&cmd, notifications) {
        return;
    }
    let target = cmd.load_target().map(|(kind, key)| (kind, key.to_owned()));
    let outcome = match cmd {
        RenderAssetCmd::Texture { key, path, filter } => match rl.load_texture(th, &path) {
            Ok(tex) => {
                debug!("Loaded texture '{}' from '{}'", key, path);
                Ok(insert_texture(tex_store, &key, tex, filter, Some(path)))
            }
            Err(e) => Err(format!("'{path}': {e}")),
        },
        RenderAssetCmd::TextureFromMemory {
            key,
            ext,
            bytes,
            filter,
        } => Image::load_image_from_mem(&ext, &bytes)
            .map_err(|e| format!("decoding {} bytes (ext '{ext}'): {e}", bytes.len()))
            .and_then(|image| {
                rl.load_texture_from_image(th, &image)
                    .map_err(|e| format!("uploading: {e}"))
            })
            .map(|tex| {
                debug!(
                    "Loaded texture '{}' from {} in-memory bytes (ext '{}')",
                    key,
                    bytes.len(),
                    ext
                );
                insert_texture(tex_store, &key, tex, filter, None)
            }),
        RenderAssetCmd::Font {
            key,
            path,
            size,
            spacing,
            skip_if_loaded,
        } => {
            if skip_if_loaded && let Some((font, spacing)) = fonts.get_with_spacing(&key) {
                debug!(
                    "process_render_asset_cmds: font '{}' already loaded, skipping",
                    key
                );
                Ok(LogicMsg::FontLoaded {
                    key,
                    metrics: extract_font_metrics(font, spacing),
                })
            } else {
                load_font_with_mipmaps(rl, th, &path, size).map(|font| {
                    debug!("Loaded font '{}' from '{}'", key, path);
                    let metrics = extract_font_metrics(&font, spacing);
                    fonts.add(&key, font, spacing);
                    LogicMsg::FontLoaded { key, metrics }
                })
            }
        }
        RenderAssetCmd::Shader {
            key,
            vs_path,
            fs_path,
        } => match rl.load_shader(th, vs_path.as_deref(), fs_path.as_deref()) {
            Ok(shader) if shader.is_shader_valid() => {
                debug!(
                    "Loaded shader '{}' (vs: {:?}, fs: {:?})",
                    key, vs_path, fs_path
                );
                shaders.add(&key, shader);
                Ok(LogicMsg::ShaderLoaded { key })
            }
            Ok(_) => Err(format!(
                "loaded but invalid (vs: {vs_path:?}, fs: {fs_path:?})"
            )),
            Err(e) => Err(format!("{e} (vs: {vs_path:?}, fs: {fs_path:?})")),
        },
        RenderAssetCmd::ShaderFromMemory {
            key,
            vs_src,
            fs_src,
        } => rl
            .load_shader_from_memory(th, vs_src.as_deref(), fs_src.as_deref())
            .map(|shader| {
                debug!("Loaded shader '{}' from memory", key);
                shaders.add(&key, shader);
                LogicMsg::ShaderLoaded { key }
            })
            .map_err(|e| format!("from memory: {e}")),
        RenderAssetCmd::RasterizeText {
            key,
            font_key,
            text,
            font_size,
            spacing,
            color,
        } => match fonts.get_or_default(&font_key) {
            None => Err(format!("font '{font_key}' is not loaded")),
            Some((font, _)) => load_texture_from_text(
                rl,
                th,
                font,
                &text,
                font_size,
                spacing,
                color_to_raylib(color),
            )
            .map(|tex| insert_texture(tex_store, &key, tex, TextureFilter::Nearest, None))
            .ok_or_else(|| format!("rasterizing text with font '{font_key}'")),
        },
        RenderAssetCmd::TilemapTexture { key, png_path } => match tex_store.get(&key) {
            Some(tex) => Ok(LogicMsg::TextureLoaded {
                key,
                width: tex.width,
                height: tex.height,
            }),
            None => match rl.load_texture(th, &png_path) {
                Ok(tex) => Ok(insert_texture(
                    tex_store,
                    &key,
                    tex,
                    TextureFilter::Nearest,
                    Some(png_path),
                )),
                Err(e) => Err(format!("'{png_path}': {e}")),
            },
        },
        RenderAssetCmd::RemoveTexture { key } => {
            tex_store.remove(&key);
            notifications.push(LogicMsg::TextureRemoved { key });
            return;
        }
        RenderAssetCmd::RemoveFont { key } => {
            fonts.remove(&key);
            notifications.push(LogicMsg::FontRemoved { key });
            return;
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
            return;
        }
        RenderAssetCmd::SetTextureFilter { key, filter } => {
            if !tex_store.set_filter(&key, filter) {
                warn!(
                    "process_render_asset_cmds: SetTextureFilter '{}': not loaded",
                    key
                );
            }
            return;
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
            return;
        }
    };
    let (kind, key) = target.expect("only load commands reach the reply");
    notifications.push(load_reply(kind, key, outcome));
}

/// Stores a freshly loaded texture under `key` and builds its
/// [`LogicMsg::TextureLoaded`] reply.
fn insert_texture(
    tex_store: &mut TextureStore,
    key: &str,
    tex: raylib::prelude::Texture2D,
    filter: TextureFilter,
    path: Option<String>,
) -> LogicMsg {
    let (width, height) = (tex.width, tex.height);
    tex_store.insert(key, tex, filter, path);
    LogicMsg::TextureLoaded {
        key: key.to_owned(),
        width,
        height,
    }
}

/// The reply to one load command: its success message, or a logged
/// [`LogicMsg::AssetLoadFailed`] for `kind`/`key` when it failed.
fn load_reply(kind: AssetKind, key: String, outcome: Result<LogicMsg, String>) -> LogicMsg {
    outcome.unwrap_or_else(|error| {
        error!("Failed to load {kind:?} '{key}': {error}");
        LogicMsg::AssetLoadFailed { kind, key, error }
    })
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
    use aberred_core::resources::font_metrics::TEXT_SPACING;

    #[test]
    fn a_successful_load_replies_with_its_own_message() {
        let reply = load_reply(
            AssetKind::Shader,
            "glow".to_owned(),
            Ok(LogicMsg::ShaderLoaded {
                key: "glow".to_owned(),
            }),
        );
        assert!(matches!(reply, LogicMsg::ShaderLoaded { key } if key == "glow"));
    }

    fn font_cmd(key: &str) -> RenderAssetCmd {
        RenderAssetCmd::Font {
            key: key.to_owned(),
            path: "f.ttf".to_owned(),
            size: 16,
            spacing: 1.0,
            skip_if_loaded: false,
        }
    }

    #[test]
    fn a_reserved_font_load_is_answered_with_asset_load_failed() {
        for key in ["default", ""] {
            let mut out = Vec::new();
            assert!(reject_reserved_font_key(&font_cmd(key), &mut out));
            assert!(matches!(
                out.as_slice(),
                [LogicMsg::AssetLoadFailed { kind: AssetKind::Font, key: k, .. }] if k == key
            ));
        }
    }

    #[test]
    fn a_reserved_font_remove_or_rename_is_dropped_without_a_reply() {
        let cmds = [
            RenderAssetCmd::RemoveFont {
                key: "default".to_owned(),
            },
            RenderAssetCmd::RenameFont {
                old_key: "default".to_owned(),
                new_key: "x".to_owned(),
            },
            RenderAssetCmd::RenameFont {
                old_key: "ui".to_owned(),
                new_key: "".to_owned(),
            },
        ];
        for cmd in &cmds {
            let mut out = Vec::new();
            assert!(reject_reserved_font_key(cmd, &mut out), "{cmd:?}");
            assert!(out.is_empty(), "{cmd:?}");
        }
    }

    #[test]
    fn an_ordinary_font_command_is_not_rejected() {
        let mut out = Vec::new();
        assert!(!reject_reserved_font_key(&font_cmd("ui"), &mut out));
        assert!(out.is_empty());
    }

    #[test]
    fn a_failed_load_replies_asset_load_failed() {
        let reply = load_reply(
            AssetKind::Texture,
            "player".to_owned(),
            Err("file not found".to_owned()),
        );
        assert!(matches!(
            reply,
            LogicMsg::AssetLoadFailed { kind: AssetKind::Texture, key, error }
                if key == "player" && error == "file not found"
        ));
    }

    /// Windowed parity check: `extract_font_metrics(...).measure_text(...)`
    /// must match raylib's real `ffi::MeasureTextEx` for a real loaded font.
    /// Opens an actual window (needs a GL context), so it never runs in
    /// `just check`. It is the only parity check for
    /// `FontMetrics::measure_text`: run it by hand on a machine with a
    /// display before landing any change to `measure_text`/
    /// `extract_font_metrics`:
    ///
    /// `cargo test -p aberred-render --lib -- --ignored font_metrics_matches_raylib_measure_text_ex`
    #[test]
    #[ignore = "opens a window; run by hand, see doc comment"]
    fn font_metrics_matches_raylib_measure_text_ex() {
        let (mut rl, thread) = raylib::init()
            .size(64, 64)
            .title("fontmetrics parity test")
            .build();

        let font = rl
            .load_font_ex(
                &thread,
                // `cargo test` runs from this crate's root; the assets live
                // at the workspace root.
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/fonts/Arcade_Cabinet.ttf"
                ),
                32,
                None,
            )
            .expect("failed to load test font");

        let metrics = extract_font_metrics(&font, TEXT_SPACING);

        let corpus = [
            "Hello, world!",
            "The quick brown fox jumps over the lazy dog.",
            "multi\nline\ntext",
            "",
            "caf\u{e9} r\u{e9}sum\u{e9}", // café résumé — accented multibyte UTF-8
            "1234567890",
        ];

        // One metrics copy per spacing, not one per measurement.
        let by_spacing = [0.0_f32, 1.0, 2.5].map(|s| (s, metrics.clone().with_spacing(s)));

        for text in corpus {
            let text_c = std::ffi::CString::new(text).unwrap();
            for font_size in [16.0_f32, 32.0, 48.0] {
                for (spacing, metrics) in &by_spacing {
                    let spacing = *spacing;
                    let expected = unsafe {
                        raylib::ffi::MeasureTextEx(*font, text_c.as_ptr(), font_size, spacing)
                    };
                    let actual = metrics.measure_text(text, font_size);
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
