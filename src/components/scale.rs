//! Scale component for 2D entities.
//!
//! The [`Scale`] component stores a 2D scale factor applied to sprites during
//! rendering. Values greater than 1.0 enlarge the sprite; values less than 1.0
//! shrink it. Negative values can be used to flip.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// 2D scale factor for sprite rendering.
///
/// The render system multiplies sprite dimensions by these values. Can be
/// animated via [`TweenScale`](super::tween::TweenScale).
#[derive(Component, Clone, Debug, Copy)]
pub struct Scale {
    pub scale: Vector2,
}
impl Scale {
    pub fn new(sx: f32, sy: f32) -> Self {
        Self {
            scale: Vector2 { x: sx, y: sy },
        }
    }
}
impl Default for Scale {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}
