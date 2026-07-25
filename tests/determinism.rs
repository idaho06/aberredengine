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
use aberredengine::protocol::raw_input::RawDeviceSnapshot;
use aberredengine::protocol::tick_input::TickInput;
use aberredengine::raylib::ffi::KeyboardKey;
use aberredengine::resources::input::InputState;
use aberredengine::resources::screensize::ScreenSize;
use aberredengine::resources::signal_intents::SignalIntent;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::systems::game_ctx::GameCtx;
use aberredengine::test_support::{TestWorld, TestWorldBuilder};

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

// --- determinism-04-tick-input.md: TickInput round-trip / empty-tick tests ---

/// Observable state after driving a `TestWorld` through the scripted
/// `TickInput` sequence below -- the "ground truth" `round_trip_tick_input_*`
/// compares across two independent runs.
#[derive(Debug, PartialEq)]
struct TickInputScenarioResult {
    up_active: bool,
    flagged: bool,
    screen_size: (i32, i32),
}

/// Drives a fresh, independently-seeded `TestWorld` through an identical
/// scripted `TickInput` sequence -- this sequence itself stands in for a
/// "recorded log" (05 adds an actual Recorder/serialization; the `TickInput`
/// values are already the loss-free record per determinism-04-tick-input.md
/// §3). Exercises all four `TickInput` fields relevant to `apply_tick_input`:
/// raw samples (held key), an empty tick, queued `SignalIntent`s, and a
/// `ScreenSize` change.
fn run_tick_input_scenario(seed: u64) -> TickInputScenarioResult {
    let mut tw = TestWorldBuilder::new()
        .deterministic(seed)
        .build()
        .expect("build should succeed");

    let mut key_down = RawDeviceSnapshot::default();
    key_down.set_key(KeyboardKey::KEY_W as u32);

    // Tick 0: a held key (W is bound to MainDirectionUp by default).
    tw.apply_tick_input(
        &TickInput {
            tick: 0,
            samples: vec![key_down],
            ..Default::default()
        },
        DT,
    );

    // Tick 1: a genuinely empty tick -- nothing sim-visible landed.
    tw.apply_tick_input(
        &TickInput {
            tick: 1,
            ..Default::default()
        },
        DT,
    );

    // Tick 2: a queued SignalIntent.
    tw.apply_tick_input(
        &TickInput {
            tick: 2,
            intents: vec![SignalIntent::SetFlag("tick_input_round_trip".into())],
            ..Default::default()
        },
        DT,
    );

    // Tick 3: a ScreenSize change.
    tw.apply_tick_input(
        &TickInput {
            tick: 3,
            screen_size: Some((800, 600)),
            ..Default::default()
        },
        DT,
    );

    let up_active = tw.world.resource::<InputState>().maindirection_up.active;
    let flagged = tw
        .world
        .resource::<WorldSignals>()
        .has_flag("tick_input_round_trip");
    let screen = tw.world.resource::<ScreenSize>();
    let screen_size = (screen.w, screen.h);

    TickInputScenarioResult {
        up_active,
        flagged,
        screen_size,
    }
}

#[test]
fn round_trip_tick_input_sequence_produces_identical_state_across_two_runs() {
    let run1 = run_tick_input_scenario(42);
    let run2 = run_tick_input_scenario(42);

    assert!(
        run1.up_active,
        "the held key from tick 0 must still read active after the \
         intervening empty/intent/screen-size ticks -- resolve_input_backlog \
         holds previous state across an empty tick rather than resetting it"
    );
    assert!(
        run1.flagged,
        "the queued SignalIntent must have been applied"
    );
    assert_eq!(run1.screen_size, (800, 600));
    assert_eq!(
        run1, run2,
        "driving two independent TestWorlds through the identical scripted \
         TickInput sequence must produce bit-identical observable state -- \
         this is the round-trip guarantee determinism-04-tick-input.md exists \
         to provide (a real Recorder/replay format is 05's job; the TickInput \
         values themselves are already the loss-free record)"
    );
}

#[test]
fn empty_tick_input_holds_previous_state_and_fires_no_new_edges() {
    let mut tw = TestWorld::new();

    let mut key_down = RawDeviceSnapshot::default();
    key_down.set_key(KeyboardKey::KEY_W as u32);
    tw.apply_tick_input(
        &TickInput {
            tick: 0,
            samples: vec![key_down],
            ..Default::default()
        },
        DT,
    );
    // `TestWorld::apply_tick_input` runs `run_sim_tick` (which clears
    // just_pressed/just_released) before returning, same as
    // `logic_thread_main` -- so `just_pressed` is a within-tick-only signal,
    // not observable from outside a completed call; only `active` (the held
    // state) survives to be checked here.
    assert!(
        tw.world.resource::<InputState>().maindirection_up.active,
        "the pressed key must read active after the tick it lands"
    );

    // A genuinely empty TickInput: no samples, no capture, no intents, no
    // screen_size change -- must hold InputState.active as-is (the key is
    // still physically down).
    tw.apply_tick_input(
        &TickInput {
            tick: 1,
            ..Default::default()
        },
        DT,
    );

    let up = tw.world.resource::<InputState>().maindirection_up;
    assert!(
        up.active,
        "an empty tick must hold the previous tick's active state, not reset it"
    );
    assert!(
        !up.just_pressed,
        "an empty tick must not fire a new just_pressed edge -- \
         resolve_input_backlog early-returns on an empty sample slice, and \
         the previous tick's edge was already cleared by clear_edges"
    );
}
