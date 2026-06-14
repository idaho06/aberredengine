//! Screen-space position component.
//!
//! The [`ScreenPosition`] component stores an entity's position in screen
//! (pixel) coordinates. Use this for UI elements that should not move with
//! the camera.
//!
//! For world-space entities, see
//! [`MapPosition`](super::mapposition::MapPosition).

use crate::components::position2d::{Position2D, ScreenSpace};

/// Screen-space position (pivot) for an entity.
///
/// Used for UI elements that should remain fixed on screen regardless of
/// camera movement. The render system draws these after the world pass.
pub type ScreenPosition = Position2D<ScreenSpace>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::position2d::test_helpers::run_shared_tests;

    #[test]
    fn shared_behavior() {
        run_shared_tests::<ScreenSpace>();
    }
}
