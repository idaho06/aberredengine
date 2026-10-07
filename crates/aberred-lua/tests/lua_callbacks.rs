//! Integration tests for Lua callback side effects.
//!
//! Runs a Lua callback through its real dispatch path (collision observer,
//! timer observer, phase system) in an ECS world and checks that the commands
//! the callback queued are drained and applied.

use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;

use aberred_core::components::boxcollider::BoxCollider;
use aberred_core::components::group::Group;
use aberred_core::components::guiinteractable::GuiInteractable;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::signals::Signals;
use aberred_core::components::timer::{Timer, TimerMode};
use aberred_core::components::ttl::Ttl;
use aberred_core::events::animation::AnimationFinished;
use aberred_core::events::collision::Overlapping;
use aberred_core::events::gui_interactable::GuiClicked;
use aberred_core::events::menu::MenuSelected;
use aberred_core::events::timer::TimerFired;
use aberred_core::events::tween::TweenFinished;
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::input::InputState;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_core::resources::worldtime::WorldTime;
use aberred_core::systems::collision_detector::collision_detector;
use aberred_core::systems::collision_rule_index::rebuild_rule_index;
use aberred_core::systems::timer::update_timers;
use aberred_lua::components::lua_on_animation_end::LuaOnAnimationEnd;
use aberred_lua::components::lua_on_click::LuaOnClick;
use aberred_lua::components::lua_on_menu_select::LuaOnMenuSelect;
use aberred_lua::components::lua_on_timer_fired::LuaOnTimerFired;
use aberred_lua::components::lua_on_tween_finished::LuaOnTweenFinished;
use aberred_lua::components::luacollision::{
    LuaCollisionContacts, LuaCollisionRule, LuaCollisionRuleIndex,
};
use aberred_lua::components::luaphase::{LuaPhase, PhaseCallbacks};
use aberred_lua::resources::lua_runtime::LuaRuntime;
use aberred_lua::systems::lua_animation_finished::lua_animation_finished_observer;
use aberred_lua::systems::lua_collision::lua_collision_observer;
use aberred_lua::systems::lua_gui_interactable_click::lua_gui_interactable_click_observer;
use aberred_lua::systems::lua_menu::lua_menu_selection_observer;
use aberred_lua::systems::lua_timer_fired::{lua_timer_fired_observer, lua_timer_removed_observer};
use aberred_lua::systems::lua_tween_finished::lua_tween_finished_observer;
use aberred_lua::systems::luaphase::lua_phase_system;

/// World with every resource the Lua dispatch paths read, plus a fresh
/// `LuaRuntime`.
fn make_lua_callback_world(delta: f32) -> World {
    let mut world = World::new();
    world.insert_resource(WorldTime {
        delta,
        ..WorldTime::default()
    });
    world.insert_resource(InputState::default());
    world.insert_resource(WorldSignals::default());
    world.init_resource::<Messages<AudioCmd>>();
    world.insert_resource(SystemsStore::new());
    world.insert_resource(AnimationStore::default());
    world.insert_resource(LuaCollisionRuleIndex::default());
    world.insert_resource(LuaCollisionContacts::default());
    world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
    world
}

/// Lua rule-index rebuild, then collision detection.
fn tick_collision_detector(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_rule_index::<LuaCollisionRule>.before(collision_detector));
    schedule.add_systems(collision_detector);
    schedule.run(world);
}

#[test]
fn collision_pipeline_triggers_lua_side_effects() {
    let mut world = make_lua_callback_world(0.0);

    {
        let lua_runtime = world.non_send::<LuaRuntime>();
        lua_runtime
            .lua()
            .load(
                r#"
                function on_player_enemy(ctx)
                    engine.collision_entity_signal_set_flag(ctx.a.id, "hit")
                    engine.collision_entity_insert_ttl(ctx.b.id, 1.5)
                end
                "#,
            )
            .exec()
            .expect("Failed to load collision Lua function");
    }

    let a = world
        .spawn((
            Group::new("player"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    let b = world
        .spawn((
            Group::new("enemy"),
            MapPosition::new(5.0, 0.0),
            BoxCollider::new(10.0, 10.0),
        ))
        .id();
    world.spawn((LuaCollisionRule::new("player", "enemy", "on_player_enemy"),));

    // Track if collision event was triggered
    let saw_collision = std::sync::Arc::new(std::sync::Mutex::new(false));
    let saw_collision_clone = saw_collision.clone();

    // Register the test observer to track collision events
    world.add_observer(move |_trigger: On<Overlapping>| {
        *saw_collision_clone.lock().unwrap() = true;
    });

    // Register the actual collision_observer that processes Lua callbacks
    world.add_observer(lua_collision_observer);

    world.flush();

    // Run collision detection - this will trigger Overlapping which fires both observers
    tick_collision_detector(&mut world);

    assert!(*saw_collision.lock().unwrap());

    let signals = world
        .get::<Signals>(a)
        .expect("Missing Signals on entity A");
    assert!(signals.has_flag("hit"));
    assert!(world.get::<Ttl>(b).is_some());
}

#[test]
fn collision_callback_error_still_drains_queued_commands() {
    let mut world = make_lua_callback_world(0.0);

    {
        let lua_runtime = world.non_send::<LuaRuntime>();
        lua_runtime
            .lua()
            .load(
                r#"
                function on_player_enemy_err(ctx)
                    engine.collision_entity_signal_set_flag(ctx.a.id, "hit")
                    local _ = ctx.nonexistent.field
                end
                "#,
            )
            .exec()
            .expect("Failed to load collision Lua function");
    }

    let a = world
        .spawn((
            Group::new("player"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((
        Group::new("enemy"),
        MapPosition::new(5.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    world.spawn((LuaCollisionRule::new(
        "player",
        "enemy",
        "on_player_enemy_err",
    ),));

    world.add_observer(lua_collision_observer);

    world.flush();

    tick_collision_detector(&mut world);

    let signals = world
        .get::<Signals>(a)
        .expect("Missing Signals on entity A");
    assert!(signals.has_flag("hit"));
}

// Command-drain ordering: a callback's queued commands are drained in a fixed
// order within one pass (spawn before clone, phase commands before
// callback-return transitions are applied), and no queue is dropped.

/// Register the Lua timer observers.
fn add_lua_timer_observers(world: &mut World) {
    world.add_observer(lua_timer_fired_observer);
    world.add_observer(lua_timer_removed_observer);
    world.flush();
}

fn tick_timers(world: &mut World) {
    world
        .run_system_once(update_timers)
        .expect("update_timers should run");
}

fn tick_lua_phases(world: &mut World) {
    world
        .run_system_once(lua_phase_system)
        .expect("lua_phase_system should run");
}

/// Regular path: spawn before clone (timer callback)
///
/// A timer callback registers a freshly spawned entity under a key, then
/// clones it.  The clone must succeed, which requires spawn to be processed
/// before clone inside the same drain pass.
#[test]
fn timer_callback_spawn_then_clone_same_drain() {
    let mut world = make_lua_callback_world(1.0);

    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function spawn_and_clone_cb(ctx, input)
                    engine.spawn():with_group("template"):register_as("tpl"):build()
                    engine.clone("tpl"):with_group("copy"):build()
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }

    world.spawn(LuaOnTimerFired::timer(
        0.5,
        "spawn_and_clone_cb",
        TimerMode::Repeat,
    ));

    add_lua_timer_observers(&mut world);
    tick_timers(&mut world);

    let copy_count = world
        .query::<&Group>()
        .iter(&world)
        .filter(|g| g.name() == "copy")
        .count();
    assert_eq!(
        copy_count, 1,
        "expected one cloned entity with group 'copy'"
    );
}

/// A one-shot Lua timer runs its callback once, then loses its `Timer` and
/// its `LuaOnTimerFired`.
#[test]
fn once_timer_runs_lua_callback_once_and_removes_timer() {
    let mut world = make_lua_callback_world(1.0);
    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                once_calls = 0
                function once_cb(ctx, input)
                    once_calls = once_calls + 1
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }
    let entity = world
        .spawn(LuaOnTimerFired::timer(0.5, "once_cb", TimerMode::Once))
        .id();
    add_lua_timer_observers(&mut world);

    for _ in 0..2 {
        tick_timers(&mut world);
    }

    let calls: i64 = world
        .non_send::<LuaRuntime>()
        .lua()
        .globals()
        .get("once_calls")
        .expect("once_calls");
    assert_eq!(calls, 1);
    assert!(world.get::<Timer>(entity).is_none());
    assert!(world.get::<LuaOnTimerFired>(entity).is_none());
    assert!(world.get_entity(entity).is_ok(), "the entity survives");
}

/// A one-shot callback that re-arms its own entity through the engine API keeps
/// the new timer: its queued command lands before the spent timer is removed.
#[test]
fn once_timer_lua_callback_can_rearm_through_engine_api() {
    let mut world = make_lua_callback_world(1.0);
    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function rearm_cb(ctx, input)
                    engine.entity_insert_lua_timer_once(ctx.id, 2.0, "again")
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }
    let entity = world
        .spawn(LuaOnTimerFired::timer(0.5, "rearm_cb", TimerMode::Once))
        .id();

    add_lua_timer_observers(&mut world);
    tick_timers(&mut world);

    let timer = world
        .get::<Timer>(entity)
        .expect("the re-armed timer must survive the spent timer's removal");
    assert_eq!(timer.duration, 2.0);
    let on_fired = world
        .get::<LuaOnTimerFired>(entity)
        .expect("the re-armed callback must survive too");
    assert_eq!(&*on_fired.callback, "again");
}

/// A zero-duration one-shot re-armed from a Lua callback fires on the next tick.
#[test]
fn once_timer_lua_callback_zero_duration_rearm_fires_next_tick() {
    let mut world = make_lua_callback_world(1.0);
    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                zero_calls = 0
                function zero_cb(ctx, input)
                    zero_calls = zero_calls + 1
                    if zero_calls == 1 then
                        engine.entity_insert_lua_timer_once(ctx.id, 0, "zero_cb")
                    end
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }
    let entity = world
        .spawn(LuaOnTimerFired::timer(0.5, "zero_cb", TimerMode::Once))
        .id();
    add_lua_timer_observers(&mut world);
    let zero_calls = |world: &World| -> i64 {
        world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .get("zero_calls")
            .expect("zero_calls")
    };

    tick_timers(&mut world);
    assert_eq!(zero_calls(&world), 1);
    assert!(world.get::<Timer>(entity).is_some(), "re-armed timer kept");

    tick_timers(&mut world);
    assert_eq!(
        zero_calls(&world),
        2,
        "the zero-duration re-arm fires on the next tick"
    );
    assert!(world.get::<Timer>(entity).is_none());
    assert!(world.get::<LuaOnTimerFired>(entity).is_none());
}

/// A timer callback's `ctx.timer` reports the firing timer's duration, elapsed
/// time and callback name.
#[test]
fn timer_callback_ctx_timer_reports_the_firing_timer() {
    let mut world = make_lua_callback_world(1.0);
    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function read_ctx_timer(ctx, input)
                    seen_duration = ctx.timer.duration
                    seen_elapsed = ctx.timer.elapsed
                    seen_callback = ctx.timer.callback
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }
    // One-shot, so the snapshot shows the un-reset elapsed time (1.0 > 0.5).
    world.spawn(LuaOnTimerFired::timer(
        0.5,
        "read_ctx_timer",
        TimerMode::Once,
    ));

    add_lua_timer_observers(&mut world);
    tick_timers(&mut world);

    let globals = world.non_send::<LuaRuntime>().lua().globals();
    let duration: f32 = globals.get("seen_duration").expect("seen_duration");
    let elapsed: f32 = globals.get("seen_elapsed").expect("seen_elapsed");
    let callback: String = globals.get("seen_callback").expect("seen_callback");
    assert_eq!(
        (duration, elapsed, callback.as_str()),
        (0.5, 1.0, "read_ctx_timer")
    );
}

/// Collision path: spawn before clone (collision callback)
///
/// A collision callback registers a freshly spawned entity then clones it.
/// Both operations go through collision-scoped queues
/// (collision_spawn / collision_clone).  The clone must succeed, which
/// requires spawn to drain before clone.
#[test]
fn collision_callback_spawn_then_clone_same_drain() {
    let mut world = make_lua_callback_world(0.0);

    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function on_coll_spawn_clone(ctx)
                    engine.collision_spawn():with_group("proj"):register_as("proj_src"):build()
                    engine.collision_clone("proj_src"):with_group("proj_copy"):build()
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }

    let _a = world
        .spawn((
            Group::new("shooter"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
        ))
        .id();
    let _b = world
        .spawn((
            Group::new("target"),
            MapPosition::new(5.0, 0.0),
            BoxCollider::new(10.0, 10.0),
        ))
        .id();
    world.spawn(LuaCollisionRule::new(
        "shooter",
        "target",
        "on_coll_spawn_clone",
    ));

    world.add_observer(lua_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let copy_count = world
        .query::<&Group>()
        .iter(&world)
        .filter(|g| g.name() == "proj_copy")
        .count();
    assert_eq!(
        copy_count, 1,
        "expected one collision-cloned entity with group 'proj_copy'"
    );
}

/// Lua phase: return-value transition takes precedence over
/// engine.phase_transition() called in the same on_update.
///
/// If the on_update callback BOTH calls `engine.phase_transition(id, "via_cmd")`
/// AND returns `"return_winner"`, the return value must win because
/// apply_callback_transitions runs after the phase drain.
#[test]
fn lua_phase_return_value_beats_phase_transition_cmd() {
    let mut world = make_lua_callback_world(0.016);

    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function idle_update(ctx, input, dt)
                    engine.phase_transition(ctx.id, "via_cmd")
                    return "return_winner"
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }

    let mut phases = rustc_hash::FxHashMap::default();
    phases.insert(
        "idle".into(),
        PhaseCallbacks {
            on_enter: None,
            on_update: Some("idle_update".into()),
            on_exit: None,
        },
    );
    phases.insert("via_cmd".into(), PhaseCallbacks::default());
    phases.insert("return_winner".into(), PhaseCallbacks::default());

    let entity = world.spawn((LuaPhase::new("idle", phases),)).id();

    // First tick: idle_update runs, queues both a PhaseCmd and a return transition.
    tick_lua_phases(&mut world);

    // The winning transition is stored in `next` after the first tick.
    let phase = world.get::<LuaPhase>(entity).unwrap();
    assert_eq!(
        phase.phase.next.as_deref(),
        Some("return_winner"),
        "return value should override the phase_transition() cmd"
    );

    // Second tick: the pending transition is applied.
    tick_lua_phases(&mut world);

    let phase = world.get::<LuaPhase>(entity).unwrap();
    assert_eq!(phase.phase.current, "return_winner");
}

/// Collision path: the phase drain does not suppress other queues.
///
/// A collision callback queues a phase transition and a world-signal mutation.
/// Both must be observed after the observer runs.
/// Guards against any queue being silently dropped by the drain order.
#[test]
fn collision_callback_phase_plus_signal_all_processed() {
    let mut world = make_lua_callback_world(0.0);

    {
        let rt = world.non_send::<LuaRuntime>();
        rt.lua()
            .load(
                r#"
                function on_multi_effect(ctx)
                    engine.collision_phase_transition(ctx.a.id, "hit")
                    engine.collision_set_flag("was_hit")
                end
            "#,
            )
            .exec()
            .expect("lua load");
    }

    let mut phases: rustc_hash::FxHashMap<String, PhaseCallbacks> =
        rustc_hash::FxHashMap::default();
    phases.insert("idle".into(), PhaseCallbacks::default());
    phases.insert("hit".into(), PhaseCallbacks::default());

    let a = world
        .spawn((
            Group::new("hero"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            LuaPhase::new("idle", phases),
        ))
        .id();
    world.spawn((
        Group::new("hazard"),
        MapPosition::new(5.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    world.spawn(LuaCollisionRule::new("hero", "hazard", "on_multi_effect"));

    world.add_observer(lua_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    // Phase transition queued
    let phase = world.get::<LuaPhase>(a).unwrap();
    assert_eq!(
        phase.phase.next.as_deref(),
        Some("hit"),
        "phase transition should be queued"
    );

    // World signal mutation applied
    let signals = world.resource::<WorldSignals>();
    assert!(
        signals.has_flag("was_hit"),
        "world signal flag should be set"
    );
}

fn load_lua(world: &World, src: &str) {
    world
        .non_send::<LuaRuntime>()
        .lua()
        .load(src)
        .exec()
        .expect("failed to load Lua source");
}

fn lua_global<T: mlua::FromLua>(world: &World, name: &str) -> T {
    world
        .non_send::<LuaRuntime>()
        .lua()
        .globals()
        .get(name)
        .unwrap()
}

#[test]
fn click_callback_reads_signals_written_since_the_last_dispatch() {
    let mut world = make_lua_callback_world(0.0);
    world.add_observer(lua_gui_interactable_click_observer);
    load_lua(
        &world,
        "function on_click(ctx) seen_score = engine.get_scalar('score') end",
    );
    let button = world
        .spawn((
            GuiInteractable::new(80.0, 24.0),
            LuaOnClick::new("on_click"),
        ))
        .id();
    world
        .resource_mut::<WorldSignals>()
        .set_scalar("score", 42.0);

    world.trigger(GuiClicked { entity: button });
    world.flush();

    assert_eq!(lua_global::<Option<f32>>(&world, "seen_score"), Some(42.0));
}

#[test]
fn click_callback_queued_commands_apply_in_the_same_dispatch() {
    let mut world = make_lua_callback_world(0.0);
    world.add_observer(lua_gui_interactable_click_observer);
    load_lua(
        &world,
        "function on_click(ctx) engine.set_flag('clicked') end",
    );
    let button = world
        .spawn((
            GuiInteractable::new(80.0, 24.0),
            LuaOnClick::new("on_click"),
        ))
        .id();

    world.trigger(GuiClicked { entity: button });
    world.flush();

    assert!(world.resource::<WorldSignals>().has_flag("clicked"));
}

#[test]
fn menu_callback_reads_signals_and_applies_queued_commands_in_the_same_dispatch() {
    let mut world = make_lua_callback_world(0.0);
    world.add_observer(lua_menu_selection_observer);
    load_lua(
        &world,
        "function on_select(ctx) seen_score = engine.get_scalar('score'); engine.set_flag('selected') end",
    );
    let menu = world.spawn(LuaOnMenuSelect::new("on_select")).id();
    world
        .resource_mut::<WorldSignals>()
        .set_scalar("score", 7.0);

    world.trigger(MenuSelected {
        entity: menu,
        item_id: "play".to_string(),
        index: 0,
    });
    world.flush();

    assert_eq!(lua_global::<Option<f32>>(&world, "seen_score"), Some(7.0));
    assert!(world.resource::<WorldSignals>().has_flag("selected"));
}

const ENTITY_CALLBACK_SRC: &str = "function on_done(ctx, input)
    got_id = ctx.id
    got_input = type(input)
    engine.set_flag('done')
end";

/// The `(ctx, input)` callback of [`ENTITY_CALLBACK_SRC`] saw `entity`'s ctx and
/// an input table, and its queued flag was applied.
fn assert_entity_callback_ran(world: &World, entity: Entity) {
    assert_eq!(
        lua_global::<Option<u64>>(world, "got_id"),
        Some(entity.to_bits())
    );
    assert_eq!(
        lua_global::<Option<String>>(world, "got_input").as_deref(),
        Some("table")
    );
    assert!(world.resource::<WorldSignals>().has_flag("done"));
}

#[test]
fn animation_end_callback_gets_ctx_and_input_and_drains_its_commands() {
    let mut world = make_lua_callback_world(0.0);
    world.add_observer(lua_animation_finished_observer);
    load_lua(&world, ENTITY_CALLBACK_SRC);
    let entity = world.spawn(LuaOnAnimationEnd::new("on_done")).id();

    world.trigger(AnimationFinished { entity });
    world.flush();

    assert_entity_callback_ran(&world, entity);
}

#[test]
fn timer_callback_gets_ctx_and_input_and_drains_its_commands() {
    let mut world = make_lua_callback_world(0.0);
    add_lua_timer_observers(&mut world);
    load_lua(&world, ENTITY_CALLBACK_SRC);
    let entity = world
        .spawn(LuaOnTimerFired::timer(1.0, "on_done", TimerMode::Repeat))
        .id();

    world.trigger(TimerFired { entity });
    world.flush();

    assert_entity_callback_ran(&world, entity);
}

#[test]
fn tween_finished_callback_gets_ctx_and_input_and_drains_its_commands() {
    let mut world = make_lua_callback_world(0.0);
    world.add_observer(lua_tween_finished_observer::<MapPosition>);
    load_lua(&world, ENTITY_CALLBACK_SRC);
    let entity = world
        .spawn(LuaOnTweenFinished::<MapPosition>::new("on_done"))
        .id();

    world.trigger(TweenFinished::<MapPosition>::new(entity));
    world.flush();

    assert_entity_callback_ran(&world, entity);
}
