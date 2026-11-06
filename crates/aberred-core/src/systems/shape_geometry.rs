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
    /// A line segment of the given `thickness` with square caps: `a` and `b`
    /// already include the cap extension of `thickness / 2` beyond the
    /// geometric endpoints, so joints close without gaps. Only the `Outline`
    /// layer emits this op.
    ThickLine {
        /// Start point, extended by half the width along the segment.
        a: Vec2,
        /// End point, extended by half the width along the segment.
        b: Vec2,
        /// Width in the same units as `a` and `b`.
        thickness: f32,
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
    /// The boundary: only `Pixel`, `Line`, `ThickLine`, `CircleLines` and `EllipseLines` ops.
    Outline,
}

/// The layers a shape draws, in draw order, with the colour each uses before tint.
///
/// An unfilled shape, a point, a line and a bezier curve draw one `Outline` layer
/// in `color`. A filled shape draws a `Fill` layer in `color`, then an `Outline`
/// layer in `outline_color` when that is set.
pub fn shape_layers(shape: &Shape) -> [Option<(ShapeLayer, Color)>; 2] {
    match (&shape.kind, shape.filled) {
        (ShapeKind::Point | ShapeKind::Line { .. } | ShapeKind::Bezier { .. }, _) | (_, false) => {
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
    fn new(shape: &Shape, pos: Vec2, scale: Vec2, rotation_degrees: f32) -> Self {
        Self {
            pos,
            origin: shape.origin,
            scale,
            rotor: Vec2::from_angle(rotation_degrees.to_radians()),
        }
    }

    /// `pos + rotate((v - origin) * scale, rotation)`; positive rotation is clockwise on the y-down screen.
    fn point(&self, v: Vec2) -> Vec2 {
        self.pos + self.rotor.rotate((v - self.origin) * self.scale)
    }

    /// The corners `(0,0), (w,0), (w,h), (0,h)` of a rectangle of `size`, transformed.
    fn rect_corners(&self, size: Vec2) -> [Vec2; 4] {
        [
            Vec2::ZERO,
            Vec2::new(size.x, 0.0),
            size,
            Vec2::new(0.0, size.y),
        ]
        .map(|p| self.point(p))
    }

    /// The four edges (top, right, bottom, left) of a rectangle's 1 px
    /// outline, transformed. Each lies half a world unit inside its side and
    /// spans the whole side. At zoom 1, for an integral position, size and
    /// scale and no rotation, every edge then runs along pixel centers with
    /// its ends between pixels and the corner pixels are covered by two
    /// edges. A GPU can break the tie for a pixel centre exactly on a line's
    /// end or edge either way, which an edge on the lattice or a ring joined
    /// at pixel centers can leave as a gap. The inset shrinks to half the size
    /// for a rectangle thinner than that.
    fn rect_outline_edges(&self, size: Vec2) -> [(Vec2, Vec2); 4] {
        let inset = (Vec2::splat(0.5) / self.scale.abs()).min(size.abs() / 2.0) * size.signum();
        let far = size - inset;
        [
            (Vec2::new(0.0, inset.y), Vec2::new(size.x, inset.y)),
            (Vec2::new(far.x, 0.0), Vec2::new(far.x, size.y)),
            (Vec2::new(size.x, far.y), Vec2::new(0.0, far.y)),
            (Vec2::new(inset.x, size.y), Vec2::new(inset.x, 0.0)),
        ]
        .map(|(a, b)| (self.point(a), self.point(b)))
    }

    /// Centre, radii and whether it is still a circle (`|sx| == |sy|`) for a `Circle` of `radius`.
    fn circle(&self, radius: f32) -> (Vec2, Vec2, bool) {
        let radii = Vec2::splat(radius) * self.scale.abs();
        let round = (radii.x - radii.y).abs() <= 4.0 * f32::EPSILON * radii.x.max(radii.y);
        (self.point(Vec2::ZERO), radii, round)
    }

    /// Centre and radii of an `Ellipse`; the centre ignores the rotation.
    fn ellipse(&self, radius: Vec2) -> (Vec2, Vec2) {
        (
            self.pos - self.origin * self.scale,
            radius * self.scale.abs(),
        )
    }
}

/// Appends the ops of one `layer` of `shape` to `out`.
///
/// `out` is not cleared, so a caller reuses one buffer per layer and steady
/// state allocates nothing. Local point `v` lands at
/// `pos + rotate((v - origin) * scale, rotation_degrees)`.
///
/// An outlined `Rect` is four full-length edges half a world unit inside its
/// sides (see `Transform::rect_outline_edges`). `Line` and `Polygon` outlines
/// are drawn at their exact transformed coordinates, on the lattice between
/// pixels. A `Bezier` is sampled at 24 equal parameter steps into 24 connected
/// segments whose first and last points are the transformed `(0, 0)` and `to`
/// exactly. An `Ellipse` ignores the rotation. A `Circle` rotates only its centre and
/// becomes an ellipse when `scale` is non-uniform. Every `Triangle` is emitted
/// counter-clockwise for raylib whatever the input winding or mirroring, and
/// zero-area triangles are skipped.
///
/// A positive [`Shape::thickness`] widens a `Line`, a `Bezier` and the outlines
/// of a `Rect` and a `Polygon` to `thickness * mean(|scale.x|, |scale.y|)` world
/// units, emitted as [`DrawOp::ThickLine`] with square caps (each segment extended
/// by half its width at both ends so joints close without gaps; overlapping caps
/// may darken semi-transparent colours). A thick `Rect` outline is inset by
/// `thickness / 2` local units so the stroke stays inside the rect bounds;
/// `Line`, `Bezier` and `Polygon` outlines straddle their path. `Point`, `Circle`
/// and `Ellipse` ignore `thickness`. A `None` or non-positive `thickness` keeps
/// the 1 px hairline.
pub fn shape_ops(
    shape: &Shape,
    layer: ShapeLayer,
    pos: Vec2,
    scale: Vec2,
    rotation_degrees: f32,
    out: &mut Vec<DrawOp>,
) {
    let xf = Transform::new(shape, pos, scale, rotation_degrees);
    let fill = layer == ShapeLayer::Fill;
    let stroke = stroke_of(shape, scale);
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
                push_segment(out, xf.point(Vec2::ZERO), xf.point(*to), stroke);
            }
        }
        ShapeKind::Bezier { c1, c2, to } => {
            if !fill {
                // An affine transform of a cubic is the cubic of the transformed
                // control points, so transforming the four control points is exact.
                push_bezier(
                    out,
                    [Vec2::ZERO, *c1, *c2, *to].map(|p| xf.point(p)),
                    stroke,
                );
            }
        }
        ShapeKind::Rect { size } => {
            if fill {
                let c = xf.rect_corners(*size);
                push_triangle(out, c[0], c[1], c[2]);
                push_triangle(out, c[0], c[2], c[3]);
            } else if let Some((t, w)) = stroke {
                // The stroke lies inside the rect bounds: the path is the rect
                // inset by t / 2 per side (clamped to half the size, like the
                // hairline inset) and the caps extend it by w / 2 along each edge.
                let inset = Vec2::splat(t / 2.0).min(size.abs() / 2.0) * size.signum();
                let far = size - inset;
                let corners = [
                    Vec2::new(inset.x, inset.y),
                    Vec2::new(far.x, inset.y),
                    far,
                    Vec2::new(inset.x, far.y),
                ]
                .map(|p| xf.point(p));
                for i in 0..4 {
                    push_thick_line(out, corners[i], corners[(i + 1) % 4], w);
                }
            } else {
                out.extend(
                    xf.rect_outline_edges(*size)
                        .map(|(a, b)| DrawOp::Line { a, b }),
                );
            }
        }
        ShapeKind::Circle { radius } => {
            let (center, radii, round) = xf.circle(*radius);
            push_round(out, fill, center, radii, round);
        }
        ShapeKind::Ellipse { radius } => {
            let (center, radii) = xf.ellipse(*radius);
            push_round(out, fill, center, radii, false);
        }
        ShapeKind::Polygon(poly) => {
            let verts = poly.vertices();
            if fill {
                for t in poly.triangles() {
                    let [a, b, c] = t.map(|i| xf.point(verts[i as usize]));
                    push_triangle(out, a, b, c);
                }
            } else {
                push_ring(
                    out,
                    verts.len(),
                    |i| xf.point(verts[i]),
                    |out, a, b| push_segment(out, a, b, stroke),
                );
            }
        }
    }
}

/// The axis-aligned box `(min, max)` of the geometry [`shape_ops`] emits for the same arguments.
///
/// The arguments follow [`shape_ops`]: `scale` may be negative and the rotation
/// is in degrees, clockwise on the y-down screen. A point has `min == max`, and
/// a polygon without vertices gets the degenerate box at `pos`. The box excludes
/// any shadow offset and the line width (hairline or `thickness`); a culling
/// caller adds both.
pub fn shape_cull_bounds(
    shape: &Shape,
    pos: Vec2,
    scale: Vec2,
    rotation_degrees: f32,
) -> (Vec2, Vec2) {
    let xf = Transform::new(shape, pos, scale, rotation_degrees);
    let around = |center: Vec2, radii: Vec2| (center - radii, center + radii);
    let found = match &shape.kind {
        ShapeKind::Point => bounding_box([xf.point(Vec2::ZERO)]),
        ShapeKind::Line { to } => bounding_box([Vec2::ZERO, *to].map(|p| xf.point(p))),
        ShapeKind::Bezier { c1, c2, to } => {
            // The curve lies inside the convex hull of its control points, so the
            // box of the four transformed control points encloses it.
            bounding_box([Vec2::ZERO, *c1, *c2, *to].map(|p| xf.point(p)))
        }
        ShapeKind::Rect { size } => bounding_box(xf.rect_corners(*size)),
        ShapeKind::Circle { radius } => {
            let (center, radii, _) = xf.circle(*radius);
            Some(around(center, radii))
        }
        ShapeKind::Ellipse { radius } => {
            let (center, radii) = xf.ellipse(*radius);
            Some(around(center, radii))
        }
        ShapeKind::Polygon(poly) => bounding_box(poly.vertices().iter().map(|&p| xf.point(p))),
    };
    found.unwrap_or((pos, pos))
}

/// The `(min, max)` corners of `points`, or `None` when there are none.
fn bounding_box(points: impl IntoIterator<Item = Vec2>) -> Option<(Vec2, Vec2)> {
    points.into_iter().fold(None, |acc, p| {
        Some(acc.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))))
    })
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

/// Appends the closed ring of `n` points as edges; two points make one edge, fewer make none.
fn push_ring(
    out: &mut Vec<DrawOp>,
    n: usize,
    point: impl Fn(usize) -> Vec2,
    push_edge: impl Fn(&mut Vec<DrawOp>, Vec2, Vec2),
) {
    if n < 2 {
        return;
    }
    let edges = if n == 2 { 1 } else { n };
    for i in 0..edges {
        push_edge(out, point(i), point((i + 1) % n));
    }
}

/// The stroke `shape` draws with at `scale` when its `thickness` is positive:
/// `(local thickness, width in the target space)`, the width being the local
/// thickness times the mean of `|scale.x|`, `|scale.y|`. `None` keeps the
/// 1 px hairline.
fn stroke_of(shape: &Shape, scale: Vec2) -> Option<(f32, f32)> {
    shape
        .effective_thickness()
        .map(|t| (t, t * (scale.x.abs() + scale.y.abs()) / 2.0))
}

/// Appends one outline segment `a -> b`: a square-capped [`DrawOp::ThickLine`]
/// of the stroke's width when there is one, else a 1 px [`DrawOp::Line`].
fn push_segment(out: &mut Vec<DrawOp>, a: Vec2, b: Vec2, stroke: Option<(f32, f32)>) {
    match stroke {
        Some((_, w)) => push_thick_line(out, a, b, w),
        None => out.push(DrawOp::Line { a, b }),
    }
}

/// Appends `a -> b` as a square-capped thick segment of width `thickness`:
/// each end is extended by `thickness / 2` along the segment direction. A
/// zero-length segment emits nothing.
fn push_thick_line(out: &mut Vec<DrawOp>, a: Vec2, b: Vec2, thickness: f32) {
    let d = b - a;
    let len = d.length();
    if len == 0.0 {
        return;
    }
    let ext = d * (thickness / 2.0 / len);
    out.push(DrawOp::ThickLine {
        a: a - ext,
        b: b + ext,
        thickness,
    });
}

/// How many equal parameter steps a [`ShapeKind::Bezier`] is sampled into.
const BEZIER_SEGMENTS: usize = 24;

/// Appends the cubic with transformed control points `p` as `BEZIER_SEGMENTS`
/// connected segments, each through `push_segment`: with a stroke each is a
/// square-capped [`DrawOp::ThickLine`] extended by half its width at both ends
/// (the cap rule of `push_thick_line`), so the joints close; without one it is
/// a 1 px [`DrawOp::Line`], whose joins can show hairline gaps. The first
/// point is `p[0]` and the last is `p[3]`, taken straight from the control
/// points, so they equal the transformed start and `to` exactly.
fn push_bezier(out: &mut Vec<DrawOp>, p: [Vec2; 4], stroke: Option<(f32, f32)>) {
    let mut prev = p[0];
    for i in 1..=BEZIER_SEGMENTS {
        let next = if i == BEZIER_SEGMENTS {
            p[3]
        } else {
            let t = i as f32 / BEZIER_SEGMENTS as f32;
            let u = 1.0 - t;
            p[0] * (u * u * u)
                + p[1] * (3.0 * u * u * t)
                + p[2] * (3.0 * u * t * t)
                + p[3] * (t * t * t)
        };
        push_segment(out, prev, next, stroke);
        prev = next;
    }
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

    fn bezier(c1: Vec2, c2: Vec2, to: Vec2) -> Shape {
        Shape::bezier(c1, c2, to, C)
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

    fn thick_lines(ops: &[DrawOp]) -> Vec<(Vec2, Vec2, f32)> {
        ops.iter()
            .filter_map(|op| match *op {
                DrawOp::ThickLine { a, b, thickness } => Some((a, b, thickness)),
                _ => None,
            })
            .collect()
    }

    /// The stroke of the thick segment `(a, b, w)` (its endpoints plus half the
    /// width on each side) lies within `lo..hi`.
    fn assert_stroke_inside((a, b, w): (Vec2, Vec2, f32), lo: Vec2, hi: Vec2) {
        let d = (b - a) / (b - a).length();
        let perp = v(-d.y, d.x) * (w / 2.0);
        for p in [a + perp, a - perp, b + perp, b - perp] {
            assert!(p.cmpge(lo).all() && p.cmple(hi).all(), "{p:?}");
        }
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
    fn a_rect_outline_is_four_full_edges_half_a_pixel_inside() {
        let l = lines(&outline(&Shape::rect(v(4.0, 2.0), C)));
        assert_eq!(
            l,
            vec![
                (v(0.0, 0.5), v(4.0, 0.5)),
                (v(3.5, 0.0), v(3.5, 2.0)),
                (v(4.0, 1.5), v(0.0, 1.5)),
                (v(0.5, 2.0), v(0.5, 0.0)),
            ]
        );
    }

    /// The inset is half a pixel on the screen, whatever the scale, so a
    /// scaled rect's outline still covers exactly the pixels of the rect.
    #[test]
    fn the_rect_outline_inset_is_half_a_screen_pixel_under_scale() {
        let l = lines(&ops(
            &Shape::rect(v(4.0, 2.0), C),
            ShapeLayer::Outline,
            v(10.0, 20.0),
            v(4.0, 2.0),
            0.0,
        ));
        assert_eq!(l[0], (v(10.0, 20.5), v(26.0, 20.5)));
        assert_eq!(l[1], (v(25.5, 20.0), v(25.5, 24.0)));
        assert_eq!(l[2], (v(26.0, 23.5), v(10.0, 23.5)));
        assert_eq!(l[3], (v(10.5, 24.0), v(10.5, 20.0)));
    }

    /// A 1 px line is a thin quad: a pixel whose centre sits exactly on the
    /// line's end or edge is covered or not depending on the GPU. The outline
    /// must cover the rect's perimeter pixels with no such tie (the pixels
    /// whose centre lies strictly inside every edge's span).
    #[test]
    fn a_rect_outline_covers_exactly_its_perimeter_pixels_without_ties() {
        let (x, y, w, h) = (192, 214, 256, 16);
        let shape = Shape::rect(v(w as f32, h as f32), C);
        let l = lines(&ops(
            &shape,
            ShapeLayer::Outline,
            v(x as f32, y as f32),
            Vec2::ONE,
            0.0,
        ));

        let mut covered = std::collections::BTreeSet::new();
        for &(a, b) in &l {
            // Across the line, a pixel centre is on the line itself.
            let horizontal = a.y == b.y;
            let (across, from, to) = if horizontal {
                (a.y, a.x.min(b.x), a.x.max(b.x))
            } else {
                (a.x, a.y.min(b.y), a.y.max(b.y))
            };
            assert_eq!(across.fract(), 0.5, "{a:?} -> {b:?}");
            for along in from.floor() as i32..to.ceil() as i32 {
                let centre = along as f32 + 0.5;
                if from < centre && centre < to {
                    let across = across.floor() as i32;
                    covered.insert(if horizontal {
                        (along, across)
                    } else {
                        (across, along)
                    });
                }
            }
        }

        let perimeter: std::collections::BTreeSet<(i32, i32)> = (x..x + w)
            .flat_map(|px| (y..y + h).map(move |py| (px, py)))
            .filter(|&(px, py)| px == x || px == x + w - 1 || py == y || py == y + h - 1)
            .collect();
        assert_eq!(covered, perimeter);
    }

    /// A rect thinner than the inset has no room for it: the edges collapse
    /// on the rect's centre line instead of swapping sides.
    #[test]
    fn a_rect_thinner_than_a_pixel_keeps_its_edges_on_their_own_sides() {
        let l = lines(&outline(&Shape::rect(v(0.5, 0.5), C)));
        assert!(
            l[0].0.y <= l[2].0.y,
            "the top edge stays above the bottom edge"
        );
        assert!(
            l[3].0.x <= l[1].0.x,
            "the left edge stays left of the right edge"
        );
        let inside = |p: Vec2| (0.0..=0.5).contains(&p.x) && (0.0..=0.5).contains(&p.y);
        assert!(l.iter().all(|&(a, b)| inside(a) && inside(b)));
    }

    #[test]
    fn a_zero_scale_rect_outline_stays_finite() {
        let l = lines(&ops(
            &Shape::rect(v(4.0, 2.0), C),
            ShapeLayer::Outline,
            v(5.0, 5.0),
            v(0.0, 1.0),
            0.0,
        ));
        assert_eq!(l.len(), 4);
        assert!(l.iter().all(|(a, b)| a.is_finite() && b.is_finite()));
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
        assert_eq!(l[0].0, v(8.0, 9.5));
        assert_eq!(l[2].0, v(12.0, 10.5));
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
        assert!(vec2_approx_eq(l[0].1, v(9.5, 14.0)));
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
    fn a_thick_line_emits_one_thick_line_with_the_mean_scaled_width() {
        let s = Shape::line(v(4.0, 0.0), C).with_thickness(2.0);
        // width = 2 * mean(|2|, |4|) = 6.
        let o = ops(&s, ShapeLayer::Outline, v(10.0, 10.0), v(2.0, 4.0), 0.0);
        let t = thick_lines(&o);
        assert_eq!(t.len(), 1);
        assert!(approx_eq(t[0].2, 6.0));
        // The segment runs (10, 10) -> (18, 10); square caps extend it by width / 2 = 3.
        assert!(vec2_approx_eq(t[0].0, v(7.0, 10.0)));
        assert!(vec2_approx_eq(t[0].1, v(21.0, 10.0)));
    }

    #[test]
    fn thick_line_caps_extend_along_the_segment_direction() {
        let s = Shape::line(v(3.0, 4.0), C).with_thickness(2.0);
        let t = thick_lines(&outline(&s));
        assert_eq!(t.len(), 1);
        assert!(approx_eq(t[0].2, 2.0));
        // Direction (3, 4) / 5, extension width / 2 = 1: (0.6, 0.8) at each end.
        assert!(vec2_approx_eq(t[0].0, v(-0.6, -0.8)));
        assert!(vec2_approx_eq(t[0].1, v(3.6, 4.8)));
    }

    #[test]
    fn a_zero_length_thick_line_emits_nothing() {
        let s = Shape::line(Vec2::ZERO, C).with_thickness(2.0);
        assert!(outline(&s).is_empty());
    }

    #[test]
    fn a_non_positive_thickness_still_draws_a_hairline() {
        for t in [0.0, -3.0] {
            let s = Shape::line(v(2.0, 1.0), C).with_thickness(t);
            assert_eq!(
                outline(&s),
                vec![DrawOp::Line {
                    a: Vec2::ZERO,
                    b: v(2.0, 1.0)
                }],
                "thickness {t}"
            );
        }
    }

    #[test]
    fn a_shape_without_thickness_emits_no_thick_lines() {
        let shapes = [
            Shape::point(C),
            Shape::line(v(2.0, 2.0), C),
            Shape::rect(v(3.0, 2.0), C),
            Shape::circle(2.0, C),
            Shape::ellipse(v(3.0, 2.0), C),
            Shape::polygon(&l_shape(), C),
        ];
        for s in &shapes {
            assert!(
                thick_lines(&ops(s, ShapeLayer::Outline, Vec2::ZERO, Vec2::ONE, 0.0)).is_empty(),
                "{s:?}"
            );
        }
    }

    #[test]
    fn a_thick_rect_outline_is_four_thick_lines_whose_stroke_stays_inside_the_rect() {
        let size = v(10.0, 6.0);
        let s = Shape::rect(size, C).with_thickness(2.0);
        let t = thick_lines(&outline(&s));
        assert_eq!(t.len(), 4);
        assert!(t.iter().all(|&(_, _, w)| approx_eq(w, 2.0)));
        // The path is the rect inset by t / 2 = 1 per side, caps extend it by width / 2 = 1.
        let expected = [
            (v(0.0, 1.0), v(10.0, 1.0)),
            (v(9.0, 0.0), v(9.0, 6.0)),
            (v(10.0, 5.0), v(0.0, 5.0)),
            (v(1.0, 6.0), v(1.0, 0.0)),
        ];
        for (i, &(a, b, _)) in t.iter().enumerate() {
            assert!(vec2_approx_eq(a, expected[i].0), "edge {i}: {a:?}");
            assert!(vec2_approx_eq(b, expected[i].1), "edge {i}: {b:?}");
        }
        // The stroke (path plus half width plus caps) never leaves the rect bounds.
        for &seg in &t {
            assert_stroke_inside(seg, Vec2::ZERO, size);
        }
    }

    #[test]
    fn a_thick_rect_outline_scales_the_inset_and_the_width_with_the_scale() {
        let size = v(10.0, 6.0);
        let s = Shape::rect(size, C).with_thickness(2.0);
        let t = thick_lines(&ops(
            &s,
            ShapeLayer::Outline,
            v(10.0, 20.0),
            v(2.0, 2.0),
            0.0,
        ));
        assert_eq!(t.len(), 4);
        // width = 2 * 2 = 4, the inset stays t / 2 = 1 local unit = 2 world units.
        assert!(t.iter().all(|&(_, _, w)| approx_eq(w, 4.0)));
        assert!(vec2_approx_eq(t[0].0, v(10.0, 22.0)));
        assert!(vec2_approx_eq(t[0].1, v(30.0, 22.0)));
        let lo = v(10.0, 20.0);
        let hi = lo + size * 2.0;
        for &seg in &t {
            assert_stroke_inside(seg, lo, hi);
        }
    }

    #[test]
    fn a_thick_polygon_outline_emits_one_thick_line_per_edge() {
        let s = Shape::polygon(&l_shape(), C).with_thickness(2.0);
        let t = thick_lines(&outline(&s));
        assert_eq!(t.len(), 6);
        assert!(t.iter().all(|&(_, _, w)| approx_eq(w, 2.0)));
        // First edge (0, 0) -> (4, 0), caps extend by 1: (-1, 0) -> (5, 0).
        assert!(vec2_approx_eq(t[0].0, v(-1.0, 0.0)));
        assert!(vec2_approx_eq(t[0].1, v(5.0, 0.0)));
        // Closing edge (0, 4) -> (0, 0), caps extend by 1 along it: (0, 5) -> (0, -1).
        assert!(vec2_approx_eq(t[5].0, v(0.0, 5.0)));
        assert!(vec2_approx_eq(t[5].1, v(0.0, -1.0)));
    }

    #[test]
    fn point_circle_and_ellipse_ignore_the_thickness() {
        let pairs = [
            (Shape::point(C), Shape::point(C).with_thickness(4.0)),
            (
                Shape::circle(2.0, C),
                Shape::circle(2.0, C).with_thickness(4.0),
            ),
            (
                Shape::ellipse(v(3.0, 2.0), C),
                Shape::ellipse(v(3.0, 2.0), C).with_thickness(4.0),
            ),
        ];
        for (plain, thick) in pairs {
            assert_eq!(outline(&plain), outline(&thick));
            assert_eq!(fill(&plain), fill(&thick));
        }
    }

    #[test]
    fn the_fill_layer_of_a_thick_shape_is_unchanged() {
        let pairs = [
            (
                Shape::rect(v(4.0, 2.0), C),
                Shape::rect(v(4.0, 2.0), C).with_thickness(3.0),
            ),
            (
                Shape::polygon(&l_shape(), C),
                Shape::polygon(&l_shape(), C).with_thickness(3.0),
            ),
        ];
        for (plain, thick) in pairs {
            assert_eq!(fill(&plain), fill(&thick));
        }
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
        let b = bezier(v(1.0, 3.0), v(4.0, -1.0), v(6.0, 2.0));
        let shapes = [
            Shape::point(C),
            Shape::line(v(2.0, 2.0), C),
            Shape::rect(v(3.0, 2.0), C),
            Shape::circle(2.0, C),
            Shape::ellipse(v(3.0, 2.0), C),
            Shape::polygon(&l_shape(), C),
            b.clone(),
            Shape::line(v(2.0, 2.0), C).with_thickness(1.5),
            Shape::rect(v(3.0, 2.0), C).with_thickness(1.5),
            Shape::polygon(&l_shape(), C).with_thickness(1.5),
            b.with_thickness(1.5),
            Shape::point(C).with_thickness(1.5),
            Shape::circle(2.0, C).with_thickness(1.5),
            Shape::ellipse(v(3.0, 2.0), C).with_thickness(1.5),
        ];
        let mut saw_thick_line = false;
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
                    if matches!(op, DrawOp::ThickLine { .. }) {
                        saw_thick_line = true;
                    }
                    assert!(
                        matches!(
                            op,
                            DrawOp::Pixel { .. }
                                | DrawOp::Line { .. }
                                | DrawOp::ThickLine { .. }
                                | DrawOp::CircleLines { .. }
                                | DrawOp::EllipseLines { .. }
                        ),
                        "{s:?} outline emitted {op:?}"
                    );
                }
            }
        }
        assert!(saw_thick_line, "the thick shapes should emit ThickLine ops");
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

    #[test]
    fn a_point_has_degenerate_bounds() {
        let (min, max) = shape_cull_bounds(&Shape::point(C), v(3.0, 4.0), Vec2::ONE, 0.0);
        assert_eq!((min, max), (v(3.0, 4.0), v(3.0, 4.0)));
    }

    #[test]
    fn a_polygon_without_vertices_has_a_degenerate_box_at_the_position() {
        let s = Shape::polygon(&[], C);
        assert_eq!(
            shape_cull_bounds(&s, v(2.0, 3.0), Vec2::ONE, 0.0),
            (v(2.0, 3.0), v(2.0, 3.0))
        );
    }

    #[test]
    fn unrotated_line_rect_and_polygon_get_their_tight_box() {
        let one = Vec2::ONE;
        assert_eq!(
            shape_cull_bounds(&Shape::line(v(-2.0, 5.0), C), v(1.0, 1.0), one, 0.0),
            (v(-1.0, 1.0), v(1.0, 6.0))
        );
        assert_eq!(
            shape_cull_bounds(&Shape::rect(v(4.0, 2.0), C), v(10.0, 10.0), one, 0.0),
            (v(10.0, 10.0), v(14.0, 12.0))
        );
        assert_eq!(
            shape_cull_bounds(&Shape::polygon(&l_shape(), C), Vec2::ZERO, one, 0.0),
            (v(0.0, 0.0), v(4.0, 4.0))
        );
    }

    #[test]
    fn a_circle_is_its_centre_plus_or_minus_the_scaled_radius() {
        let (min, max) = shape_cull_bounds(&Shape::circle(5.0, C), v(10.0, 10.0), v(2.0, 3.0), 0.0);
        assert_eq!((min, max), (v(0.0, -5.0), v(20.0, 25.0)));
    }

    #[test]
    fn an_ellipse_box_ignores_rotation() {
        let s = Shape::ellipse(v(4.0, 2.0), C).with_origin(v(1.0, 1.0));
        let at0 = shape_cull_bounds(&s, v(10.0, 10.0), v(2.0, 3.0), 0.0);
        assert_eq!(at0, (v(0.0, 1.0), v(16.0, 13.0)));
        assert_eq!(shape_cull_bounds(&s, v(10.0, 10.0), v(2.0, 3.0), 77.0), at0);
    }

    #[test]
    fn a_rotated_rect_gets_the_exact_box_of_its_rotated_corners() {
        let (min, max) =
            shape_cull_bounds(&Shape::rect(v(4.0, 2.0), C), v(10.0, 10.0), Vec2::ONE, 90.0);
        assert!(vec2_approx_eq(min, v(8.0, 10.0)));
        assert!(vec2_approx_eq(max, v(10.0, 14.0)));
    }

    #[test]
    fn origin_shifts_the_box_and_moves_a_circle_under_rotation() {
        let r = Shape::rect(v(4.0, 2.0), C).with_origin(v(2.0, 1.0));
        assert_eq!(
            shape_cull_bounds(&r, v(10.0, 10.0), Vec2::ONE, 0.0),
            (v(8.0, 9.0), v(12.0, 11.0))
        );
        let c = Shape::circle(1.0, C).with_origin(v(1.0, 0.0));
        let (min, max) = shape_cull_bounds(&c, Vec2::ZERO, Vec2::ONE, 90.0);
        assert!(vec2_approx_eq(min, v(-1.0, -2.0)));
        assert!(vec2_approx_eq(max, v(1.0, 0.0)));
    }

    #[test]
    fn negative_scale_keeps_min_below_max() {
        for s in [
            Shape::rect(v(4.0, 2.0), C),
            Shape::circle(2.0, C),
            Shape::polygon(&l_shape(), C),
        ] {
            let (min, max) = shape_cull_bounds(&s, v(5.0, 5.0), v(-2.0, -1.0), 20.0);
            assert!(min.x <= max.x && min.y <= max.y, "{s:?}");
        }
    }

    #[test]
    fn the_box_encloses_every_emitted_vertex() {
        let s = Shape::polygon(&l_shape(), C).with_origin(v(1.0, 2.0));
        let (min, max) = shape_cull_bounds(&s, v(5.0, 5.0), v(1.5, -2.0), 33.0);
        for (a, b) in lines(&ops(
            &s,
            ShapeLayer::Outline,
            v(5.0, 5.0),
            v(1.5, -2.0),
            33.0,
        )) {
            for p in [a, b] {
                assert!(p.cmpge(min).all() && p.cmple(max).all());
            }
        }
    }

    #[test]
    fn a_bezier_starts_and_ends_exactly_at_the_transformed_origin_and_to() {
        let to = v(10.0, 4.0);
        let s = bezier(v(2.0, 5.0), v(8.0, -3.0), to);
        let (pos, scale, rot) = (v(7.0, 9.0), v(2.0, 3.0), 25.0);
        let l = lines(&ops(&s, ShapeLayer::Outline, pos, scale, rot));
        assert_eq!(l.len(), 24);
        let xf = Transform::new(&s, pos, scale, rot);
        assert_eq!(l[0].0, xf.point(Vec2::ZERO));
        assert_eq!(l[23].1, xf.point(to));
    }

    #[test]
    fn a_hairline_bezier_emits_24_connected_segments() {
        let s = bezier(v(1.0, 4.0), v(6.0, -2.0), v(8.0, 3.0));
        let l = lines(&ops(
            &s,
            ShapeLayer::Outline,
            v(3.0, 4.0),
            v(2.0, 1.0),
            40.0,
        ));
        assert_eq!(l.len(), 24);
        for pair in l.windows(2) {
            assert_eq!(pair[0].1, pair[1].0, "segment ends join the next start");
        }
    }

    #[test]
    fn the_bezier_samples_match_the_cubic_polynomial() {
        let (c1, c2, to) = (v(0.0, 3.0), v(3.0, 3.0), v(3.0, 0.0));
        let s = bezier(c1, c2, to);
        let l = lines(&outline(&s));
        // The end of segment 11 samples t = 12 / 24 = 0.5:
        // P(0.5) = 0.125 P0 + 0.375 P1 + 0.375 P2 + 0.125 P3 = (1.5, 2.25).
        let expected = Vec2::ZERO * 0.125 + c1 * 0.375 + c2 * 0.375 + to * 0.125;
        assert!(vec2_approx_eq(l[11].1, expected));
        assert!(vec2_approx_eq(l[11].1, v(1.5, 2.25)));
    }

    #[test]
    fn a_straight_cubic_bezier_yields_collinear_points() {
        // c1 and c2 on the chord from (0, 0) to `to` make the curve the chord itself.
        let to = v(24.0, 0.0);
        let s = bezier(v(8.0, 0.0), v(16.0, 0.0), to);
        let l = lines(&outline(&s));
        assert_eq!(l.len(), 24);
        for &(a, b) in &l {
            assert_eq!(a.y, 0.0);
            assert_eq!(b.y, 0.0);
        }
        // Under rotation the points stay on the transformed chord. Compare the
        // distance to the chord line (unit direction), not the raw cross product,
        // whose rotation error scales with the chord length.
        let (pos, scale, rot) = (v(2.0, 3.0), Vec2::ONE, 90.0);
        let l = lines(&ops(&s, ShapeLayer::Outline, pos, scale, rot));
        let xf = Transform::new(&s, pos, scale, rot);
        let chord_a = xf.point(Vec2::ZERO);
        let chord = xf.point(to) - chord_a;
        let dir = chord / chord.length();
        for &(a, b) in &l {
            for p in [a, b] {
                assert!(approx_eq(dir.perp_dot(p - chord_a), 0.0), "{p:?}");
            }
        }
    }

    #[test]
    fn the_bezier_box_encloses_every_emitted_vertex_under_rotation_scale_and_mirroring() {
        let to = v(6.0, 5.0);
        let s = bezier(v(2.0, 8.0), v(9.0, -1.0), to).with_origin(v(1.0, 2.0));
        let pos = v(5.0, 5.0);
        for (scale, rot) in [
            (Vec2::ONE, 90.0),
            (v(2.0, 3.0), 33.0),
            (v(-2.0, -1.0), 20.0),
            (v(1.0, -3.0), 0.0),
        ] {
            let (min, max) = shape_cull_bounds(&s, pos, scale, rot);
            assert!(min.x <= max.x && min.y <= max.y, "{scale:?} {rot}");
            let l = lines(&ops(&s, ShapeLayer::Outline, pos, scale, rot));
            assert_eq!(l.len(), 24, "{scale:?} {rot}");
            for &(a, b) in &l {
                for p in [a, b] {
                    assert!(
                        p.cmpge(min).all() && p.cmple(max).all(),
                        "{p:?} outside {min:?}..{max:?} at {scale:?} {rot}"
                    );
                }
            }
            let xf = Transform::new(&s, pos, scale, rot);
            assert_eq!(l[0].0, xf.point(Vec2::ZERO));
            assert_eq!(l[23].1, xf.point(to));
        }
    }

    #[test]
    fn a_filled_bezier_with_an_outline_color_draws_one_outline_layer_and_no_fill() {
        let plain = bezier(v(1.0, 2.0), v(3.0, 4.0), v(5.0, 6.0));
        let s = plain.clone().filled().with_outline_color(Color::BLUE);
        assert_eq!(shape_layers(&s), [Some((ShapeLayer::Outline, C)), None]);
        assert!(fill(&s).is_empty());
        assert_eq!(shape_layers(&plain), [Some((ShapeLayer::Outline, C)), None]);
        assert!(fill(&plain).is_empty());
    }

    #[test]
    fn a_hairline_bezier_emits_only_line_ops() {
        let s = bezier(v(2.0, 5.0), v(8.0, -3.0), v(10.0, 4.0));
        let o = ops(&s, ShapeLayer::Outline, v(10.0, 10.0), v(2.0, 4.0), 30.0);
        assert_eq!(lines(&o).len(), 24);
        assert!(thick_lines(&o).is_empty());
        assert!(triangles(&o).is_empty());
    }

    #[test]
    fn a_thick_bezier_emits_only_thick_lines_of_the_mean_scaled_width() {
        let s = bezier(v(2.0, 5.0), v(8.0, -3.0), v(10.0, 4.0)).with_thickness(2.0);
        // width = 2 * mean(|2|, |4|) = 6.
        let o = ops(&s, ShapeLayer::Outline, v(10.0, 10.0), v(2.0, 4.0), 0.0);
        let t = thick_lines(&o);
        assert_eq!(t.len(), 24);
        assert!(t.iter().all(|&(_, _, w)| approx_eq(w, 6.0)));
        assert!(lines(&o).is_empty());
    }

    #[test]
    fn every_thick_bezier_segment_is_extended_by_half_the_width_at_both_ends() {
        let to = v(9.0, 0.0);
        // A straight cubic on the x-axis: every polyline segment runs along +x.
        let s = bezier(v(3.0, 0.0), v(6.0, 0.0), to).with_thickness(2.0);
        let t = thick_lines(&outline(&s));
        assert_eq!(t.len(), 24);
        let mut prev = v(0.0, 0.0);
        for &(a, b, w) in &t {
            assert!(approx_eq(w, 2.0));
            let d = (b - a) / (b - a).length();
            assert!(vec2_approx_eq(d, v(1.0, 0.0)), "direction {d:?}");
            // Square caps: the geometric endpoints sit w / 2 inside `a` and `b`,
            // so the unextended polyline stays connected from (0, 0) to `to`.
            let ga = a + d * (w / 2.0);
            let gb = b - d * (w / 2.0);
            assert!(vec2_approx_eq(ga, prev), "cap rule: {ga:?} vs {prev:?}");
            prev = gb;
        }
        assert!(vec2_approx_eq(prev, to));
    }

    #[test]
    fn a_degenerate_thick_bezier_emits_nothing() {
        let s = bezier(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO).with_thickness(2.0);
        assert!(outline(&s).is_empty());
    }

    #[test]
    fn reusing_the_bezier_buffer_does_not_grow_its_capacity() {
        let s = bezier(v(1.0, 5.0), v(7.0, -2.0), v(9.0, 3.0));
        let mut out = Vec::new();
        shape_ops(
            &s,
            ShapeLayer::Outline,
            Vec2::ZERO,
            Vec2::ONE,
            0.0,
            &mut out,
        );
        assert_eq!(out.len(), 24);
        let cap = out.capacity();
        for _ in 0..3 {
            out.clear();
            shape_ops(
                &s,
                ShapeLayer::Outline,
                Vec2::ZERO,
                Vec2::ONE,
                0.0,
                &mut out,
            );
            assert_eq!(out.len(), 24);
        }
        assert_eq!(out.capacity(), cap);
    }
}
