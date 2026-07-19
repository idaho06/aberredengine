//! Determinism regression guard (determinism-02-deterministic-schedule.md).
//!
//! Runs an identical scripted scenario -- entity spawns AND despawns
//! interleaved across multiple ticks, plus a collision -- against two fresh
//! `TestWorld`s and asserts the exact `Entity` ids allocated (not just
//! component values, which could coincidentally match while ids diverge)
//! are identical between the two runs. This is the actual nondeterminism
//! source phase 02 fixes (pinning the sim schedule's executor to
//! single-threaded removes cross-run `Commands`-application/entity-
//! allocation-order jitter) -- a scenario that only spawns once in
//! `on_setup` would pass trivially and prove nothing, since two runs of a
//! single-threaded, already-deterministic executor always agree; the
//! despawn-then-respawn step exercises entity slot/generation reuse
//! specifically. Doubles as the precursor regression guard phase 05
//! (replays) needs at larger scale.

#![cfg(feature = "test-support")]

use aberredengine::bevy_ecs::prelude::*;
use aberredengine::components::boxcollider::BoxCollider;
use aberredengine::components::collision::{BoxSides, CollisionRule};
use aberredengine::components::group::Group;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::systems::game_ctx::GameCtx;
use aberredengine::test_support::TestWorld;

mod common;
use common::DT;

fn collision_bump_flag(_a: Entity, _b: Entity, _sa: &BoxSides, _sb: &BoxSides, ctx: &mut GameCtx) {
    ctx.world_signals.set_flag("determinism_test_collided");
}

/// Records every entity id this scenario allocates/frees, in the exact
/// order the scripted steps below issue them -- the ground truth this test
/// compares across two independent runs.
#[derive(Resource, Default)]
struct DeterminismLog {
    spawned: Vec<Entity>,
    despawned: Vec<Entity>,
}

/// Drives the scripted scenario purely off how many times it has run while
/// `Playing` (a private `Local<u32>` step counter), not off `WorldTime`'s
/// shared `frame_count` -- `TestWorld::tick_to_play`'s internal tick count
/// before reaching `Playing` is an implementation detail this test
/// shouldn't depend on, even though it happens to be stable today.
fn scenario_driver(
    mut commands: Commands,
    mut log: ResMut<DeterminismLog>,
    mut step: Local<u32>,
    mut entity_a: Local<Option<Entity>>,
) {
    *step += 1;
    match *step {
        // Tick 1: spawn two overlapping colliders in matching groups plus
        // their collision rule.
        1 => {
            let a = commands
                .spawn((
                    Group::new("a"),
                    MapPosition::new(0.0, 0.0),
                    BoxCollider::new(10.0, 10.0),
                ))
                .id();
            log.spawned.push(a);
            *entity_a = Some(a);

            let b = commands
                .spawn((
                    Group::new("b"),
                    MapPosition::new(2.0, 2.0),
                    BoxCollider::new(10.0, 10.0),
                ))
                .id();
            log.spawned.push(b);

            let rule = commands
                .spawn(CollisionRule::rust("a", "b", collision_bump_flag))
                .id();
            log.spawned.push(rule);
        }
        // Tick 3: spawn an unrelated, non-overlapping entity (churns the
        // allocator without touching the collision pair above).
        3 => {
            let c = commands
                .spawn((
                    Group::new("a"),
                    MapPosition::new(500.0, 500.0),
                    BoxCollider::new(10.0, 10.0),
                ))
                .id();
            log.spawned.push(c);
        }
        // Tick 5: despawn entity A, freeing its slot.
        5 => {
            if let Some(a) = entity_a.take() {
                commands.entity(a).despawn();
                log.despawned.push(a);
            }
        }
        // Tick 6: spawn again immediately after the despawn above -- the
        // slot-reuse case (index/generation allocation) this test exists to
        // exercise.
        6 => {
            let d = commands
                .spawn((
                    Group::new("b"),
                    MapPosition::new(1000.0, 1000.0),
                    BoxCollider::new(10.0, 10.0),
                ))
                .id();
            log.spawned.push(d);
        }
        _ => {}
    }
}

struct ScenarioResult {
    spawned: Vec<Entity>,
    despawned: Vec<Entity>,
    live_entities: Vec<Entity>,
    collided: bool,
}

fn run_scenario() -> ScenarioResult {
    let mut tw = TestWorld::builder()
        .add_system(scenario_driver)
        .build()
        .expect("build should succeed");
    tw.world.insert_resource(DeterminismLog::default());

    tw.tick(8, DT);

    let log = tw.world.resource::<DeterminismLog>();
    let spawned = log.spawned.clone();
    let despawned = log.despawned.clone();
    let collided = tw
        .world
        .resource::<WorldSignals>()
        .has_flag("determinism_test_collided");
    let live_entities: Vec<Entity> = tw.world.query::<Entity>().iter(&tw.world).collect();

    ScenarioResult {
        spawned,
        despawned,
        live_entities,
        collided,
    }
}

#[test]
fn identical_scenario_allocates_identical_entity_ids_across_runs() {
    let run1 = run_scenario();
    let run2 = run_scenario();

    assert!(
        run1.collided && run2.collided,
        "the scripted overlap must fire the collision rule in both runs, \
         or this test isn't exercising collision_detector's pairwise \
         Entity-ordered iteration at all"
    );
    assert_eq!(
        run1.spawned, run2.spawned,
        "spawned entity ids (including the post-despawn slot-reuse spawn) \
         must be bit-identical across two runs of the identical scripted \
         scenario"
    );
    assert_eq!(
        run1.despawned, run2.despawned,
        "despawned entity ids must be bit-identical across two runs"
    );
    assert_eq!(
        run1.live_entities, run2.live_entities,
        "the full live-entity set, in query iteration order, must be \
         bit-identical across two runs (archetype/table order is \
         deterministic given identical spawn/despawn history)"
    );
}
