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

/// Default tolerance for [`approx_eq`] and [`vec2_approx_eq`].
pub const EPSILON: f32 = 1e-6;

/// `|a - b| < EPSILON`.
pub fn approx_eq(a: f32, b: f32) -> bool {
    approx_eq_eps(a, b, EPSILON)
}

/// `|a - b| < eps` (strict).
pub fn approx_eq_eps(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

/// Both axes within [`EPSILON`].
pub fn vec2_approx_eq(a: Vec2, b: Vec2) -> bool {
    vec2_approx_eq_eps(a, b, EPSILON)
}

/// Both axes within `eps` (strict, unlike glam's `abs_diff_eq`).
pub fn vec2_approx_eq_eps(a: Vec2, b: Vec2, eps: f32) -> bool {
    approx_eq_eps(a.x, b.x, eps) && approx_eq_eps(a.y, b.y, eps)
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
    fn approx_eq_eps_is_strict() {
        assert!(approx_eq_eps(0.0, 0.49, 0.5));
        assert!(approx_eq_eps(0.49, 0.0, 0.5));
        assert!(!approx_eq_eps(0.0, 0.5, 0.5));
        assert!(approx_eq(1.0, 1.0 + EPSILON / 2.0));
        assert!(!approx_eq(1.0, 1.0 + EPSILON * 2.0));
    }

    #[test]
    fn vec2_approx_eq_checks_both_axes() {
        assert!(vec2_approx_eq_eps(Vec2::ZERO, Vec2::new(0.49, -0.49), 0.5));
        assert!(!vec2_approx_eq_eps(Vec2::ZERO, Vec2::new(0.0, 0.5), 0.5));
        assert!(!vec2_approx_eq(Vec2::ZERO, Vec2::new(EPSILON * 2.0, 0.0)));
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
