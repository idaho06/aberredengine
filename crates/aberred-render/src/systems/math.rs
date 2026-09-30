//! `crate::math` <-> raylib scalar-type conversions, render-thread-only.
//!
//! `aberred_core::math::Color`/`Rect` are layout-identical to raylib's own
//! `Color`/`Rectangle` (asserted in `crate::math`'s own tests), so these are
//! plain field copies, not real conversions. `aberred_core::resources::camera2d::Camera2D`
//! is not layout-identical to raylib's `Camera2D` (its `target`/`offset`
//! fields are `Vec2`, not raylib's `Vector2`), so its conversion functions
//! delegate to `vec2_to_raylib`/`vec2_from_raylib` below for those two
//! fields. They live here — not in `crate::math`/`crate::resources::camera2d`
//! themselves — because those modules must stay raylib-free; only the
//! render tree is allowed to know raylib exists.
//!
//! Plain functions, not `impl From`: `aberred-core` and `raylib` are
//! genuinely separate crates from this (`aberred-render`) crate's point of
//! view, so `impl From<Color> for raylib::prelude::Color` would name two
//! foreign types and violate the orphan rule. `RaylibWorldDraw` below
//! sidesteps the equivalent problem for `WorldDraw`/`RaylibDraw` via a
//! newtype instead; `vec2_to_raylib`/`vec2_from_raylib` already used plain
//! functions for the same reason even before the crate split, since `Vec2`
//! is a bare `glam` re-export, not a local newtype.

use aberred_core::components::shadow::Shadow;
use aberred_core::components::tint::Tint;
use aberred_core::math::{Color, Rect, Vec2};
use aberred_core::resources::camera2d::Camera2D;
use aberred_core::resources::texturefilter::TextureFilter;
use aberred_core::systems::scene_dispatch::WorldDraw;

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
        raylib::prelude::RaylibDraw::draw_line_v(
            self.0,
            vec2_to_raylib(start),
            vec2_to_raylib(end),
            color_to_raylib(color),
        );
    }

    fn draw_line_ex(&mut self, start_pos: Vec2, end_pos: Vec2, thick: f32, color: Color) {
        raylib::prelude::RaylibDraw::draw_line_ex(
            self.0,
            vec2_to_raylib(start_pos),
            vec2_to_raylib(end_pos),
            thick,
            color_to_raylib(color),
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
        raylib::prelude::RaylibDraw::draw_line_dashed(
            self.0,
            vec2_to_raylib(start_pos),
            vec2_to_raylib(end_pos),
            dash_size,
            space_size,
            color_to_raylib(color),
        );
    }

    fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        raylib::prelude::RaylibDraw::draw_line(self.0, x1, y1, x2, y2, color_to_raylib(color));
    }
}

/// Resolves a sprite's color for a draw call: an optional [`Tint`] *replaces*
/// `Color::WHITE` (see [`Tint`]'s own doc comment), converted to raylib's
/// `Color` at this one call site instead of at every sprite draw path.
pub(super) fn resolve_sprite_tint(maybe_tint: Option<Tint>) -> raylib::prelude::Color {
    color_to_raylib(maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE))
}

/// Resolves a text item's color for a draw call: an optional [`Tint`]
/// *multiplies* with the base color (see [`Tint`]'s own doc comment),
/// converted to raylib's `Color` at this one call site instead of at every
/// text draw path.
pub(super) fn resolve_text_tint(maybe_tint: Option<Tint>, base: Color) -> raylib::prelude::Color {
    color_to_raylib(maybe_tint.map(|t| t.multiply(base)).unwrap_or(base))
}

/// Resolves a [`Shadow`]'s color for a draw call.
pub(super) fn shadow_color(shadow: Shadow) -> raylib::prelude::Color {
    color_to_raylib(shadow.color)
}

pub(super) fn color_to_raylib(c: Color) -> raylib::prelude::Color {
    raylib::prelude::Color::new(c.r, c.g, c.b, c.a)
}

pub(super) fn rect_to_raylib(r: Rect) -> raylib::prelude::Rectangle {
    raylib::prelude::Rectangle::new(r.x, r.y, r.width, r.height)
}

pub(super) fn camera2d_to_raylib(c: Camera2D) -> raylib::prelude::Camera2D {
    raylib::prelude::Camera2D {
        target: vec2_to_raylib(c.target),
        offset: vec2_to_raylib(c.offset),
        rotation: c.rotation,
        zoom: c.zoom,
    }
}

pub(super) fn camera2d_from_raylib(c: raylib::prelude::Camera2D) -> Camera2D {
    Camera2D {
        target: vec2_from_raylib(c.target),
        offset: vec2_from_raylib(c.offset),
        rotation: c.rotation,
        zoom: c.zoom,
    }
}

pub(super) fn vec2_to_raylib(v: Vec2) -> raylib::prelude::Vector2 {
    raylib::prelude::Vector2::new(v.x, v.y)
}

pub(super) fn vec2_from_raylib(v: raylib::prelude::Vector2) -> Vec2 {
    Vec2::new(v.x, v.y)
}

/// Projects a raylib-space point through an already-core-typed camera,
/// delegating to the pure-Rust `aberred_core::systems::input::screen_to_world2d`.
///
/// Takes `camera` pre-converted (rather than raylib's `Camera2D`) so a
/// caller projecting several points against the same camera in a loop
/// (e.g. `compute_view_bounds`'s 4-corner closure) converts it once and
/// passes a reference, instead of re-converting on every call.
pub(super) fn screen_to_world2d_raylib(
    pos: raylib::prelude::Vector2,
    camera: &Camera2D,
) -> raylib::prelude::Vector2 {
    vec2_to_raylib(aberred_core::systems::input::screen_to_world2d(
        vec2_from_raylib(pos),
        camera,
    ))
}

/// Maps [`TextureFilter`] to raylib's `TextureFilter` FFI constant. The enum
/// itself stays in `crate::resources::texturefilter` (engine-owned, no
/// raylib reference); only this conversion needs raylib, so it lives here
/// alongside the other render-boundary shims.
pub(crate) fn texture_filter_to_ffi(filter: TextureFilter) -> i32 {
    use raylib::ffi::TextureFilter as FfiTextureFilter;
    match filter {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera2d_round_trips_through_raylib() {
        let c = Camera2D {
            target: Vec2::new(1.0, 2.0),
            offset: Vec2::new(3.0, 4.0),
            rotation: 45.0,
            zoom: 2.0,
        };
        let rc = camera2d_to_raylib(c);
        let back = camera2d_from_raylib(rc);
        assert_eq!(c, back);
    }

    fn rgba(c: raylib::prelude::Color) -> (u8, u8, u8, u8) {
        (c.r, c.g, c.b, c.a)
    }

    #[test]
    fn texture_filter_to_ffi_maps_each_filter_to_its_raylib_constant() {
        use raylib::ffi::TextureFilter as F;
        let expected = [
            (TextureFilter::Nearest, F::TEXTURE_FILTER_POINT),
            (TextureFilter::Bilinear, F::TEXTURE_FILTER_BILINEAR),
            (TextureFilter::Trilinear, F::TEXTURE_FILTER_TRILINEAR),
            (TextureFilter::Anisotropic4x, F::TEXTURE_FILTER_ANISOTROPIC_4X),
            (TextureFilter::Anisotropic8x, F::TEXTURE_FILTER_ANISOTROPIC_8X),
            (TextureFilter::Anisotropic16x, F::TEXTURE_FILTER_ANISOTROPIC_16X),
        ];
        assert_eq!(expected.len(), TextureFilter::ALL.len());
        for (filter, ffi) in expected {
            assert_eq!(texture_filter_to_ffi(filter), ffi as i32, "{filter:?}");
        }
    }

    #[test]
    fn conversions_to_raylib_keep_every_field_in_place() {
        assert_eq!(rgba(color_to_raylib(Color::new(1, 2, 3, 4))), (1, 2, 3, 4));

        let r = rect_to_raylib(Rect::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!((r.x, r.y, r.width, r.height), (1.0, 2.0, 3.0, 4.0));

        let v = vec2_to_raylib(Vec2::new(5.0, 6.0));
        assert_eq!((v.x, v.y), (5.0, 6.0));
        assert_eq!(vec2_from_raylib(v), Vec2::new(5.0, 6.0));

        // Field-by-field, not just a round trip: a symmetric target/offset
        // swap would still round-trip.
        let c = camera2d_to_raylib(Camera2D {
            target: Vec2::new(1.0, 2.0),
            offset: Vec2::new(3.0, 4.0),
            rotation: 45.0,
            zoom: 2.0,
        });
        assert_eq!((c.target.x, c.target.y), (1.0, 2.0));
        assert_eq!((c.offset.x, c.offset.y), (3.0, 4.0));
        assert_eq!((c.rotation, c.zoom), (45.0, 2.0));
    }

    #[test]
    fn sprite_tint_replaces_white_and_text_tint_multiplies_the_base() {
        let tint = Tint::new(255, 128, 0, 255);
        assert_eq!(rgba(resolve_sprite_tint(None)), (255, 255, 255, 255));
        assert_eq!(rgba(resolve_sprite_tint(Some(tint))), (255, 128, 0, 255));

        let base = Color::new(200, 200, 200, 100);
        assert_eq!(rgba(resolve_text_tint(None, base)), (200, 200, 200, 100));
        assert_eq!(rgba(resolve_text_tint(Some(tint), base)), (200, 100, 0, 100));
    }

    #[test]
    fn shadow_color_is_the_shadows_own_color() {
        let shadow = Shadow::new(2.0, 3.0, 10, 20, 30, 40);
        assert_eq!(rgba(shadow_color(shadow)), (10, 20, 30, 40));
    }

    #[test]
    fn screen_to_world2d_raylib_projects_through_the_camera() {
        let camera = Camera2D {
            target: Vec2::new(100.0, 50.0),
            offset: Vec2::new(10.0, 20.0),
            rotation: 0.0,
            zoom: 2.0,
        };
        let world = screen_to_world2d_raylib(raylib::prelude::Vector2::new(30.0, 40.0), &camera);
        // (screen - offset) / zoom + target
        assert_eq!((world.x, world.y), (110.0, 60.0));
    }
}
