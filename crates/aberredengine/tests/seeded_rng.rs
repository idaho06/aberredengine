//! Seeded-RNG regression guard (determinism-03-seeded-rng.md).
//!
//! Runs an identical scripted particle-emission scenario against two
//! independent `TestWorld`s built via `TestWorldBuilder::deterministic(seed)`
//! -- the real production seeding path (`EngineBuilder::deterministic` /
//! `TestWorldBuilder::deterministic` -> `LogicInit` -> `setup_logic_world`),
//! not a resource poked in after the fact. Same seed must reproduce a
//! bit-identical particle stream; different seeds must diverge.

#![cfg(feature = "test-support")]

use aberredengine::bevy_ecs::prelude::*;
use aberredengine::core::components::emittedparticle::EmittedParticle;
use aberredengine::core::components::mapposition::MapPosition;
use aberredengine::core::components::particleemitter::ParticleEmitter;
use aberredengine::core::components::rigidbody::RigidBody;
use aberredengine::core::components::ttl::Ttl;
use aberredengine::test_support::TestWorldBuilder;

mod common;
use common::DT;

/// One sampled particle's state -- a whole vector of these is compared
/// across runs (not a single scalar) so a coincidental same-seed/
/// different-seed collision on any one field can't produce a flaky result.
#[derive(Debug, Clone, PartialEq)]
struct ParticleSample {
    pos: (f32, f32),
    velocity: (f32, f32),
    ttl: Option<f32>,
}

/// Drives the scripted scenario purely off a private `Local<u32>` step
/// counter (mirrors `tests/determinism.rs`'s `scenario_driver`), not
/// `WorldTime`'s shared `frame_count`.
fn scenario_driver(
    mut commands: Commands,
    mut step: Local<u32>,
    mut template: Local<Option<Entity>>,
) {
    *step += 1;
    match *step {
        // Tick 1: spawn the particle template.
        1 => {
            let t = commands
                .spawn((MapPosition::new(0.0, 0.0), RigidBody::new()))
                .id();
            *template = Some(t);
        }
        // Tick 2: spawn the emitter, referencing the template (guaranteed
        // flushed by now). Non-degenerate ranges so every `random_f32_range`
        // call site (rect offset x/y, arc angle, speed, TTL) and the
        // template-pick draw are all exercised.
        2 => {
            let t = template.expect("template spawned on tick 1");
            commands.spawn((
                MapPosition::new(0.0, 0.0),
                ParticleEmitter {
                    templates: vec![t],
                    shape: aberredengine::core::components::particleemitter::EmitterShape::Rect {
                        width: 40.0,
                        height: 40.0,
                    },
                    particles_per_emission: 3,
                    emissions_per_second: 1000.0,
                    emissions_remaining: 20,
                    initial_emissions_remaining: 20,
                    arc_degrees: (-45.0, 45.0),
                    speed_range: (10.0, 200.0),
                    ttl: aberredengine::core::components::particleemitter::TtlSpec::Range {
                        min: 0.5,
                        max: 3.0,
                    },
                    ..Default::default()
                },
            ));
        }
        _ => {}
    }
}

fn run_scenario(seed: u64) -> Vec<ParticleSample> {
    let mut tw = TestWorldBuilder::new()
        .deterministic(seed)
        .add_system(scenario_driver)
        .build()
        .expect("build should succeed");

    tw.tick(10, DT);

    let mut samples: Vec<ParticleSample> = tw
        .world
        .query::<(&MapPosition, &RigidBody, Option<&Ttl>, &EmittedParticle)>()
        .iter(&tw.world)
        .map(|(pos, rb, ttl, _)| ParticleSample {
            pos: (pos.pos.x, pos.pos.y),
            velocity: (rb.velocity.x, rb.velocity.y),
            ttl: ttl.map(|t| t.remaining),
        })
        .collect();
    // Query iteration order isn't itself under test here (determinism.rs
    // covers entity-id/iteration-order determinism); sort so this test only
    // asserts on the *sampled values*.
    samples.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap());
    samples
}

#[test]
fn same_seed_reproduces_identical_particle_stream() {
    let run1 = run_scenario(42);
    let run2 = run_scenario(42);

    assert!(
        !run1.is_empty(),
        "no particles were emitted -- this test isn't exercising SimRng draws at all"
    );
    assert_eq!(
        run1, run2,
        "same seed must reproduce a bit-identical particle stream"
    );
}

#[test]
fn different_seeds_diverge() {
    let run1 = run_scenario(42);
    let run2 = run_scenario(1337);

    assert!(
        !run1.is_empty() && !run2.is_empty(),
        "no particles were emitted -- this test isn't exercising SimRng draws at all"
    );
    assert_ne!(
        run1, run2,
        "different seeds must diverge -- if this coincidentally fails, verify the \
         builder's .deterministic(seed) is actually threaded through to SimRng"
    );
}
