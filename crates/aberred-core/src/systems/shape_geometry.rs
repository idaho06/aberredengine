//! Pure shape geometry: the draw ops a [`Shape`] produces, in world or screen space.
//!
//! The render thread maps each [`DrawOp`] 1:1 to a raylib draw call. Because
//! the ops need only core math types, the geometry is testable without a window.

use crate::components::shape::{Shape, ShapeKind};
use crate::math::{Color, Vec2, cross};

/// One primitive draw call, without a colour (the caller picks it per layer).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawOp {
    /// A single pixel.
    Pixel {
        /// Pixel position.
        p: Vec2,
    },
    /// A 1 px line segment.
    Line {
        /// Start point.
        a: Vec2,
        /// End point.
        b: Vec2,
    },
    /// A filled triangle in counter-clockwise order for raylib (`cross(b - a, c - a) < 0` in y-down coordinates).
    Triangle {
        /// First vertex.
        a: Vec2,
        /// Second vertex.
        b: Vec2,
        /// Third vertex.
        c: Vec2,
    },
    /// A filled circle.
    Circle {
        /// Centre.
        center: Vec2,
        /// Radius.
        radius: f32,
    },
    /// A 1 px circle outline.
    CircleLines {
        /// Centre.
        center: Vec2,
        /// Radius.
        radius: f32,
    },
    /// A filled axis-aligned ellipse.
    Ellipse {
        /// Centre.
        center: Vec2,
        /// Horizontal and vertical radii.
        radius: Vec2,
    },
    /// A 1 px axis-aligned ellipse outline.
    EllipseLines {
        /// Centre.
        center: Vec2,
        /// Horizontal and vertical radii.
        radius: Vec2,
    },
}

/// Which part of a shape a set of ops draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeLayer {
    /// The interior: only `Triangle`, `Circle` and `Ellipse` ops.
    Fill,
    /// The 1 px boundary: only `Pixel`, `Line`, `CircleLines` and `EllipseLines` ops.
    Outline,
}

/// The layers a shape draws, in draw order, with the colour each uses before tint.
///
/// An unfilled shape, a point and a line draw one `Outline` layer in `color`.
/// A filled shape draws a `Fill` layer in `color`, then an `Outline` layer in
/// `outline_color` when that is set.
pub fn shape_layers(shape: &Shape) -> [Option<(ShapeLayer, Color)>; 2] {
    match (&shape.kind, shape.filled) {
        (ShapeKind::Point | ShapeKind::Line { .. }, _) | (_, false) => {
            [Some((ShapeLayer::Outline, shape.color)), None]
        }
        _ => [
            Some((ShapeLayer::Fill, shape.color)),
            shape.outline_color.map(|c| (ShapeLayer::Outline, c)),
        ],
    }
}

/// Maps local shape coordinates to the draw surface.
struct Transform {
    pos: Vec2,
    origin: Vec2,
    scale: Vec2,
    /// Unit rotor of the rotation, built once per call (`sin_cos` through glam's `libm` path).
    rotor: Vec2,
}

impl Transform {
    /// `pos + rotate((v - origin) * scale, rotation)`; positive rotation is clockwise on the y-down screen.
    fn point(&self, v: Vec2) -> Vec2 {
        self.pos + self.rotor.rotate((v - self.origin) * self.scale)
    }
}

/// Appends the ops of one `layer` of `shape` to `out`.
///
/// `out` is not cleared, so a caller reuses one buffer per layer and steady
/// state allocates nothing. Local point `v` lands at
/// `pos + rotate((v - origin) * scale, rotation_degrees)`.
///
/// An `Ellipse` ignores the rotation. A `Circle` rotates only its centre and
/// becomes an ellipse when `scale` is non-uniform. Every `Triangle` is emitted
/// counter-clockwise for raylib whatever the input winding or mirroring, and
/// zero-area triangles are skipped.
pub fn shape_ops(
    shape: &Shape,
    layer: ShapeLayer,
    pos: Vec2,
    scale: Vec2,
    rotation_degrees: f32,
    out: &mut Vec<DrawOp>,
) {
    let xf = Transform {
        pos,
        origin: shape.origin,
        scale,
        rotor: Vec2::from_angle(rotation_degrees.to_radians()),
    };
    let fill = layer == ShapeLayer::Fill;
    match &shape.kind {
        ShapeKind::Point => {
            if !fill {
                out.push(DrawOp::Pixel {
                    p: xf.point(Vec2::ZERO),
                });
            }
        }
        ShapeKind::Line { to } => {
            if !fill {
                out.push(DrawOp::Line {
                    a: xf.point(Vec2::ZERO),
                    b: xf.point(*to),
                });
            }
        }
        ShapeKind::Rect { size } => {
            let c = [
                Vec2::ZERO,
                Vec2::new(size.x, 0.0),
                *size,
                Vec2::new(0.0, size.y),
            ]
            .map(|p| xf.point(p));
            if fill {
                push_triangle(out, c[0], c[1], c[2]);
                push_triangle(out, c[0], c[2], c[3]);
            } else {
                push_ring(out, c.len(), |i| c[i]);
            }
        }
        ShapeKind::Circle { radius } => {
            let radii = Vec2::splat(*radius) * scale.abs();
            let round = (radii.x - radii.y).abs() <= 4.0 * f32::EPSILON * radii.x.max(radii.y);
            push_round(out, fill, xf.point(Vec2::ZERO), radii, round);
        }
        ShapeKind::Ellipse { radius } => {
            let center = pos - shape.origin * scale;
            push_round(out, fill, center, *radius * scale.abs(), false);
        }
        ShapeKind::Polygon(poly) => {
            let verts = poly.vertices();
            if fill {
                for t in poly.triangles() {
                    let [a, b, c] = t.map(|i| xf.point(verts[i as usize]));
                    push_triangle(out, a, b, c);
                }
            } else {
                push_ring(out, verts.len(), |i| xf.point(verts[i]));
            }
        }
    }
}

/// Appends a triangle in raylib's counter-clockwise order, or nothing if it is flat.
fn push_triangle(out: &mut Vec<DrawOp>, a: Vec2, b: Vec2, c: Vec2) {
    let turn = cross(a, b, c);
    if turn < 0.0 {
        out.push(DrawOp::Triangle { a, b, c });
    } else if turn > 0.0 {
        out.push(DrawOp::Triangle { a, b: c, c: b });
    }
}

/// Appends the closed ring of `n` points as lines; two points make one line, fewer make none.
fn push_ring(out: &mut Vec<DrawOp>, n: usize, point: impl Fn(usize) -> Vec2) {
    if n < 2 {
        return;
    }
    let edges = if n == 2 { 1 } else { n };
    out.extend((0..edges).map(|i| DrawOp::Line {
        a: point(i),
        b: point((i + 1) % n),
    }));
}

/// Appends a circle (`round`, radius `radii.x`) or an ellipse, filled or outlined.
fn push_round(out: &mut Vec<DrawOp>, fill: bool, center: Vec2, radii: Vec2, round: bool) {
    out.push(match (round, fill) {
        (true, true) => DrawOp::Circle {
            center,
            radius: radii.x,
        },
        (true, false) => DrawOp::CircleLines {
            center,
            radius: radii.x,
        },
        (false, true) => DrawOp::Ellipse {
            center,
            radius: radii,
        },
        (false, false) => DrawOp::EllipseLines {
            center,
            radius: radii,
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::shape::Shape;
    use crate::math::{Color, Vec2};
    use crate::testing::{approx_eq, vec2_approx_eq};

    const C: Color = Color::RED;

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    fn ops(shape: &Shape, layer: ShapeLayer, pos: Vec2, scale: Vec2, rot: f32) -> Vec<DrawOp> {
        let mut out = Vec::new();
        shape_ops(shape, layer, pos, scale, rot, &mut out);
        out
    }

    fn outline(shape: &Shape) -> Vec<DrawOp> {
        ops(shape, ShapeLayer::Outline, Vec2::ZERO, Vec2::ONE, 0.0)
    }

    fn fill(shape: &Shape) -> Vec<DrawOp> {
        ops(shape, ShapeLayer::Fill, Vec2::ZERO, Vec2::ONE, 0.0)
    }

    fn lines(ops: &[DrawOp]) -> Vec<(Vec2, Vec2)> {
        ops.iter()
            .filter_map(|op| match *op {
                DrawOp::Line { a, b } => Some((a, b)),
                _ => None,
            })
            .collect()
    }

    fn triangles(ops: &[DrawOp]) -> Vec<[Vec2; 3]> {
        ops.iter()
            .filter_map(|op| match *op {
                DrawOp::Triangle { a, b, c } => Some([a, b, c]),
                _ => None,
            })
            .collect()
    }

    fn l_shape() -> Vec<Vec2> {
        vec![
            v(0.0, 0.0),
            v(4.0, 0.0),
            v(4.0, 2.0),
            v(2.0, 2.0),
            v(2.0, 4.0),
            v(0.0, 4.0),
        ]
    }

    #[test]
    fn a_point_is_one_pixel_at_the_position() {
        let o = ops(
            &Shape::point(C),
            ShapeLayer::Outline,
            v(3.0, 4.0),
            Vec2::ONE,
            0.0,
        );
        assert_eq!(o, vec![DrawOp::Pixel { p: v(3.0, 4.0) }]);
        assert!(
            ops(
                &Shape::point(C),
                ShapeLayer::Fill,
                v(3.0, 4.0),
                Vec2::ONE,
                0.0
            )
            .is_empty()
        );
    }

    #[test]
    fn a_line_transforms_both_endpoints() {
        let s = Shape::line(v(2.0, 1.0), C);
        let o = ops(&s, ShapeLayer::Outline, v(10.0, 10.0), v(2.0, 3.0), 0.0);
        assert_eq!(
            o,
            vec![DrawOp::Line {
                a: v(10.0, 10.0),
                b: v(14.0, 13.0)
            }]
        );
    }

    #[test]
    fn a_rect_outline_is_a_closed_ring_over_its_corners() {
        let l = lines(&outline(&Shape::rect(v(4.0, 2.0), C)));
        assert_eq!(
            l,
            vec![
                (v(0.0, 0.0), v(4.0, 0.0)),
                (v(4.0, 0.0), v(4.0, 2.0)),
                (v(4.0, 2.0), v(0.0, 2.0)),
                (v(0.0, 2.0), v(0.0, 0.0)),
            ]
        );
    }

    #[test]
    fn a_rect_fill_is_two_triangles_covering_its_area() {
        let t = triangles(&fill(&Shape::rect(v(4.0, 2.0), C)));
        assert_eq!(t.len(), 2);
        let area: f32 = t.iter().map(|t| cross(t[0], t[1], t[2]).abs() / 2.0).sum();
        assert!(approx_eq(area, 8.0));
    }

    #[test]
    fn origin_at_the_centre_centres_the_rect_on_the_position() {
        let s = Shape::rect(v(4.0, 2.0), C).with_origin(v(2.0, 1.0));
        let l = lines(&ops(&s, ShapeLayer::Outline, v(10.0, 10.0), Vec2::ONE, 0.0));
        assert_eq!(l[0].0, v(8.0, 9.0));
        assert_eq!(l[2].0, v(12.0, 11.0));
    }

    #[test]
    fn scale_grows_geometry_around_the_pivot() {
        let s = Shape::line(v(2.0, 0.0), C).with_origin(v(1.0, 0.0));
        let o = ops(&s, ShapeLayer::Outline, v(10.0, 10.0), v(3.0, 1.0), 0.0);
        assert_eq!(
            o,
            vec![DrawOp::Line {
                a: v(7.0, 10.0),
                b: v(13.0, 10.0)
            }]
        );
    }

    #[test]
    fn rotation_is_clockwise_on_the_y_down_screen() {
        let l = lines(&ops(
            &Shape::rect(v(4.0, 2.0), C),
            ShapeLayer::Outline,
            v(10.0, 10.0),
            Vec2::ONE,
            90.0,
        ));
        assert!(vec2_approx_eq(l[0].1, v(10.0, 14.0)));
    }

    #[test]
    fn the_pivot_does_not_move_under_rotation_or_scale() {
        let s = Shape::rect(v(4.0, 2.0), C).with_origin(v(2.0, 1.0));
        let l = lines(&ops(
            &s,
            ShapeLayer::Outline,
            v(10.0, 10.0),
            v(2.0, 3.0),
            37.0,
        ));
        let centre = (l[0].0 + l[1].0 + l[2].0 + l[3].0) / 4.0;
        assert!(vec2_approx_eq(centre, v(10.0, 10.0)));
    }

    #[test]
    fn a_uniformly_scaled_circle_stays_a_circle() {
        let s = Shape::circle(5.0, C);
        let o = ops(&s, ShapeLayer::Outline, v(1.0, 2.0), v(-2.0, 2.0), 0.0);
        assert_eq!(
            o,
            vec![DrawOp::CircleLines {
                center: v(1.0, 2.0),
                radius: 10.0
            }]
        );
        let f = ops(
            &s.clone().filled(),
            ShapeLayer::Fill,
            v(1.0, 2.0),
            v(2.0, 2.0),
            0.0,
        );
        assert_eq!(
            f,
            vec![DrawOp::Circle {
                center: v(1.0, 2.0),
                radius: 10.0
            }]
        );
    }

    #[test]
    fn a_circle_under_non_uniform_scale_becomes_an_ellipse() {
        let s = Shape::circle(5.0, C);
        let o = ops(&s, ShapeLayer::Outline, Vec2::ZERO, v(2.0, -3.0), 0.0);
        assert_eq!(
            o,
            vec![DrawOp::EllipseLines {
                center: Vec2::ZERO,
                radius: v(10.0, 15.0)
            }]
        );
        let f = ops(&s, ShapeLayer::Fill, Vec2::ZERO, v(2.0, 3.0), 0.0);
        assert_eq!(
            f,
            vec![DrawOp::Ellipse {
                center: Vec2::ZERO,
                radius: v(10.0, 15.0)
            }]
        );
    }

    #[test]
    fn an_ellipse_scales_per_axis_and_ignores_rotation() {
        let s = Shape::ellipse(v(4.0, 2.0), C).with_origin(v(1.0, 1.0));
        let at0 = ops(&s, ShapeLayer::Outline, v(10.0, 10.0), v(2.0, 3.0), 0.0);
        let at90 = ops(&s, ShapeLayer::Outline, v(10.0, 10.0), v(2.0, 3.0), 90.0);
        assert_eq!(at0, at90);
        assert_eq!(
            at0,
            vec![DrawOp::EllipseLines {
                center: v(8.0, 7.0),
                radius: v(8.0, 6.0)
            }]
        );
    }

    #[test]
    fn a_circle_centre_rotates_about_the_origin() {
        let s = Shape::circle(1.0, C).with_origin(v(1.0, 0.0));
        let o = ops(&s, ShapeLayer::Outline, Vec2::ZERO, Vec2::ONE, 90.0);
        let DrawOp::CircleLines { center, .. } = o[0] else {
            panic!("{o:?}")
        };
        assert!(vec2_approx_eq(center, v(0.0, -1.0)));
        let unrotated = ops(
            &Shape::circle(1.0, C),
            ShapeLayer::Outline,
            v(3.0, 3.0),
            Vec2::ONE,
            90.0,
        );
        assert_eq!(
            unrotated,
            vec![DrawOp::CircleLines {
                center: v(3.0, 3.0),
                radius: 1.0
            }]
        );
    }

    #[test]
    fn a_polygon_outline_is_a_closed_ring() {
        let l = lines(&outline(&Shape::polygon(&l_shape(), C)));
        assert_eq!(l.len(), 6);
        assert_eq!(l[5], (v(0.0, 4.0), v(0.0, 0.0)));
    }

    #[test]
    fn a_two_vertex_polygon_outlines_as_one_line_and_fewer_as_nothing() {
        assert_eq!(
            lines(&outline(&Shape::polygon(&[v(0.0, 0.0), v(3.0, 0.0)], C))).len(),
            1
        );
        assert!(outline(&Shape::polygon(&[v(1.0, 1.0)], C)).is_empty());
        assert!(outline(&Shape::polygon(&[], C)).is_empty());
    }

    #[test]
    fn a_polygon_fill_uses_the_stored_triangles() {
        let t = triangles(&fill(&Shape::polygon(&l_shape(), C)));
        assert_eq!(t.len(), 4);
        let area: f32 = t.iter().map(|t| cross(t[0], t[1], t[2]).abs() / 2.0).sum();
        assert!(approx_eq(area, 12.0));
    }

    #[test]
    fn shape_layers_follow_the_colour_model() {
        let blue = Color::BLUE;
        let layers = |s: &Shape| shape_layers(s).into_iter().flatten().collect::<Vec<_>>();
        let outlined = Shape::rect(v(1.0, 1.0), C).with_outline_color(blue);
        assert_eq!(layers(&outlined), vec![(ShapeLayer::Outline, C)]);
        let filled = Shape::rect(v(1.0, 1.0), C).filled();
        assert_eq!(layers(&filled), vec![(ShapeLayer::Fill, C)]);
        assert_eq!(
            layers(&filled.with_outline_color(blue)),
            vec![(ShapeLayer::Fill, C), (ShapeLayer::Outline, blue)]
        );
        let point = Shape::point(C).filled().with_outline_color(blue);
        assert_eq!(layers(&point), vec![(ShapeLayer::Outline, C)]);
        let line = Shape::line(v(1.0, 1.0), C)
            .filled()
            .with_outline_color(blue);
        assert_eq!(layers(&line), vec![(ShapeLayer::Outline, C)]);
    }

    #[test]
    fn each_layer_emits_only_its_own_op_kinds() {
        let shapes = [
            Shape::point(C),
            Shape::line(v(2.0, 2.0), C),
            Shape::rect(v(3.0, 2.0), C),
            Shape::circle(2.0, C),
            Shape::ellipse(v(3.0, 2.0), C),
            Shape::polygon(&l_shape(), C),
        ];
        for s in &shapes {
            for scale in [Vec2::ONE, v(2.0, 3.0)] {
                for op in ops(s, ShapeLayer::Fill, Vec2::ZERO, scale, 30.0) {
                    assert!(
                        matches!(
                            op,
                            DrawOp::Triangle { .. }
                                | DrawOp::Circle { .. }
                                | DrawOp::Ellipse { .. }
                        ),
                        "{s:?} fill emitted {op:?}"
                    );
                }
                for op in ops(s, ShapeLayer::Outline, Vec2::ZERO, scale, 30.0) {
                    assert!(
                        matches!(
                            op,
                            DrawOp::Pixel { .. }
                                | DrawOp::Line { .. }
                                | DrawOp::CircleLines { .. }
                                | DrawOp::EllipseLines { .. }
                        ),
                        "{s:?} outline emitted {op:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_triangle_is_counter_clockwise_for_raylib() {
        let ccw = l_shape();
        let cw: Vec<Vec2> = ccw.iter().rev().copied().collect();
        let shapes = [
            Shape::rect(v(3.0, 2.0), C),
            Shape::polygon(&ccw, C),
            Shape::polygon(&cw, C),
        ];
        for s in &shapes {
            for (scale, rot) in [
                (Vec2::ONE, 0.0),
                (v(-1.0, 1.0), 0.0),
                (v(1.0, -2.0), 0.0),
                (Vec2::ONE, 90.0),
            ] {
                let t = triangles(&ops(s, ShapeLayer::Fill, v(5.0, 5.0), scale, rot));
                assert!(!t.is_empty());
                assert!(
                    t.iter().all(|t| cross(t[0], t[1], t[2]) < 0.0),
                    "{s:?} {scale:?} {rot}"
                );
            }
        }
    }

    #[test]
    fn raylibs_own_example_triangle_is_left_unchanged() {
        let top = v(0.0, -1.0);
        let (bl, br) = (v(-1.0, 0.0), v(1.0, 0.0));
        let s = Shape::polygon(&[top, bl, br], C);
        assert_eq!(triangles(&fill(&s)), vec![[top, bl, br]]);
    }

    #[test]
    fn zero_area_triangles_are_skipped() {
        let flat = ops(
            &Shape::rect(v(3.0, 2.0), C),
            ShapeLayer::Fill,
            Vec2::ZERO,
            v(0.0, 1.0),
            0.0,
        );
        assert!(flat.is_empty());
    }

    #[test]
    fn reusing_the_buffer_does_not_grow_its_capacity() {
        let s = Shape::polygon(&l_shape(), C);
        let mut out = Vec::new();
        shape_ops(&s, ShapeLayer::Fill, Vec2::ZERO, Vec2::ONE, 0.0, &mut out);
        let cap = out.capacity();
        for _ in 0..3 {
            out.clear();
            shape_ops(&s, ShapeLayer::Fill, Vec2::ZERO, Vec2::ONE, 0.0, &mut out);
        }
        assert_eq!(out.capacity(), cap);
    }
}
