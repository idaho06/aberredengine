//! Shared helpers for unit and integration tests across the workspace.
//!
//! Compiled under `cfg(test)` for core's own tests, and under the
//! `test-support` feature for every other crate's tests (enabled through their
//! `[dev-dependencies]`). Plain functions only. These helpers never build the
//! sim/present schedules; a test that depends on system ordering uses the
//! facade's `TestWorld` instead.

use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use glam::Vec2;

use crate::protocol::audio::AudioCmd;
use crate::resources::appstate::AppState;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::gameconfig::GameConfig;
use crate::resources::input_bindings::InputBindings;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::sim_rng::SimRng;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

/// Allowed error, in float steps (`f32::EPSILON` scaled to the compared
/// values' magnitude), for [`approx_eq`] and [`vec2_approx_eq`]. The worst
/// error measured across the test suite (2026-09-30) is 7.3 steps.
pub const ULPS: f32 = 16.0;

/// `a` and `b` are within [`ULPS`] float steps of each other at their own
/// magnitude: `|a - b| <= ULPS * f32::EPSILON * max(|a|, |b|, 1)`.
///
/// The scale never drops below 1, so a result expected to be 0 tolerates
/// the same absolute error as one near 1. A zero computed from much larger
/// inputs (e.g. a 1000-unit vector rotated 90 degrees) carries error
/// proportional to those inputs and may need an explicit tolerance.
pub fn approx_eq(a: f32, b: f32) -> bool {
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= ULPS * f32::EPSILON * scale
}

/// [`approx_eq`] on both axes.
pub fn vec2_approx_eq(a: Vec2, b: Vec2) -> bool {
    approx_eq(a.x, b.x) && approx_eq(a.y, b.y)
}

/// Insert the resources a `GameCtx` system param reads, with default values
/// (`SimRng` seeded with 0). Callers that need a specific `WorldTime` insert
/// their own afterward.
pub fn insert_game_ctx_resources(world: &mut World) {
    world.insert_resource(WorldSignals::default());
    world.insert_resource(AppState::default());
    world.insert_resource(Messages::<AudioCmd>::default());
    world.insert_resource(WorldTime::default());
    world.insert_resource(GameConfig::default());
    world.insert_resource(PostProcessShader::default());
    world.insert_resource(CameraFollowConfig::default());
    world.insert_resource(InputBindings::default());
    world.insert_resource(SimRng::from_seed(0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::game_ctx::GameCtx;
    use bevy_ecs::system::RunSystemOnce;

    #[test]
    fn approx_eq_tolerance_scales_with_magnitude() {
        // Same absolute error (2e-5): ~2 float steps at 100, ~168 at 1.
        assert!(approx_eq(99.99998, 100.0));
        assert!(!approx_eq(1.00002, 1.0));
        // 1e-3 apart: inside the tolerance at 1000, far outside at 10.
        assert!(approx_eq(1000.001, 1000.0));
        assert!(!approx_eq(10.001, 10.0));
    }

    #[test]
    fn approx_eq_floors_scale_at_one_near_zero() {
        // cos(90°) residue on a 20-unit vector: ~7 float steps at scale 1.
        assert!(approx_eq(-8.742278e-7, 0.0));
        assert!(!approx_eq(1e-5, 0.0));
    }

    #[test]
    fn vec2_approx_eq_checks_both_axes() {
        assert!(vec2_approx_eq(
            Vec2::new(99.99998, -99.99998),
            Vec2::new(100.0, -100.0)
        ));
        assert!(!vec2_approx_eq(
            Vec2::new(100.0, 1.00002),
            Vec2::new(100.0, 1.0)
        ));
    }

    #[test]
    fn insert_game_ctx_resources_satisfies_game_ctx() {
        let mut world = World::new();
        insert_game_ctx_resources(&mut world);
        world
            .run_system_once(|_ctx: GameCtx| {})
            .expect("GameCtx should find every resource it needs");
    }
}
