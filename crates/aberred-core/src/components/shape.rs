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
#[derive(Clone, Debug, PartialEq)]
pub struct Polygon {
    vertices: Arc<[Vec2]>,
    triangles: Arc<[[u32; 3]]>,
}

impl Polygon {
    /// Create a polygon from its vertices.
    pub fn new(vertices: &[Vec2]) -> Self {
        Self {
            vertices: vertices.into(),
            triangles: Arc::from([]),
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
}
