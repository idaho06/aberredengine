//! Shared 2D transform composition math.
//!
//! Used by [`crate::systems::propagate_transforms`] for world-space hierarchy
//! propagation. Operates on a plain, space-agnostic struct (not
//! [`GlobalTransform2D`]) specifically so screen-space code with no notion of
//! `GlobalTransform2D` can reuse the same composition math without depending
//! on a world-space component.

use raylib::math::Vector2;

/// A position/rotation/scale triple, decoupled from any particular
/// coordinate space or component type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transform2D {
    pub pos: Vector2,
    pub rot_degrees: f32,
    pub scale: Vector2,
}

/// Compose a child's local transform against its parent's, in parent space.
///
/// Order: scale -> rotate -> translate for position; additive rotation;
/// multiplicative scale.
///
/// Calling this with `parent.rot_degrees = 0.0` and `parent.scale = (1, 1)`
/// degenerates to plain addition of `local.pos` onto `parent.pos` — the
/// translate-only case screen-space callers need.
pub(crate) fn compose_transform(parent: Transform2D, local: Transform2D) -> Transform2D {
    let scaled_offset = Vector2 {
        x: local.pos.x * parent.scale.x,
        y: local.pos.y * parent.scale.y,
    };
    let rotated_offset = scaled_offset.rotated(parent.rot_degrees.to_radians());
    Transform2D {
        pos: Vector2 {
            x: parent.pos.x + rotated_offset.x,
            y: parent.pos.y + rotated_offset.y,
        },
        rot_degrees: parent.rot_degrees + local.rot_degrees,
        scale: Vector2 {
            x: parent.scale.x * local.scale.x,
            y: parent.scale.y * local.scale.y,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn vec_approx_eq(a: Vector2, b: Vector2) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y)
    }

    #[test]
    fn full_srt_composition() {
        let parent = Transform2D {
            pos: Vector2 { x: 100.0, y: 50.0 },
            rot_degrees: 90.0,
            scale: Vector2 { x: 2.0, y: 2.0 },
        };
        let local = Transform2D {
            pos: Vector2 { x: 10.0, y: 0.0 },
            rot_degrees: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
        };

        let result = compose_transform(parent, local);

        // local.pos (10,0) scaled by parent.scale (2,2) -> (20,0)
        // rotated by 90deg -> (0,20)
        // translated by parent.pos (100,50) -> (100,70)
        assert!(vec_approx_eq(result.pos, Vector2 { x: 100.0, y: 70.0 }));
        assert!(approx_eq(result.rot_degrees, 90.0));
        assert!(vec_approx_eq(result.scale, Vector2 { x: 2.0, y: 2.0 }));
    }

    #[test]
    fn translate_only_degenerates_to_addition() {
        let parent = Transform2D {
            pos: Vector2 { x: 100.0, y: 100.0 },
            rot_degrees: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
        };
        let local = Transform2D {
            pos: Vector2 { x: 20.0, y: 40.0 },
            rot_degrees: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
        };

        let result = compose_transform(parent, local);

        assert!(vec_approx_eq(result.pos, Vector2 { x: 120.0, y: 140.0 }));
        assert!(approx_eq(result.rot_degrees, local.rot_degrees));
        assert!(vec_approx_eq(result.scale, local.scale));
    }
}
