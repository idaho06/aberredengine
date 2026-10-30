//! Draws [`Shape`]s: turns the core draw ops of each layer into raylib calls.
//!
//! The pass sequence (shadow, then one pass per layer) is a pure function,
//! [`shape_passes`]; only [`draw_shape_ops`] touches raylib, as a 1:1 match
//! over the op kinds.

use raylib::prelude::*;

use super::math::{resolve_multiply_tint, shadow_color, vec2_to_raylib};
use aberred_core::components::shadow::Shadow;
use aberred_core::components::shape::Shape;
use aberred_core::components::tint::Tint;
use aberred_core::math::Vec2;
use aberred_core::systems::shape_geometry::{DrawOp, shape_layers, shape_ops};

/// Where and how a shape is drawn: the transform handed to `shape_ops`.
#[derive(Clone, Copy)]
pub(super) struct ShapePlacement {
    pub(super) pos: Vec2,
    pub(super) scale: Vec2,
    pub(super) rotation_degrees: f32,
}

impl ShapePlacement {
    /// Unscaled, unrotated placement at `pos` (screen-space shapes).
    pub(super) fn at(pos: Vec2) -> Self {
        Self {
            pos,
            scale: Vec2::ONE,
            rotation_degrees: 0.0,
        }
    }
}

/// Emits the draw passes of one shape in draw order: the shadow silhouette
/// first (the shape's first layer, moved by `shadow.offset` and drawn in the
/// shadow colour, ignoring `tint`), then one pass per layer in the layer's
/// colour multiplied by `tint`.
///
/// `ops` is scratch space reused across calls; each `emit` call sees only the
/// ops of its own pass.
pub(super) fn shape_passes(
    shape: &Shape,
    place: ShapePlacement,
    maybe_tint: Option<Tint>,
    maybe_shadow: Option<Shadow>,
    ops: &mut Vec<DrawOp>,
    mut emit: impl FnMut(&[DrawOp], Color),
) {
    let layers = shape_layers(shape);
    let ShapePlacement {
        pos,
        scale,
        rotation_degrees,
    } = place;
    if let (Some(shadow), Some((layer, _))) = (maybe_shadow, layers[0]) {
        ops.clear();
        shape_ops(
            shape,
            layer,
            pos + shadow.offset,
            scale,
            rotation_degrees,
            ops,
        );
        emit(ops, shadow_color(shadow));
    }
    for (layer, color) in layers.into_iter().flatten() {
        ops.clear();
        shape_ops(shape, layer, pos, scale, rotation_degrees, ops);
        emit(ops, resolve_multiply_tint(maybe_tint, color));
    }
}

/// Draws `ops` in `color`: one raylib call per op.
pub(super) fn draw_shape_ops(d: &mut impl RaylibDraw, ops: &[DrawOp], color: Color) {
    for op in ops {
        match *op {
            DrawOp::Pixel { p } => d.draw_pixel_v(vec2_to_raylib(p), color),
            DrawOp::Line { a, b } => d.draw_line_v(vec2_to_raylib(a), vec2_to_raylib(b), color),
            DrawOp::Triangle { a, b, c } => d.draw_triangle(
                vec2_to_raylib(a),
                vec2_to_raylib(b),
                vec2_to_raylib(c),
                color,
            ),
            DrawOp::Circle { center, radius } => {
                d.draw_circle_v(vec2_to_raylib(center), radius, color)
            }
            DrawOp::CircleLines { center, radius } => {
                d.draw_circle_lines_v(vec2_to_raylib(center), radius, color)
            }
            DrawOp::Ellipse { center, radius } => {
                d.draw_ellipse_v(vec2_to_raylib(center), radius.x, radius.y, color)
            }
            DrawOp::EllipseLines { center, radius } => {
                d.draw_ellipse_lines_v(vec2_to_raylib(center), radius.x, radius.y, color)
            }
        }
    }
}

/// Draws one already-resolved shape: shadow pass, then each layer.
pub(super) fn draw_shape_item(
    d: &mut impl RaylibDraw,
    shape: &Shape,
    place: ShapePlacement,
    maybe_tint: Option<Tint>,
    maybe_shadow: Option<Shadow>,
    ops: &mut Vec<DrawOp>,
) {
    shape_passes(
        shape,
        place,
        maybe_tint,
        maybe_shadow,
        ops,
        |pass, color| draw_shape_ops(d, pass, color),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::math::Color as CoreColor;

    const RED: CoreColor = CoreColor::RED;
    const BLUE: CoreColor = CoreColor::BLUE;

    fn rgba(c: Color) -> (u8, u8, u8, u8) {
        (c.r, c.g, c.b, c.a)
    }

    /// Every pass `shape_passes` emits: (ops, colour).
    fn passes(
        shape: &Shape,
        place: ShapePlacement,
        tint: Option<Tint>,
        shadow: Option<Shadow>,
    ) -> Vec<(Vec<DrawOp>, (u8, u8, u8, u8))> {
        let mut out = Vec::new();
        let mut scratch = Vec::new();
        shape_passes(shape, place, tint, shadow, &mut scratch, |ops, c| {
            out.push((ops.to_vec(), rgba(c)));
        });
        out
    }

    fn rect() -> Shape {
        Shape::rect(Vec2::new(4.0, 2.0), RED)
    }

    #[test]
    fn an_outlined_shape_is_one_outline_pass_in_its_colour() {
        let p = passes(&rect(), ShapePlacement::at(Vec2::ZERO), None, None);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].1, (255, 0, 0, 255));
        assert_eq!(p[0].0.len(), 4);
        assert!(p[0].0.iter().all(|op| matches!(op, DrawOp::Line { .. })));
    }

    #[test]
    fn a_filled_shape_with_an_outline_colour_fills_then_outlines() {
        let s = rect().filled().with_outline_color(BLUE);
        let p = passes(&s, ShapePlacement::at(Vec2::ZERO), None, None);
        assert_eq!(p.len(), 2);
        assert!(
            p[0].0
                .iter()
                .all(|op| matches!(op, DrawOp::Triangle { .. }))
        );
        assert_eq!(p[0].1, (255, 0, 0, 255));
        assert!(p[1].0.iter().all(|op| matches!(op, DrawOp::Line { .. })));
        assert_eq!(p[1].1, (0, 0, 255, 255));
    }

    #[test]
    fn tint_multiplies_every_layer() {
        let s = rect().filled().with_outline_color(BLUE);
        let tint = Tint::new(128, 128, 128, 255);
        let p = passes(&s, ShapePlacement::at(Vec2::ZERO), Some(tint), None);
        assert_eq!(p[0].1, rgba(resolve_multiply_tint(Some(tint), RED)));
        assert_eq!(p[1].1, rgba(resolve_multiply_tint(Some(tint), BLUE)));
        assert_ne!(p[0].1, (255, 0, 0, 255));
    }

    #[test]
    fn the_shadow_pass_comes_first_in_the_shadow_colour_and_ignores_tint() {
        let shadow = Shadow::new(3.0, 5.0, 10, 20, 30, 40);
        let tint = Tint::new(128, 128, 128, 255);
        let p = passes(
            &rect(),
            ShapePlacement::at(Vec2::new(10.0, 10.0)),
            Some(tint),
            Some(shadow),
        );
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].1, (10, 20, 30, 40));
        assert_eq!(p[1].1, rgba(resolve_multiply_tint(Some(tint), RED)));
    }

    #[test]
    fn the_shadow_is_the_main_silhouette_moved_by_the_unrotated_offset() {
        let place = ShapePlacement {
            pos: Vec2::new(10.0, 10.0),
            scale: Vec2::new(2.0, 1.0),
            rotation_degrees: 90.0,
        };
        let shadow = Shadow::new(3.0, 5.0, 0, 0, 0, 128);
        let p = passes(&rect(), place, None, Some(shadow));
        let offset = Vec2::new(3.0, 5.0);
        let (main, shade) = (&p[1].0, &p[0].0);
        assert_eq!(main.len(), shade.len());
        for (m, s) in main.iter().zip(shade) {
            let (DrawOp::Line { a: ma, b: mb }, DrawOp::Line { a: sa, b: sb }) = (m, s) else {
                panic!("expected lines, got {m:?} / {s:?}");
            };
            assert!((*sa - *ma - offset).length() < 1e-4 && (*sb - *mb - offset).length() < 1e-4);
        }
    }

    #[test]
    fn a_filled_shapes_shadow_is_its_fill_silhouette_only() {
        let s = rect().filled().with_outline_color(BLUE);
        let shadow = Shadow::default_color(1.0, 1.0);
        let p = passes(&s, ShapePlacement::at(Vec2::ZERO), None, Some(shadow));
        assert_eq!(p.len(), 3);
        assert!(
            p[0].0
                .iter()
                .all(|op| matches!(op, DrawOp::Triangle { .. }))
        );
    }

    #[test]
    fn each_pass_sees_only_its_own_ops() {
        let s = rect().filled().with_outline_color(BLUE);
        let p = passes(
            &s,
            ShapePlacement::at(Vec2::ZERO),
            None,
            Some(Shadow::default_color(1.0, 1.0)),
        );
        let counts: Vec<usize> = p.iter().map(|(ops, _)| ops.len()).collect();
        assert_eq!(counts, vec![2, 2, 4]);
    }

    #[test]
    fn a_shape_that_draws_nothing_emits_empty_passes_not_stale_ops() {
        let empty = Shape::polygon(&[], RED);
        let p = passes(&empty, ShapePlacement::at(Vec2::ZERO), None, None);
        assert!(p.iter().all(|(ops, _)| ops.is_empty()));
    }
}
