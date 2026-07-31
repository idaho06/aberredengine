//! `crate::math` <-> raylib scalar-type conversions, render-thread-only.
//!
//! `crate::math::Color`/`Rect` are layout-identical to raylib's own
//! `Color`/`Rectangle` (asserted in `crate::math`'s own tests), so these are
//! plain field copies, not real conversions. They live here — not in
//! `crate::math` itself — because that module must stay raylib-free (see its
//! doc comment); only the render tree is allowed to know raylib exists.
//!
//! NOTE: once the workspace split (`docs/plans/workspaces-implementation.md`)
//! makes `aberred-render` and `raylib` genuinely separate crates, `aberred-render`
//! will own neither `crate::math::Color`/`Rect` nor `raylib::prelude::Color`/
//! `Rectangle`, and these `impl From` blocks will hit Rust's orphan rule. See
//! the parent plan's §5.2 finding (the `WorldDraw` blanket-impl case) for the
//! same problem; the fix there (a newtype wrapper) applies here too.

use crate::components::shadow::Shadow;
use crate::components::tint::Tint;
use crate::math::{Color, Rect};

/// Resolves a sprite's color for a draw call: an optional [`Tint`] *replaces*
/// `Color::WHITE` (see [`Tint`]'s own doc comment), converted to raylib's
/// `Color` at this one call site instead of at every sprite draw path.
pub(super) fn resolve_sprite_tint(maybe_tint: Option<Tint>) -> raylib::prelude::Color {
    maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE).into()
}

/// Resolves a text item's color for a draw call: an optional [`Tint`]
/// *multiplies* with the base color (see [`Tint`]'s own doc comment),
/// converted to raylib's `Color` at this one call site instead of at every
/// text draw path.
pub(super) fn resolve_text_tint(maybe_tint: Option<Tint>, base: Color) -> raylib::prelude::Color {
    maybe_tint.map(|t| t.multiply(base)).unwrap_or(base).into()
}

/// Resolves a [`Shadow`]'s color for a draw call.
pub(super) fn shadow_color(shadow: Shadow) -> raylib::prelude::Color {
    shadow.color.into()
}

impl From<Color> for raylib::prelude::Color {
    fn from(c: Color) -> Self {
        raylib::prelude::Color::new(c.r, c.g, c.b, c.a)
    }
}

impl From<raylib::prelude::Color> for Color {
    fn from(c: raylib::prelude::Color) -> Self {
        Color::new(c.r, c.g, c.b, c.a)
    }
}

impl From<Rect> for raylib::prelude::Rectangle {
    fn from(r: Rect) -> Self {
        raylib::prelude::Rectangle::new(r.x, r.y, r.width, r.height)
    }
}

impl From<raylib::prelude::Rectangle> for Rect {
    fn from(r: raylib::prelude::Rectangle) -> Self {
        Rect::new(r.x, r.y, r.width, r.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_round_trips_through_raylib() {
        let c = Color::new(10, 20, 30, 40);
        let rc: raylib::prelude::Color = c.into();
        let back: Color = rc.into();
        assert_eq!(c, back);
    }

    #[test]
    fn rect_round_trips_through_raylib() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        let rr: raylib::prelude::Rectangle = r.into();
        let back: Rect = rr.into();
        assert_eq!(r, back);
    }
}
