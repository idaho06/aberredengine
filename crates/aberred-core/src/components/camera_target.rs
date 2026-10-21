//! Camera target marker component.
//!
//! Entities with [`CameraTarget`] are candidates for the camera-follow system.
//! When multiple targets exist, the one with the highest
//! [`priority`](CameraTarget::priority) is chosen. Its [`zoom`](CameraTarget::zoom)
//! is then lerped into the camera each frame via
//! [`CameraFollowConfig::zoom_lerp_speed`](crate::resources::camerafollowconfig::CameraFollowConfig::zoom_lerp_speed).

use bevy_ecs::prelude::Component;

/// Marks an entity as a candidate for camera following.
///
/// The camera-follow system selects the entity with the highest `priority`
/// value. Ties are broken deterministically by [`Entity`](bevy_ecs::entity::Entity) id.
#[derive(Component, Clone, Copy, Debug)]
pub struct CameraTarget {
    /// Selection priority. Higher values win. Default is `0`.
    pub priority: u8,
    /// Desired camera zoom when this is the winning target.
    /// Applied smoothly each frame via `CameraFollowConfig::zoom_lerp_speed`.
    /// Default is `1.0`.
    pub zoom: f32,
}

impl Default for CameraTarget {
    fn default() -> Self {
        Self {
            priority: 0,
            zoom: 1.0,
        }
    }
}

impl CameraTarget {
    /// Create a camera target with the given priority and default zoom (`1.0`).
    pub fn new(priority: u8) -> Self {
        Self {
            priority,
            zoom: 1.0,
        }
    }

    /// Set the desired camera zoom for this target (builder).
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_defaults_to_one() {
        let default = CameraTarget::default();
        assert_eq!((default.priority, default.zoom), (0, 1.0));
        assert_eq!(CameraTarget::new(10).zoom, 1.0);
    }
}
