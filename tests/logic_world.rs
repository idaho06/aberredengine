//! Consumer tests for `aberredengine::test_support::TestWorld`.
//!
//! These exercise the REAL logic-thread `World`/`sim`/`present` schedules
//! headlessly -- see `src/test_support.rs`'s module doc for what's real vs.
//! stubbed. Each test targets one seam: input edge resolution,
//! spawn/collision, snapshot build, and the async font-metrics contract.

#![cfg(feature = "test-support")]

use aberredengine::bevy_ecs::prelude::*;
use aberredengine::components::boxcollider::BoxCollider;
use aberredengine::components::collision::{BoxSides, CollisionRule};
use aberredengine::components::dynamictext::DynamicText;
use aberredengine::components::group::Group;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::sprite::Sprite;
use aberredengine::components::zindex::ZIndex;
use aberredengine::engine_app::SimSet;
use aberredengine::protocol::raw_input::RawDeviceSnapshot;
use aberredengine::protocol::render_assets::RenderAssetCmd;
use aberredengine::protocol::render_logic::RenderMsg;
use aberredengine::raylib::ffi::KeyboardKey;
use aberredengine::resources::fontmetrics::{FontMetrics, GlyphMetrics};
use aberredengine::resources::gamestate::{GameState, GameStates, NextGameState};
use aberredengine::resources::input::InputState;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::systems::game_ctx::GameCtx;
use aberredengine::test_support::TestWorld;
use raylib::prelude::Color;
use rustc_hash::FxHashMap;

const DT: f32 = 1.0 / 60.0;

/// (a) Input pipeline: a synthetic key-down sample resolves into exactly one
/// `just_pressed` edge, consumed by the tick that observes it.
#[test]
fn input_edge_fires_exactly_once_and_clears() {
    let mut tw = TestWorld::new();

    let mut raw = RawDeviceSnapshot {
        window_w: 800,
        window_h: 600,
        ..Default::default()
    };
    raw.set_key(KeyboardKey::KEY_SPACE as u32);
    tw.send_input(raw);

    {
        let input = tw.world.resource::<InputState>();
        assert!(
            input.action_1.just_pressed,
            "edge must be visible before the tick consumes it"
        );
    }

    tw.tick(1, DT);
    {
        let input = tw.world.resource::<InputState>();
        assert!(input.action_1.active, "held state must survive the tick");
        assert!(
            !input.action_1.just_pressed,
            "edge must be consumed (cleared) after one sim tick observes it"
        );
    }

    // A second tick with no new input must not resurrect the edge.
    tw.tick(1, DT);
    let input = tw.world.resource::<InputState>();
    assert!(!input.action_1.just_pressed);
    assert!(input.action_1.active, "still held");
}

fn collision_bump_flag(_a: Entity, _b: Entity, _sa: &BoxSides, _sb: &BoxSides, ctx: &mut GameCtx) {
    ctx.world_signals.set_flag("collided");
}

/// (b) Spawn two overlapping entities in matching groups with a Rust
/// collision rule; tick until overlap is detected and the rule's callback's
/// `WorldSignals` side-effect lands.
#[test]
fn spawn_and_collide_fires_rust_collision_rule() {
    let mut tw = TestWorld::new();
    tw.tick_to_play(DT, 8);

    tw.world
        .spawn(CollisionRule::rust("a", "b", collision_bump_flag));
    tw.world.spawn((
        Group::new("a"),
        MapPosition::new(0.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    tw.world.spawn((
        Group::new("b"),
        MapPosition::new(2.0, 2.0),
        BoxCollider::new(10.0, 10.0),
    ));

    tw.tick(1, DT);

    assert!(
        tw.world.resource::<WorldSignals>().has_flag("collided"),
        "overlapping colliders in matching groups must fire the Rust collision rule"
    );
}

/// (c) Spawn a map sprite; `present()` and assert it appears in the
/// published snapshot's `map_sprites` with the right position/z.
#[test]
fn present_publishes_spawned_sprite_in_snapshot() {
    let mut tw = TestWorld::new();

    let sprite = Sprite {
        tex_key: "player".into(),
        width: 16.0,
        height: 16.0,
        offset: Default::default(),
        origin: Default::default(),
        flip_h: false,
        flip_v: false,
    };
    let entity = tw
        .world
        .spawn((sprite, MapPosition::new(3.0, 4.0), ZIndex(2.0)))
        .id();

    let snapshot = tw.present();

    let entry = snapshot
        .map_sprites
        .iter()
        .find(|e| e.entity == entity)
        .expect("spawned sprite must appear in the published snapshot");
    assert_eq!(entry.position.pos.x, 3.0);
    assert_eq!(entry.position.pos.y, 4.0);
    assert_eq!(entry.z_index.0, 2.0);
    assert_eq!(entry.sprite.tex_key.as_ref(), "player");
}

/// (d) Asset round-trip: queue a font load, assert it lands on
/// `sent_to_render`; then fake the async metrics reply and assert
/// `dynamictext_size_system` reacts on the very next tick (locks the async
/// `FontMetricsStore` contract documented in CLAUDE.md).
#[test]
fn font_load_forwards_and_metrics_reply_sizes_text_next_tick() {
    let mut tw = TestWorld::new();

    tw.world
        .resource_mut::<Messages<RenderAssetCmd>>()
        .write(RenderAssetCmd::Font {
            id: "test_font".to_string(),
            path: "assets/fonts/does_not_matter.ttf".to_string(),
            size: 20,
            skip_if_loaded: false,
        });

    let text = tw
        .world
        .spawn(DynamicText::new("a", "test_font", 20.0, Color::WHITE))
        .id();

    tw.tick(1, DT);

    let forwarded = tw.sent_to_render.try_recv().expect(
        "queued RenderAssetCmd::Font must be forwarded as RenderMsg::Asset within one tick",
    );
    match forwarded {
        RenderMsg::Asset(RenderAssetCmd::Font { id, .. }) => assert_eq!(id, "test_font"),
        other => panic!("expected RenderMsg::Asset(Font), got {other:?}"),
    }

    // Metrics haven't arrived yet: size stays at its zero default.
    let size_before = tw.world.get::<DynamicText>(text).unwrap().size();
    assert_eq!(size_before.x, 0.0);
    assert_eq!(size_before.y, 0.0);

    let mut glyphs = FxHashMap::default();
    glyphs.insert(
        'a' as i32,
        GlyphMetrics {
            advance_x: 10,
            offset_x: 0,
            rec_width: 10.0,
        },
    );
    tw.deliver_font_metrics("test_font", FontMetrics::new(10, glyphs));

    // `dynamictext_size_system` only re-measures on `Changed<DynamicText>` --
    // metrics arriving alone doesn't retroactively resize an entity that
    // hasn't been touched since. A legitimate re-touch (here, `set_text`)
    // is what triggers the retry once metrics are available.
    tw.world.get_mut::<DynamicText>(text).unwrap().set_text("a");

    tw.tick(1, DT);

    let size_after = tw.world.get::<DynamicText>(text).unwrap().size();
    assert_eq!(
        size_after.x, 20.0,
        "advance_x(10) * scale(20/10) for a single 'a'"
    );
    assert_eq!(
        size_after.y, 20.0,
        "text_height == font_size for single-line text"
    );
}

#[derive(Resource, Default)]
struct Counter(u32);

fn increment_counter(mut counter: ResMut<Counter>) {
    counter.0 += 1;
}

/// A `setup` hook that does NOT auto-transition to `Playing` (unlike
/// [`TestWorld`]'s default), so the test controls exactly when `Playing` is
/// reached instead of guessing at the default hook's transition timing.
fn setup_without_auto_play() {}

/// (e) `TestWorldBuilder::add_system`: the registered system must be gated
/// by `run_if(state_is_playing)` through the harness, exactly like
/// production's `EngineBuilder::add_system` -- it must not run before
/// `Playing`, and must run exactly once per tick once `Playing` is reached.
#[test]
fn add_system_runs_once_per_tick_only_while_playing() {
    let mut tw = TestWorld::builder()
        .on_setup(setup_without_auto_play)
        .add_system(increment_counter)
        .build()
        .expect("build should succeed");
    tw.world.insert_resource(Counter::default());

    tw.tick(3, DT);
    assert!(
        !matches!(tw.world.resource::<GameState>().get(), GameStates::Playing),
        "setup_without_auto_play must never request a transition to Playing"
    );
    assert_eq!(
        tw.world.resource::<Counter>().0,
        0,
        "add_system's run_if(state_is_playing) must suppress the system before Playing"
    );

    tw.world
        .resource_mut::<NextGameState>()
        .set(GameStates::Playing);
    tw.tick_to_play(DT, 8);
    let count_at_play = tw.world.resource::<Counter>().0;

    tw.tick(1, DT);
    assert_eq!(
        tw.world.resource::<Counter>().0,
        count_at_play + 1,
        "system must run exactly once per tick once Playing"
    );
}

#[derive(Resource, Default)]
struct OrderLog(Vec<&'static str>);

fn log_movement(mut log: ResMut<OrderLog>) {
    log.0.push("movement");
}

fn log_collision(mut log: ResMut<OrderLog>) {
    log.0.push("collision");
}

/// (f) `TestWorldBuilder::configure_schedule`: a system placed in
/// `SimSet::Movement` must run before one placed in `SimSet::Collision`,
/// proving `SimSet` placement resolves through the harness's real `sim`
/// schedule (not just that the registrar closures were invoked).
#[test]
fn configure_schedule_respects_simset_ordering() {
    let mut tw = TestWorld::builder()
        .configure_schedule(|schedule| {
            schedule.add_systems(log_movement.in_set(SimSet::Movement));
        })
        .configure_schedule(|schedule| {
            schedule.add_systems(log_collision.in_set(SimSet::Collision));
        })
        .build()
        .expect("build should succeed");
    tw.world.insert_resource(OrderLog::default());

    tw.tick(1, DT);

    assert_eq!(
        tw.world.resource::<OrderLog>().0,
        vec!["movement", "collision"],
        "SimSet::Movement must run before SimSet::Collision"
    );
}

#[derive(Event, Debug, Clone, Copy)]
struct HarnessProbeEvent;

fn on_harness_probe_event(_trigger: On<HarnessProbeEvent>, mut signals: ResMut<WorldSignals>) {
    signals.set_flag("observer_fired");
}

fn trigger_harness_probe_event(mut commands: Commands) {
    commands.trigger(HarnessProbeEvent);
}

/// (g) `TestWorldBuilder::add_observer`: a persistent observer registered
/// through the harness must fire on a triggered event, same as production's
/// `EngineBuilder::add_observer`.
#[test]
fn add_observer_fires_on_triggered_event() {
    let mut tw = TestWorld::builder()
        .add_observer(on_harness_probe_event)
        .add_system(trigger_harness_probe_event)
        .build()
        .expect("build should succeed");

    tw.tick_to_play(DT, 8);
    tw.tick(1, DT);

    assert!(
        tw.world
            .resource::<WorldSignals>()
            .has_flag("observer_fired"),
        "observer registered via TestWorldBuilder::add_observer must fire on the triggered event"
    );
}
