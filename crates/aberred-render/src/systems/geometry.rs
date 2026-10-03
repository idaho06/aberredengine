use raylib::prelude::*;

use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::components::sprite::Sprite;

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
    use aberred_core::testing::{approx_eq, sprite_with_origin};

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
        assert!(
            approx_eq(rotated_width, 600.0),
            "Rotated width {} should be ~600",
            rotated_width,
        );
        assert!(
            approx_eq(rotated_height, 800.0),
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
        let sprite = sprite_with_origin(32.0, 48.0, 16.0, 24.0);
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
        let sprite = sprite_with_origin(32.0, 48.0, 16.0, 24.0);
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
        let sprite = sprite_with_origin(32.0, 32.0, 16.0, 16.0);
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
        let sprite = sprite_with_origin(64.0, 64.0, 32.0, 32.0);
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
