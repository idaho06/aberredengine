//! Engine-owned, raylib-free scalar math types.
//!
//! `Vec2` re-exports `glam::Vec2` so core never depends on raylib's own
//! vector type; `Color`/`Rect` are hand-written, layout-identical
//! replacements for raylib's `Color`/`Rectangle` so the render tree's
//! conversions (`crates/aberred-render/src/systems/math.rs`) are free. This
//! module must stay free of any `raylib` reference — that split is the
//! entire point of `aberred-core` existing as a separate crate.

pub use glam::Vec2;

/// Linear interpolation, generic over anything with the right operator set
/// (covers both scalar `f32` and `Vec2`).
pub fn lerp<T>(a: T, b: T, t: f32) -> T
where
    T: Copy + std::ops::Add<Output = T> + std::ops::Sub<Output = T> + std::ops::Mul<f32, Output = T>,
{
    a + (b - a) * t
}

/// Rotate `v` by `radians` (counter-clockwise, standard math convention).
///
/// Replaces raylib's `Vector2::rotated(angle)` — glam's own `Vec2::rotate`
/// takes a unit rotor (complex-number-style), not a raw angle, so this
/// composes it with `Vec2::from_angle` rather than hand-rolling the same
/// `sin`/`cos` formula a second time. Also keeps rotation on glam's own
/// `sin_cos` (routes through the `libm` feature enabled in `Cargo.toml`,
/// same determinism rationale as the rest of this module).
pub fn rotate(v: Vec2, radians: f32) -> Vec2 {
    Vec2::from_angle(radians).rotate(v)
}

/// RGBA color, `u8` channels. Layout-identical to raylib's `Color` (see the
/// `color_layout_matches_raylib` test) so the render tree's `From`/`Into`
/// shims are a bit-for-bit reinterpretation, not a conversion.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    // Full named palette, ported 1:1 from `sola-raylib 6.2.0`'s own `Color`
    // constants (`core/color.rs`) — that crate extends raylib's classic
    // 26-color set with the CSS/X11 named palette, so "raylib-compatible"
    // here means matching this dependency's actual constant values, not the
    // smaller canonical raylib palette. Re-verify against that source if
    // `sola-raylib` is ever upgraded and its palette changes.
    pub const INDIANRED: Color = Color::new(205, 92, 92, 255);
    pub const LIGHTCORAL: Color = Color::new(240, 128, 128, 255);
    pub const SALMON: Color = Color::new(250, 128, 114, 255);
    pub const DARKSALMON: Color = Color::new(233, 150, 122, 255);
    pub const LIGHTSALMON: Color = Color::new(255, 160, 122, 255);
    pub const CRIMSON: Color = Color::new(220, 20, 60, 255);
    pub const RED: Color = Color::new(255, 0, 0, 255);
    pub const FIREBRICK: Color = Color::new(178, 34, 34, 255);
    pub const DARKRED: Color = Color::new(139, 0, 0, 255);
    pub const PINK: Color = Color::new(255, 192, 203, 255);
    pub const LIGHTPINK: Color = Color::new(255, 182, 193, 255);
    pub const HOTPINK: Color = Color::new(255, 105, 180, 255);
    pub const DEEPPINK: Color = Color::new(255, 20, 147, 255);
    pub const MEDIUMVIOLETRED: Color = Color::new(199, 21, 133, 255);
    pub const PALEVIOLETRED: Color = Color::new(219, 112, 147, 255);
    pub const CORAL: Color = Color::new(255, 127, 80, 255);
    pub const TOMATO: Color = Color::new(255, 99, 71, 255);
    pub const ORANGERED: Color = Color::new(255, 69, 0, 255);
    pub const DARKORANGE: Color = Color::new(255, 140, 0, 255);
    pub const ORANGE: Color = Color::new(255, 165, 0, 255);
    pub const GOLD: Color = Color::new(255, 215, 0, 255);
    pub const YELLOW: Color = Color::new(255, 255, 0, 255);
    pub const LIGHTYELLOW: Color = Color::new(255, 255, 224, 255);
    pub const LEMONCHIFFON: Color = Color::new(255, 250, 205, 255);
    pub const LIGHTGOLDENRODYELLOW: Color = Color::new(250, 250, 210, 255);
    pub const PAPAYAWHIP: Color = Color::new(255, 239, 213, 255);
    pub const MOCCASIN: Color = Color::new(255, 228, 181, 255);
    pub const PEACHPUFF: Color = Color::new(255, 218, 185, 255);
    pub const PALEGOLDENROD: Color = Color::new(238, 232, 170, 255);
    pub const KHAKI: Color = Color::new(240, 230, 140, 255);
    pub const DARKKHAKI: Color = Color::new(189, 183, 107, 255);
    pub const LAVENDER: Color = Color::new(230, 230, 250, 255);
    pub const THISTLE: Color = Color::new(216, 191, 216, 255);
    pub const PLUM: Color = Color::new(221, 160, 221, 255);
    pub const VIOLET: Color = Color::new(238, 130, 238, 255);
    pub const ORCHID: Color = Color::new(218, 112, 214, 255);
    pub const FUCHSIA: Color = Color::new(255, 0, 255, 255);
    pub const MAGENTA: Color = Color::new(255, 0, 255, 255);
    pub const MEDIUMORCHID: Color = Color::new(186, 85, 211, 255);
    pub const MEDIUMPURPLE: Color = Color::new(147, 112, 219, 255);
    pub const REBECCAPURPLE: Color = Color::new(102, 51, 153, 255);
    pub const BLUEVIOLET: Color = Color::new(138, 43, 226, 255);
    pub const DARKVIOLET: Color = Color::new(148, 0, 211, 255);
    pub const DARKORCHID: Color = Color::new(153, 50, 204, 255);
    pub const DARKMAGENTA: Color = Color::new(139, 0, 139, 255);
    pub const PURPLE: Color = Color::new(128, 0, 128, 255);
    pub const DARKPURPLE: Color = Color::new(112, 31, 126, 255);
    pub const INDIGO: Color = Color::new(75, 0, 130, 255);
    pub const SLATEBLUE: Color = Color::new(106, 90, 205, 255);
    pub const DARKSLATEBLUE: Color = Color::new(72, 61, 139, 255);
    pub const MEDIUMSLATEBLUE: Color = Color::new(123, 104, 238, 255);
    pub const GREENYELLOW: Color = Color::new(173, 255, 47, 255);
    pub const CHARTREUSE: Color = Color::new(127, 255, 0, 255);
    pub const LAWNGREEN: Color = Color::new(124, 252, 0, 255);
    pub const LIME: Color = Color::new(0, 255, 0, 255);
    pub const LIMEGREEN: Color = Color::new(50, 205, 50, 255);
    pub const PALEGREEN: Color = Color::new(152, 251, 152, 255);
    pub const LIGHTGREEN: Color = Color::new(144, 238, 144, 255);
    pub const MEDIUMSPRINGGREEN: Color = Color::new(0, 250, 154, 255);
    pub const SPRINGGREEN: Color = Color::new(0, 255, 127, 255);
    pub const MEDIUMSEAGREEN: Color = Color::new(60, 179, 113, 255);
    pub const SEAGREEN: Color = Color::new(46, 139, 87, 255);
    pub const FORESTGREEN: Color = Color::new(34, 139, 34, 255);
    pub const GREEN: Color = Color::new(0, 128, 0, 255);
    pub const DARKGREEN: Color = Color::new(0, 100, 0, 255);
    pub const YELLOWGREEN: Color = Color::new(154, 205, 50, 255);
    pub const OLIVEDRAB: Color = Color::new(107, 142, 35, 255);
    pub const OLIVE: Color = Color::new(128, 128, 0, 255);
    pub const DARKOLIVEGREEN: Color = Color::new(85, 107, 47, 255);
    pub const MEDIUMAQUAMARINE: Color = Color::new(102, 205, 170, 255);
    pub const DARKSEAGREEN: Color = Color::new(143, 188, 139, 255);
    pub const LIGHTSEAGREEN: Color = Color::new(32, 178, 170, 255);
    pub const DARKCYAN: Color = Color::new(0, 139, 139, 255);
    pub const TEAL: Color = Color::new(0, 128, 128, 255);
    pub const AQUA: Color = Color::new(0, 255, 255, 255);
    pub const CYAN: Color = Color::new(0, 255, 255, 255);
    pub const LIGHTCYAN: Color = Color::new(224, 255, 255, 255);
    pub const PALETURQUOISE: Color = Color::new(175, 238, 238, 255);
    pub const AQUAMARINE: Color = Color::new(127, 255, 212, 255);
    pub const TURQUOISE: Color = Color::new(64, 224, 208, 255);
    pub const MEDIUMTURQUOISE: Color = Color::new(72, 209, 204, 255);
    pub const DARKTURQUOISE: Color = Color::new(0, 206, 209, 255);
    pub const CADETBLUE: Color = Color::new(95, 158, 160, 255);
    pub const STEELBLUE: Color = Color::new(70, 130, 180, 255);
    pub const LIGHTSTEELBLUE: Color = Color::new(176, 196, 222, 255);
    pub const POWDERBLUE: Color = Color::new(176, 224, 230, 255);
    pub const LIGHTBLUE: Color = Color::new(173, 216, 230, 255);
    pub const SKYBLUE: Color = Color::new(135, 206, 235, 255);
    pub const LIGHTSKYBLUE: Color = Color::new(135, 206, 250, 255);
    pub const DEEPSKYBLUE: Color = Color::new(0, 191, 255, 255);
    pub const DODGERBLUE: Color = Color::new(30, 144, 255, 255);
    pub const CORNFLOWERBLUE: Color = Color::new(100, 149, 237, 255);
    pub const ROYALBLUE: Color = Color::new(65, 105, 225, 255);
    pub const BLUE: Color = Color::new(0, 0, 255, 255);
    pub const MEDIUMBLUE: Color = Color::new(0, 0, 205, 255);
    pub const DARKBLUE: Color = Color::new(0, 0, 139, 255);
    pub const NAVY: Color = Color::new(0, 0, 128, 255);
    pub const MIDNIGHTBLUE: Color = Color::new(25, 25, 112, 255);
    pub const CORNSILK: Color = Color::new(255, 248, 220, 255);
    pub const BLANCHEDALMOND: Color = Color::new(255, 235, 205, 255);
    pub const BISQUE: Color = Color::new(255, 228, 196, 255);
    pub const NAVAJOWHITE: Color = Color::new(255, 222, 173, 255);
    pub const WHEAT: Color = Color::new(245, 222, 179, 255);
    pub const BURLYWOOD: Color = Color::new(222, 184, 135, 255);
    pub const TAN: Color = Color::new(210, 180, 140, 255);
    pub const ROSYBROWN: Color = Color::new(188, 143, 143, 255);
    pub const SANDYBROWN: Color = Color::new(244, 164, 96, 255);
    pub const GOLDENROD: Color = Color::new(218, 165, 32, 255);
    pub const DARKGOLDENROD: Color = Color::new(184, 134, 11, 255);
    pub const PERU: Color = Color::new(205, 133, 63, 255);
    pub const CHOCOLATE: Color = Color::new(210, 105, 30, 255);
    pub const SADDLEBROWN: Color = Color::new(139, 69, 19, 255);
    pub const SIENNA: Color = Color::new(160, 82, 45, 255);
    pub const BROWN: Color = Color::new(165, 42, 42, 255);
    pub const DARKBROWN: Color = Color::new(76, 63, 47, 255);
    pub const MAROON: Color = Color::new(128, 0, 0, 255);
    pub const WHITE: Color = Color::new(255, 255, 255, 255);
    pub const SNOW: Color = Color::new(255, 250, 250, 255);
    pub const HONEYDEW: Color = Color::new(240, 255, 240, 255);
    pub const MINTCREAM: Color = Color::new(245, 255, 250, 255);
    pub const AZURE: Color = Color::new(240, 255, 255, 255);
    pub const ALICEBLUE: Color = Color::new(240, 248, 255, 255);
    pub const GHOSTWHITE: Color = Color::new(248, 248, 255, 255);
    pub const WHITESMOKE: Color = Color::new(245, 245, 245, 255);
    pub const SEASHELL: Color = Color::new(255, 245, 238, 255);
    pub const BEIGE: Color = Color::new(245, 245, 220, 255);
    pub const OLDLACE: Color = Color::new(253, 245, 230, 255);
    pub const FLORALWHITE: Color = Color::new(255, 250, 240, 255);
    pub const IVORY: Color = Color::new(255, 255, 240, 255);
    pub const ANTIQUEWHITE: Color = Color::new(250, 235, 215, 255);
    pub const LINEN: Color = Color::new(250, 240, 230, 255);
    pub const LAVENDERBLUSH: Color = Color::new(255, 240, 245, 255);
    pub const MISTYROSE: Color = Color::new(255, 228, 225, 255);
    pub const GAINSBORO: Color = Color::new(220, 220, 220, 255);
    pub const LIGHTGRAY: Color = Color::new(211, 211, 211, 255);
    pub const SILVER: Color = Color::new(192, 192, 192, 255);
    pub const DARKGRAY: Color = Color::new(169, 169, 169, 255);
    pub const GRAY: Color = Color::new(128, 128, 128, 255);
    pub const DIMGRAY: Color = Color::new(105, 105, 105, 255);
    pub const LIGHTSLATEGRAY: Color = Color::new(119, 136, 153, 255);
    pub const SLATEGRAY: Color = Color::new(112, 128, 144, 255);
    pub const DARKSLATEGRAY: Color = Color::new(47, 79, 79, 255);
    pub const BLACK: Color = Color::new(0, 0, 0, 255);
    pub const BLANK: Color = Color::new(0, 0, 0, 0);
    pub const RAYWHITE: Color = Color::new(245, 245, 245, 255);
}

/// Axis-aligned rectangle. Layout-identical to raylib's `Rectangle` (see the
/// `rect_layout_matches_raylib` test).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// AABB overlap test. Replaces raylib's `Rectangle::check_collision_recs`.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// Point-in-rect test. Replaces raylib's `Rectangle::check_collision_point_rec`.
    pub fn contains_point(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.width && p.y >= self.y && p.y <= self.y + self.height
    }

    /// Overlap rectangle of two rects, or `None` if they don't overlap.
    /// Replaces raylib's `Rectangle::get_collision_rec`. Computes each edge
    /// once rather than deferring to `overlaps` and redoing the same
    /// min/max arithmetic — this sits on the collision-detection hot path.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        if x2 <= x || y2 <= y {
            return None;
        }
        Some(Rect::new(x, y, x2 - x, y2 - y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn rotate_by_90_degrees() {
        let r = rotate(Vec2::new(10.0, 0.0), std::f32::consts::FRAC_PI_2);
        assert!(approx_eq(r.x, 0.0));
        assert!(approx_eq(r.y, 10.0));
    }

    #[test]
    fn rotate_by_zero_is_identity() {
        let v = Vec2::new(3.0, 4.0);
        let r = rotate(v, 0.0);
        assert!(approx_eq(r.x, v.x));
        assert!(approx_eq(r.y, v.y));
    }

    #[test]
    fn lerp_zero_stays() {
        assert!(approx_eq(lerp(10.0, 50.0, 0.0), 10.0));
    }

    #[test]
    fn lerp_one_reaches_target() {
        assert!(approx_eq(lerp(10.0, 50.0, 1.0), 50.0));
    }

    #[test]
    fn lerp_half_is_midpoint() {
        assert!(approx_eq(lerp(0.0, 100.0, 0.5), 50.0));
    }

    #[test]
    fn lerp_vec2_zero_stays() {
        let a = Vec2::new(10.0, 20.0);
        let b = Vec2::new(50.0, 60.0);
        let r = lerp(a, b, 0.0);
        assert!(approx_eq(r.x, 10.0));
        assert!(approx_eq(r.y, 20.0));
    }

    #[test]
    fn lerp_vec2_one_reaches_target() {
        let a = Vec2::new(10.0, 20.0);
        let b = Vec2::new(50.0, 60.0);
        let r = lerp(a, b, 1.0);
        assert!(approx_eq(r.x, 50.0));
        assert!(approx_eq(r.y, 60.0));
    }

    #[test]
    fn lerp_vec2_half_is_midpoint() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(100.0, 200.0);
        let r = lerp(a, b, 0.5);
        assert!(approx_eq(r.x, 50.0));
        assert!(approx_eq(r.y, 100.0));
    }

    #[test]
    fn color_new_sets_fields() {
        let c = Color::new(1, 2, 3, 4);
        assert_eq!((c.r, c.g, c.b, c.a), (1, 2, 3, 4));
    }

    #[test]
    fn color_layout_matches_raylib() {
        // raylib::prelude::Color is #[repr(C)] { r: u8, g: u8, b: u8, a: u8 } —
        // 4 bytes, no padding. A version bump that changes this layout must
        // fail here, not silently miscompile the render-tree From/Into shims.
        assert_eq!(size_of::<Color>(), 4);
        assert_eq!(std::mem::offset_of!(Color, r), 0);
        assert_eq!(std::mem::offset_of!(Color, g), 1);
        assert_eq!(std::mem::offset_of!(Color, b), 2);
        assert_eq!(std::mem::offset_of!(Color, a), 3);
    }

    #[test]
    fn rect_layout_matches_raylib() {
        // raylib::prelude::Rectangle is #[repr(C)] { x, y, width, height: f32 }.
        assert_eq!(size_of::<Rect>(), 16);
        assert_eq!(std::mem::offset_of!(Rect, x), 0);
        assert_eq!(std::mem::offset_of!(Rect, y), 4);
        assert_eq!(std::mem::offset_of!(Rect, width), 8);
        assert_eq!(std::mem::offset_of!(Rect, height), 12);
    }

    #[test]
    fn rect_overlaps_detects_intersection() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        let c = Rect::new(20.0, 20.0, 5.0, 5.0);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn rect_contains_point() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains_point(Vec2::new(5.0, 5.0)));
        assert!(!r.contains_point(Vec2::new(15.0, 5.0)));
    }

    #[test]
    fn rect_intersection_no_overlap_is_none() {
        // Mirrors sola-raylib 6.2.0's `get_collision_rec` doctest
        // (core/collision.rs:22-29).
        let r1 = Rect::new(0.0, 0.0, 10.0, 10.0);
        let r2 = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert_eq!(r1.intersection(&r2), None);
    }

    #[test]
    fn rect_intersection_self_is_self() {
        let r1 = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(r1.intersection(&r1), Some(r1));
    }

    #[test]
    fn rect_intersection_partial_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersection(&b), Some(Rect::new(5.0, 5.0, 5.0, 5.0)));
    }
}
