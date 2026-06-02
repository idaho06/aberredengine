//! Shared 2D camera resource.
//!
//! Wraps raylib's [`raylib::prelude::Camera2D`] so that systems can agree on
//! a single world/screen transform. Update this resource to pan/zoom the view.

use bevy_ecs::prelude::Resource;
use raylib::prelude::{Camera2D, Rectangle};

use crate::resources::screensize::ScreenSize;

/// ECS resource that holds the active 2D camera parameters.
///
/// Typically inserted during setup or scene loading, read by render systems, and optionally
/// mutated by camera-controller systems.
#[derive(Resource)]
pub struct Camera2DRes(pub Camera2D);

impl Camera2DRes {
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
