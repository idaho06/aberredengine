//! World-space position component.
//!
//! The [`MapPosition`] component stores an entity's position in world
//! coordinates. It serves as the pivot point used by rendering and collision
//! systems.
//!
//! For screen-space UI elements, see
//! [`ScreenPosition`](super::screenposition::ScreenPosition).

use crate::components::position2d::{Position2D, WorldSpace};

/// World-space position (pivot) for an entity.
///
/// This position commonly represents the pivot used by other components such
/// as [`Sprite`](super::sprite::Sprite) and [`BoxCollider`](super::boxcollider::BoxCollider)
/// to compute rendering and collision bounds.
pub type MapPosition = Position2D<WorldSpace>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::position2d::test_helpers::run_shared_tests;

    #[test]
    fn shared_behavior() {
        run_shared_tests::<WorldSpace>();
    }
}
