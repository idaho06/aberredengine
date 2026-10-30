//! Drawable primitives: point, line, rectangle, circle, ellipse and polygon.
//!
//! A [`Shape`] carries its geometry in local coordinates plus its colours and a pivot `origin`.

use std::sync::Arc;

use bevy_ecs::prelude::Component;

use crate::math::{Color, Vec2};

/// A drawable primitive: a point, line, rectangle, circle, ellipse or polygon.
///
/// A `Shape` draws when its entity also carries a `ZIndex` and a position: a
/// `WorldPosition` places it on the map (the camera applies), a `ScreenPosition`
/// places it on the screen. `Tint` multiplies its colours and `Shadow` draws a
/// silhouette behind it.
///
/// Lines and outlines are 1 px wide. `color` is the fill colour of a `filled`
/// shape and the line colour of an outlined shape, a point or a line. A filled
/// shape with an `outline_color` draws the fill first and a 1 px outline on top.
/// `filled` and `outline_color` do not affect `Point` and `Line`, and
/// `outline_color` does not affect an unfilled shape.
///
/// All geometry is in local coordinates. `origin` is the local point that sits
/// on the entity position and is the centre of `Scale` and `Rotation`, like
/// `Sprite::origin`.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Shape {
    /// The geometry.
    pub kind: ShapeKind,
    /// The fill colour when `filled`, else the outline colour.
    pub color: Color,
    /// Fill the interior instead of drawing only the outline.
    pub filled: bool,
    /// Colour of an extra 1 px outline drawn over a filled shape.
    pub outline_color: Option<Color>,
    /// Local point that sits on the entity position and pivots `Scale` and `Rotation`.
    pub origin: Vec2,
}

/// The geometry of a [`Shape`], in local coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum ShapeKind {
    /// One pixel at the local origin `(0, 0)`.
    Point,
    /// A segment from `(0, 0)` to `to`.
    Line {
        /// End of the segment.
        to: Vec2,
    },
    /// An axis-aligned rectangle with its top-left corner at `(0, 0)`.
    Rect {
        /// Width and height.
        size: Vec2,
    },
    /// A circle centred at `(0, 0)`.
    ///
    /// Rotation moves only the centre (about `origin`); the circle itself is
    /// rotation-invariant. Under a non-uniform `Scale` it is drawn as an ellipse.
    Circle {
        /// Radius before scaling.
        radius: f32,
    },
    /// An axis-aligned ellipse centred at `(0, 0)`. It ignores `Rotation`.
    Ellipse {
        /// Horizontal and vertical radii before scaling.
        radius: Vec2,
    },
    /// A polygon given by its vertices, convex or concave, without holes.
    Polygon(Polygon),
}

/// A polygon's vertices in local coordinates, shared cheaply between clones.
///
/// The fill triangulation is computed once in [`Polygon::new`]. Two polygons
/// are equal when they share their vertex storage or hold the same vertices.
#[derive(Clone, Debug)]
pub struct Polygon {
    vertices: Arc<[Vec2]>,
    triangles: Arc<[[u32; 3]]>,
}

impl Polygon {
    /// Create a polygon from its vertices and triangulate it by ear clipping.
    ///
    /// Either winding works. Fewer than three vertices give no triangles, and
    /// degenerate or self-intersecting input still terminates, falling back to
    /// a fan over the vertices left when no ear exists.
    pub fn new(vertices: &[Vec2]) -> Self {
        Self {
            vertices: vertices.into(),
            triangles: triangulate(vertices).into(),
        }
    }

    /// The vertices in local coordinates.
    pub fn vertices(&self) -> &[Vec2] {
        &self.vertices
    }

    /// Triangles filling the polygon, as indices into [`vertices`](Self::vertices).
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
}

impl PartialEq for Polygon {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.vertices, &other.vertices) || self.vertices == other.vertices
    }
}

fn cross(o: Vec2, a: Vec2, b: Vec2) -> f32 {
    (a - o).perp_dot(b - o)
}

/// Whether `p` lies inside or on the boundary of triangle `abc`.
fn in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

/// Appends triangle `t` (indices into `pts`) unless it has zero area.
fn push_triangle(tris: &mut Vec<[u32; 3]>, pts: &[Vec2], t: [usize; 3]) {
    if cross(pts[t[0]], pts[t[1]], pts[t[2]]) != 0.0 {
        tris.push(t.map(|i| i as u32));
    }
}

/// Ear-clips `pts` into index triples that refer to the original array.
fn triangulate(pts: &[Vec2]) -> Vec<[u32; 3]> {
    if pts.len() < 3 {
        return Vec::new();
    }
    let mut tris = Vec::with_capacity(pts.len() - 2);
    let twice_area: f32 = (0..pts.len())
        .map(|i| pts[i].perp_dot(pts[(i + 1) % pts.len()]))
        .sum();
    let orient = twice_area.signum();
    let mut rest: Vec<usize> = (0..pts.len()).collect();
    while rest.len() > 3 {
        let n = rest.len();
        let ear = (0..n).find(|&i| {
            let (p, c, q) = (rest[(i + n - 1) % n], rest[i], rest[(i + 1) % n]);
            let tri = [pts[p], pts[c], pts[q]];
            let turn = cross(tri[0], tri[1], tri[2]);
            // A flat vertex is dropped without emitting a triangle.
            turn == 0.0
                || (turn * orient > 0.0
                    && rest.iter().all(|&o| {
                        tri.contains(&pts[o]) || !in_triangle(pts[o], tri[0], tri[1], tri[2])
                    }))
        });
        let Some(i) = ear else {
            break;
        };
        push_triangle(
            &mut tris,
            pts,
            [rest[(i + n - 1) % n], rest[i], rest[(i + 1) % n]],
        );
        rest.remove(i);
    }
    // Three vertices remain, or no ear exists: fan over what is left.
    for k in 1..rest.len() - 1 {
        push_triangle(&mut tris, pts, [rest[0], rest[k], rest[k + 1]]);
    }
    tris
}

impl Shape {
    fn new(kind: ShapeKind, color: Color) -> Self {
        Self {
            kind,
            color,
            filled: false,
            outline_color: None,
            origin: Vec2::ZERO,
        }
    }

    /// A single pixel.
    pub fn point(color: Color) -> Self {
        Self::new(ShapeKind::Point, color)
    }

    /// A line from the local origin to `to`.
    pub fn line(to: Vec2, color: Color) -> Self {
        Self::new(ShapeKind::Line { to }, color)
    }

    /// A rectangle of the given size with its top-left corner at the local origin.
    pub fn rect(size: Vec2, color: Color) -> Self {
        Self::new(ShapeKind::Rect { size }, color)
    }

    /// A circle of the given radius centred on the local origin.
    pub fn circle(radius: f32, color: Color) -> Self {
        Self::new(ShapeKind::Circle { radius }, color)
    }

    /// An axis-aligned ellipse with radii `(rx, ry)` centred on the local origin.
    pub fn ellipse(radius: Vec2, color: Color) -> Self {
        Self::new(ShapeKind::Ellipse { radius }, color)
    }

    /// A polygon through `points`, in local coordinates.
    pub fn polygon(points: &[Vec2], color: Color) -> Self {
        Self::new(ShapeKind::Polygon(Polygon::new(points)), color)
    }

    /// Fill the interior with `color`.
    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }

    /// Draw a 1 px outline in `color` over a filled shape.
    pub fn with_outline_color(mut self, color: Color) -> Self {
        self.outline_color = Some(color);
        self
    }

    /// Set the local pivot point.
    pub fn with_origin(mut self, origin: Vec2) -> Self {
        self.origin = origin;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::RED;
    const BLUE: Color = Color::BLUE;

    #[test]
    fn constructors_pick_their_kind() {
        let to = Vec2::new(3.0, 4.0);
        let size = Vec2::new(5.0, 6.0);
        assert_eq!(Shape::point(RED).kind, ShapeKind::Point);
        assert_eq!(Shape::line(to, RED).kind, ShapeKind::Line { to });
        assert_eq!(Shape::rect(size, RED).kind, ShapeKind::Rect { size });
        assert_eq!(
            Shape::circle(2.5, RED).kind,
            ShapeKind::Circle { radius: 2.5 }
        );
        assert_eq!(
            Shape::ellipse(size, RED).kind,
            ShapeKind::Ellipse { radius: size }
        );
    }

    #[test]
    fn polygon_constructor_keeps_the_vertices() {
        let pts = [
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(0.0, 4.0),
        ];
        match Shape::polygon(&pts, RED).kind {
            ShapeKind::Polygon(poly) => assert_eq!(poly.vertices(), &pts),
            other => panic!("expected a polygon, got {other:?}"),
        }
    }

    #[test]
    fn new_shapes_store_the_colour_and_start_outlined_with_a_zero_origin() {
        let s = Shape::circle(1.0, RED);
        assert_eq!(s.color, RED);
        assert!(!s.filled);
        assert_eq!(s.outline_color, None);
        assert_eq!(s.origin, Vec2::ZERO);
    }

    #[test]
    fn builders_set_their_field_and_chain_without_losing_earlier_ones() {
        let s = Shape::rect(Vec2::new(2.0, 2.0), RED)
            .filled()
            .with_outline_color(BLUE)
            .with_origin(Vec2::new(1.0, 1.0));
        assert!(s.filled);
        assert_eq!(s.outline_color, Some(BLUE));
        assert_eq!(s.origin, Vec2::new(1.0, 1.0));
        assert_eq!(s.color, RED);
        assert_eq!(
            s.kind,
            ShapeKind::Rect {
                size: Vec2::new(2.0, 2.0)
            }
        );
    }

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    fn shoelace_area(pts: &[Vec2]) -> f32 {
        let n = pts.len();
        let twice: f32 = (0..n)
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                a.x * b.y - b.x * a.y
            })
            .sum();
        twice.abs() / 2.0
    }

    fn triangle_area_sum(poly: &Polygon) -> f32 {
        let pts = poly.vertices();
        poly.triangles()
            .iter()
            .map(|t| shoelace_area(&[pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]]))
            .sum()
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
    fn a_triangle_is_one_triangle() {
        let poly = Polygon::new(&[v(0.0, 0.0), v(4.0, 0.0), v(0.0, 4.0)]);
        assert_eq!(poly.triangles().len(), 1);
        assert_eq!(triangle_area_sum(&poly), 8.0);
    }

    #[test]
    fn a_convex_quad_is_two_triangles() {
        let poly = Polygon::new(&[v(0.0, 0.0), v(4.0, 0.0), v(4.0, 4.0), v(0.0, 4.0)]);
        assert_eq!(poly.triangles().len(), 2);
        assert_eq!(triangle_area_sum(&poly), 16.0);
    }

    #[test]
    fn a_concave_l_shape_triangulates_to_its_exact_area() {
        let pts = l_shape();
        let poly = Polygon::new(&pts);
        assert_eq!(poly.triangles().len(), 4);
        assert_eq!(triangle_area_sum(&poly), shoelace_area(&pts));
    }

    #[test]
    fn winding_does_not_change_the_triangulated_area() {
        let ccw = l_shape();
        let cw: Vec<Vec2> = ccw.iter().rev().copied().collect();
        let (a, b) = (Polygon::new(&ccw), Polygon::new(&cw));
        assert_eq!(b.triangles().len(), 4);
        assert_eq!(triangle_area_sum(&a), triangle_area_sum(&b));
    }

    #[test]
    fn a_vertex_collinear_with_its_neighbours_keeps_area_and_emits_no_flat_triangle() {
        let pts = [
            v(0.0, 0.0),
            v(2.0, 0.0),
            v(4.0, 0.0),
            v(4.0, 4.0),
            v(0.0, 4.0),
        ];
        let poly = Polygon::new(&pts);
        assert!(poly.triangles().len() <= pts.len() - 2);
        assert_eq!(triangle_area_sum(&poly), 16.0);
        for t in poly.triangles() {
            let tri = t.map(|i| pts[i as usize]);
            assert_ne!(shoelace_area(&tri), 0.0);
        }
    }

    #[test]
    fn degenerate_input_terminates_without_panicking() {
        let bow_tie = Polygon::new(&[v(0.0, 0.0), v(4.0, 4.0), v(4.0, 0.0), v(0.0, 4.0)]);
        let line = Polygon::new(&[v(0.0, 0.0), v(1.0, 0.0), v(2.0, 0.0), v(3.0, 0.0)]);
        assert!(bow_tie.triangles().iter().flatten().all(|&i| i < 4));
        assert!(line.triangles().is_empty());
    }

    #[test]
    fn fewer_than_three_vertices_have_no_triangles() {
        assert!(Polygon::new(&[]).triangles().is_empty());
        assert!(Polygon::new(&[v(0.0, 0.0)]).triangles().is_empty());
        assert!(
            Polygon::new(&[v(0.0, 0.0), v(1.0, 1.0)])
                .triangles()
                .is_empty()
        );
    }

    #[test]
    fn triangle_indices_address_the_original_vertices() {
        let poly = Polygon::new(&l_shape());
        let n = poly.vertices().len() as u32;
        assert!(poly.triangles().iter().flatten().all(|&i| i < n));
    }

    #[test]
    fn polygons_compare_by_vertex_content() {
        let pts = l_shape();
        let a = Polygon::new(&pts);
        assert_eq!(a, a.clone());
        assert_eq!(a, Polygon::new(&pts));
        assert_ne!(a, Polygon::new(&pts[..4]));
    }
}
