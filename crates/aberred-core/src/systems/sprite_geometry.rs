//! Pure sprite geometry shared by the render thread's draw path and logic-side
//! world-space hit testing (e.g. mouse picking).
//!
//! Everything here uses core math types only, so logic-side code can compute
//! exactly where a sprite is drawn without touching the render crate.

use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::sprite::Sprite;
use crate::math::{Rect, Vec2};

/// World-space geometry of one sprite draw: the destination rectangle, the
/// scaled pivot, and the rotation.
///
/// It follows the `draw_texture_pro(tex, src, dest, origin, rotation, tint)`
/// convention: local coordinate `(origin.x, origin.y)` maps to world position
/// `(dest.x, dest.y)`, and the sprite rotates around that point. Without
/// rotation the visual top-left is at `(dest.x - origin.x, dest.y - origin.y)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteRenderGeometry {
    /// Anchor position (`x`, `y`) and scaled size (`width`, `height`).
    pub dest: Rect,
    /// Pivot offset from the visual top-left, scaled.
    pub origin: Vec2,
    /// Rotation in degrees, clockwise.
    pub rotation: f32,
}

#[cfg(test)]
impl SpriteRenderGeometry {
    /// World-space position of the anchor/pivot (always `(dest.x, dest.y)`).
    fn anchor_world_pos(&self) -> Vec2 {
        Vec2::new(self.dest.x, self.dest.y)
    }

    /// Visual top-left corner (ignoring rotation).
    fn visual_top_left(&self) -> Vec2 {
        Vec2::new(self.dest.x - self.origin.x, self.dest.y - self.origin.y)
    }

    /// Visual bottom-right corner (ignoring rotation).
    fn visual_bottom_right(&self) -> Vec2 {
        Vec2::new(
            self.dest.x - self.origin.x + self.dest.width,
            self.dest.y - self.origin.y + self.dest.height,
        )
    }
}

/// Compute where a sprite is drawn: the destination rectangle, scaled pivot
/// and rotation for a sprite at `pos` with optional `scale` and `rot`.
pub fn compute_sprite_geometry(
    pos: &MapPosition,
    sprite: &Sprite,
    scale: Option<&Scale>,
    rot: Option<&Rotation>,
) -> SpriteRenderGeometry {
    let scale = scale.map_or(Vec2::ONE, |s| s.scale);
    SpriteRenderGeometry {
        dest: Rect::new(
            pos.pos.x,
            pos.pos.y,
            sprite.width * scale.x,
            sprite.height * scale.y,
        ),
        origin: sprite.origin * scale,
        rotation: rot.map_or(0.0, |r| r.degrees),
    }
}

/// Resolve the effective world-space transform for an entity, preferring
/// `GlobalTransform2D` (hierarchy) over the entity's own local components.
#[inline]
pub fn resolve_world_transform(
    pos: MapPosition,
    maybe_scale: Option<Scale>,
    maybe_rot: Option<Rotation>,
    maybe_gt: Option<GlobalTransform2D>,
) -> (MapPosition, Option<Scale>, Option<Rotation>) {
    if let Some(gt) = maybe_gt {
        (
            MapPosition::from_vec(gt.position),
            Some(Scale { scale: gt.scale }),
            Some(Rotation {
                degrees: gt.rotation_degrees,
            }),
        )
    } else {
        (pos, maybe_scale, maybe_rot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec2;
    use crate::testing::approx_eq;

    fn make_sprite(w: f32, h: f32, origin_x: f32, origin_y: f32) -> Sprite {
        Sprite::new("test", w, h).with_origin(Vec2::new(origin_x, origin_y))
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
}
