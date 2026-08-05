//! Shared constants and helpers for integration tests under `tests/` (each
//! sibling file there is its own compiled crate, so this lives in a
//! `common/mod.rs` subdirectory -- the standard Cargo idiom for sharing code
//! between integration test binaries without `common` itself becoming one).
//!
//! Each consuming binary only compiles the module, not just the items it
//! calls, so `dead_code` fires per binary for whichever half of this file
//! that binary doesn't use -- allowed here rather than at every call site.
#![allow(dead_code)]

use aberredengine::bevy_ecs::prelude::World;
use aberredengine::core::resources::sim_rng::SimRng;

/// Fixed per-tick `dt` used by `TestWorld::tick`/`tick_to_play` calls across
/// the integration test suite (60Hz-equivalent).
pub const DT: f32 = 1.0 / 60.0;

/// Insert a placeholder [`SimRng`] into a hand-built `World` (one that isn't
/// going through `TestWorldBuilder`, which already inserts a real one via
/// `setup_logic_world`). Seed value is irrelevant -- these worlds never
/// assert on RNG output, they just need any `GameCtx`-taking
/// system/observer to find the resource.
pub fn insert_sim_rng(world: &mut World) {
    world.insert_resource(SimRng::from_seed(0));
}
