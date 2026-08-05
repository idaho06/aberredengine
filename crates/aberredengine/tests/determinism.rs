//! Determinism regression guard (determinism-02-deterministic-schedule.md),
//! plus the phase 05 (replays) verification harness
//! (determinism-05-replays.md): state hash double-run/divergence tests, the
//! replay recorder/player round trip, codec bit-exactness, empty-tick RLE,
//! header validation, and a golden hash regression test.
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

// --- determinism-05-replays.md: state hash + replay recorder/player -------

use aberredengine::EngineError;
use aberredengine::protocol::replay::{
    REPLAY_FORMAT_VERSION, REPLAY_MAGIC, ReplayHeader, config_digest,
};
use aberredengine::resources::gameconfig::GameConfig;
use aberredengine::test_support::{
    hash_world_state, read_tick_inputs_from_replay, validate_replay_header,
    write_tick_inputs_to_replay,
};

/// A short scripted `TickInput` sequence exercising a `SignalIntent` on
/// tick 1 (identical every call) and, depending on `flag_on_tick_2`, either
/// a second intent or nothing on tick 2 -- the fork point
/// `divergent_input_diverges_hash_trail` checks against.
fn scripted_ticks(flag_on_tick_2: bool) -> Vec<TickInput> {
    vec![
        TickInput {
            tick: 0,
            ..Default::default()
        },
        TickInput {
            tick: 1,
            intents: vec![SignalIntent::SetFlag("a".into())],
            ..Default::default()
        },
        TickInput {
            tick: 2,
            intents: if flag_on_tick_2 {
                vec![SignalIntent::SetFlag("b".into())]
            } else {
                Vec::new()
            },
            ..Default::default()
        },
        TickInput {
            tick: 3,
            ..Default::default()
        },
    ]
}

/// Drive a fresh, seeded `TestWorld` through `ticks`, collecting
/// `hash_world_state` after each applied tick.
fn run_hash_trail(seed: u64, ticks: &[TickInput]) -> Vec<u64> {
    let mut tw = TestWorldBuilder::new()
        .deterministic(seed)
        .build()
        .expect("build should succeed");
    let mut trail = Vec::with_capacity(ticks.len());
    for ti in ticks {
        tw.apply_tick_input(ti, DT);
        trail.push(hash_world_state(&tw.world));
    }
    trail
}

#[test]
fn double_run_same_seed_same_hash_trail() {
    let ticks = scripted_ticks(true);
    let trail1 = run_hash_trail(7, &ticks);
    let trail2 = run_hash_trail(7, &ticks);
    assert_eq!(
        trail1, trail2,
        "same seed + same TickInput sequence must produce an identical hash \
         trail -- a hash that silently omits a mutated field would still \
         pass this test only by accident, but a hash that isn't a pure \
         function of world state would fail it"
    );
}

#[test]
fn divergent_input_diverges_hash_trail() {
    // Negative control: without this test, a hash function that returns a
    // constant would pass the double-run test above and prove nothing.
    let trail1 = run_hash_trail(7, &scripted_ticks(true));
    let trail2 = run_hash_trail(7, &scripted_ticks(false));

    assert_eq!(
        trail1[0], trail2[0],
        "ticks before the fork must still match"
    );
    assert_eq!(
        trail1[1], trail2[1],
        "ticks before the fork must still match"
    );
    assert_ne!(
        trail1[2], trail2[2],
        "the tick where input differs (a SignalIntent present in one run, \
         absent in the other) must diverge -- proves the hash actually \
         covers WorldSignals"
    );
    assert_ne!(
        trail1[3], trail2[3],
        "divergence must persist on later ticks (the flag stays set)"
    );
}

fn temp_replay_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "aberredengine_test_{name}_{}.replay",
        std::process::id()
    ))
}

fn placeholder_header(seed: u64) -> ReplayHeader {
    ReplayHeader {
        magic: REPLAY_MAGIC,
        format_version: REPLAY_FORMAT_VERSION,
        engine_build_id: "test".into(),
        seed,
        sim_hz: 240.0,
        config_digest: 0,
        scene_id: String::new(),
        game_version: "test".into(),
    }
}

#[test]
fn record_replay_roundtrip_matches_live_hash_trail() {
    let ticks = scripted_ticks(true);
    let live_trail = run_hash_trail(9, &ticks);

    let path = temp_replay_path("roundtrip");
    write_tick_inputs_to_replay(&path, placeholder_header(9), &ticks)
        .expect("write_tick_inputs_to_replay should succeed");
    let (_header, read_ticks) = read_tick_inputs_from_replay(&path, ticks.len());
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        read_ticks, ticks,
        "TickInput sequence must round-trip through the replay file \
         field-for-field"
    );

    let replay_trail = run_hash_trail(9, &read_ticks);
    assert_eq!(
        live_trail, replay_trail,
        "replaying the round-tripped TickInput sequence must reproduce the \
         exact same hash trail as the original live run"
    );
}

#[test]
fn codec_roundtrip_preserves_f32_bits() {
    let sample = RawDeviceSnapshot {
        mouse_x: f32::NAN,
        mouse_y: -0.0f32,
        scroll_y: f32::MIN_POSITIVE / 2.0, // subnormal
        ..Default::default()
    };
    let ti = TickInput {
        tick: 5,
        samples: vec![sample],
        ..Default::default()
    };

    let bytes = postcard::to_allocvec(&ti).expect("postcard encode should succeed");
    let decoded: TickInput = postcard::from_bytes(&bytes).expect("postcard decode should succeed");

    let orig = &ti.samples[0];
    let round = &decoded.samples[0];
    assert_eq!(
        orig.mouse_x.to_bits(),
        round.mouse_x.to_bits(),
        "NaN payload/sign must round-trip bit-exact, not just compare equal \
         under IEEE 754 rules (NaN != NaN)"
    );
    assert_eq!(
        orig.mouse_y.to_bits(),
        round.mouse_y.to_bits(),
        "-0.0 must round-trip distinct from +0.0"
    );
    assert_eq!(
        orig.scroll_y.to_bits(),
        round.scroll_y.to_bits(),
        "a subnormal value must round-trip bit-exact"
    );
}

#[test]
fn empty_tick_run_length_roundtrip() {
    let ticks: Vec<TickInput> = (0..10_000u64)
        .map(|tick| TickInput {
            tick,
            ..Default::default()
        })
        .collect();

    let path = temp_replay_path("empty_run");
    write_tick_inputs_to_replay(&path, placeholder_header(1), &ticks)
        .expect("write_tick_inputs_to_replay should succeed");
    let (_header, read_ticks) = read_tick_inputs_from_replay(&path, ticks.len());
    let size = std::fs::metadata(&path)
        .expect("replay file should exist")
        .len();
    let _ = std::fs::remove_file(&path);

    assert_eq!(read_ticks.len(), 10_000);
    assert!(read_ticks.iter().all(TickInput::is_empty));
    assert!(
        size < 200,
        "10,000 empty ticks must compress to a handful of small entries via \
         run-length encoding, not one entry per tick -- file was {size} bytes"
    );
}

fn valid_header_for(config: &GameConfig) -> ReplayHeader {
    ReplayHeader {
        magic: REPLAY_MAGIC,
        format_version: REPLAY_FORMAT_VERSION,
        engine_build_id: "test".into(),
        seed: 1,
        sim_hz: config.sim_hz,
        config_digest: config_digest(config),
        scene_id: String::new(),
        game_version: "test".into(),
    }
}

#[test]
fn replay_refuses_sim_hz_mismatch() {
    let config = GameConfig::new();
    let mut header = valid_header_for(&config);
    header.sim_hz += 1.0;
    assert!(matches!(
        validate_replay_header(&header, &config),
        Err(EngineError::ReplaySimHzMismatch { .. })
    ));
}

#[test]
fn replay_refuses_format_version_mismatch() {
    let config = GameConfig::new();
    let mut header = valid_header_for(&config);
    header.format_version += 1;
    assert!(matches!(
        validate_replay_header(&header, &config),
        Err(EngineError::ReplayVersionMismatch { .. })
    ));
}

#[test]
fn replay_refuses_config_digest_mismatch() {
    let config = GameConfig::new();
    let mut header = valid_header_for(&config);
    header.config_digest ^= 1;
    assert!(matches!(
        validate_replay_header(&header, &config),
        Err(EngineError::ReplayConfigMismatch { .. })
    ));
}

#[test]
fn replay_accepts_matching_header() {
    let config = GameConfig::new();
    let header = valid_header_for(&config);
    assert!(validate_replay_header(&header, &config).is_ok());
}

/// Final `hash_world_state` after driving a small, fixed Rust-only scenario
/// (one spawned entity plus [`scripted_ticks`]) through a `TestWorld` seeded
/// with a fixed seed.
fn golden_scenario_final_hash(seed: u64) -> u64 {
    let mut tw = TestWorldBuilder::new()
        .deterministic(seed)
        .build()
        .expect("build should succeed");
    tw.world.spawn((
        Group::new("golden"),
        MapPosition::new(1.0, 2.0),
        BoxCollider::new(4.0, 4.0),
    ));

    let mut last = 0u64;
    for ti in &scripted_ticks(true) {
        tw.apply_tick_input(ti, DT);
        last = hash_world_state(&tw.world);
    }
    last
}

#[test]
fn golden_replay_rust_scene_matches_checked_in_trail() {
    // Golden value pinned against current sim/hash behavior -- this is the
    // CI regression net determinism-05-replays.md asks for: any change to
    // the hashed component/resource list, or any sim-behavior change that
    // affects a hashed field, changes this value. That's the point -- it
    // forces a conscious decision (update GOLDEN_HASH, and consider
    // bumping REPLAY_FORMAT_VERSION if old replay files would now diverge)
    // instead of a silent regression.
    //
    // Bumped for lua-refactor phase 04: registering
    // `rebuild_collision_rule_index` in the sim schedule's `SimSet::Collision`
    // (`.before(collision_detector)`) changes the deterministic
    // single-threaded executor's per-tick system sequence, even though this
    // scenario spawns no `CollisionRule` entity for the new system to act on.
    const GOLDEN_HASH: u64 = 0x9f71_4dc6_36ff_e0a3;
    let actual = golden_scenario_final_hash(42);
    assert_eq!(
        actual, GOLDEN_HASH,
        "golden hash changed -- if this is an intentional sim/hash change, \
         update GOLDEN_HASH to {actual:#x}"
    );
}
