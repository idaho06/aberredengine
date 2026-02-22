//! Computed world-space transform for entities in a hierarchy.
//!
//! When an entity has a [`ChildOf`](bevy_ecs::hierarchy::ChildOf) parent, its
//! [`MapPosition`](super::mapposition::MapPosition), [`Rotation`](super::rotation::Rotation),
//! and [`Scale`](super::scale::Scale) are interpreted as local to the parent.
//! The [`propagate_transforms`](crate::systems::propagate_transforms::propagate_transforms)
//! system computes the resulting world-space values and stores them here.

use bevy_ecs::prelude::*;
use raylib::math::Vector2;

/// Computed world-space transform for hierarchical entities.
///
/// This component is automatically managed by the transform propagation system.
/// For root entities (no parent), it mirrors the local MapPosition/Rotation/Scale.
/// For child entities, it contains the composed result of the full ancestor chain.
#[derive(Component, Clone, Copy, Debug)]
pub struct GlobalTransform2D {
    /// World-space position.
    pub position: Vector2,
    /// World-space rotation in degrees.
    pub rotation_degrees: f32,
    /// World-space scale.
    pub scale: Vector2,
}

impl Default for GlobalTransform2D {
    fn default() -> Self {
        Self {
            position: Vector2 { x: 0.0, y: 0.0 },
            rotation_degrees: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
        }
    }
}
