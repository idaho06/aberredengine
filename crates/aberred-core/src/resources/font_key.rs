//! Reserved default font key and key resolution.
//!
//! The engine embeds a default font under [`DEFAULT_FONT_KEY`]; `""` is an
//! alias for it. Every font lookup, on both threads, resolves its key through
//! [`resolve_font_key`] so drawing and measuring never disagree about which
//! font a key means.

/// Key of the embedded default font.
pub const DEFAULT_FONT_KEY: &str = "default";

/// Pixels added between glyphs when drawing and measuring text.
pub const TEXT_SPACING: f32 = 1.0;

/// Glyph spacing of the default font. It is a pixel font whose advance is
/// exactly its cell width, so any extra spacing opens gaps in box-drawing and
/// block glyphs.
pub const DEFAULT_FONT_SPACING: f32 = 0.0;

/// The glyph spacing for text drawn with the font that `resolved_key` names
/// (a key already resolved by [`resolve_font_key`]).
pub fn font_spacing(resolved_key: &str) -> f32 {
    if resolved_key == DEFAULT_FONT_KEY {
        DEFAULT_FONT_SPACING
    } else {
        TEXT_SPACING
    }
}

/// Whether `key` names the default font (`"default"` or its `""` alias). A
/// game cannot load, remove or rename a reserved key.
pub fn is_reserved_font_key(key: &str) -> bool {
    key.is_empty() || key == DEFAULT_FONT_KEY
}

/// The key to use for `key`: `key` itself when `is_known` accepts it, else
/// [`DEFAULT_FONT_KEY`] (the empty alias included).
pub fn resolve_font_key<'a>(key: &'a str, is_known: impl Fn(&str) -> bool) -> &'a str {
    if !key.is_empty() && is_known(key) {
        key
    } else {
        DEFAULT_FONT_KEY
    }
}

/// Whether falling back for `key` deserves a warning: an unknown key that is
/// not reserved and has no load still pending (a font that is not loaded yet
/// is not a typo).
pub fn font_key_needs_warning(key: &str, is_known: bool, is_pending: bool) -> bool {
    !is_reserved_font_key(key) && !is_known && !is_pending
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(k: &str) -> bool {
        k == "ui"
    }

    #[test]
    fn the_default_font_has_no_glyph_spacing_and_other_fonts_keep_the_standard_one() {
        assert_eq!(font_spacing(DEFAULT_FONT_KEY), DEFAULT_FONT_SPACING);
        assert_eq!(DEFAULT_FONT_SPACING, 0.0);
        assert_eq!(font_spacing("ui"), TEXT_SPACING);
    }

    #[test]
    fn empty_key_resolves_to_default() {
        assert_eq!(resolve_font_key("", known), DEFAULT_FONT_KEY);
    }

    #[test]
    fn default_key_stays_default() {
        assert_eq!(resolve_font_key("default", known), "default");
    }

    #[test]
    fn known_key_stays() {
        assert_eq!(resolve_font_key("ui", known), "ui");
    }

    #[test]
    fn unknown_key_falls_back_to_default() {
        assert_eq!(resolve_font_key("typo", known), DEFAULT_FONT_KEY);
    }

    #[test]
    fn reserved_keys_are_default_and_empty() {
        assert!(is_reserved_font_key("default"));
        assert!(is_reserved_font_key(""));
        assert!(!is_reserved_font_key("ui"));
    }

    #[test]
    fn reserved_keys_never_need_a_warning() {
        assert!(!font_key_needs_warning("", false, false));
        assert!(!font_key_needs_warning("default", false, false));
    }

    #[test]
    fn known_key_needs_no_warning() {
        assert!(!font_key_needs_warning("ui", true, false));
    }

    #[test]
    fn unknown_pending_key_needs_no_warning() {
        assert!(!font_key_needs_warning("ui", false, true));
    }

    #[test]
    fn unknown_unpending_key_needs_a_warning() {
        assert!(font_key_needs_warning("typo", false, false));
    }
}
