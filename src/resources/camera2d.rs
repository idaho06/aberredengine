//! Shared 2D camera resource.
//!
//! Wraps raylib's [`raylib::prelude::Camera2D`] so that systems can agree on
//! a single world/screen transform. Update this resource to pan/zoom the view.

use bevy_ecs::prelude::Resource;
use raylib::prelude::{Camera2D, Rectangle, Vector2};

use crate::resources::screensize::ScreenSize;

/// ECS resource that holds the active 2D camera parameters.
///
/// Typically inserted during setup or scene loading, read by render systems, and optionally
/// mutated by camera-controller systems.
#[derive(Resource)]
pub struct Camera2DRes(pub Camera2D);

impl Camera2DRes {
    /// Returns a copy of the camera with `target` rounded to the nearest integer pixel.
    ///
    /// Pass this to `begin_mode2D` instead of `self.0` to prevent sub-pixel GPU sampling
    /// from bleeding sprite atlas tiles into their neighbors during camera movement.
    /// The stored `Camera2DRes` is unchanged so game-logic systems keep full float precision.
    pub fn pixel_snapped(&self) -> Camera2D {
        Camera2D {
            target: Vector2 {
                x: self.0.target.x.round(),
                y: self.0.target.y.round(),
            },
            ..self.0
        }
    }

    /// World-visible rectangle computed from the pixel-snapped camera target.
    ///
    /// Use alongside [`pixel_snapped`](Self::pixel_snapped) when the render pass needs
    /// a culling rectangle that matches the snapped GPU transform.
    pub fn world_visible_rect_snapped(&self, screen: &ScreenSize) -> Rectangle {
        Camera2DRes(self.pixel_snapped()).world_visible_rect(screen)
    }

    /// Returns the visible world-space rectangle for the current camera state.
    ///
    /// Assumes zero rotation. Under non-zero `rotation` the visible area is a rotated
    /// rectangle; this function returns its axis-aligned bounding box, which is only
    /// exact when `rotation == 0`.
    ///
    /// Guards against `zoom == 0` via `f32::EPSILON` to avoid division-by-zero.
    pub fn world_visible_rect(&self, screen: &ScreenSize) -> Rectangle {
        let zoom = self.0.zoom.max(f32::EPSILON);
        let w = screen.w as f32 / zoom;
        let h = screen.h as f32 / zoom;
        let x = self.0.target.x - self.0.offset.x / zoom;
        let y = self.0.target.y - self.0.offset.y / zoom;
        Rectangle {
            x,
            y,
            width: w,
            height: h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use raylib::prelude::Vector2;

    fn make_camera(target: Vector2, offset: Vector2, zoom: f32) -> Camera2DRes {
        Camera2DRes(Camera2D {
            target,
            offset,
            rotation: 0.0,
            zoom,
        })
    }

    #[test]
    fn view_rect_default_camera() {
        let cam = make_camera(
            Vector2 { x: 0.0, y: 0.0 },
            Vector2 { x: 320.0, y: 180.0 },
            1.0,
        );
        let screen = ScreenSize { w: 640, h: 360 };
        let r = cam.world_visible_rect(&screen);
        assert!((r.x - -320.0).abs() < 1e-4);
        assert!((r.y - -180.0).abs() < 1e-4);
        assert!((r.width - 640.0).abs() < 1e-4);
        assert!((r.height - 360.0).abs() < 1e-4);
    }

    #[test]
    fn view_rect_zoom_2x() {
        let cam = make_camera(
            Vector2 { x: 0.0, y: 0.0 },
            Vector2 { x: 320.0, y: 180.0 },
            2.0,
        );
        let screen = ScreenSize { w: 640, h: 360 };
        let r = cam.world_visible_rect(&screen);
        assert!((r.x - -160.0).abs() < 1e-4);
        assert!((r.y - -90.0).abs() < 1e-4);
        assert!((r.width - 320.0).abs() < 1e-4);
        assert!((r.height - 180.0).abs() < 1e-4);
    }

    #[test]
    fn pixel_snapped_rounds_target() {
        let cam = make_camera(
            Vector2 { x: 10.7, y: -3.2 },
            Vector2 { x: 320.0, y: 180.0 },
            1.0,
        );
        let snapped = cam.pixel_snapped();
        assert!((snapped.target.x - 11.0).abs() < 1e-6);
        assert!((snapped.target.y - -3.0).abs() < 1e-6);
    }

    #[test]
    fn pixel_snapped_preserves_other_fields() {
        let cam = Camera2DRes(Camera2D {
            target: Vector2 { x: 1.5, y: 2.5 },
            offset: Vector2 { x: 100.0, y: 200.0 },
            rotation: 45.0,
            zoom: 2.0,
        });
        let snapped = cam.pixel_snapped();
        assert!((snapped.offset.x - 100.0).abs() < 1e-6);
        assert!((snapped.offset.y - 200.0).abs() < 1e-6);
        assert!((snapped.rotation - 45.0).abs() < 1e-6);
        assert!((snapped.zoom - 2.0).abs() < 1e-6);
    }

    #[test]
    fn world_visible_rect_snapped_matches_snapped_camera() {
        let cam = make_camera(
            Vector2 { x: 10.7, y: -3.2 },
            Vector2 { x: 320.0, y: 180.0 },
            1.0,
        );
        let screen = ScreenSize { w: 640, h: 360 };
        let snapped_rect = cam.world_visible_rect_snapped(&screen);
        let snapped_cam = Camera2DRes(cam.pixel_snapped());
        let expected = snapped_cam.world_visible_rect(&screen);
        assert!((snapped_rect.x - expected.x).abs() < 1e-4);
        assert!((snapped_rect.y - expected.y).abs() < 1e-4);
        assert!((snapped_rect.width - expected.width).abs() < 1e-4);
        assert!((snapped_rect.height - expected.height).abs() < 1e-4);
    }

    #[test]
    fn view_rect_zoom_zero_no_panic() {
        let cam = make_camera(
            Vector2 { x: 0.0, y: 0.0 },
            Vector2 { x: 320.0, y: 180.0 },
            0.0,
        );
        let screen = ScreenSize { w: 640, h: 360 };
        let r = cam.world_visible_rect(&screen);
        assert!(r.width.is_finite());
        assert!(r.height.is_finite());
    }
}
