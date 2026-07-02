//! CPU-side font metrics resource.
//!
//! [`FontMetricsStore`] holds per-glyph measurement data extracted from each
//! loaded font at load time, keyed by the same string ID used by
//! [`FontStore`](crate::resources::fontstore::FontStore). This lets
//! logic-side systems (e.g. `dynamictext_size_system`) measure text without
//! touching the GL-bound `FontStore`/`NonSend<RaylibHandle>` — a requirement
//! for splitting simulation logic onto its own thread, separate from the
//! render/GL thread.
//!
//! [`FontMetrics::measure_text`] is a line-for-line port of raylib's
//! `MeasureTextEx`/`GetGlyphIndex` (`rtext.c`, vendored in
//! `sola-raylib-sys` 6.2.0). Any behavior change to those C functions in a
//! future raylib upgrade must be re-ported here.

use bevy_ecs::prelude::Resource;
use raylib::ffi;
use raylib::math::Vector2;
use rustc_hash::FxHashMap;

/// raylib's default `textLineSpacing` (`rtext.c`'s static global, default
/// `2`, only changeable via `SetTextLineSpacing()`). Nothing in this engine
/// calls that function, so it is safe to hardcode here.
const TEXT_LINE_SPACING: f32 = 2.0;

/// Fallback codepoint raylib's `GetGlyphIndex` looks for when the requested
/// codepoint isn't in the font (`'?'`).
const FALLBACK_CODEPOINT: i32 = 63;

/// CPU-side metrics for one glyph, extracted from a GL-loaded `ffi::Font`.
#[derive(Debug, Clone, Copy)]
pub struct GlyphMetrics {
    pub advance_x: i32,
    pub offset_x: i32,
    pub rec_width: f32,
}

/// CPU-side measurement data for one font, keyed by codepoint. `Send + Sync`
/// so it can live on a logic thread separate from the GL context that
/// loaded the font.
#[derive(Debug, Clone, Default)]
pub struct FontMetrics {
    pub base_size: i32,
    /// Keyed by Unicode codepoint (`GlyphInfo.value`), mirroring raylib's
    /// `font.glyphs[i].value`. The `'?'` fallback glyph (if present) lives
    /// here under its own codepoint (63) — no separate field needed, see
    /// [`resolve_glyph`](Self::resolve_glyph).
    pub glyphs: FxHashMap<i32, GlyphMetrics>,
    /// Metrics of the first glyph in the font's original array order —
    /// raylib's final fallback when the codepoint isn't found and no `'?'`
    /// glyph exists (`GetGlyphIndex` falls back to index 0).
    pub(crate) first_glyph: Option<GlyphMetrics>,
}

impl FontMetrics {
    /// Extract CPU-side metrics from a loaded `ffi::Font`. Must be called
    /// while the owning [`raylib::prelude::Font`] wrapper (or its `ffi::Font`
    /// resource) is still alive — `recs`/`glyphs` are raw pointers that
    /// `UnloadFont` frees on drop.
    pub fn extract(font: &ffi::Font) -> Self {
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

        Self {
            base_size: font.baseSize,
            glyphs,
            first_glyph,
        }
    }

    /// Resolve a codepoint to its glyph metrics, replicating raylib's
    /// `GetGlyphIndex` fallback chain: exact match, then the `'?'` glyph,
    /// then the font's first glyph (index 0) if no `'?'` glyph exists.
    fn resolve_glyph(&self, codepoint: i32) -> Option<GlyphMetrics> {
        self.glyphs
            .get(&codepoint)
            .or_else(|| self.glyphs.get(&FALLBACK_CODEPOINT))
            .copied()
            .or(self.first_glyph)
    }

    /// Faithful port of raylib's `MeasureTextEx` (`rtext.c`). `spacing` is
    /// the extra pixels inserted between glyphs (same argument raylib
    /// takes).
    pub fn measure_text(&self, text: &str, font_size: f32, spacing: f32) -> Vector2 {
        if text.is_empty() || self.base_size == 0 {
            return Vector2::new(0.0, 0.0);
        }

        let scale_factor = font_size / self.base_size as f32;

        let mut text_width: f32 = 0.0;
        let mut temp_text_width: f32 = 0.0;
        let mut text_height = font_size;
        let mut char_count: i32 = 0;
        let mut max_char_count: i32 = 0;

        for ch in text.chars() {
            char_count += 1;
            if ch != '\n' {
                let codepoint = ch as i32;
                if let Some(glyph) = self.resolve_glyph(codepoint) {
                    if glyph.advance_x > 0 {
                        text_width += glyph.advance_x as f32;
                    } else {
                        text_width += glyph.rec_width + glyph.offset_x as f32;
                    }
                }
            } else {
                if temp_text_width < text_width {
                    temp_text_width = text_width;
                }
                char_count = 0;
                text_width = 0.0;
                text_height += font_size + TEXT_LINE_SPACING;
            }
            if max_char_count < char_count {
                max_char_count = char_count;
            }
        }

        if temp_text_width < text_width {
            temp_text_width = text_width;
        }

        let width = temp_text_width * scale_factor + (max_char_count - 1).max(0) as f32 * spacing;
        Vector2::new(width, text_height)
    }
}

/// Map of font keys to CPU-side measurement data, populated at every
/// font-load site alongside `FontStore`. See the module docs above.
#[derive(Resource, Default)]
pub struct FontMetricsStore(pub FxHashMap<String, FontMetrics>);

/// Tracks which font keys have already logged a "missing from
/// `FontMetricsStore`" warning, so `dynamictext_size_system` warns once per
/// key instead of every frame. Same warn-once-per-key shape as
/// [`GuiThemeWarnCache`](crate::resources::guitheme::GuiThemeWarnCache),
/// sharing its underlying bookkeeping via
/// [`warn_once::first_seen`](crate::resources::warn_once::first_seen).
#[derive(Resource, Default)]
pub struct FontMetricsWarnCache(rustc_hash::FxHashSet<std::sync::Arc<str>>);

impl FontMetricsWarnCache {
    /// Returns `true` the first time `key` is reported missing, `false` on
    /// every subsequent call for the same key.
    pub fn warn_once(&mut self, key: &str) -> bool {
        crate::resources::warn_once::first_seen(&mut self.0, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(advance_x: i32, offset_x: i32, rec_width: f32) -> GlyphMetrics {
        GlyphMetrics {
            advance_x,
            offset_x,
            rec_width,
        }
    }

    /// Build a fixture with monospace-ish glyphs for 'a'..'z', a distinct
    /// '?' fallback glyph, and a zero-advance glyph 'Z' (exercises the
    /// rec_width+offset_x fallback branch).
    fn fixture() -> FontMetrics {
        let mut glyphs = FxHashMap::default();
        for c in 'a'..='z' {
            glyphs.insert(c as i32, glyph(10, 0, 8.0));
        }
        glyphs.insert('?' as i32, glyph(6, 0, 5.0));
        glyphs.insert('Z' as i32, glyph(0, 2, 7.0)); // advance_x == 0 -> rec_width+offset_x = 9.0

        let first_glyph = glyphs[&('a' as i32)];

        FontMetrics {
            base_size: 20,
            glyphs,
            first_glyph: Some(first_glyph),
        }
    }

    #[test]
    fn single_line_width_uses_advance_x() {
        let m = fixture();
        // "abc" @ font_size == base_size (scale 1.0), spacing 1.0:
        // width = 3*10 * 1.0 + (3-1)*1.0 = 32.0
        let size = m.measure_text("abc", 20.0, 1.0);
        assert_eq!(size.x, 32.0);
        assert_eq!(size.y, 20.0);
    }

    #[test]
    fn zero_advance_glyph_falls_back_to_rec_width_plus_offset() {
        let m = fixture();
        // "Z" -> advance_x == 0, so width = rec_width(7) + offset_x(2) = 9.0
        // scale 1.0, spacing 0 -> (1-1)*spacing = 0
        let size = m.measure_text("Z", 20.0, 0.0);
        assert_eq!(size.x, 9.0);
    }

    #[test]
    fn missing_codepoint_falls_back_to_question_mark_glyph() {
        let m = fixture();
        // '9' isn't in the fixture -> falls back to '?' (advance_x=6)
        let size = m.measure_text("9", 20.0, 0.0);
        assert_eq!(size.x, 6.0);
    }

    #[test]
    fn missing_codepoint_falls_back_to_first_glyph_when_no_question_mark() {
        let mut m = fixture();
        m.glyphs.remove(&('?' as i32));
        // '9' missing, no '?' glyph -> falls back to first_glyph ('a', advance_x=10)
        let size = m.measure_text("9", 20.0, 0.0);
        assert_eq!(size.x, 10.0);
    }

    #[test]
    fn multiline_height_and_width_use_longest_line() {
        let m = fixture();
        // "aa\na" : line 1 width = 20 (2 chars), line 2 width = 10 (1 char)
        // longest line char count = 2 -> spacing term = (2-1)*1.0 = 1.0
        // temp_text_width tracks the widest line's raw width = 20.0
        // height = font_size + 1*(font_size + TEXT_LINE_SPACING) = 20 + 22 = 42
        let size = m.measure_text("aa\na", 20.0, 1.0);
        assert_eq!(size.x, 20.0 + 1.0);
        assert_eq!(size.y, 42.0);
    }

    #[test]
    fn scale_factor_applies_when_font_size_differs_from_base_size() {
        let m = fixture();
        // "a" @ font_size 40 (base_size 20) -> scale 2.0
        // width = 10 * 2.0 + (1-1)*spacing = 20.0
        let size = m.measure_text("a", 40.0, 1.0);
        assert_eq!(size.x, 20.0);
        // height is NOT scaled - it's font_size directly for a single line
        assert_eq!(size.y, 40.0);
    }

    #[test]
    fn empty_text_measures_zero() {
        let m = fixture();
        let size = m.measure_text("", 20.0, 1.0);
        assert_eq!(size.x, 0.0);
        assert_eq!(size.y, 0.0);
    }

    /// Windowed parity check: `FontMetrics::extract(...).measure_text(...)`
    /// must match raylib's real `ffi::MeasureTextEx` for a real loaded font.
    /// Opens an actual window (needs a GL context), so this does NOT run in
    /// CI — run manually (`cargo test --features lua -- --ignored
    /// font_metrics_matches_raylib_measure_text_ex`) before landing any
    /// change to `measure_text`/`extract`.
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

        let metrics = FontMetrics::extract(&font);

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
