//! Draws [`Shape`]s: turns the core draw ops of each layer into raylib calls.
//!
//! The pass sequence (shadow, then one pass per layer) is a pure function,
//! [`shape_passes`]; only [`draw_shape_ops`] touches raylib, as a 1:1 match
//! over the op kinds.

use raylib::prelude::*;

use super::geometry::bounds_overlap_view;
use super::math::{resolve_multiply_tint, shadow_color, vec2_to_raylib};
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::components::shadow::Shadow;
use aberred_core::components::shape::Shape;
use aberred_core::components::tint::Tint;
use aberred_core::components::world_position::WorldPosition;
use aberred_core::math::Vec2;
use aberred_core::systems::shape_geometry::{DrawOp, shape_cull_bounds, shape_layers, shape_ops};

/// Where and how a shape is drawn: the transform handed to `shape_ops`.
#[derive(Clone, Copy)]
pub(super) struct ShapePlacement {
    pub(super) pos: Vec2,
    pub(super) scale: Vec2,
    pub(super) rotation_degrees: f32,
}

impl ShapePlacement {
    /// Placement from a resolved world transform; an absent scale is 1 and an
    /// absent rotation is 0 degrees.
    pub(super) fn from_resolved(
        pos: &WorldPosition,
        scale: Option<&Scale>,
        rotation: Option<&Rotation>,
    ) -> Self {
        Self {
            pos: pos.pos,
            scale: scale.map_or(Vec2::ONE, |s| s.scale),
            rotation_degrees: rotation.map_or(0.0, |r| r.degrees),
        }
    }

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

/// Whether any part of the shape (or its shadow) can overlap the view box
/// `view_min..view_max`, with a 1 px margin for line width.
pub(super) fn shape_in_view(
    shape: &Shape,
    place: ShapePlacement,
    maybe_shadow: Option<Shadow>,
    view_min: Vector2,
    view_max: Vector2,
) -> bool {
    let (min, max) = shape_cull_bounds(shape, place.pos, place.scale, place.rotation_degrees);
    let margin = Vec2::ONE + maybe_shadow.map_or(Vec2::ZERO, |s| s.offset.abs());
    bounds_overlap_view(
        vec2_to_raylib(min - margin),
        vec2_to_raylib(max + margin),
        view_min,
        view_max,
    )
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

    type Rgba = (u8, u8, u8, u8);
    /// One pass `shape_passes` emits: (ops, colour).
    type Pass = (Vec<DrawOp>, Rgba);

    fn rgba(c: Color) -> Rgba {
        (c.r, c.g, c.b, c.a)
    }

    /// Every pass `shape_passes` emits.
    fn passes(
        shape: &Shape,
        place: ShapePlacement,
        tint: Option<Tint>,
        shadow: Option<Shadow>,
    ) -> Vec<Pass> {
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

    fn view() -> (Vector2, Vector2) {
        (Vector2::new(0.0, 0.0), Vector2::new(100.0, 100.0))
    }

    fn visible(shape: &Shape, place: ShapePlacement, shadow: Option<Shadow>) -> bool {
        let (min, max) = view();
        shape_in_view(shape, place, shadow, min, max)
    }

    #[test]
    fn a_shape_inside_the_view_is_visible_and_one_far_outside_is_not() {
        assert!(visible(
            &rect(),
            ShapePlacement::at(Vec2::new(50.0, 50.0)),
            None
        ));
        assert!(!visible(
            &rect(),
            ShapePlacement::at(Vec2::new(500.0, 50.0)),
            None
        ));
        assert!(!visible(
            &rect(),
            ShapePlacement::at(Vec2::new(-50.0, 50.0)),
            None
        ));
    }

    #[test]
    fn a_one_pixel_line_just_outside_the_view_still_counts_as_visible() {
        let line = Shape::line(Vec2::new(0.0, 10.0), RED);
        // The line sits at x = 100.5: past the view edge by half a pixel, inside the 1 px margin.
        assert!(visible(
            &line,
            ShapePlacement::at(Vec2::new(100.5, 0.0)),
            None
        ));
        assert!(!visible(
            &line,
            ShapePlacement::at(Vec2::new(102.0, 0.0)),
            None
        ));
    }

    #[test]
    fn a_shadow_widens_the_cull_box_by_its_offset() {
        let place = ShapePlacement::at(Vec2::new(-10.0, 50.0));
        assert!(!visible(&rect(), place, None));
        let shadow = Shadow::new(20.0, 0.0, 0, 0, 0, 128);
        assert!(visible(&rect(), place, Some(shadow)));
        let flipped = Shadow::new(-20.0, 0.0, 0, 0, 0, 128);
        assert!(visible(&rect(), place, Some(flipped)));
    }

    #[test]
    fn rotation_and_scale_move_the_cull_box() {
        let place = ShapePlacement {
            pos: Vec2::new(-3.0, 50.0),
            scale: Vec2::ONE,
            rotation_degrees: 90.0,
        };
        // Rotated 90 degrees clockwise the (4, 2) rect spans x in [-5, -3]: left of the view.
        assert!(!visible(&rect(), place, None));
        // Mirroring y before the rotation flips that extent toward +x, into the view.
        let mirrored = ShapePlacement {
            scale: Vec2::new(1.0, -50.0),
            ..place
        };
        assert!(visible(&rect(), mirrored, None));
    }

    #[test]
    fn a_resolved_placement_defaults_an_absent_scale_and_rotation() {
        let pos = WorldPosition::new(3.0, 4.0);
        let bare = ShapePlacement::from_resolved(&pos, None, None);
        assert_eq!(
            (bare.pos, bare.scale, bare.rotation_degrees),
            (Vec2::new(3.0, 4.0), Vec2::ONE, 0.0)
        );
        let full = ShapePlacement::from_resolved(
            &pos,
            Some(&Scale::new(2.0, 3.0)),
            Some(&Rotation { degrees: 45.0 }),
        );
        assert_eq!(
            (full.scale, full.rotation_degrees),
            (Vec2::new(2.0, 3.0), 45.0)
        );
    }
}
