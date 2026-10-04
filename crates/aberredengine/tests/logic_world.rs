//! Consumer tests for `aberredengine::test_support::TestWorld`.
//!
//! These exercise the REAL logic-thread `World`/`sim`/`present` schedules
//! headlessly -- see `src/test_support.rs`'s module doc for what's real vs.
//! stubbed. Each test targets one seam: input edge resolution,
//! spawn/collision, snapshot build, and the async font-metrics contract.

#![cfg(feature = "test-support")]

use aberredengine::bevy_ecs::prelude::*;
use aberredengine::core::components::boxcollider::BoxCollider;
use aberredengine::core::components::collision::{BoxSides, CollisionRule};
use aberredengine::core::components::dynamictext::DynamicText;
use aberredengine::core::components::group::Group;
use aberredengine::core::components::mapposition::MapPosition;
use aberredengine::core::components::phase::Phase;
use aberredengine::core::components::scene::SceneName;
use aberredengine::core::components::sprite::Sprite;
use aberredengine::core::components::timer::Timer;
use aberredengine::core::components::zindex::ZIndex;
use aberredengine::core::events::asset::{AssetLoadFailed, AssetLoaded};
use aberredengine::core::events::input::InputAction;
use aberredengine::core::events::phase::{PhaseEntered, PhaseExited};
use aberredengine::core::events::scene::{SceneEntered, SceneExited};
use aberredengine::core::events::timer::TimerFired;
use aberredengine::core::math::Color;
use aberredengine::core::protocol::asset_kind::AssetKind;
use aberredengine::core::protocol::audio::AudioMessage;
use aberredengine::core::protocol::raw_input::RawDeviceSnapshot;
use aberredengine::core::protocol::render_assets::RenderAssetCmd;
use aberredengine::core::protocol::render_logic::{LogicMsg, RenderMsg};
use aberredengine::core::protocol::tick_input::TickInput;
use aberredengine::core::resources::fontmetrics::{FontMetrics, GlyphMetrics};
use aberredengine::core::resources::gamestate::{GameState, GameStates, NextGameState};
use aberredengine::core::resources::input::InputState;
use aberredengine::core::resources::loaded_assets::LoadedAssets;
use aberredengine::core::resources::pending_assets::PendingAssets;
use aberredengine::core::resources::signal_intents::SignalIntent;
use aberredengine::core::resources::signal_keys as sk;
use aberredengine::core::resources::worldsignals::WorldSignals;
use aberredengine::core::systems::GameCtx;
use aberredengine::core::systems::asset_loader::AssetLoader;
use aberredengine::core::systems::scene_dispatch::in_scene;
use aberredengine::engine_app::SimSet;
use aberredengine::raylib::ffi::KeyboardKey;
use aberredengine::test_support::{TestWorld, TestWorldBuilder};
use rustc_hash::FxHashMap;

mod common;
use common::DT;

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
            input.action(InputAction::Action1).just_pressed,
            "edge must be visible before the tick consumes it"
        );
    }

    tw.tick(1, DT);
    {
        let input = tw.world.resource::<InputState>();
        assert!(
            input.action(InputAction::Action1).active,
            "held state must survive the tick"
        );
        assert!(
            !input.action(InputAction::Action1).just_pressed,
            "edge must be consumed (cleared) after one sim tick observes it"
        );
    }

    // A second tick with no new input must not resurrect the edge.
    tw.tick(1, DT);
    let input = tw.world.resource::<InputState>();
    assert!(!input.action(InputAction::Action1).just_pressed);
    assert!(input.action(InputAction::Action1).active, "still held");
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

/// A `Timer` ticked by the real sim schedule triggers `TimerFired` on its
/// entity on the tick that reaches `duration`, and not before.
#[test]
fn timer_fires_timer_fired_through_sim_schedule() {
    let mut tw = TestWorld::new();
    tw.tick_to_play(DT, 8);

    tw.world.spawn(Timer::new(DT * 2.5)).observe(
        |_: On<TimerFired>, mut signals: ResMut<WorldSignals>| {
            signals.set_flag("timer_fired");
        },
    );

    tw.tick(2, DT);
    assert!(
        !tw.world.resource::<WorldSignals>().has_flag("timer_fired"),
        "elapsed 2*DT < 2.5*DT: the timer must not fire yet"
    );

    tw.tick(1, DT);
    assert!(
        tw.world.resource::<WorldSignals>().has_flag("timer_fired"),
        "elapsed 3*DT >= 2.5*DT: TimerFired must reach the entity's observer"
    );
}

#[derive(Resource, Default)]
struct RequestRun(bool);

/// Requests `idle -> run` on every `Phase` once `RequestRun` is set.
fn request_run(mut request: ResMut<RequestRun>, mut phases: Query<&mut Phase>) {
    if std::mem::take(&mut request.0) {
        for mut phase in &mut phases {
            phase.next = Some("run".into());
        }
    }
}

#[derive(Resource, Default)]
struct PhaseLog(Vec<String>);

fn log_phase_entered(ev: On<PhaseEntered>, mut log: ResMut<PhaseLog>) {
    log.0.push(format!("entered:{}", ev.name));
}

fn log_phase_exited(ev: On<PhaseExited>, mut log: ResMut<PhaseLog>) {
    log.0.push(format!("exited:{}", ev.name));
}

fn take_phase_log(tw: &mut TestWorld) -> Vec<String> {
    std::mem::take(&mut tw.world.resource_mut::<PhaseLog>().0)
}

/// A transition requested by a system in `SimSet::ScriptUpdate` at tick N is
/// applied, and its events fire, at tick N+1, never at N.
#[test]
fn phase_transition_requested_in_script_update_applies_next_tick() {
    let mut tw = TestWorld::builder()
        .add_system(request_run)
        .add_observer(log_phase_entered)
        .add_observer(log_phase_exited)
        .build()
        .expect("build should succeed");
    tw.world.init_resource::<RequestRun>();
    tw.world.init_resource::<PhaseLog>();
    tw.tick_to_play(DT, 8);

    let entity = tw.world.spawn(Phase::new("idle")).id();
    tw.tick(1, DT);
    assert_eq!(take_phase_log(&mut tw), ["entered:idle"]);

    tw.world.resource_mut::<RequestRun>().0 = true;
    tw.tick(1, DT);
    assert!(
        take_phase_log(&mut tw).is_empty(),
        "tick N: the request must not apply in the tick that made it"
    );
    let phase = tw.world.get::<Phase>(entity).unwrap();
    assert_eq!(phase.current, "idle");
    assert_eq!(phase.next.as_deref(), Some("run"));

    tw.tick(1, DT);
    assert_eq!(take_phase_log(&mut tw), ["exited:idle", "entered:run"]);
    assert_eq!(tw.world.get::<Phase>(entity).unwrap().current, "run");
}

/// A transition requested by an observer (here of `TimerFired`, triggered
/// late in the tick) also applies at the start of the next tick.
#[test]
fn phase_transition_requested_by_observer_applies_next_tick() {
    let mut tw = TestWorld::builder()
        .add_observer(log_phase_entered)
        .add_observer(log_phase_exited)
        .build()
        .expect("build should succeed");
    tw.world.init_resource::<PhaseLog>();
    tw.tick_to_play(DT, 8);

    let entity = tw
        .world
        .spawn((Phase::new("idle"), Timer::once(DT * 0.5)))
        .observe(|ev: On<TimerFired>, mut phases: Query<&mut Phase>| {
            phases.get_mut(ev.entity).unwrap().next = Some("run".into());
        })
        .id();

    tw.tick(1, DT);
    assert_eq!(take_phase_log(&mut tw), ["entered:idle"]);
    assert_eq!(
        tw.world.get::<Phase>(entity).unwrap().next.as_deref(),
        Some("run"),
        "the timer fired this tick and requested the transition"
    );

    tw.tick(1, DT);
    assert_eq!(take_phase_log(&mut tw), ["exited:idle", "entered:run"]);
}

/// (c) Spawn a map sprite; `present()` and assert it appears in the
/// published snapshot's `map_sprites` with the right position/z.
#[test]
fn present_publishes_spawned_sprite_in_snapshot() {
    let mut tw = TestWorld::new();

    let sprite = Sprite::new("player", 16.0, 16.0);
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
/// `FontMetricsStore` contract: metrics arrive after the load request, and
/// text sizing picks them up once they land).
#[test]
fn font_load_forwards_and_metrics_reply_sizes_text_next_tick() {
    let mut tw = TestWorld::new();

    tw.world
        .resource_mut::<Messages<RenderAssetCmd>>()
        .write(RenderAssetCmd::Font {
            key: "test_font".to_string(),
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
        RenderMsg::Asset(RenderAssetCmd::Font { key, .. }) => assert_eq!(key, "test_font"),
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

/// A `setup` hook that requests no state: the engine moves to `Playing` on
/// its own once it has run.
fn setup_that_requests_nothing() {}

/// `.on_setup()` with a hook that never touches `NextGameState` still
/// reaches `Playing`, through the automatic Setup -> Playing transition.
#[test]
fn setup_hook_that_requests_nothing_reaches_playing() {
    let mut tw = TestWorld::builder()
        .on_setup(setup_that_requests_nothing)
        .build()
        .expect("build should succeed");
    tw.tick_to_play(DT, 8);
}

/// (e) `TestWorldBuilder::add_system`: the registered system must be gated
/// by `run_if(state_is_playing)` through the harness, exactly like
/// production's `EngineBuilder::add_system` -- it runs exactly once per tick
/// while `Playing`, and stops once the game leaves `Playing`.
#[test]
fn add_system_runs_once_per_tick_only_while_playing() {
    let mut tw = TestWorld::builder()
        .add_system(increment_counter)
        .build()
        .expect("build should succeed");
    tw.world.insert_resource(Counter::default());

    tw.tick_to_play(DT, 8);
    let count_at_play = tw.world.resource::<Counter>().0;
    tw.tick(1, DT);
    assert_eq!(
        tw.world.resource::<Counter>().0,
        count_at_play + 1,
        "system must run exactly once per tick once Playing"
    );

    tw.world
        .resource_mut::<NextGameState>()
        .set(GameStates::Quitting);
    tw.tick(1, DT);
    let count_at_quit = tw.world.resource::<Counter>().0;
    tw.tick(3, DT);
    assert!(matches!(
        tw.world.resource::<GameState>().get(),
        GameStates::Quitting
    ));
    assert_eq!(
        tw.world.resource::<Counter>().0,
        count_at_quit,
        "add_system's run_if(state_is_playing) must suppress the system outside Playing"
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

const INTENT_PROBE_FLAG: &str = "test:intent_probe";

/// What the `ScriptUpdate` probe saw on its most recent run.
#[derive(Resource, Default)]
struct IntentProbe(Option<bool>);

fn record_intent_flag(signals: Res<WorldSignals>, mut probe: ResMut<IntentProbe>) {
    probe.0 = Some(signals.has_flag(INTENT_PROBE_FLAG));
}

/// (h) `SignalIntents` timing: intents delivered with a tick's input are
/// drained by `apply_signal_intents` in `SimSet::ApplyIntents`, the first set
/// of the sim pipeline, so scene logic in `SimSet::ScriptUpdate` sees them on
/// that same tick -- not one tick later.
#[test]
fn signal_intent_is_visible_to_script_update_on_its_delivery_tick() {
    let mut tw = TestWorld::builder()
        .add_system(record_intent_flag)
        .build()
        .expect("build should succeed");
    tw.world.insert_resource(IntentProbe::default());
    tw.tick_to_play(DT, 8);

    tw.tick(1, DT);
    assert_eq!(
        tw.world.resource::<IntentProbe>().0,
        Some(false),
        "flag must be unset before the intent is delivered"
    );

    tw.apply_tick_input(
        &TickInput {
            intents: vec![SignalIntent::SetFlag(INTENT_PROBE_FLAG.into())],
            ..Default::default()
        },
        DT,
    );
    assert_eq!(
        tw.world.resource::<IntentProbe>().0,
        Some(true),
        "ScriptUpdate must see an intent on the tick it is delivered \
         (ApplyIntents runs before ScriptUpdate)"
    );
}

const DOOMED_KEY: &str = "test:doomed";

fn despawn_doomed(mut commands: Commands, signals: Res<WorldSignals>) {
    if let Some(entity) = signals.get_entity(DOOMED_KEY) {
        commands.entity(entity).despawn();
    }
}

/// A plain Rust despawn (no Lua `EntityCmd`) must not leave a `WorldSignals`
/// registration resolving to a dead entity past the end of the tick.
#[test]
fn rust_despawn_prunes_world_signals_registration_same_tick() {
    let mut tw = TestWorld::builder()
        .add_system(despawn_doomed)
        .build()
        .expect("build should succeed");
    tw.tick_to_play(DT, 8);

    let doomed = tw.world.spawn(MapPosition::new(0.0, 0.0)).id();
    let survivor = tw.world.spawn(MapPosition::new(0.0, 0.0)).id();
    {
        let mut signals = tw.world.resource_mut::<WorldSignals>();
        signals.set_entity(DOOMED_KEY, doomed);
        signals.set_entity("test:survivor", survivor);
    }

    tw.tick(1, DT);

    assert!(
        tw.world.get_entity(doomed).is_err(),
        "despawn must have applied"
    );
    let signals = tw.world.resource::<WorldSignals>();
    assert!(
        signals.get_entity(DOOMED_KEY).is_none(),
        "registration of an entity despawned this tick must be pruned by tick end"
    );
    assert_eq!(signals.get_entity("test:survivor"), Some(survivor));
}

/// The `quit_game` flag quits a Rust-only game (no Lua): set while
/// `Playing`, the next tick reaches `Quitting`, and `quit_game` runs exactly
/// once even though entering `Quitting` sets the flag again.
#[test]
fn quit_game_flag_quits_a_rust_only_game_exactly_once() {
    let mut tw = TestWorld::new();
    tw.tick_to_play(DT, 8);
    tw.world.resource_mut::<WorldSignals>().request_quit();

    tw.tick(5, DT);

    assert!(matches!(
        tw.world.resource::<GameState>().get(),
        GameStates::Quitting
    ));
    let quits = tw
        .sent_to_render
        .try_iter()
        .filter(|msg| matches!(msg, RenderMsg::Quit))
        .count();
    assert_eq!(quits, 1, "quit_game runs exactly once");
}

#[derive(EntityEvent)]
struct Ping {
    entity: Entity,
}

/// The SceneManager switch cleans up through `SceneCleanup`: the observer
/// of a `Persistent` entity survives, a scene entity's observer does not.
#[test]
fn scene_switch_keeps_observers_of_persistent_entities() {
    use aberredengine::core::components::persistent::Persistent;

    let mut tw = TestWorld::builder()
        .add_scene("a")
        .add_scene("b")
        .initial_scene("a")
        .build()
        .expect("build should succeed");
    tw.tick_to_play(DT, 8);

    tw.world.spawn(Persistent).observe(|_: On<Ping>| {});
    tw.world.spawn_empty().observe(|_: On<Ping>| {});
    tw.world.flush();
    let observer_count = |world: &mut World| world.query::<&Observer>().iter(world).count();
    let observers_before = observer_count(&mut tw.world);

    tw.world.resource_mut::<WorldSignals>().request_scene("b");
    tw.tick(2, DT);

    assert_eq!(
        observer_count(&mut tw.world),
        observers_before - 1,
        "only the scene entity's observer goes"
    );
}

/// A group tracked through `.track_group()` keeps its count published
/// across a SceneManager scene switch, which resets per-scene tracking.
#[test]
fn track_group_survives_a_scene_switch() {
    use aberredengine::core::components::persistent::Persistent;

    let mut tw = TestWorld::builder()
        .add_scene("a")
        .add_scene("b")
        .initial_scene("a")
        .track_group("enemies")
        .build()
        .expect("build should succeed");
    tw.tick_to_play(DT, 8);
    tw.world.spawn((Persistent, Group::new("enemies")));
    tw.tick(1, DT);
    let count = |tw: &TestWorld| {
        tw.world
            .resource::<WorldSignals>()
            .get_group_count("enemies")
    };
    assert_eq!(count(&tw), Some(1));

    tw.world.resource_mut::<WorldSignals>().request_scene("b");
    tw.tick(2, DT);

    assert_eq!(
        count(&tw),
        Some(1),
        "count is published again after the switch"
    );
}

#[derive(Resource, Default)]
struct SceneLog(Vec<String>);

fn log_scene_entered(ev: On<SceneEntered>, names: Query<&SceneName>, mut log: ResMut<SceneLog>) {
    let target = names.get(ev.scene).map(|n| &*n.0).unwrap_or("?");
    log.0.push(format!(
        "enter {} (entity {target}) from {:?}",
        ev.name,
        ev.previous.as_deref()
    ));
}

fn log_scene_exited(ev: On<SceneExited>, mut log: ResMut<SceneLog>) {
    log.0.push(format!("exit {} -> {}", ev.name, ev.next));
}

/// The real SceneManager wiring spawns a scene entity per scene and triggers
/// `SceneEntered` for the initial scene, then `SceneExited`/`SceneEntered` on
/// every switch.
#[test]
fn scene_switches_trigger_scene_events() {
    let mut tw = TestWorld::builder()
        .add_scene("a")
        .add_scene("b")
        .initial_scene("a")
        .add_observer(log_scene_entered)
        .add_observer(log_scene_exited)
        .build()
        .expect("build should succeed");
    tw.world.init_resource::<SceneLog>();
    tw.tick_to_play(DT, 8);

    tw.world.resource_mut::<WorldSignals>().request_scene("b");
    tw.tick(2, DT);

    assert_eq!(
        tw.world.resource::<SceneLog>().0,
        [
            "enter a (entity a) from None",
            "exit a -> b",
            "enter b (entity b) from Some(\"a\")",
        ]
    );
}

#[derive(Resource, Default)]
struct SceneTicks(u32);

fn count_scene_ticks(mut ticks: ResMut<SceneTicks>) {
    ticks.0 += 1;
}

/// Counts `count_scene_ticks` runs before and after switching from `a` to
/// `b`; `register` adds the system under test to a two-scene builder.
fn scene_ticks_before_and_after_switching_to_b(
    register: impl FnOnce(TestWorldBuilder) -> TestWorldBuilder,
) -> (u32, u32) {
    let builder = TestWorld::builder()
        .add_scene("a")
        .add_scene("b")
        .initial_scene("a");
    let mut tw = register(builder).build().expect("build should succeed");
    tw.world.init_resource::<SceneTicks>();
    tw.tick_to_play(DT, 8);
    tw.tick(3, DT);
    let before = tw.world.resource::<SceneTicks>().0;

    tw.world.resource_mut::<WorldSignals>().request_scene("b");
    tw.tick(1, DT); // the switch lands at the end of this tick
    tw.tick(3, DT);
    (before, tw.world.resource::<SceneTicks>().0)
}

/// A system gated on `in_scene("b")` runs only while `b` is active.
#[test]
fn in_scene_gates_a_system_to_its_scene() {
    let ticks = scene_ticks_before_and_after_switching_to_b(|builder| {
        builder.add_system_if(count_scene_ticks, in_scene("b"))
    });
    assert_eq!(ticks, (0, 3));
}

/// Scene conditions combine like any run condition.
#[test]
fn add_system_if_takes_combined_scene_conditions() {
    let (before, after) = scene_ticks_before_and_after_switching_to_b(|builder| {
        builder.add_system_if(count_scene_ticks, in_scene("a").or_else(in_scene("b")))
    });
    assert!(before >= 3, "runs in scene a: {before}");
    assert_eq!(
        after - before,
        4,
        "and keeps running across the switch to b"
    );
}

/// `add_scene_system("b", ..)` is the short form of the above.
#[test]
fn add_scene_system_runs_only_in_its_scene() {
    let ticks = scene_ticks_before_and_after_switching_to_b(|builder| {
        builder.add_scene_system("b", count_scene_ticks)
    });
    assert_eq!(ticks, (0, 3));
}

/// `on_scene_enter`/`on_scene_exit` observe one scene's entity: they fire
/// only for that scene, and keep firing on every later visit.
#[test]
fn scene_enter_and_exit_observers_fire_only_for_their_scene() {
    let mut tw = TestWorld::builder()
        .add_scene("a")
        .add_scene("b")
        .initial_scene("a")
        .on_scene_enter("b", |ev: On<SceneEntered>, mut log: ResMut<SceneLog>| {
            log.0.push(format!("enter {}", ev.name));
        })
        .on_scene_exit("b", |ev: On<SceneExited>, mut log: ResMut<SceneLog>| {
            log.0.push(format!("exit {} -> {}", ev.name, ev.next));
        })
        .build()
        .expect("build should succeed");
    tw.world.init_resource::<SceneLog>();
    tw.tick_to_play(DT, 8);

    for target in ["b", "a", "b"] {
        tw.world
            .resource_mut::<WorldSignals>()
            .request_scene(target);
        tw.tick(2, DT);
    }

    assert_eq!(
        tw.world.resource::<SceneLog>().0,
        ["enter b", "exit b -> a", "enter b"]
    );
}

/// A builder with neither scenes nor Lua enters an implicit `"main"` scene:
/// `SceneEntered` fires once and `"main"` scene systems run.
#[test]
fn a_builder_without_scenes_enters_the_main_scene() {
    let mut tw = TestWorld::builder()
        .add_observer(log_scene_entered)
        .add_scene_system(sk::MAIN_SCENE, count_scene_ticks)
        .build()
        .expect("build should succeed");
    tw.world.init_resource::<SceneLog>();
    tw.world.init_resource::<SceneTicks>();
    tw.tick_to_play(DT, 8);
    let before = tw.world.resource::<SceneTicks>().0;
    tw.tick(3, DT);

    assert_eq!(
        tw.world.resource::<SceneLog>().0,
        ["enter main (entity main) from None"]
    );
    assert_eq!(tw.world.resource::<SceneTicks>().0 - before, 3);
}

#[derive(Component)]
struct LoadingText;

/// A two-scene game with a `"loading"` scene shown while one texture loads.
fn loading_scene_world(extra: impl FnOnce(TestWorldBuilder) -> TestWorldBuilder) -> TestWorld {
    let builder = TestWorld::builder()
        .add_scene("loading")
        .add_scene("game")
        .initial_scene("game")
        .loading_scene("loading")
        .on_setup(
            |mut assets: AssetLoader, mut commands: Commands| -> Result {
                // The loading scene is entered right after this hook, during build().
                commands.init_resource::<SceneLog>();
                assets.load_texture("player", "player.png")?;
                Ok(())
            },
        )
        .add_observer(log_scene_entered)
        .add_observer(log_scene_exited)
        .on_scene_enter("loading", |_: On<SceneEntered>, mut commands: Commands| {
            commands.spawn(LoadingText);
        })
        .add_scene_system("loading", count_scene_ticks)
        .add_scene_system("game", increment_counter);
    let mut tw = extra(builder).build().expect("build should succeed");
    tw.world.init_resource::<SceneTicks>();
    tw.world.init_resource::<Counter>();
    tw
}

fn loading_texts(tw: &mut TestWorld) -> usize {
    tw.world
        .query_filtered::<(), With<LoadingText>>()
        .iter(&tw.world)
        .count()
}

/// The loading scene is active while Setup waits for assets: its systems run,
/// the initial scene's don't. Once the load is answered, the game leaves it
/// for the initial scene, tearing down what it spawned.
#[test]
fn loading_scene_is_active_while_setup_waits_for_assets() {
    let mut tw = loading_scene_world(|builder| builder);

    tw.tick(3, DT);
    assert_eq!(*tw.world.resource::<GameState>().get(), GameStates::Setup);
    assert_eq!(
        tw.world.resource::<SceneLog>().0,
        ["enter loading (entity loading) from None"]
    );
    assert!(
        tw.world.resource::<SceneTicks>().0 > 0,
        "loading system runs"
    );
    assert_eq!(tw.world.resource::<Counter>().0, 0, "game system idle");
    assert_eq!(loading_texts(&mut tw), 1);

    tw.deliver_texture_dims("player", 8, 8);
    tw.tick(2, DT);
    let loading_ticks = tw.world.resource::<SceneTicks>().0;
    tw.tick(2, DT);

    assert_eq!(*tw.world.resource::<GameState>().get(), GameStates::Playing);
    assert_eq!(
        tw.world.resource::<SceneLog>().0,
        [
            "enter loading (entity loading) from None",
            "exit loading -> game",
            "enter game (entity game) from Some(\"loading\")",
        ]
    );
    assert_eq!(
        tw.world.resource::<SceneTicks>().0,
        loading_ticks,
        "loading system stopped"
    );
    assert!(tw.world.resource::<Counter>().0 > 0, "game system runs");
    assert_eq!(
        loading_texts(&mut tw),
        0,
        "torn down with the loading scene"
    );
}

/// A scene switch requested from the loading scene doesn't fire again after
/// the engine leaves it: leaving the loading scene is the engine's job.
#[test]
fn a_switch_requested_during_loading_does_not_repeat_after_it() {
    let mut tw = loading_scene_world(|builder| {
        builder.add_scene_system("loading", |mut signals: ResMut<WorldSignals>| {
            signals.request_scene("game");
        })
    });

    tw.tick(2, DT);
    tw.deliver_texture_dims("player", 8, 8);
    tw.tick(4, DT);

    let entered_game = tw
        .world
        .resource::<SceneLog>()
        .0
        .iter()
        .filter(|line| line.starts_with("enter game"))
        .count();
    assert_eq!(entered_game, 1);
}

#[derive(Resource, Default)]
struct LoadLog(Vec<String>);

fn log_loaded(ev: On<AssetLoaded>, mut log: ResMut<LoadLog>) {
    log.0.push(format!("loaded {:?} {}", ev.kind, ev.key));
}

fn log_failed(ev: On<AssetLoadFailed>, mut log: ResMut<LoadLog>) {
    log.0
        .push(format!("failed {:?} {}: {}", ev.kind, ev.key, ev.error));
}

/// Loads queued through `AssetLoader` stay in `PendingAssets` until the
/// render or audio reply arrives; each reply settles one load and triggers
/// one `AssetLoaded`/`AssetLoadFailed`.
#[test]
fn load_replies_settle_pending_assets_and_trigger_events_once() {
    let mut tw = TestWorld::builder()
        .on_setup(|mut assets: AssetLoader| -> Result {
            assets.load_texture("player", "player.png")?;
            assets.load_font("arcade", "arcade.ttf", 8)?;
            assets.load_sound("jump", "jump.wav")?;
            Ok(())
        })
        .add_observer(log_loaded)
        .add_observer(log_failed)
        .build()
        .unwrap();
    tw.world.init_resource::<LoadLog>();

    tw.tick(2, DT);
    let pending = tw.world.resource::<PendingAssets>();
    assert_eq!(pending.len(), 3);
    assert!(pending.contains(AssetKind::Texture, "player"));
    assert!(pending.contains(AssetKind::Font, "arcade"));
    assert!(pending.contains(AssetKind::Sound, "jump"));

    tw.deliver_logic_msg(LogicMsg::TextureLoaded {
        key: "player".into(),
        width: 16,
        height: 16,
    });
    tw.deliver_logic_msg(LogicMsg::AssetLoadFailed {
        kind: AssetKind::Font,
        key: "arcade".into(),
        error: "missing".into(),
    });
    tw.audio_msgs_tx
        .send(AudioMessage::FxLoaded { id: "jump".into() })
        .unwrap();
    tw.tick(1, DT);

    assert!(tw.world.resource::<PendingAssets>().is_empty());
    let expected = [
        "loaded Texture player",
        "failed Font arcade: missing",
        "loaded Sound jump",
    ];
    assert_eq!(tw.world.resource::<LoadLog>().0, expected);

    tw.tick(3, DT);
    assert_eq!(
        tw.world.resource::<LoadLog>().0,
        expected,
        "events fire once"
    );
}

/// An `AssetLoaded` observer sees the reply's data already applied.
#[test]
fn asset_loaded_observers_see_the_texture_dims() {
    let mut tw = TestWorld::builder()
        .add_observer(
            |ev: On<AssetLoaded>, assets: AssetLoader, mut log: ResMut<LoadLog>| {
                log.0.push(format!("{:?}", assets.texture_size(&ev.key)));
            },
        )
        .build()
        .unwrap();
    tw.world.init_resource::<LoadLog>();

    tw.deliver_logic_msg(LogicMsg::TextureLoaded {
        key: "player".into(),
        width: 16,
        height: 24,
    });
    assert_eq!(tw.world.resource::<LoadLog>().0, ["Some((16, 24))"]);
}

fn state(tw: &TestWorld) -> GameStates {
    tw.world.resource::<GameState>().get().clone()
}

/// A `TestWorld` whose setup hook queues one texture and one sound, ticked
/// until both loads are pending.
fn loading_world() -> TestWorld {
    let mut tw = TestWorld::builder()
        .on_setup(|mut assets: AssetLoader| -> Result {
            assets.load_texture("player", "player.png")?;
            assets.load_sound("jump", "jump.wav")?;
            Ok(())
        })
        .build()
        .unwrap();
    tw.tick(4, DT);
    assert_eq!(tw.world.resource::<PendingAssets>().len(), 2);
    tw
}

fn texture_reply() -> LogicMsg {
    LogicMsg::TextureLoaded {
        key: "player".into(),
        width: 8,
        height: 8,
    }
}

/// Setup lasts while loads are pending and ends with the tick that settles
/// the last one; here the last reply is the texture's.
#[test]
fn setup_waits_for_loads_and_ends_with_the_last_render_reply() {
    let mut tw = loading_world();
    assert_eq!(state(&tw), GameStates::Setup);

    tw.audio_msgs_tx
        .send(AudioMessage::FxLoaded { id: "jump".into() })
        .unwrap();
    tw.tick(1, DT);
    assert_eq!(
        state(&tw),
        GameStates::Setup,
        "the texture is still loading"
    );

    tw.deliver_logic_msg(texture_reply());
    tw.tick(1, DT);
    assert_eq!(state(&tw), GameStates::Playing);
}

/// Same timing when the last reply is the sound's: audio replies settle in
/// `SimSet::AudioPump`, before `finish_setup` at the end of the tick.
#[test]
fn setup_ends_with_the_last_audio_reply_on_the_same_tick() {
    let mut tw = loading_world();
    tw.deliver_logic_msg(texture_reply());
    tw.tick(1, DT);
    assert_eq!(state(&tw), GameStates::Setup, "the sound is still loading");

    tw.audio_msgs_tx
        .send(AudioMessage::FxLoaded { id: "jump".into() })
        .unwrap();
    tw.tick(1, DT);
    assert_eq!(state(&tw), GameStates::Playing);
}

/// A failed load is answered too, so it doesn't hold Setup.
#[test]
fn a_failed_load_does_not_block_setup() {
    let mut tw = loading_world();
    tw.audio_msgs_tx
        .send(AudioMessage::FxLoadFailed {
            id: "jump".into(),
            error: "missing".into(),
        })
        .unwrap();
    tw.deliver_logic_msg(LogicMsg::AssetLoadFailed {
        kind: AssetKind::Texture,
        key: "player".into(),
        error: "missing".into(),
    });
    tw.tick(1, DT);
    assert_eq!(state(&tw), GameStates::Playing);
}

/// A setup hook that requests `Playing` itself waits for its loads too.
#[test]
fn a_setup_hook_requesting_playing_still_waits_for_its_loads() {
    let mut tw = TestWorld::builder()
        .on_setup(
            |mut assets: AssetLoader, mut next: ResMut<NextGameState>| -> Result {
                assets.load_texture("player", "player.png")?;
                next.set(GameStates::Playing);
                Ok(())
            },
        )
        .build()
        .unwrap();
    tw.tick(4, DT);
    assert_eq!(state(&tw), GameStates::Setup);

    tw.deliver_logic_msg(texture_reply());
    tw.tick(1, DT);
    assert_eq!(state(&tw), GameStates::Playing);
}

/// `LoadedAssets` follows the replies: successful loads add a key, failures
/// don't, and removals, renames and unloads take keys away.
#[test]
fn loaded_assets_follow_load_remove_rename_and_unload_replies() {
    let mut tw = TestWorld::new();
    let loaded =
        |tw: &TestWorld, kind, key| tw.world.resource::<LoadedAssets>().contains(kind, key);

    tw.deliver_texture_dims("player", 8, 8);
    tw.deliver_logic_msg(LogicMsg::ShaderLoaded { key: "glow".into() });
    tw.deliver_logic_msg(LogicMsg::AssetLoadFailed {
        kind: AssetKind::Font,
        key: "arcade".into(),
        error: "missing".into(),
    });
    assert!(loaded(&tw, AssetKind::Texture, "player"));
    assert!(loaded(&tw, AssetKind::Shader, "glow"));
    assert!(!loaded(&tw, AssetKind::Font, "arcade"));

    tw.deliver_logic_msg(LogicMsg::TextureRenamed {
        old_key: "player".into(),
        new_key: "hero".into(),
    });
    assert!(loaded(&tw, AssetKind::Texture, "hero"));
    tw.deliver_logic_msg(LogicMsg::TextureRemoved { key: "hero".into() });
    assert!(!loaded(&tw, AssetKind::Texture, "hero"));

    for msg in [
        AudioMessage::FxLoaded { id: "jump".into() },
        AudioMessage::FxLoaded { id: "coin".into() },
        AudioMessage::MusicLoaded { id: "bgm".into() },
    ] {
        tw.audio_msgs_tx.send(msg).unwrap();
    }
    tw.tick(1, DT);
    assert!(loaded(&tw, AssetKind::Sound, "jump"));
    assert!(loaded(&tw, AssetKind::Music, "bgm"));

    tw.audio_msgs_tx
        .send(AudioMessage::FxUnloaded { id: "jump".into() })
        .unwrap();
    tw.audio_msgs_tx
        .send(AudioMessage::MusicUnloadedAll)
        .unwrap();
    tw.tick(1, DT);
    assert!(!loaded(&tw, AssetKind::Sound, "jump"));
    assert!(loaded(&tw, AssetKind::Sound, "coin"));
    assert!(!loaded(&tw, AssetKind::Music, "bgm"));
}
/// Hooks and `add_system` systems may return bevy's `Result`, so they can
/// use `?`.
#[test]
fn hooks_and_systems_may_return_result() {
    let mut tw = TestWorld::builder()
        .on_setup(|mut signals: ResMut<WorldSignals>| -> Result {
            signals.set_flag("setup_ran");
            Ok(())
        })
        .add_system(|mut signals: ResMut<WorldSignals>| -> Result {
            signals.set_flag("system_ran");
            Ok(())
        })
        .build()
        .unwrap();
    tw.tick_to_play(DT, 4);
    tw.tick(1, DT);
    let signals = tw.world.resource::<WorldSignals>();
    assert!(signals.has_flag("setup_ran"));
    assert!(signals.has_flag("system_ran"));
}
