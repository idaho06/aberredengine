//! Rotation component for 2D entities.
//!
//! The [`Rotation`] component stores a rotation angle in degrees. The render
//! system uses this value when drawing sprites to rotate them around their
//! origin.

use bevy_ecs::prelude::Component;

/// Rotation angle in degrees for 2D rendering.
///
/// Positive values rotate clockwise. Used by the render system when drawing
/// sprites and can be animated via [`TweenRotation`](super::tween::TweenRotation).
#[derive(Component, Clone, Debug, Copy, Default)]
pub struct Rotation {
    pub degrees: f32,
}
