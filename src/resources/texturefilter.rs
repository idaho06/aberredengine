//! Texture filtering mode shared by [`crate::resources::texturestore::TextureStore`]
//! and [`crate::resources::rendertarget::RenderTarget`].

use raylib::ffi::TextureFilter as FfiTextureFilter;

/// Texture sampling filter mode.
///
/// `Nearest` (point/nearest-neighbor) is sharp and avoids sprite atlas
/// bleeding -- the right choice for pixel art. The remaining variants trade
/// sharpness for smoother scaling/rotation, which suits high-resolution or
/// vector-style art (e.g. a rotating asteroid sprite).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum TextureFilter {
    /// Point/nearest-neighbor filtering - sharp pixels, no blur.
    #[default]
    Nearest,
    /// Bilinear filtering - smooth scaling with interpolation.
    Bilinear,
    /// Trilinear filtering - bilinear plus mipmap interpolation.
    Trilinear,
    /// 4x anisotropic filtering.
    Anisotropic4x,
    /// 8x anisotropic filtering.
    Anisotropic8x,
    /// 16x anisotropic filtering.
    Anisotropic16x,
}

impl TextureFilter {
    /// Map to the raylib `TextureFilter` FFI constant.
    pub(crate) fn to_ffi(self) -> i32 {
        match self {
            TextureFilter::Nearest => FfiTextureFilter::TEXTURE_FILTER_POINT as i32,
            TextureFilter::Bilinear => FfiTextureFilter::TEXTURE_FILTER_BILINEAR as i32,
            TextureFilter::Trilinear => FfiTextureFilter::TEXTURE_FILTER_TRILINEAR as i32,
            TextureFilter::Anisotropic4x => FfiTextureFilter::TEXTURE_FILTER_ANISOTROPIC_4X as i32,
            TextureFilter::Anisotropic8x => FfiTextureFilter::TEXTURE_FILTER_ANISOTROPIC_8X as i32,
            TextureFilter::Anisotropic16x => {
                FfiTextureFilter::TEXTURE_FILTER_ANISOTROPIC_16X as i32
            }
        }
    }
}

impl TextureFilter {
    /// All variants, in declaration order. Used by the editor to populate filter
    /// pickers without hand-maintaining a duplicate list.
    pub const ALL: [TextureFilter; 6] = [
        TextureFilter::Nearest,
        TextureFilter::Bilinear,
        TextureFilter::Trilinear,
        TextureFilter::Anisotropic4x,
        TextureFilter::Anisotropic8x,
        TextureFilter::Anisotropic16x,
    ];
}

impl TextureFilter {
    /// Canonical string form, the inverse of [`FromStr`](std::str::FromStr).
    pub fn as_str(self) -> &'static str {
        match self {
            TextureFilter::Nearest => "nearest",
            TextureFilter::Bilinear => "bilinear",
            TextureFilter::Trilinear => "trilinear",
            TextureFilter::Anisotropic4x => "anisotropic_4x",
            TextureFilter::Anisotropic8x => "anisotropic_8x",
            TextureFilter::Anisotropic16x => "anisotropic_16x",
        }
    }
}

impl std::str::FromStr for TextureFilter {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "nearest" => Ok(TextureFilter::Nearest),
            "bilinear" => Ok(TextureFilter::Bilinear),
            "trilinear" => Ok(TextureFilter::Trilinear),
            "anisotropic_4x" => Ok(TextureFilter::Anisotropic4x),
            "anisotropic_8x" => Ok(TextureFilter::Anisotropic8x),
            "anisotropic_16x" => Ok(TextureFilter::Anisotropic16x),
            _ => Err(()),
        }
    }
}

impl TextureFilter {
    /// Parse an optional filter string, warning and falling back to
    /// [`TextureFilter::default`] (`Nearest`) if absent or unrecognized.
    ///
    /// `context` identifies the texture in the warning message (e.g. its key/id).
    pub fn from_opt_str_or_warn(filter: Option<&str>, context: &str) -> Self {
        filter
            .map(|s| {
                s.parse().unwrap_or_else(|_| {
                    log::warn!("Unknown texture filter '{s}' for '{context}', using 'nearest'");
                    Self::default()
                })
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_nearest() {
        assert_eq!(TextureFilter::default(), TextureFilter::Nearest);
    }

    #[test]
    fn from_str_parses_known_values() {
        assert_eq!("nearest".parse(), Ok(TextureFilter::Nearest));
        assert_eq!("bilinear".parse(), Ok(TextureFilter::Bilinear));
        assert_eq!("trilinear".parse(), Ok(TextureFilter::Trilinear));
        assert_eq!("anisotropic_4x".parse(), Ok(TextureFilter::Anisotropic4x));
        assert_eq!("anisotropic_8x".parse(), Ok(TextureFilter::Anisotropic8x));
        assert_eq!("anisotropic_16x".parse(), Ok(TextureFilter::Anisotropic16x));
    }

    #[test]
    fn as_str_round_trips_through_from_str() {
        for filter in TextureFilter::ALL {
            assert_eq!(filter.as_str().parse(), Ok(filter));
        }
    }

    #[test]
    fn from_str_rejects_unknown_values() {
        assert_eq!("".parse::<TextureFilter>(), Err(()));
        assert_eq!("Nearest".parse::<TextureFilter>(), Err(()));
        assert_eq!("smooth".parse::<TextureFilter>(), Err(()));
    }

    #[test]
    fn to_ffi_maps_to_distinct_raylib_constants() {
        use std::collections::HashSet;
        let ffi_values: HashSet<i32> = TextureFilter::ALL.iter().map(|f| f.to_ffi()).collect();
        assert_eq!(ffi_values.len(), TextureFilter::ALL.len());
    }
}
