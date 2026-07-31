//! `crate::math` <-> raylib scalar-type conversions, render-thread-only.
//!
//! `crate::math::Color`/`Rect` are layout-identical to raylib's own
//! `Color`/`Rectangle` (asserted in `crate::math`'s own tests), so these are
//! plain field copies, not real conversions. `crate::resources::camera2d::Camera2D`
//! is not layout-identical to raylib's `Camera2D` (its `target`/`offset`
//! fields are `Vec2`, not raylib's `Vector2`), so its `From` impls delegate
//! to `vec2_to_raylib`/`vec2_from_raylib` below for those two fields. They
//! live here — not in `crate::math`/`crate::resources::camera2d` themselves —
//! because those modules must stay raylib-free; only the render tree is
//! allowed to know raylib exists.
//!
//! NOTE: once the workspace split (`docs/plans/workspaces-implementation.md`)
//! makes `aberred-render` and `raylib` genuinely separate crates, `aberred-render`
//! will own neither `crate::math::Color`/`Rect` nor `raylib::prelude::Color`/
//! `Rectangle`, and these `impl From` blocks will hit Rust's orphan rule.
//! `RaylibWorldDraw` below already sidesteps the equivalent problem for
//! `WorldDraw`/`RaylibDraw` via a newtype instead of a blanket impl — the
//! same fix would apply here too if these `impl From` blocks ever need it.

use crate::components::shadow::Shadow;
use crate::components::tint::Tint;
use crate::math::{Color, Rect, Vec2};
use crate::resources::camera2d::Camera2D;
use crate::systems::scene_dispatch::WorldDraw;

/// Adapts any raylib draw handle to the core-owned [`WorldDraw`] trait.
///
/// A newtype instead of a blanket `impl<T: RaylibDraw> WorldDraw for T` —
/// once core/render split into separate crates, `aberred-render` owns
/// neither `WorldDraw` (core) nor `RaylibDraw` (raylib), so a blanket impl
/// would hit Rust's orphan rule. This newtype is owned by render, so the
/// impl is legal regardless of crate split.
pub(super) struct RaylibWorldDraw<'a, T: raylib::prelude::RaylibDraw>(pub &'a mut T);

impl<T: raylib::prelude::RaylibDraw> WorldDraw for RaylibWorldDraw<'_, T> {
    fn draw_line_v(&mut self, start: Vec2, end: Vec2, color: Color) {
        let color: raylib::prelude::Color = color.into();
        raylib::prelude::RaylibDraw::draw_line_v(
            self.0,
            vec2_to_raylib(start),
            vec2_to_raylib(end),
            color,
        );
    }

    fn draw_line_ex(&mut self, start_pos: Vec2, end_pos: Vec2, thick: f32, color: Color) {
        let color: raylib::prelude::Color = color.into();
        raylib::prelude::RaylibDraw::draw_line_ex(
            self.0,
            vec2_to_raylib(start_pos),
            vec2_to_raylib(end_pos),
            thick,
            color,
        );
    }

    fn draw_line_dashed(
        &mut self,
        start_pos: Vec2,
        end_pos: Vec2,
        dash_size: i32,
        space_size: i32,
        color: Color,
    ) {
        let color: raylib::prelude::Color = color.into();
        raylib::prelude::RaylibDraw::draw_line_dashed(
            self.0,
            vec2_to_raylib(start_pos),
            vec2_to_raylib(end_pos),
            dash_size,
            space_size,
            color,
        );
    }

    fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        let color: raylib::prelude::Color = color.into();
        raylib::prelude::RaylibDraw::draw_line(self.0, x1, y1, x2, y2, color);
    }
}

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

impl From<Camera2D> for raylib::prelude::Camera2D {
    fn from(c: Camera2D) -> Self {
        raylib::prelude::Camera2D {
            target: vec2_to_raylib(c.target),
            offset: vec2_to_raylib(c.offset),
            rotation: c.rotation,
            zoom: c.zoom,
        }
    }
}

impl From<raylib::prelude::Camera2D> for Camera2D {
    fn from(c: raylib::prelude::Camera2D) -> Self {
        Camera2D {
            target: vec2_from_raylib(c.target),
            offset: vec2_from_raylib(c.offset),
            rotation: c.rotation,
            zoom: c.zoom,
        }
    }
}

// `Vec2` is a bare `pub use glam::Vec2;` re-export (Phase 1 decision), not a
// local newtype like `Color`/`Rect` — so unlike those, `impl From<Vec2> for
// raylib::prelude::Vector2` would violate the orphan rule (neither `Vec2`
// nor `Vector2` nor `From` are local to this crate). Plain conversion
// functions instead.
pub(super) fn vec2_to_raylib(v: Vec2) -> raylib::prelude::Vector2 {
    raylib::prelude::Vector2::new(v.x, v.y)
}

pub(super) fn vec2_from_raylib(v: raylib::prelude::Vector2) -> Vec2 {
    Vec2::new(v.x, v.y)
}

/// Projects a raylib-space point through an already-core-typed camera,
/// delegating to the pure-Rust `crate::systems::input::screen_to_world2d`.
///
/// Takes `camera` pre-converted (rather than raylib's `Camera2D`) so a
/// caller projecting several points against the same camera in a loop
/// (e.g. `compute_view_bounds`'s 4-corner closure) converts it once and
/// passes a reference, instead of re-converting on every call.
pub(super) fn screen_to_world2d_raylib(
    pos: raylib::prelude::Vector2,
    camera: &Camera2D,
) -> raylib::prelude::Vector2 {
    vec2_to_raylib(crate::systems::input::screen_to_world2d(
        vec2_from_raylib(pos),
        camera,
    ))
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

    #[test]
    fn camera2d_round_trips_through_raylib() {
        let c = Camera2D {
            target: Vec2::new(1.0, 2.0),
            offset: Vec2::new(3.0, 4.0),
            rotation: 45.0,
            zoom: 2.0,
        };
        let rc: raylib::prelude::Camera2D = c.into();
        let back: Camera2D = rc.into();
        assert_eq!(c, back);
    }
}
