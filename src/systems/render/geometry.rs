use raylib::prelude::*;

use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::sprite::Sprite;

/// Computed geometry for a sprite draw call via Raylib's `draw_texture_pro`.
///
/// In Raylib, `draw_texture_pro(tex, src, dest, origin, rotation, tint)` places
/// the texture so that local coordinate `(origin.x, origin.y)` maps to world
/// position `(dest.x, dest.y)`. Without rotation the visual top-left is at
/// `(dest.x - origin.x, dest.y - origin.y)`.
#[cfg_attr(test, derive(Debug))]
pub(super) struct SpriteRenderGeometry {
    pub(super) dest: Rectangle,
    pub(super) origin: Vector2,
    pub(super) rotation: f32,
}

#[cfg(test)]
impl SpriteRenderGeometry {
    /// World-space position of the anchor/pivot (always `(dest.x, dest.y)`).
    fn anchor_world_pos(&self) -> Vector2 {
        Vector2 {
            x: self.dest.x,
            y: self.dest.y,
        }
    }

    /// Visual top-left corner (ignoring rotation).
    fn visual_top_left(&self) -> Vector2 {
        Vector2 {
            x: self.dest.x - self.origin.x,
            y: self.dest.y - self.origin.y,
        }
    }

    /// Visual bottom-right corner (ignoring rotation).
    fn visual_bottom_right(&self) -> Vector2 {
        Vector2 {
            x: self.dest.x - self.origin.x + self.dest.width,
            y: self.dest.y - self.origin.y + self.dest.height,
        }
    }
}

/// Pure geometry calculation for sprite rendering.
///
/// Computes the destination rectangle, scaled origin, and rotation that Raylib's
/// `draw_texture_pro` needs. Extracted from the render loop so it can be tested
/// without a GPU context.
pub(super) fn compute_sprite_geometry(
    pos: &MapPosition,
    sprite: &Sprite,
    scale: Option<&Scale>,
    rot: Option<&Rotation>,
) -> SpriteRenderGeometry {
    let mut dest = Rectangle {
        x: pos.pos.x,
        y: pos.pos.y,
        width: sprite.width,
        height: sprite.height,
    };

    if let Some(scale) = scale {
        dest.width *= scale.scale.x;
        dest.height *= scale.scale.y;
    }

    let mut origin = Vector2 {
        x: sprite.origin.x,
        y: sprite.origin.y,
    };
    if let Some(scale) = scale {
        origin.x *= scale.scale.x;
        origin.y *= scale.scale.y;
    }

    let rotation = rot.map_or(0.0, |r| r.degrees);

    SpriteRenderGeometry {
        dest,
        origin,
        rotation,
    }
}

/// Compute the world-space AABB that fully contains the camera's visible area.
///
/// Converts all 4 screen corners to world space, then takes the min/max to form
/// a conservative bounding box. With a rotated camera, the 2-corner approach
/// (top-left + bottom-right) misses the other two corners which may extend
/// further, causing sprites near edges to be culled while still visible.
pub(super) fn compute_view_bounds(
    screen_w: f32,
    screen_h: f32,
    camera: Camera2D,
    screen_to_world: impl Fn(Vector2, Camera2D) -> Vector2,
) -> (Vector2, Vector2) {
    let corners = [
        screen_to_world(Vector2 { x: 0.0, y: 0.0 }, camera),
        screen_to_world(
            Vector2 {
                x: screen_w,
                y: 0.0,
            },
            camera,
        ),
        screen_to_world(
            Vector2 {
                x: 0.0,
                y: screen_h,
            },
            camera,
        ),
        screen_to_world(
            Vector2 {
                x: screen_w,
                y: screen_h,
            },
            camera,
        ),
    ];
    let view_min = Vector2 {
        x: corners[0]
            .x
            .min(corners[1].x)
            .min(corners[2].x)
            .min(corners[3].x),
        y: corners[0]
            .y
            .min(corners[1].y)
            .min(corners[2].y)
            .min(corners[3].y),
    };
    let view_max = Vector2 {
        x: corners[0]
            .x
            .max(corners[1].x)
            .max(corners[2].x)
            .max(corners[3].x),
        y: corners[0]
            .y
            .max(corners[1].y)
            .max(corners[2].y)
            .max(corners[3].y),
    };
    (view_min, view_max)
}

/// Compute the world-space AABB of a sprite for culling, accounting for scale and rotation.
///
/// For rotated sprites, uses a bounding circle (conservative but fast): the radius is the
/// distance from the anchor to the farthest corner of the scaled sprite, and the AABB is
/// expanded to contain that circle. For non-rotated sprites, returns the tight scaled AABB.
pub(super) fn compute_sprite_cull_bounds(
    pos: &MapPosition,
    sprite: &Sprite,
    scale: Option<&Scale>,
    rot: Option<&Rotation>,
) -> (Vector2, Vector2) {
    let (sx, sy) = scale.map_or((1.0, 1.0), |s| (s.scale.x, s.scale.y));

    let scaled_w = sprite.width * sx;
    let scaled_h = sprite.height * sy;
    let scaled_ox = sprite.origin.x * sx;
    let scaled_oy = sprite.origin.y * sy;

    let is_rotated = rot.is_some_and(|r| r.degrees.abs() > f32::EPSILON);

    if is_rotated {
        // Bounding circle: radius = max distance from anchor to any corner of the scaled rect
        let corners = [
            (scaled_ox, scaled_oy),
            (scaled_w - scaled_ox, scaled_oy),
            (scaled_ox, scaled_h - scaled_oy),
            (scaled_w - scaled_ox, scaled_h - scaled_oy),
        ];
        let radius = corners
            .iter()
            .map(|(dx, dy)| (dx * dx + dy * dy).sqrt())
            .fold(0.0_f32, f32::max);

        let min = Vector2 {
            x: pos.pos.x - radius,
            y: pos.pos.y - radius,
        };
        let max = Vector2 {
            x: pos.pos.x + radius,
            y: pos.pos.y + radius,
        };
        (min, max)
    } else {
        let min = Vector2 {
            x: pos.pos.x - scaled_ox,
            y: pos.pos.y - scaled_oy,
        };
        let max = Vector2 {
            x: min.x + scaled_w,
            y: min.y + scaled_h,
        };
        (min, max)
    }
}

/// Resolve the effective world-space transform for an entity, preferring
/// `GlobalTransform2D` (hierarchy) over the entity's own local components.
#[inline]
pub(super) fn resolve_world_transform(
    pos: MapPosition,
    maybe_scale: Option<Scale>,
    maybe_rot: Option<Rotation>,
    maybe_gt: Option<GlobalTransform2D>,
) -> (MapPosition, Option<Scale>, Option<Rotation>) {
    if let Some(gt) = maybe_gt {
        (
            MapPosition { pos: gt.position },
            Some(Scale { scale: gt.scale }),
            Some(Rotation {
                degrees: gt.rotation_degrees,
            }),
        )
    } else {
        (pos, maybe_scale, maybe_rot)
    }
}

/// Draw a rotated rectangle outline in world space.
///
/// Rotates the 4 corners of `dest` around the anchor point `(dest.x, dest.y)`
/// by `rotation` degrees (clockwise, matching Raylib's convention) and draws
/// 4 line segments connecting them.
pub(super) fn draw_rotated_rect_lines(
    d: &mut impl RaylibDraw,
    dest: Rectangle,
    origin: Vector2,
    rotation: f32,
    color: Color,
) {
    let angle = rotation.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    // 4 un-rotated corner offsets relative to the anchor point
    let corners_local: [(f32, f32); 4] = [
        (-origin.x, -origin.y),
        (dest.width - origin.x, -origin.y),
        (dest.width - origin.x, dest.height - origin.y),
        (-origin.x, dest.height - origin.y),
    ];

    let rotate = |(cx, cy): (f32, f32)| -> Vector2 {
        Vector2 {
            x: dest.x + cx * cos_a - cy * sin_a,
            y: dest.y + cx * sin_a + cy * cos_a,
        }
    };

    let pts: [Vector2; 4] = [
        rotate(corners_local[0]),
        rotate(corners_local[1]),
        rotate(corners_local[2]),
        rotate(corners_local[3]),
    ];

    d.draw_line_v(pts[0], pts[1], color);
    d.draw_line_v(pts[1], pts[2], color);
    d.draw_line_v(pts[2], pts[3], color);
    d.draw_line_v(pts[3], pts[0], color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn make_sprite(w: f32, h: f32, origin_x: f32, origin_y: f32) -> Sprite {
        Sprite {
            tex_key: Arc::from("test"),
            width: w,
            height: h,
            offset: Vector2 { x: 0.0, y: 0.0 },
            origin: Vector2 {
                x: origin_x,
                y: origin_y,
            },
            flip_h: false,
            flip_v: false,
        }
    }

    // --- Anchor preservation tests ---

    #[test]
    fn anchor_preserved_with_center_origin() {
        let pos = MapPosition::new(100.0, 100.0);
        let sprite = make_sprite(32.0, 32.0, 16.0, 16.0);

        for scale_factor in [0.5_f32, 1.0, 2.0, 3.0, 10.0] {
            let scale = Scale::new(scale_factor, scale_factor);
            let geom = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);
            let anchor = geom.anchor_world_pos();
            assert!(
                approx_eq(anchor.x, 100.0) && approx_eq(anchor.y, 100.0),
                "Center origin: anchor drifted to ({}, {}) at scale {}",
                anchor.x,
                anchor.y,
                scale_factor
            );
        }
    }

    #[test]
    fn anchor_preserved_with_topleft_origin() {
        let pos = MapPosition::new(50.0, 75.0);
        let sprite = make_sprite(64.0, 48.0, 0.0, 0.0);

        for scale_factor in [0.25_f32, 1.0, 4.0] {
            let scale = Scale::new(scale_factor, scale_factor);
            let geom = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);
            let anchor = geom.anchor_world_pos();
            assert!(
                approx_eq(anchor.x, 50.0) && approx_eq(anchor.y, 75.0),
                "Top-left origin: anchor drifted to ({}, {}) at scale {}",
                anchor.x,
                anchor.y,
                scale_factor
            );
        }
    }

    #[test]
    fn anchor_preserved_with_arbitrary_origin() {
        let pos = MapPosition::new(200.0, 150.0);
        let sprite = make_sprite(32.0, 48.0, 10.0, 20.0);

        for (sx, sy) in [(1.0, 1.0), (2.0, 2.0), (0.5, 0.5), (3.0, 1.5)] {
            let scale = Scale::new(sx, sy);
            let geom = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);
            let anchor = geom.anchor_world_pos();
            assert!(
                approx_eq(anchor.x, 200.0) && approx_eq(anchor.y, 150.0),
                "Arbitrary origin: anchor drifted to ({}, {}) at scale ({}, {})",
                anchor.x,
                anchor.y,
                sx,
                sy
            );
        }
    }

    // --- Proportional scaling test ---

    #[test]
    fn visual_bounds_scale_proportionally() {
        let pos = MapPosition::new(100.0, 100.0);
        let sprite = make_sprite(32.0, 32.0, 10.0, 10.0);

        let geom_1x = compute_sprite_geometry(&pos, &sprite, None, None);
        let tl_1x = geom_1x.visual_top_left();
        let br_1x = geom_1x.visual_bottom_right();

        // At 2x scale, distances from anchor to each edge should double
        let scale = Scale::new(2.0, 2.0);
        let geom_2x = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);
        let tl_2x = geom_2x.visual_top_left();
        let br_2x = geom_2x.visual_bottom_right();

        // Distance from anchor (100,100) to left edge
        let dist_left_1x = 100.0 - tl_1x.x;
        let dist_left_2x = 100.0 - tl_2x.x;
        assert!(
            approx_eq(dist_left_2x, dist_left_1x * 2.0),
            "Left edge distance: 1x={}, 2x={} (expected {})",
            dist_left_1x,
            dist_left_2x,
            dist_left_1x * 2.0
        );

        // Distance from anchor to right edge
        let dist_right_1x = br_1x.x - 100.0;
        let dist_right_2x = br_2x.x - 100.0;
        assert!(
            approx_eq(dist_right_2x, dist_right_1x * 2.0),
            "Right edge distance: 1x={}, 2x={} (expected {})",
            dist_right_1x,
            dist_right_2x,
            dist_right_1x * 2.0
        );

        // Distance from anchor to top edge
        let dist_top_1x = 100.0 - tl_1x.y;
        let dist_top_2x = 100.0 - tl_2x.y;
        assert!(
            approx_eq(dist_top_2x, dist_top_1x * 2.0),
            "Top edge distance: 1x={}, 2x={} (expected {})",
            dist_top_1x,
            dist_top_2x,
            dist_top_1x * 2.0
        );

        // Distance from anchor to bottom edge
        let dist_bottom_1x = br_1x.y - 100.0;
        let dist_bottom_2x = br_2x.y - 100.0;
        assert!(
            approx_eq(dist_bottom_2x, dist_bottom_1x * 2.0),
            "Bottom edge distance: 1x={}, 2x={} (expected {})",
            dist_bottom_1x,
            dist_bottom_2x,
            dist_bottom_1x * 2.0
        );
    }

    // --- Non-uniform scale ---

    #[test]
    fn non_uniform_scale_preserves_anchor() {
        let pos = MapPosition::new(100.0, 100.0);
        let sprite = make_sprite(32.0, 32.0, 16.0, 16.0);
        let scale = Scale::new(2.0, 0.5);

        let geom = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);

        // Anchor must stay at entity position
        let anchor = geom.anchor_world_pos();
        assert!(approx_eq(anchor.x, 100.0) && approx_eq(anchor.y, 100.0));

        // Width doubled, height halved
        assert!(approx_eq(geom.dest.width, 64.0));
        assert!(approx_eq(geom.dest.height, 16.0));

        // Origin scaled per-axis
        assert!(approx_eq(geom.origin.x, 32.0));
        assert!(approx_eq(geom.origin.y, 8.0));
    }

    // --- Identity / no-scale equivalence ---

    #[test]
    fn unit_scale_matches_no_scale() {
        let pos = MapPosition::new(42.0, 77.0);
        let sprite = make_sprite(24.0, 36.0, 8.0, 12.0);
        let unit = Scale::new(1.0, 1.0);

        let geom_none = compute_sprite_geometry(&pos, &sprite, None, None);
        let geom_unit = compute_sprite_geometry(&pos, &sprite, Some(&unit), None);

        assert!(approx_eq(geom_none.dest.x, geom_unit.dest.x));
        assert!(approx_eq(geom_none.dest.y, geom_unit.dest.y));
        assert!(approx_eq(geom_none.dest.width, geom_unit.dest.width));
        assert!(approx_eq(geom_none.dest.height, geom_unit.dest.height));
        assert!(approx_eq(geom_none.origin.x, geom_unit.origin.x));
        assert!(approx_eq(geom_none.origin.y, geom_unit.origin.y));
        assert!(approx_eq(geom_none.rotation, geom_unit.rotation));
    }

    // --- Rotation passthrough ---

    #[test]
    fn default_rotation_is_zero() {
        let pos = MapPosition::new(0.0, 0.0);
        let sprite = make_sprite(32.0, 32.0, 0.0, 0.0);
        let geom = compute_sprite_geometry(&pos, &sprite, None, None);
        assert!(approx_eq(geom.rotation, 0.0));
    }

    #[test]
    fn rotation_passes_through() {
        let pos = MapPosition::new(0.0, 0.0);
        let sprite = make_sprite(32.0, 32.0, 16.0, 16.0);
        let rot = Rotation { degrees: 45.0 };
        let geom = compute_sprite_geometry(&pos, &sprite, None, Some(&rot));
        assert!(approx_eq(geom.rotation, 45.0));
    }

    // --- View bounds tests ---

    /// Mock screen_to_world: applies camera transform (translate + rotate + zoom) mathematically.
    fn mock_screen_to_world(screen_pos: Vector2, cam: Camera2D) -> Vector2 {
        // Reverse of Raylib's Camera2D: screen -> world
        // 1. Translate screen pos relative to camera offset
        let dx = screen_pos.x - cam.offset.x;
        let dy = screen_pos.y - cam.offset.y;
        // 2. Undo zoom
        let dx = dx / cam.zoom;
        let dy = dy / cam.zoom;
        // 3. Undo rotation (rotate by -rotation)
        let angle = -cam.rotation.to_radians();
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rx = dx * cos_a - dy * sin_a;
        let ry = dx * sin_a + dy * cos_a;
        // 4. Translate to world
        Vector2 {
            x: rx + cam.target.x,
            y: ry + cam.target.y,
        }
    }

    fn make_camera(
        target_x: f32,
        target_y: f32,
        offset_x: f32,
        offset_y: f32,
        rotation: f32,
        zoom: f32,
    ) -> Camera2D {
        Camera2D {
            target: Vector2 {
                x: target_x,
                y: target_y,
            },
            offset: Vector2 {
                x: offset_x,
                y: offset_y,
            },
            rotation,
            zoom,
        }
    }

    #[test]
    fn view_bounds_no_rotation() {
        // Camera centered at origin, offset at screen center, no rotation, zoom 1x
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 0.0, 1.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // With no rotation, the 4-corner approach should match the 2-corner result exactly
        assert!(approx_eq(view_min.x, -400.0));
        assert!(approx_eq(view_min.y, -300.0));
        assert!(approx_eq(view_max.x, 400.0));
        assert!(approx_eq(view_max.y, 300.0));
    }

    #[test]
    fn view_bounds_45_degree_rotation() {
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 45.0, 1.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // At 45°, the AABB should be larger than the unrotated screen rect
        let no_rot_cam = make_camera(0.0, 0.0, 400.0, 300.0, 0.0, 1.0);
        let (nr_min, nr_max) = compute_view_bounds(800.0, 600.0, no_rot_cam, mock_screen_to_world);

        let rotated_width = view_max.x - view_min.x;
        let unrotated_width = nr_max.x - nr_min.x;
        assert!(
            rotated_width > unrotated_width,
            "Rotated width {} should be larger than unrotated {}",
            rotated_width,
            unrotated_width,
        );

        let rotated_height = view_max.y - view_min.y;
        let unrotated_height = nr_max.y - nr_min.y;
        assert!(
            rotated_height > unrotated_height,
            "Rotated height {} should be larger than unrotated {}",
            rotated_height,
            unrotated_height,
        );
    }

    #[test]
    fn view_bounds_90_degree_rotation() {
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 90.0, 1.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // At 90°, width and height effectively swap
        let rotated_width = view_max.x - view_min.x;
        let rotated_height = view_max.y - view_min.y;

        // Original screen: 800x600, so rotated AABB should be ~600 wide and ~800 tall
        // Use relaxed tolerance for trig floating point accumulation
        assert!(
            (rotated_width - 600.0).abs() < 0.001,
            "Rotated width {} should be ~600",
            rotated_width,
        );
        assert!(
            (rotated_height - 800.0).abs() < 0.001,
            "Rotated height {} should be ~800",
            rotated_height,
        );
    }

    #[test]
    fn view_bounds_with_zoom() {
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 0.0, 2.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // Zoom 2x halves the world-space extents
        assert!(approx_eq(view_min.x, -200.0));
        assert!(approx_eq(view_min.y, -150.0));
        assert!(approx_eq(view_max.x, 200.0));
        assert!(approx_eq(view_max.y, 150.0));
    }

    // --- Sprite cull bounds tests ---

    #[test]
    fn sprite_cull_bounds_no_scale_no_rot() {
        let pos = MapPosition::new(100.0, 200.0);
        let sprite = make_sprite(32.0, 48.0, 16.0, 24.0);
        let (min, max) = compute_sprite_cull_bounds(&pos, &sprite, None, None);

        // min = pos - origin, max = min + size
        assert!(approx_eq(min.x, 84.0));
        assert!(approx_eq(min.y, 176.0));
        assert!(approx_eq(max.x, 116.0));
        assert!(approx_eq(max.y, 224.0));
    }

    #[test]
    fn sprite_cull_bounds_with_scale() {
        let pos = MapPosition::new(100.0, 200.0);
        let sprite = make_sprite(32.0, 48.0, 16.0, 24.0);
        let scale = Scale::new(2.0, 2.0);
        let (min, max) = compute_sprite_cull_bounds(&pos, &sprite, Some(&scale), None);

        // scaled: w=64, h=96, ox=32, oy=48
        assert!(approx_eq(min.x, 68.0));
        assert!(approx_eq(min.y, 152.0));
        assert!(approx_eq(max.x, 132.0));
        assert!(approx_eq(max.y, 248.0));
    }

    #[test]
    fn sprite_cull_bounds_with_rotation() {
        let pos = MapPosition::new(100.0, 100.0);
        let sprite = make_sprite(32.0, 32.0, 16.0, 16.0);
        let rot = Rotation { degrees: 45.0 };
        let (min, max) = compute_sprite_cull_bounds(&pos, &sprite, None, Some(&rot));

        // Bounding circle radius = sqrt(16^2 + 16^2) = sqrt(512) ≈ 22.627
        let radius = (16.0_f32 * 16.0 + 16.0 * 16.0).sqrt();
        assert!(approx_eq(min.x, 100.0 - radius));
        assert!(approx_eq(min.y, 100.0 - radius));
        assert!(approx_eq(max.x, 100.0 + radius));
        assert!(approx_eq(max.y, 100.0 + radius));

        // The bounding circle AABB should be larger than the non-rotated AABB
        let (nr_min, nr_max) = compute_sprite_cull_bounds(&pos, &sprite, None, None);
        let rot_area = (max.x - min.x) * (max.y - min.y);
        let nr_area = (nr_max.x - nr_min.x) * (nr_max.y - nr_min.y);
        assert!(
            rot_area > nr_area,
            "Rotated bounds area {} should be larger than non-rotated {}",
            rot_area,
            nr_area,
        );
    }

    #[test]
    fn rotated_sprite_near_edge_not_culled() {
        // Regression test: a rotated sprite near the view edge should not be falsely culled.
        // Camera at origin, 800x600 screen, zoom 1x, no rotation.
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 0.0, 1.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // Sprite at the right edge of view, rotated 45°. Its AABB center is just
        // outside the unscaled bounds but the bounding circle overlaps.
        let pos = MapPosition::new(410.0, 0.0);
        let sprite = make_sprite(64.0, 64.0, 32.0, 32.0);
        let rot = Rotation { degrees: 45.0 };
        let (min, max) = compute_sprite_cull_bounds(&pos, &sprite, None, Some(&rot));

        // The bounding circle radius = sqrt(32^2 + 32^2) ≈ 45.25
        // So min.x ≈ 410 - 45.25 = 364.75, which is < view_max.x = 400
        let overlap =
            !(max.x < view_min.x || min.x > view_max.x || max.y < view_min.y || min.y > view_max.y);
        assert!(
            overlap,
            "Rotated sprite near edge should not be culled. Sprite bounds: ({}, {}) - ({}, {}), View: ({}, {}) - ({}, {})",
            min.x, min.y, max.x, max.y, view_min.x, view_min.y, view_max.x, view_max.y,
        );
    }
}
