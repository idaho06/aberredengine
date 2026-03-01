//! Camera target marker component.
//!
//! Entities with [`CameraTarget`] are candidates for the camera-follow system.
//! When multiple targets exist, the one with the highest
//! [`priority`](CameraTarget::priority) is chosen.

use bevy_ecs::prelude::Component;

/// Marks an entity as a candidate for camera following.
///
/// The camera-follow system selects the entity with the highest `priority`
/// value. Ties are broken deterministically by [`Entity`](bevy_ecs::entity::Entity) id.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CameraTarget {
    /// Selection priority. Higher values win. Default is `0`.
    pub priority: u8,
}

impl CameraTarget {
    /// Create a camera target with the given priority.
    pub fn new(priority: u8) -> Self {
        Self { priority }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_priority_is_zero() {
        let target = CameraTarget::default();
        assert_eq!(target.priority, 0);
    }

    #[test]
    fn new_sets_priority() {
        let target = CameraTarget::new(10);
        assert_eq!(target.priority, 10);
    }
}
