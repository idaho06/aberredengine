//! The embedded default font (Departure Mono, SIL OFL 1.1).
//!
//! The bytes are the unmodified upstream `.otf`; its license text ships next
//! to it in `fonts/`. `DEFAULT_FONT_CHARS` is the set of characters rasterized
//! into its atlas: `DEFAULT_FONT_RANGES` intersected with the glyphs the
//! font actually has, so raylib never rasterizes `.notdef` boxes.

use aberred_core::error::EngineError;
use aberred_core::resources::font_metrics::FontMetrics;
use raylib::prelude::{Font, RaylibHandle, RaylibThread};
#[cfg(test)]
use std::ops::RangeInclusive;

use crate::systems::extract_font_metrics;

/// The embedded font file.
pub(crate) const DEFAULT_FONT_BYTES: &[u8] =
    include_bytes!("../../fonts/DepartureMono-Regular.otf");

/// Glyph spacing the default font loads with. It is a pixel font whose advance
/// is exactly its cell width, so any extra spacing opens gaps in box-drawing
/// and block glyphs.
pub const DEFAULT_FONT_SPACING: f32 = 0.0;

/// Pixel height the default font is rasterized at, one design pixel per
/// screen pixel. raylib scales by `size / (ascender - descender)`, so this is
/// the font's cell height in design pixels; drawing at a multiple of it keeps
/// every glyph pixel on the screen pixel grid.
pub(crate) const DEFAULT_FONT_SIZE: i32 = 14;

/// Unicode ranges the default font is expected to cover: Latin, punctuation,
/// numerals, currency, math, arrows, and the graphical sets (box drawing,
/// block elements, geometric shapes) that let a progress bar be drawn with
/// text only.
#[cfg(test)]
pub(crate) const DEFAULT_FONT_RANGES: &[RangeInclusive<u32>] = &[
    0x0020..=0x007E,   // Basic Latin
    0x00A0..=0x00FF,   // Latin-1 Supplement
    0x0100..=0x017F,   // Latin Extended-A
    0x2000..=0x206F,   // General Punctuation
    0x2070..=0x209F,   // Superscripts and Subscripts
    0x20A0..=0x20CF,   // Currency Symbols
    0x2116..=0x2116,   // №
    0x2150..=0x218F,   // Number Forms (fractions)
    0x2190..=0x21FF,   // Arrows
    0x2200..=0x22FF,   // Mathematical Operators
    0x2500..=0x257F,   // Box Drawing
    0x2580..=0x259F,   // Block Elements
    0x25A0..=0x25FF,   // Geometric Shapes
    0x2605..=0x2606,   // Stars
    0x2660..=0x266F,   // Card suits and music notes
    0x2726..=0x2727,   // Stars
    0x276C..=0x2771,   // Ornamental brackets
    0x1FB82..=0x1FB8B, // Legacy Computing eighths
];

/// Characters rasterized into the default font's atlas, in ascending order.
pub(crate) const DEFAULT_FONT_CHARS: &str = "\
    \u{20}\u{21}\u{22}\u{23}\u{24}\u{25}\u{26}\u{27}\u{28}\u{29}\
    \u{2A}\u{2B}\u{2C}\u{2D}\u{2E}\u{2F}\u{30}\u{31}\u{32}\u{33}\
    \u{34}\u{35}\u{36}\u{37}\u{38}\u{39}\u{3A}\u{3B}\u{3C}\u{3D}\
    \u{3E}\u{3F}\u{40}\u{41}\u{42}\u{43}\u{44}\u{45}\u{46}\u{47}\
    \u{48}\u{49}\u{4A}\u{4B}\u{4C}\u{4D}\u{4E}\u{4F}\u{50}\u{51}\
    \u{52}\u{53}\u{54}\u{55}\u{56}\u{57}\u{58}\u{59}\u{5A}\u{5B}\
    \u{5C}\u{5D}\u{5E}\u{5F}\u{60}\u{61}\u{62}\u{63}\u{64}\u{65}\
    \u{66}\u{67}\u{68}\u{69}\u{6A}\u{6B}\u{6C}\u{6D}\u{6E}\u{6F}\
    \u{70}\u{71}\u{72}\u{73}\u{74}\u{75}\u{76}\u{77}\u{78}\u{79}\
    \u{7A}\u{7B}\u{7C}\u{7D}\u{7E}\u{A0}\u{A1}\u{A2}\u{A3}\u{A4}\
    \u{A5}\u{A6}\u{A7}\u{A8}\u{A9}\u{AA}\u{AB}\u{AC}\u{AD}\u{AE}\
    \u{AF}\u{B0}\u{B1}\u{B2}\u{B3}\u{B4}\u{B5}\u{B6}\u{B7}\u{B8}\
    \u{B9}\u{BA}\u{BB}\u{BC}\u{BD}\u{BE}\u{BF}\u{C0}\u{C1}\u{C2}\
    \u{C3}\u{C4}\u{C5}\u{C6}\u{C7}\u{C8}\u{C9}\u{CA}\u{CB}\u{CC}\
    \u{CD}\u{CE}\u{CF}\u{D0}\u{D1}\u{D2}\u{D3}\u{D4}\u{D5}\u{D6}\
    \u{D7}\u{D8}\u{D9}\u{DA}\u{DB}\u{DC}\u{DD}\u{DE}\u{DF}\u{E0}\
    \u{E1}\u{E2}\u{E3}\u{E4}\u{E5}\u{E6}\u{E7}\u{E8}\u{E9}\u{EA}\
    \u{EB}\u{EC}\u{ED}\u{EE}\u{EF}\u{F0}\u{F1}\u{F2}\u{F3}\u{F4}\
    \u{F5}\u{F6}\u{F7}\u{F8}\u{F9}\u{FA}\u{FB}\u{FC}\u{FD}\u{FE}\
    \u{FF}\u{100}\u{101}\u{102}\u{103}\u{104}\u{105}\u{106}\u{107}\u{108}\
    \u{109}\u{10A}\u{10B}\u{10C}\u{10D}\u{10E}\u{10F}\u{110}\u{111}\u{112}\
    \u{113}\u{114}\u{115}\u{116}\u{117}\u{118}\u{119}\u{11A}\u{11B}\u{11C}\
    \u{11D}\u{11E}\u{11F}\u{120}\u{121}\u{122}\u{123}\u{124}\u{125}\u{126}\
    \u{127}\u{128}\u{129}\u{12A}\u{12B}\u{12C}\u{12D}\u{12E}\u{12F}\u{130}\
    \u{131}\u{132}\u{133}\u{134}\u{135}\u{136}\u{137}\u{138}\u{139}\u{13A}\
    \u{13B}\u{13C}\u{13D}\u{13E}\u{13F}\u{140}\u{141}\u{142}\u{143}\u{144}\
    \u{145}\u{146}\u{147}\u{148}\u{14A}\u{14B}\u{14C}\u{14D}\u{14E}\u{14F}\
    \u{150}\u{151}\u{152}\u{153}\u{154}\u{155}\u{156}\u{157}\u{158}\u{159}\
    \u{15A}\u{15B}\u{15C}\u{15D}\u{15E}\u{15F}\u{160}\u{161}\u{162}\u{163}\
    \u{164}\u{165}\u{166}\u{167}\u{168}\u{169}\u{16A}\u{16B}\u{16C}\u{16D}\
    \u{16E}\u{16F}\u{170}\u{171}\u{172}\u{173}\u{174}\u{175}\u{176}\u{177}\
    \u{178}\u{179}\u{17A}\u{17B}\u{17C}\u{17D}\u{17E}\u{17F}\u{2010}\u{2013}\
    \u{2014}\u{2015}\u{2018}\u{2019}\u{201A}\u{201C}\u{201D}\u{201E}\u{2020}\u{2021}\
    \u{2022}\u{2026}\u{2027}\u{2030}\u{2032}\u{2033}\u{2039}\u{203A}\u{2044}\u{204A}\
    \u{2052}\u{2070}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}\u{2080}\u{2081}\
    \u{2082}\u{2083}\u{2084}\u{2085}\u{2086}\u{2087}\u{2088}\u{2089}\u{20A3}\u{20A9}\
    \u{20AA}\u{20AC}\u{20AE}\u{20B4}\u{20B8}\u{20B9}\u{20BA}\u{20BD}\u{20BF}\u{2116}\
    \u{2150}\u{2151}\u{2153}\u{2154}\u{2155}\u{2156}\u{2157}\u{2158}\u{2159}\u{215A}\
    \u{215B}\u{215C}\u{215D}\u{215E}\u{2189}\u{2190}\u{2191}\u{2192}\u{2193}\u{2195}\
    \u{2196}\u{2197}\u{2198}\u{2199}\u{21B0}\u{21B1}\u{21B2}\u{21B3}\u{21B4}\u{21B5}\
    \u{2202}\u{2206}\u{220F}\u{2211}\u{2212}\u{2219}\u{221A}\u{221E}\u{222B}\u{2248}\
    \u{2260}\u{2264}\u{2265}\u{2500}\u{2501}\u{2502}\u{2503}\u{2504}\u{2505}\
    \u{2506}\u{2507}\u{2508}\u{2509}\u{250A}\u{250B}\u{250C}\u{250D}\u{250E}\u{250F}\
    \u{2510}\u{2511}\u{2512}\u{2513}\u{2514}\u{2515}\u{2516}\u{2517}\u{2518}\u{2519}\
    \u{251A}\u{251B}\u{251C}\u{251D}\u{251E}\u{251F}\u{2520}\u{2521}\u{2522}\u{2523}\
    \u{2524}\u{2525}\u{2526}\u{2527}\u{2528}\u{2529}\u{252A}\u{252B}\u{252C}\u{252D}\
    \u{252E}\u{252F}\u{2530}\u{2531}\u{2532}\u{2533}\u{2534}\u{2535}\u{2536}\u{2537}\
    \u{2538}\u{2539}\u{253A}\u{253B}\u{253C}\u{253D}\u{253E}\u{253F}\u{2540}\u{2541}\
    \u{2542}\u{2543}\u{2544}\u{2545}\u{2546}\u{2547}\u{2548}\u{2549}\u{254A}\u{254B}\
    \u{254C}\u{254D}\u{254E}\u{254F}\u{2550}\u{2551}\u{2552}\u{2553}\u{2554}\u{2555}\
    \u{2556}\u{2557}\u{2558}\u{2559}\u{255A}\u{255B}\u{255C}\u{255D}\u{255E}\u{255F}\
    \u{2560}\u{2561}\u{2562}\u{2563}\u{2564}\u{2565}\u{2566}\u{2567}\u{2568}\u{2569}\
    \u{256A}\u{256B}\u{256C}\u{256D}\u{256E}\u{256F}\u{2570}\u{2571}\u{2572}\u{2573}\
    \u{2574}\u{2575}\u{2576}\u{2577}\u{2578}\u{2579}\u{257A}\u{257B}\u{257C}\u{257D}\
    \u{257E}\u{257F}\u{2580}\u{2581}\u{2582}\u{2583}\u{2584}\u{2585}\u{2586}\u{2587}\
    \u{2588}\u{2589}\u{258A}\u{258B}\u{258C}\u{258D}\u{258E}\u{258F}\u{2590}\u{2591}\
    \u{2592}\u{2593}\u{2594}\u{2595}\u{2596}\u{2597}\u{2598}\u{2599}\u{259A}\u{259B}\
    \u{259C}\u{259D}\u{259E}\u{259F}\u{25A0}\u{25A1}\u{25AA}\u{25AB}\u{25B2}\u{25B3}\
    \u{25B7}\u{25BA}\u{25BC}\u{25BD}\u{25C1}\u{25C4}\u{25CA}\u{25CC}\u{2605}\u{2606}\u{2660}\u{2663}\u{2665}\u{2666}\u{2669}\u{266A}\u{266B}\u{266C}\
    \u{2726}\u{2727}\u{276C}\u{276D}\u{276E}\u{276F}\u{2770}\u{2771}\u{1FB82}\u{1FB83}\
    \u{1FB84}\u{1FB85}\u{1FB86}\u{1FB87}\u{1FB88}\u{1FB89}\u{1FB8A}\u{1FB8B}\
    ";

/// Rasterizes the embedded default font into a GL texture with point
/// sampling (no mipmaps: they blur a pixel font) and extracts its CPU-side
/// metrics. Needs a window and GL context.
pub fn load_default_font(
    rl: &mut RaylibHandle,
    th: &RaylibThread,
) -> Result<(Font, FontMetrics), EngineError> {
    let font = rl
        .load_font_from_memory(
            th,
            ".otf",
            DEFAULT_FONT_BYTES,
            DEFAULT_FONT_SIZE,
            Some(DEFAULT_FONT_CHARS),
        )
        .map_err(|e| EngineError::DefaultFont {
            message: e.to_string(),
        })?;
    // SAFETY: `font.texture` is the live texture raylib just created for
    // this font, and a GL context exists (the caller holds `rl`).
    unsafe {
        raylib::ffi::SetTextureFilter(
            font.texture,
            raylib::ffi::TextureFilter::TEXTURE_FILTER_POINT as i32,
        );
    }
    let metrics = extract_font_metrics(&font, DEFAULT_FONT_SPACING);
    Ok((font, metrics))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::fontstore::FontStore;
    use ttf_parser::Face;

    fn face() -> Face<'static> {
        Face::parse(DEFAULT_FONT_BYTES, 0).expect("the embedded font parses")
    }

    fn covered_by_ranges(face: &Face) -> Vec<char> {
        DEFAULT_FONT_RANGES
            .iter()
            .flat_map(|r| r.clone())
            .filter_map(char::from_u32)
            .filter(|&c| face.glyph_index(c).is_some())
            .collect()
    }

    #[test]
    fn the_native_size_puts_glyph_edges_on_whole_pixels() {
        let face = face();
        let a = face.glyph_index('A').unwrap();
        let bbox = face.glyph_bounding_box(a).unwrap();
        let cell = i32::from(face.ascender()) - i32::from(face.descender());
        let edges = [
            i32::from(face.glyph_hor_advance(a).unwrap()),
            i32::from(bbox.x_min),
            i32::from(bbox.x_max),
            i32::from(bbox.y_min),
            i32::from(bbox.y_max),
        ];
        const { assert!(DEFAULT_FONT_SIZE > 0) };
        for edge in edges {
            assert_eq!(
                edge * DEFAULT_FONT_SIZE % cell,
                0,
                "{edge} font units are not whole pixels at size {DEFAULT_FONT_SIZE}"
            );
        }
    }

    #[test]
    fn every_listed_char_has_a_glyph() {
        let face = face();
        for c in DEFAULT_FONT_CHARS.chars() {
            assert!(
                face.glyph_index(c).is_some(),
                "no glyph for U+{:04X}",
                c as u32
            );
        }
    }

    #[test]
    fn the_list_is_exactly_the_ranges_the_font_covers() {
        let expected: String = covered_by_ranges(&face()).into_iter().collect();
        assert_eq!(DEFAULT_FONT_CHARS, expected);
    }

    #[test]
    fn the_list_is_sorted_and_deduplicated() {
        let chars: Vec<char> = DEFAULT_FONT_CHARS.chars().collect();
        assert!(chars.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn the_list_contains_the_characters_games_rely_on() {
        for c in "█▏▎▍▌▋▊▉░▒▓━┃ñ¿€≤→★♥".chars() {
            assert!(DEFAULT_FONT_CHARS.contains(c), "missing {c:?}");
        }
    }

    #[test]
    fn the_font_is_cff_and_monospaced() {
        let face = face();
        assert!(
            face.tables().cff.is_some(),
            "expected OpenType/CFF outlines"
        );
        assert!(face.is_monospaced());
    }

    /// Windowed check of the embedded font: it rasterizes (the OTF/CFF
    /// outlines go through raylib's stb_truetype), the glyphs the games rely on
    /// exist, the font is monospaced, and `extract_font_metrics` agrees with
    /// raylib's `MeasureTextEx`. Opens a window, so it never runs in
    /// `just check`; run it on a machine with a display:
    ///
    /// `cargo test -p aberred-render --lib -- --ignored --nocapture default_font_loads`
    #[test]
    #[ignore = "opens a window; run by hand, see doc comment"]
    fn default_font_loads_and_measures_like_raylib() {
        let (mut rl, thread) = raylib::init()
            .size(64, 64)
            .title("default font test")
            .build();

        let (font, metrics) = load_default_font(&mut rl, &thread).expect("the default font loads");

        let advances: std::collections::BTreeSet<i32> = "A\u{f1}\u{2588}\u{258f}\u{2192}\u{20ac}"
            .chars()
            .map(|c| {
                metrics
                    .glyphs
                    .get(&(c as i32))
                    .unwrap_or_else(|| panic!("no glyph for {c:?}"))
                    .advance_x
            })
            .collect();
        assert_eq!(advances.len(), 1, "monospaced advances: {advances:?}");

        let missing: Vec<String> = DEFAULT_FONT_CHARS
            .chars()
            .filter(|&c| !metrics.glyphs.contains_key(&(c as i32)))
            .map(|c| format!("U+{:04X}", c as u32))
            .collect();
        assert!(missing.is_empty(), "listed but not loaded: {missing:?}");

        let sample = "Loading... 42% \u{2588}\u{2588}\u{258c}";
        assert_eq!(metrics.spacing, DEFAULT_FONT_SPACING);
        let ours = metrics.measure_text(sample, DEFAULT_FONT_SIZE as f32);
        let theirs = unsafe {
            let c = std::ffi::CString::new(sample).unwrap();
            raylib::ffi::MeasureTextEx(
                *font.as_ref(),
                c.as_ptr(),
                DEFAULT_FONT_SIZE as f32,
                DEFAULT_FONT_SPACING,
            )
        };
        assert_eq!((ours.x, ours.y), (theirs.x, theirs.y));

        eprintln!(
            "default font: atlas {}x{}, base size {}, {} glyphs, advance {:?}",
            font.texture.width,
            font.texture.height,
            font.baseSize,
            metrics.glyphs.len(),
            advances
        );

        // The store hands the default font and its spacing to every lookup
        // that names it, is empty or is unknown.
        let store = FontStore::with_default(font, DEFAULT_FONT_SPACING);
        for key in ["default", "", "typo"] {
            let (_, spacing) = store.get_or_default(key).expect("the default font");
            assert_eq!(spacing, DEFAULT_FONT_SPACING, "{key:?}");
        }
    }
}
