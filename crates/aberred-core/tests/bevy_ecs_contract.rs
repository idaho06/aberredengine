//! bevy_ecs contract tests.
//!
//! Each test pins one bevy_ecs behavior that engine code depends on; the
//! comment above it names that code. Basic insert/get/query/spawn/despawn
//! behavior is not tested here — every engine test already exercises it.
//! When upgrading bevy_ecs, a failure here points at the engine code that
//! needs revisiting:
//!
//! ```sh
//! cargo test --test bevy_ecs_contract
//! ```

use aberred_core::components::persistent::Persistent;
use aberred_core::resources::collision_contacts::CollisionContacts;
use aberred_core::systems::gamestate::clean_all_entities;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;
use bevy_ecs::system::{RunSystemOnce, SystemParam, SystemState};
use std::sync::{Arc, Mutex};

#[derive(Component, Debug, Clone, PartialEq)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Component, Debug, Clone, PartialEq)]
struct Health(i32);

#[derive(Component, Debug, Clone, PartialEq)]
struct NamedGroup(String);

#[derive(Resource, Debug, Default)]
struct Counter(i32);

#[derive(Resource, Debug, Default)]
struct DebugOn(bool);

#[derive(Event, Debug, Clone)]
struct Overlapping {
    a: Entity,
    b: Entity,
}

#[derive(Event, Debug, Clone)]
struct SimpleEvent;

#[derive(Debug, Clone, Message)]
struct TestMessage;

/// NonSend resource (like `LuaRuntime` / raylib handles).
struct NonSendHandle {
    value: i32,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl NonSendHandle {
    fn new(value: i32) -> Self {
        NonSendHandle {
            value,
            _not_send: std::marker::PhantomData,
        }
    }
}

fn increment_counter(mut counter: ResMut<Counter>) {
    counter.0 += 1;
}

fn run_scene_cleanup(world: &mut World) {
    world.init_resource::<CollisionContacts>();
    world
        .run_system_once(clean_all_entities)
        .expect("clean_all_entities should run");
}

fn message_count(world: &mut World) -> usize {
    let mut state = SystemState::<MessageReader<TestMessage>>::new(world);
    let mut reader = state.get_mut(world).expect("Message reader should fetch");
    reader.read().count()
}

// =============================================================================
// Messages
// =============================================================================

// `update_bevy_render_asset_cmds` / `update_bevy_audio_cmds` age
// `Messages<RenderAssetCmd>` / `Messages<AudioCmd>` with `update()`: a message
// must stay readable for one update and be gone after the second.
#[test]
fn messages_cleared_after_second_update() {
    let mut world = World::new();
    world.init_resource::<Messages<TestMessage>>();

    world
        .resource_mut::<Messages<TestMessage>>()
        .write(TestMessage);
    world.resource_mut::<Messages<TestMessage>>().update();
    assert_eq!(message_count(&mut world), 1);

    world.resource_mut::<Messages<TestMessage>>().update();
    assert_eq!(message_count(&mut world), 0);
}

// =============================================================================
// Observers
// =============================================================================

// Engine events (`Overlapping`, `AnimationFinished`, ...) are observed
// via `add_observer` and fired with `World::trigger`.
#[test]
fn observer_receives_event_payload() {
    let mut world = World::new();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_clone = seen.clone();
    world.add_observer(move |trigger: On<Overlapping>| {
        let event = trigger.event();
        seen_clone.lock().unwrap().extend([event.a, event.b]);
    });
    world.flush();

    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();
    world.trigger(Overlapping { a: e1, b: e2 });

    assert_eq!(*seen.lock().unwrap(), vec![e1, e2]);
}

// Systems trigger events via `Commands::trigger` (input -> `InputEvent`,
// GUI hit-test -> `GuiClicked`); `pump_render_msgs` relies on
// the observer running as soon as the command queue is applied.
#[test]
fn commands_trigger_fires_observer_on_apply() {
    let mut world = World::new();

    let received = Arc::new(Mutex::new(false));
    let received_clone = received.clone();
    world.add_observer(move |_trigger: On<SimpleEvent>| {
        *received_clone.lock().unwrap() = true;
    });
    world.flush();

    let mut state = SystemState::<Commands>::new(&mut world);
    state
        .get_mut(&mut world)
        .expect("Commands should fetch")
        .trigger(SimpleEvent);
    assert!(!*received.lock().unwrap(), "must not fire before apply");

    state.apply(&mut world);
    assert!(*received.lock().unwrap());
}

#[derive(EntityEvent)]
struct Ping {
    entity: Entity,
}

fn count_ping(_trigger: On<Ping>, mut counter: ResMut<Counter>) {
    counter.0 += 1;
}

// `CollisionEnded` targets the rule entity, which may already be despawned
// when the pair stops touching. Triggered through a system's `Commands` (the
// engine's call path): a global observer still sees the event, the dead
// entity's own observers are gone, and the trigger does not panic.
#[test]
fn commands_trigger_at_despawned_target_reaches_global_observers_only() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let global = Arc::new(Mutex::new(Vec::new()));
    let global_clone = global.clone();
    world.add_observer(move |trigger: On<Ping>| {
        global_clone.lock().unwrap().push(trigger.event().entity);
    });

    let target = world.spawn_empty().observe(count_ping).id();
    world.flush();
    world.despawn(target);

    let mut state = SystemState::<Commands>::new(&mut world);
    state
        .get_mut(&mut world)
        .expect("Commands should fetch")
        .trigger(Ping { entity: target });
    state.apply(&mut world);

    assert_eq!(*global.lock().unwrap(), vec![target]);
    assert_eq!(world.resource::<Counter>().0, 0);
}

// =============================================================================
// Registered systems
// =============================================================================

fn add_to_health(entity: In<Entity>, mut query: Query<&mut Health>) {
    if let Ok(mut health) = query.get_mut(*entity) {
        health.0 += 50;
    }
}

// `SystemsStore.entity_map` holds `SystemId<In<Entity>>`, run via
// `commands.run_system_with(id, entity)` (`lua_commands/entity_cmd.rs`).
#[test]
fn registered_system_with_entity_input() {
    let mut world = World::new();
    let entity = world.spawn(Health(100)).id();

    let system_id = world.register_system(add_to_health);
    world.run_system_with(system_id, entity).unwrap();

    assert_eq!(world.get::<Health>(entity).unwrap().0, 150);
}

// =============================================================================
// World access patterns
// =============================================================================

// `receive_snapshot` / `reconcile_*` (aberred-render) mutate the world while
// holding a resource via `resource_scope`, and the resource is back afterwards.
#[test]
fn resource_scope_allows_world_access_and_restores_resource() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    world.resource_scope(|world, mut counter: Mut<Counter>| {
        world.spawn(Health(1));
        counter.0 += 100;
    });

    assert_eq!(world.resource::<Counter>().0, 100);
    assert_eq!(world.query::<&Health>().iter(&world).count(), 1);
}

// `LuaRuntime` (logic) and `FontStore`/`ShaderStore`/`RenderTarget` (render)
// are NonSend resources read and written from systems.
#[test]
fn non_send_resources_in_systems() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_non_send(NonSendHandle::new(50));

    fn read_non_send(handle: NonSend<NonSendHandle>, mut counter: ResMut<Counter>) {
        counter.0 = handle.value;
    }
    fn write_non_send(mut handle: NonSendMut<NonSendHandle>) {
        handle.value = 999;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems((read_non_send, write_non_send).chain());
    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 50);
    assert_eq!(world.non_send::<NonSendHandle>().value, 999);
}

// `send_render_mirrors` diffs against `Local<Option<T>>` previous-frame values:
// a system's `Local` must persist across repeated `schedule.run()` calls.
#[test]
fn local_persists_across_schedule_runs() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    fn system_with_local(mut local: Local<i32>, mut counter: ResMut<Counter>) {
        *local += 1;
        counter.0 = *local;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(system_with_local);
    for expected in 1..=3 {
        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().0, expected);
    }
}

// `.run_if(state_is_playing)` gates most gameplay systems.
#[test]
fn run_if_false_skips_system() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_resource(DebugOn(false));

    fn is_debug(flag: Res<DebugOn>) -> bool {
        flag.0
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(increment_counter.run_if(is_debug));
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 0);

    world.resource_mut::<DebugOn>().0 = true;
    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 1);
}

/// Bundled params, like the engine's `RenderResources` / `MirrorQueries`.
#[derive(SystemParam)]
struct BundledResources<'w> {
    counter: ResMut<'w, Counter>,
    flag: Res<'w, DebugOn>,
}

// Derived `SystemParam` bundles (`RenderResources`, `DebugResources`,
// `MirrorQueries`) are fetched through `SystemState` in engine tests.
#[test]
fn derived_system_param_via_system_state() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_resource(DebugOn(true));

    let mut state = SystemState::<BundledResources>::new(&mut world);
    let mut res = state
        .get_mut(&mut world)
        .expect("Bundled resources should fetch");
    assert!(res.flag.0);
    res.counter.0 = 99;
    state.apply(&mut world);

    assert_eq!(world.resource::<Counter>().0, 99);
}

// `collision_detector` iterates unique pairs with `iter_combinations_mut`.
#[test]
fn iter_combinations_mut_yields_each_unordered_pair_once() {
    let mut world = World::new();
    for x in 1..=3 {
        world.spawn(Position {
            x: x as f32,
            y: 0.0,
        });
    }

    let mut state = SystemState::<Query<&mut Position>>::new(&mut world);
    let mut query = state
        .get_mut(&mut world)
        .expect("Position query should fetch");

    let mut pairs = Vec::new();
    let mut combos = query.iter_combinations_mut();
    while let Some([a, b]) = combos.fetch_next() {
        let (lo, hi) = if a.x < b.x { (a.x, b.x) } else { (b.x, a.x) };
        pairs.push((lo as i32, hi as i32));
    }
    pairs.sort();

    assert_eq!(pairs, vec![(1, 2), (1, 3), (2, 3)]);
}

// Lua entity cloning (`spawn_cmd.rs`), `particle_emitter_system` and
// `tilemap_spawn_system` call `clone_and_spawn` on a source that may itself
// have been spawned earlier in the same command buffer.
#[test]
fn clone_and_spawn_after_deferred_spawn_same_buffer() {
    let mut world = World::new();

    let mut sys_state: SystemState<Commands> = SystemState::new(&mut world);
    {
        let mut commands = sys_state
            .get_mut(&mut world)
            .expect("Commands should fetch");

        let mut source_ec = commands.spawn_empty();
        let source = source_ec.id();
        source_ec.insert(NamedGroup("template".to_string()));

        let mut source_ref = commands.entity(source);
        let mut clone_ec = source_ref.clone_and_spawn();
        clone_ec.insert(NamedGroup("copy".to_string()));
    }
    sys_state.apply(&mut world);

    let copy_count = world
        .query::<&NamedGroup>()
        .iter(&world)
        .filter(|g| g.0 == "copy")
        .count();
    assert_eq!(copy_count, 1);
}

// Render sprite/text sorting and `gui_hit_test_system` break z-index ties by
// `Entity`'s `Ord`, which must be a total order agreeing with `to_bits()`
// (see system-order.md: deterministic, but NOT correlated with spawn order —
// that part is deliberately not asserted here).
#[test]
fn entity_ord_agrees_with_to_bits() {
    let mut world = World::new();
    let mut entities: Vec<Entity> = (0..16).map(|_| world.spawn_empty().id()).collect();
    // Recycle a few slots so generations differ too.
    for e in entities.drain(4..8) {
        world.despawn(e);
    }
    entities.extend((0..4).map(|_| world.spawn_empty().id()));

    let mut by_ord = entities.clone();
    by_ord.sort();
    let mut by_bits = entities;
    by_bits.sort_by_key(|e| e.to_bits());

    assert_eq!(by_ord, by_bits);
}

// `ChildOf` is used for lifecycle only in GUI widgets (`gui_layout.rs`,
// `guioffset.rs`): despawning a parent must cascade-despawn its whole subtree.
#[test]
fn cascade_despawn_removes_children() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child = world.spawn(ChildOf(parent)).id();
    let grandchild = world.spawn(ChildOf(child)).id();

    world.flush();

    // All three should exist
    assert!(world.get_entity(parent).is_ok());
    assert!(world.get_entity(child).is_ok());
    assert!(world.get_entity(grandchild).is_ok());

    // Despawning the parent cascades to child and grandchild.
    world.despawn(parent);

    assert!(
        world.get_entity(parent).is_err(),
        "Parent should be despawned"
    );
    assert!(
        world.get_entity(child).is_err(),
        "Child should be cascade-despawned"
    );
    assert!(
        world.get_entity(grandchild).is_err(),
        "Grandchild should be cascade-despawned"
    );
}

// =============================================================================
// Scene cleanup (`clean_all_entities` / `SceneCleanup`)
// =============================================================================
// Observers, registered systems and resources are all entities in bevy_ecs
// 0.18+/0.19, so the scene-switch cleanup query would despawn them unless
// they carry `Persistent` (observers, `SystemsStore` entries) or are excluded
// via `IsResource` (resources).

#[test]
fn cleanup_despawns_non_persistent_observers_only() {
    let mut world = World::new();

    let persistent_hits = Arc::new(Mutex::new(0));
    let transient_hits = Arc::new(Mutex::new(0));
    let (p, t) = (persistent_hits.clone(), transient_hits.clone());
    world.spawn((
        Observer::new(move |_: On<SimpleEvent>| *p.lock().unwrap() += 1),
        Persistent,
    ));
    world.spawn(Observer::new(move |_: On<SimpleEvent>| {
        *t.lock().unwrap() += 1
    }));
    world.spawn(Position { x: 0.0, y: 0.0 });
    world.flush();

    run_scene_cleanup(&mut world);
    world.trigger(SimpleEvent);

    assert_eq!(*persistent_hits.lock().unwrap(), 1);
    assert_eq!(*transient_hits.lock().unwrap(), 0);
    assert_eq!(world.query::<&Position>().iter(&world).count(), 0);
}

// `EntityCommands::observe` spawns its observer as a separate entity
// without `Persistent`; cleanup must keep it while the watched entity is
// `Persistent` (`SceneCleanup`), and bevy drops it with a watched scene entity.
fn observer_count(world: &mut World) -> usize {
    world.query::<&Observer>().iter(world).count()
}

#[test]
fn cleanup_keeps_entity_observers_of_persistent_entities() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    let kept = world.spawn(Persistent).observe(count_ping).id();
    world.flush();

    run_scene_cleanup(&mut world);
    world.trigger(Ping { entity: kept });

    assert_eq!(world.resource::<Counter>().0, 1);
}

#[test]
fn cleanup_despawns_scene_entities_with_their_observers() {
    let mut world = World::new();
    let gone = world.spawn_empty().observe(count_ping).id();
    world.flush();

    run_scene_cleanup(&mut world);

    assert!(world.get_entity(gone).is_err());
    assert_eq!(observer_count(&mut world), 0);
}

#[test]
fn cleanup_despawns_non_persistent_registered_systems_only() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let persistent_id = world.register_system(increment_counter);
    world.entity_mut(persistent_id.entity()).insert(Persistent);
    let transient_id = world.register_system(increment_counter);

    run_scene_cleanup(&mut world);

    world.run_system(persistent_id).unwrap();
    assert_eq!(world.resource::<Counter>().0, 1);
    assert!(world.run_system(transient_id).is_err());
}

// Resources are entities too; `CleanableEntity`'s `Without<IsResource>` must
// keep cleanup from deleting them.
#[test]
fn cleanup_keeps_resources() {
    let mut world = World::new();
    world.insert_resource(Counter(7));

    run_scene_cleanup(&mut world);

    assert_eq!(world.resource::<Counter>().0, 7);
}

// After cleanup, the engine reaches persistent observers/systems through
// `Commands` (input system triggers events; `gamestate`/`scene_dispatch`
// call `commands.run_system(switch_scene)`).
#[test]
fn commands_reach_persistent_observer_and_system_after_cleanup() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let hits = Arc::new(Mutex::new(0));
    let hits_clone = hits.clone();
    world.spawn((
        Observer::new(move |_: On<SimpleEvent>| *hits_clone.lock().unwrap() += 1),
        Persistent,
    ));
    let system_id = world.register_system(increment_counter);
    world.entity_mut(system_id.entity()).insert(Persistent);
    world.flush();

    run_scene_cleanup(&mut world);

    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world).expect("Commands should fetch");
    commands.trigger(SimpleEvent);
    commands.run_system(system_id);
    state.apply(&mut world);

    assert_eq!(*hits.lock().unwrap(), 1);
    assert_eq!(world.resource::<Counter>().0, 1);
}
