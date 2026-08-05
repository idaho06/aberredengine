//! Deterministic simulation RNG.
//!
//! [`SimRng`] is the *only* source of randomness sim-schedule systems may
//! draw from -- never a `Local<fastrand::Rng>`, never a `rand`/`fastrand`
//! global. Render/audio/present-schedule code, and any conditionally
//! compiled debug path, must not draw from it either: its internal state
//! IS sim state, and must advance identically on every replay of a session.

use bevy_ecs::prelude::Resource;

/// The engine's single [`fastrand::Rng`] instance, available to sim-schedule
/// systems and to Rust game callbacks via `GameCtx::sim_rng`. The tuple field
/// is `pub` so game code can call any `fastrand::Rng` method directly (e.g.
/// `ctx.sim_rng.0.f32()`) -- `SimRng` deliberately doesn't wrap that surface.
///
/// Deliberately does **not** implement/derive `Default` -- a `Default` impl
/// would silently entropy-seed via `fastrand::Rng::new()`, defeating the
/// "exactly one construction path" goal. Construct via [`SimRng::from_seed`]
/// everywhere (production seeding in `setup_logic_world`, and test-world
/// helpers that need a placeholder instance) rather than
/// `SimRng(fastrand::Rng::with_seed(seed))` directly, so there's one named
/// entry point to grep for. `setup_logic_world` picks the seed:
/// - deterministic mode (`EngineBuilder::deterministic(seed)`): the given
///   `seed`.
/// - non-deterministic mode: a throwaway
///   `fastrand::Rng::new()` read back via `.get_seed()` to get a concrete
///   seed to log (`info!`) and pass to `from_seed`, so a surprising session
///   can be attributed after the fact.
#[derive(Resource)]
pub struct SimRng(pub fastrand::Rng);

impl SimRng {
    /// Construct from a concrete seed -- the one blessed construction path;
    /// see the struct doc comment.
    pub fn from_seed(seed: u64) -> Self {
        Self(fastrand::Rng::with_seed(seed))
    }
}
