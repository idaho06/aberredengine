//! Bevy ECS Integration Tests
//!
//! These tests verify that bevy_ecs behaves as expected by the Aberred Engine.
//! They serve as a compatibility layer to detect breaking changes when upgrading
//! bevy_ecs versions.
//!
//! # Test Categories
//!
//! 1. **World & Resources** - Resource insertion, retrieval, mutability
//! 2. **Entity & Component** - Spawning, despawning, component operations
//! 3. **Query Patterns** - Filters, combinations, optional components
//! 4. **Events & Observers** - Event emission, observer registration
//! 5. **System Registration** - SystemId, run_system, In<Entity>
//! 6. **MessageWriter/Reader** - Message queue behavior
//! 7. **SystemParam** - Derived system parameters
//! 8. **Commands** - Deferred operations
//! 9. **Local/NonSend** - System-local state, thread-local resources
//!
//! # Usage
//!
//! Run these tests after upgrading bevy_ecs to detect API changes:
//!
//! ```sh
//! cargo test --test bevy_ecs_integration
//! ```

use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;
use bevy_ecs::system::{SystemParam, SystemState};
use std::sync::{Arc, Mutex};

// =============================================================================
// Test Components, Resources, and Events
// =============================================================================

#[derive(Component, Debug, Clone, PartialEq)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Component, Debug, Clone, PartialEq)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Component, Debug, Clone, PartialEq, Default)]
struct Health(i32);

#[derive(Component, Debug, Clone)]
struct Name(String);

#[derive(Component, Debug, Clone, PartialEq)]
struct GroupTag(String);

/// Marker component used like engine's Persistent
#[derive(Component, Debug, Clone)]
struct Persistent;

/// Marker component for filtering
#[derive(Component, Debug, Clone)]
struct Player;

/// Marker component for filtering
#[derive(Component, Debug, Clone)]
struct Enemy;

#[derive(Resource, Debug, Default)]
struct GameTime {
    delta: f32,
    elapsed: f32,
}

#[derive(Resource, Debug, Default)]
struct Counter(i32);

#[derive(Resource, Debug)]
struct Config {
    debug_mode: bool,
    max_entities: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            debug_mode: false,
            max_entities: 1000,
        }
    }
}

#[derive(Event, Debug, Clone)]
struct CollisionEvent {
    a: Entity,
    b: Entity,
}

#[derive(Event, Debug, Clone)]
struct DamageEvent {
    entity: Entity,
    amount: i32,
}

#[derive(Event, Debug, Clone)]
struct SimpleEvent(i32);

// =============================================================================
// CATEGORY 1: World & Resource Tests
// =============================================================================

#[test]
fn world_insert_resource() {
    let mut world = World::new();
    world.insert_resource(GameTime {
        delta: 0.016,
        elapsed: 0.0,
    });

    let time = world.resource::<GameTime>();
    assert!((time.delta - 0.016).abs() < f32::EPSILON);
}

#[test]
fn world_get_resource_mut() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    {
        let mut counter = world.resource_mut::<Counter>();
        counter.0 += 10;
    }

    let counter = world.resource::<Counter>();
    assert_eq!(counter.0, 10);
}

#[test]
fn world_get_resource_or_insert() {
    let mut world = World::new();

    // First access creates the resource
    world.get_resource_or_insert_with(|| Counter(5));
    assert_eq!(world.resource::<Counter>().0, 5);

    // Second access doesn't change existing value
    world.get_resource_or_insert_with(|| Counter(999));
    assert_eq!(world.resource::<Counter>().0, 5);
}

#[test]
fn world_contains_resource() {
    let mut world = World::new();

    assert!(!world.contains_resource::<GameTime>());
    world.insert_resource(GameTime::default());
    assert!(world.contains_resource::<GameTime>());
}

#[test]
fn world_remove_resource() {
    let mut world = World::new();
    world.insert_resource(Counter(42));

    let removed = world.remove_resource::<Counter>();
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().0, 42);
    assert!(!world.contains_resource::<Counter>());
}

#[test]
fn world_multiple_resources() {
    let mut world = World::new();
    world.insert_resource(GameTime {
        delta: 0.016,
        elapsed: 1.0,
    });
    world.insert_resource(Counter(7));
    world.insert_resource(Config {
        debug_mode: true,
        max_entities: 500,
    });

    let time = world.resource::<GameTime>();
    let counter = world.resource::<Counter>();
    let config = world.resource::<Config>();

    assert!((time.elapsed - 1.0).abs() < f32::EPSILON);
    assert_eq!(counter.0, 7);
    assert!(config.debug_mode);
    assert_eq!(config.max_entities, 500);
}

#[test]
fn world_resource_scope() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    world.resource_scope(|_world, mut counter: Mut<Counter>| {
        counter.0 += 100;
    });

    assert_eq!(world.resource::<Counter>().0, 100);
}

#[test]
fn world_init_resource() {
    let mut world = World::new();
    world.init_resource::<Counter>();

    // Uses Default::default()
    assert_eq!(world.resource::<Counter>().0, 0);
}

// =============================================================================
// CATEGORY 2: Entity & Component Tests
// =============================================================================

#[test]
fn entity_spawn_with_components() {
    let mut world = World::new();

    let entity = world
        .spawn((
            Position { x: 10.0, y: 20.0 },
            Velocity { x: 1.0, y: 2.0 },
            Health(100),
        ))
        .id();

    assert!(world.get::<Position>(entity).is_some());
    assert!(world.get::<Velocity>(entity).is_some());
    assert!(world.get::<Health>(entity).is_some());

    let pos = world.get::<Position>(entity).unwrap();
    assert!((pos.x - 10.0).abs() < f32::EPSILON);
}

#[test]
fn entity_spawn_empty() {
    let mut world = World::new();

    let entity = world.spawn_empty().id();
    assert!(world.get_entity(entity).is_ok());
}

#[test]
fn entity_despawn() {
    let mut world = World::new();

    let entity = world.spawn((Position { x: 0.0, y: 0.0 },)).id();
    assert!(world.get_entity(entity).is_ok());

    world.despawn(entity);
    assert!(world.get_entity(entity).is_err());
}

#[test]
fn entity_insert_component() {
    let mut world = World::new();

    let entity = world.spawn((Position { x: 0.0, y: 0.0 },)).id();

    world.entity_mut(entity).insert(Velocity { x: 5.0, y: 5.0 });

    let vel = world.get::<Velocity>(entity).unwrap();
    assert!((vel.x - 5.0).abs() < f32::EPSILON);
}

#[test]
fn entity_remove_component() {
    let mut world = World::new();

    let entity = world
        .spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 1.0 }))
        .id();

    world.entity_mut(entity).remove::<Velocity>();

    assert!(world.get::<Position>(entity).is_some());
    assert!(world.get::<Velocity>(entity).is_none());
}

#[test]
fn entity_get_mut_component() {
    let mut world = World::new();

    let entity = world.spawn((Health(100),)).id();

    if let Some(mut health) = world.get_mut::<Health>(entity) {
        health.0 -= 25;
    }

    assert_eq!(world.get::<Health>(entity).unwrap().0, 75);
}

#[test]
fn entity_contains_component() {
    let mut world = World::new();

    let entity = world.spawn((Position { x: 0.0, y: 0.0 }, Player)).id();

    assert!(world.entity(entity).contains::<Position>());
    assert!(world.entity(entity).contains::<Player>());
    assert!(!world.entity(entity).contains::<Enemy>());
}

#[test]
fn entity_spawn_batch() {
    let mut world = World::new();

    let entities: Vec<Entity> = (0..10)
        .map(|i| {
            world
                .spawn((Position {
                    x: i as f32,
                    y: i as f32,
                },))
                .id()
        })
        .collect();

    assert_eq!(entities.len(), 10);

    for (i, &entity) in entities.iter().enumerate() {
        let pos = world.get::<Position>(entity).unwrap();
        assert!((pos.x - i as f32).abs() < f32::EPSILON);
    }
}

#[test]
fn entity_replace_component() {
    let mut world = World::new();

    let entity = world.spawn((Health(100),)).id();
    world.entity_mut(entity).insert(Health(50));

    assert_eq!(world.get::<Health>(entity).unwrap().0, 50);
}

#[test]
fn entity_spawn_with_marker() {
    let mut world = World::new();

    let player = world.spawn((Position { x: 0.0, y: 0.0 }, Player)).id();
    let enemy = world.spawn((Position { x: 5.0, y: 5.0 }, Enemy)).id();

    assert!(world.entity(player).contains::<Player>());
    assert!(!world.entity(player).contains::<Enemy>());
    assert!(world.entity(enemy).contains::<Enemy>());
    assert!(!world.entity(enemy).contains::<Player>());
}

// =============================================================================
// CATEGORY 3: Query Pattern Tests
// =============================================================================

#[test]
fn query_basic_iteration() {
    let mut world = World::new();

    world.spawn((Position { x: 1.0, y: 1.0 },));
    world.spawn((Position { x: 2.0, y: 2.0 },));
    world.spawn((Position { x: 3.0, y: 3.0 },));

    let mut count = 0;
    let mut sum = 0.0;

    let mut state = SystemState::<Query<&Position>>::new(&mut world);
    let query = state.get(&world);

    for pos in query.iter() {
        count += 1;
        sum += pos.x;
    }

    assert_eq!(count, 3);
    assert!((sum - 6.0).abs() < f32::EPSILON);
}

#[test]
fn query_mutable_iteration() {
    let mut world = World::new();

    world.spawn((Health(100),));
    world.spawn((Health(50),));

    let mut state = SystemState::<Query<&mut Health>>::new(&mut world);
    let mut query = state.get_mut(&mut world);

    for mut health in query.iter_mut() {
        health.0 += 10;
    }

    state.apply(&mut world);

    let mut state2 = SystemState::<Query<&Health>>::new(&mut world);
    let query2 = state2.get(&world);

    let total: i32 = query2.iter().map(|h| h.0).sum();
    assert_eq!(total, 170);
}

#[test]
fn query_with_filter() {
    let mut world = World::new();

    world.spawn((Position { x: 1.0, y: 1.0 }, Player));
    world.spawn((Position { x: 2.0, y: 2.0 }, Enemy));
    world.spawn((Position { x: 3.0, y: 3.0 }, Player));

    let mut state = SystemState::<Query<&Position, With<Player>>>::new(&mut world);
    let query = state.get(&world);

    let count = query.iter().count();
    assert_eq!(count, 2);
}

#[test]
fn query_without_filter() {
    let mut world = World::new();

    world.spawn((Position { x: 1.0, y: 1.0 }, Player));
    world.spawn((Position { x: 2.0, y: 2.0 }, Enemy));
    world.spawn((Position { x: 3.0, y: 3.0 },));

    let mut state = SystemState::<Query<&Position, Without<Player>>>::new(&mut world);
    let query = state.get(&world);

    let count = query.iter().count();
    assert_eq!(count, 2);
}

#[test]
fn query_optional_component() {
    let mut world = World::new();

    world.spawn((Position { x: 1.0, y: 1.0 }, Velocity { x: 1.0, y: 0.0 }));
    world.spawn((Position { x: 2.0, y: 2.0 },));

    let mut state = SystemState::<Query<(&Position, Option<&Velocity>)>>::new(&mut world);
    let query = state.get(&world);

    let mut with_vel = 0;
    let mut without_vel = 0;

    for (_pos, vel) in query.iter() {
        if vel.is_some() {
            with_vel += 1;
        } else {
            without_vel += 1;
        }
    }

    assert_eq!(with_vel, 1);
    assert_eq!(without_vel, 1);
}

#[test]
fn query_get_single() {
    let mut world = World::new();

    world.spawn((Position { x: 5.0, y: 5.0 }, Player));

    let mut state = SystemState::<Query<&Position, With<Player>>>::new(&mut world);
    let query = state.get(&world);

    let result = query.single().unwrap();
    assert!((result.x - 5.0).abs() < f32::EPSILON);
}

#[test]
fn query_entity_access() {
    let mut world = World::new();

    let entity = world.spawn((Position { x: 1.0, y: 1.0 },)).id();

    let mut state = SystemState::<Query<(Entity, &Position)>>::new(&mut world);
    let query = state.get(&world);

    let (queried_entity, _pos) = query.single().unwrap();
    assert_eq!(queried_entity, entity);
}

#[test]
fn query_combinations_mut() {
    // This pattern is used in collision detection
    let mut world = World::new();

    world.spawn((Position { x: 1.0, y: 1.0 },));
    world.spawn((Position { x: 2.0, y: 2.0 },));
    world.spawn((Position { x: 3.0, y: 3.0 },));

    let mut state = SystemState::<Query<(Entity, &Position)>>::new(&mut world);
    let query = state.get(&world);

    let mut pairs = 0;
    for [_a, _b] in query.iter_combinations::<2>() {
        pairs += 1;
    }

    // 3 choose 2 = 3 pairs
    assert_eq!(pairs, 3);
}

#[test]
fn query_multiple_components() {
    let mut world = World::new();

    world.spawn((
        Position { x: 1.0, y: 1.0 },
        Velocity { x: 0.5, y: 0.5 },
        Health(100),
    ));

    let mut state = SystemState::<Query<(&Position, &Velocity, &Health)>>::new(&mut world);
    let query = state.get(&world);

    let (pos, vel, health) = query.single().unwrap();
    assert!((pos.x - 1.0).abs() < f32::EPSILON);
    assert!((vel.x - 0.5).abs() < f32::EPSILON);
    assert_eq!(health.0, 100);
}

#[test]
fn query_or_filter() {
    let mut world = World::new();

    world.spawn((Position { x: 1.0, y: 1.0 }, Player));
    world.spawn((Position { x: 2.0, y: 2.0 }, Enemy));
    world.spawn((Position { x: 3.0, y: 3.0 },)); // No marker

    let mut state =
        SystemState::<Query<&Position, Or<(With<Player>, With<Enemy>)>>>::new(&mut world);
    let query = state.get(&world);

    let count = query.iter().count();
    assert_eq!(count, 2);
}

#[test]
fn query_get_by_entity() {
    let mut world = World::new();

    let entity = world.spawn((Position { x: 42.0, y: 42.0 },)).id();
    world.spawn((Position { x: 0.0, y: 0.0 },));

    let mut state = SystemState::<Query<&Position>>::new(&mut world);
    let query = state.get(&world);

    let pos = query.get(entity).unwrap();
    assert!((pos.x - 42.0).abs() < f32::EPSILON);
}

// =============================================================================
// CATEGORY 4: Events & Observers Tests
// =============================================================================

#[test]
fn observer_receives_triggered_event() {
    let mut world = World::new();

    let event_received = Arc::new(Mutex::new(false));
    let event_received_clone = event_received.clone();

    world.add_observer(move |_trigger: On<SimpleEvent>| {
        *event_received_clone.lock().unwrap() = true;
    });
    world.flush();

    world.trigger(SimpleEvent(42));

    assert!(*event_received.lock().unwrap());
}

#[test]
fn observer_receives_event_data() {
    let mut world = World::new();

    let received_value = Arc::new(Mutex::new(0));
    let received_clone = received_value.clone();

    world.add_observer(move |trigger: On<SimpleEvent>| {
        *received_clone.lock().unwrap() = trigger.event().0;
    });
    world.flush();

    world.trigger(SimpleEvent(123));

    assert_eq!(*received_value.lock().unwrap(), 123);
}

#[test]
fn observer_multiple_triggers() {
    let mut world = World::new();

    let count = Arc::new(Mutex::new(0));
    let count_clone = count.clone();

    world.add_observer(move |_trigger: On<SimpleEvent>| {
        *count_clone.lock().unwrap() += 1;
    });
    world.flush();

    world.trigger(SimpleEvent(1));
    world.trigger(SimpleEvent(2));
    world.trigger(SimpleEvent(3));

    assert_eq!(*count.lock().unwrap(), 3);
}

#[test]
fn observer_spawned_as_entity() {
    // Pattern from engine: world.spawn((Observer::new(...), Persistent))
    let mut world = World::new();

    let event_received = Arc::new(Mutex::new(false));
    let event_received_clone = event_received.clone();

    let observer = Observer::new(move |_trigger: On<SimpleEvent>| {
        *event_received_clone.lock().unwrap() = true;
    });

    world.spawn((observer, Persistent));
    world.flush();

    world.trigger(SimpleEvent(42));

    assert!(*event_received.lock().unwrap());
}

#[test]
fn observer_with_entity_context() {
    let mut world = World::new();

    let entity_ids = Arc::new(Mutex::new(Vec::new()));
    let entity_ids_clone = entity_ids.clone();

    world.add_observer(move |trigger: On<CollisionEvent>| {
        let event = trigger.event();
        entity_ids_clone.lock().unwrap().push(event.a);
        entity_ids_clone.lock().unwrap().push(event.b);
    });
    world.flush();

    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();

    world.trigger(CollisionEvent { a: e1, b: e2 });

    let ids = entity_ids.lock().unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], e1);
    assert_eq!(ids[1], e2);
}

#[test]
fn observer_multiple_observers_same_event() {
    let mut world = World::new();

    let counter1 = Arc::new(Mutex::new(0));
    let counter2 = Arc::new(Mutex::new(0));
    let c1 = counter1.clone();
    let c2 = counter2.clone();

    world.add_observer(move |_trigger: On<SimpleEvent>| {
        *c1.lock().unwrap() += 1;
    });
    world.add_observer(move |_trigger: On<SimpleEvent>| {
        *c2.lock().unwrap() += 10;
    });
    world.flush();

    world.trigger(SimpleEvent(0));

    assert_eq!(*counter1.lock().unwrap(), 1);
    assert_eq!(*counter2.lock().unwrap(), 10);
}

#[test]
fn commands_trigger_event() {
    let mut world = World::new();

    let received = Arc::new(Mutex::new(false));
    let received_clone = received.clone();

    world.add_observer(move |_trigger: On<SimpleEvent>| {
        *received_clone.lock().unwrap() = true;
    });
    world.flush();

    // Use Commands to trigger
    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world);
    commands.trigger(SimpleEvent(1));
    state.apply(&mut world);

    assert!(*received.lock().unwrap());
}

#[test]
fn observer_different_event_types() {
    let mut world = World::new();

    let simple_count = Arc::new(Mutex::new(0));
    let damage_count = Arc::new(Mutex::new(0));
    let sc = simple_count.clone();
    let dc = damage_count.clone();

    world.add_observer(move |_trigger: On<SimpleEvent>| {
        *sc.lock().unwrap() += 1;
    });
    world.add_observer(move |_trigger: On<DamageEvent>| {
        *dc.lock().unwrap() += 1;
    });
    world.flush();

    let entity = world.spawn_empty().id();

    world.trigger(SimpleEvent(1));
    world.trigger(DamageEvent { entity, amount: 10 });
    world.trigger(SimpleEvent(2));

    assert_eq!(*simple_count.lock().unwrap(), 2);
    assert_eq!(*damage_count.lock().unwrap(), 1);
}

#[test]
fn world_flush_required_for_observers() {
    let mut world = World::new();

    let received = Arc::new(Mutex::new(false));
    let received_clone = received.clone();

    world.add_observer(move |_trigger: On<SimpleEvent>| {
        *received_clone.lock().unwrap() = true;
    });

    // Without flush, observer might not be registered
    world.flush();

    world.trigger(SimpleEvent(1));

    assert!(*received.lock().unwrap());
}

// =============================================================================
// CATEGORY 5: System Registration Tests
// =============================================================================

fn increment_counter(mut counter: ResMut<Counter>) {
    counter.0 += 1;
}

fn add_to_counter(amount: In<i32>, mut counter: ResMut<Counter>) {
    counter.0 += *amount;
}

fn entity_health_system(entity: In<Entity>, mut query: Query<&mut Health>) {
    if let Ok(mut health) = query.get_mut(*entity) {
        health.0 += 50;
    }
}

fn update_positions(mut query: Query<(&mut Position, &Velocity)>) {
    for (mut pos, vel) in query.iter_mut() {
        pos.x += vel.x;
        pos.y += vel.y;
    }
}

#[test]
fn system_register_and_run() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(increment_counter);
    world.run_system(system_id).unwrap();

    assert_eq!(world.resource::<Counter>().0, 1);
}

#[test]
fn system_run_multiple_times() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(increment_counter);
    world.run_system(system_id).unwrap();
    world.run_system(system_id).unwrap();
    world.run_system(system_id).unwrap();

    assert_eq!(world.resource::<Counter>().0, 3);
}

#[test]
fn system_with_input() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(add_to_counter);
    world.run_system_with(system_id, 10).unwrap();

    assert_eq!(world.resource::<Counter>().0, 10);
}

#[test]
fn system_with_entity_input() {
    let mut world = World::new();

    let entity = world.spawn((Health(100),)).id();

    let system_id = world.register_system(entity_health_system);
    world.run_system_with(system_id, entity).unwrap();

    assert_eq!(world.get::<Health>(entity).unwrap().0, 150);
}

#[test]
fn schedule_basic() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let mut schedule = Schedule::default();
    schedule.add_systems(increment_counter);

    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 1);
}

#[test]
fn schedule_multiple_systems() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let mut schedule = Schedule::default();
    schedule.add_systems(increment_counter);
    schedule.add_systems(increment_counter);

    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 2);
}

#[test]
fn schedule_system_ordering_chain() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    fn multiply_counter(mut counter: ResMut<Counter>) {
        counter.0 *= 2;
    }

    let mut schedule = Schedule::default();
    // Chain ensures: increment first, then multiply
    schedule.add_systems((increment_counter, multiply_counter).chain());

    schedule.run(&mut world);

    // (0 + 1) * 2 = 2
    assert_eq!(world.resource::<Counter>().0, 2);
}

#[test]
fn schedule_run_if_condition() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_resource(Config {
        debug_mode: true,
        max_entities: 100,
    });

    fn is_debug(config: Res<Config>) -> bool {
        config.debug_mode
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(increment_counter.run_if(is_debug));

    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 1);
}

#[test]
fn schedule_run_if_false() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_resource(Config {
        debug_mode: false,
        max_entities: 100,
    });

    fn is_debug(config: Res<Config>) -> bool {
        config.debug_mode
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(increment_counter.run_if(is_debug));

    schedule.run(&mut world);

    // System didn't run because condition was false
    assert_eq!(world.resource::<Counter>().0, 0);
}

#[test]
fn schedule_with_queries() {
    let mut world = World::new();

    world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 1.0 }));
    world.spawn((Position { x: 5.0, y: 5.0 }, Velocity { x: -1.0, y: -1.0 }));

    let mut schedule = Schedule::default();
    schedule.add_systems(update_positions);

    schedule.run(&mut world);

    let mut state = SystemState::<Query<&Position>>::new(&mut world);
    let query = state.get(&world);

    let positions: Vec<_> = query.iter().collect();
    assert!(
        (positions[0].x - 1.0).abs() < f32::EPSILON || (positions[0].x - 4.0).abs() < f32::EPSILON
    );
}

#[test]
fn system_commands_run_system() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(increment_counter);

    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world);
    commands.run_system(system_id);
    state.apply(&mut world);

    assert_eq!(world.resource::<Counter>().0, 1);
}

#[test]
fn schedule_after_ordering() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    fn set_to_ten(mut counter: ResMut<Counter>) {
        counter.0 = 10;
    }

    fn add_five(mut counter: ResMut<Counter>) {
        counter.0 += 5;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(set_to_ten);
    schedule.add_systems(add_five.after(set_to_ten));

    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 15);
}

// =============================================================================
// CATEGORY 6: MessageWriter/Reader Tests
// =============================================================================

#[derive(Debug, Clone, Message)]
struct TestMessage {
    value: i32,
}

#[test]
fn messages_write_and_read() {
    let mut world = World::new();
    world.init_resource::<Messages<TestMessage>>();

    // Write messages
    {
        let mut state = SystemState::<MessageWriter<TestMessage>>::new(&mut world);
        let mut writer = state.get_mut(&mut world);
        writer.write(TestMessage { value: 42 });
        writer.write(TestMessage { value: 100 });
        state.apply(&mut world);
    }

    // Update messages to make them readable
    world.resource_mut::<Messages<TestMessage>>().update();

    // Read messages
    {
        let mut state = SystemState::<MessageReader<TestMessage>>::new(&mut world);
        let mut reader = state.get_mut(&mut world);
        let messages: Vec<_> = reader.read().collect();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].value, 42);
        assert_eq!(messages[1].value, 100);
    }
}

#[test]
fn messages_write_batch() {
    let mut world = World::new();
    world.init_resource::<Messages<TestMessage>>();

    // Write batch
    {
        let mut state = SystemState::<MessageWriter<TestMessage>>::new(&mut world);
        let mut writer = state.get_mut(&mut world);
        writer.write_batch(vec![
            TestMessage { value: 1 },
            TestMessage { value: 2 },
            TestMessage { value: 3 },
        ]);
        state.apply(&mut world);
    }

    world.resource_mut::<Messages<TestMessage>>().update();

    // Count messages
    {
        let mut state = SystemState::<MessageReader<TestMessage>>::new(&mut world);
        let mut reader = state.get_mut(&mut world);
        let count = reader.read().count();
        assert_eq!(count, 3);
    }
}

#[test]
fn messages_cleared_after_update() {
    let mut world = World::new();
    world.init_resource::<Messages<TestMessage>>();

    // Write and update
    {
        let mut state = SystemState::<MessageWriter<TestMessage>>::new(&mut world);
        let mut writer = state.get_mut(&mut world);
        writer.write(TestMessage { value: 42 });
        state.apply(&mut world);
    }
    world.resource_mut::<Messages<TestMessage>>().update();

    // Read once
    {
        let mut state = SystemState::<MessageReader<TestMessage>>::new(&mut world);
        let mut reader = state.get_mut(&mut world);
        assert_eq!(reader.read().count(), 1);
    }

    // Update again (clears old messages)
    world.resource_mut::<Messages<TestMessage>>().update();

    // Read again - should be empty
    {
        let mut state = SystemState::<MessageReader<TestMessage>>::new(&mut world);
        let mut reader = state.get_mut(&mut world);
        assert_eq!(reader.read().count(), 0);
    }
}

#[test]
fn messages_in_systems() {
    let mut world = World::new();
    world.init_resource::<Messages<TestMessage>>();
    world.insert_resource(Counter(0));

    fn write_system(mut writer: MessageWriter<TestMessage>) {
        writer.write(TestMessage { value: 5 });
    }

    fn update_system(mut msgs: ResMut<Messages<TestMessage>>) {
        msgs.update();
    }

    fn read_system(mut reader: MessageReader<TestMessage>, mut counter: ResMut<Counter>) {
        for msg in reader.read() {
            counter.0 += msg.value;
        }
    }

    let mut schedule = Schedule::default();
    schedule.add_systems((write_system, update_system, read_system).chain());

    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 5);
}

#[test]
fn messages_multiple_readers() {
    let mut world = World::new();
    world.init_resource::<Messages<TestMessage>>();
    world.insert_resource(Counter(0));

    // Write message
    {
        let mut state = SystemState::<MessageWriter<TestMessage>>::new(&mut world);
        let mut writer = state.get_mut(&mut world);
        writer.write(TestMessage { value: 10 });
        state.apply(&mut world);
    }
    world.resource_mut::<Messages<TestMessage>>().update();

    // First reader
    {
        let mut state = SystemState::<MessageReader<TestMessage>>::new(&mut world);
        let mut reader = state.get_mut(&mut world);
        let sum: i32 = reader.read().map(|m| m.value).sum();
        assert_eq!(sum, 10);
    }

    // Second reader should still see the messages (they're only cleared on next update)
    {
        let mut state = SystemState::<MessageReader<TestMessage>>::new(&mut world);
        let mut reader = state.get_mut(&mut world);
        let sum: i32 = reader.read().map(|m| m.value).sum();
        assert_eq!(sum, 10);
    }
}

#[test]
fn messages_from_iterator() {
    let mut world = World::new();
    world.init_resource::<Messages<TestMessage>>();

    // Write from iterator
    {
        let mut state = SystemState::<MessageWriter<TestMessage>>::new(&mut world);
        let mut writer = state.get_mut(&mut world);
        writer.write_batch((0..5).map(|i| TestMessage { value: i }));
        state.apply(&mut world);
    }

    world.resource_mut::<Messages<TestMessage>>().update();

    // Verify all messages
    {
        let mut state = SystemState::<MessageReader<TestMessage>>::new(&mut world);
        let mut reader = state.get_mut(&mut world);
        let values: Vec<_> = reader.read().map(|m| m.value).collect();
        assert_eq!(values, vec![0, 1, 2, 3, 4]);
    }
}

// =============================================================================
// CATEGORY 7: SystemParam Tests
// =============================================================================

/// Custom SystemParam bundling multiple resources (like engine's RenderResources)
#[derive(SystemParam)]
struct BundledResources<'w> {
    counter: ResMut<'w, Counter>,
    config: Res<'w, Config>,
}

/// Custom SystemParam with optional resource (like engine's maybe_debug pattern)
#[derive(SystemParam)]
struct OptionalResources<'w> {
    counter: ResMut<'w, Counter>,
    config: Option<Res<'w, Config>>,
}

#[test]
fn system_param_basic() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_resource(Config::default());

    fn use_bundled(mut res: BundledResources) {
        if !res.config.debug_mode {
            res.counter.0 += 1;
        }
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(use_bundled);
    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 1);
}

#[test]
fn system_param_optional_present() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_resource(Config {
        debug_mode: true,
        max_entities: 0,
    });

    fn use_optional(mut res: OptionalResources) {
        if let Some(config) = &res.config {
            if config.debug_mode {
                res.counter.0 += 10;
            }
        }
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(use_optional);
    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 10);
}

#[test]
fn system_param_optional_missing() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    // Config NOT inserted

    fn use_optional(mut res: OptionalResources) {
        if res.config.is_none() {
            res.counter.0 += 5;
        }
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(use_optional);
    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 5);
}

/// SystemParam with queries (like engine's ContextQueries)
#[derive(SystemParam)]
struct EntityQueries<'w, 's> {
    positions: Query<'w, 's, &'static Position>,
    velocities: Query<'w, 's, &'static Velocity>,
}

#[test]
fn system_param_with_queries() {
    let mut world = World::new();

    world.spawn((Position { x: 1.0, y: 1.0 }, Velocity { x: 0.5, y: 0.5 }));
    world.spawn((Position { x: 2.0, y: 2.0 },));

    world.insert_resource(Counter(0));

    fn count_entities(queries: EntityQueries, mut counter: ResMut<Counter>) {
        counter.0 = queries.positions.iter().count() as i32;
        counter.0 += queries.velocities.iter().count() as i32 * 10;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(count_entities);
    schedule.run(&mut world);

    // 2 positions + 1 velocity * 10 = 2 + 10 = 12
    assert_eq!(world.resource::<Counter>().0, 12);
}

#[test]
fn system_param_in_system_state() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_resource(Config::default());

    let mut state = SystemState::<BundledResources>::new(&mut world);
    let mut res = state.get_mut(&mut world);
    res.counter.0 = 99;
    state.apply(&mut world);

    assert_eq!(world.resource::<Counter>().0, 99);
}

#[test]
fn system_param_multiple_instances() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_resource(Config {
        debug_mode: true,
        max_entities: 50,
    });

    fn system1(mut res: BundledResources) {
        res.counter.0 += res.config.max_entities as i32;
    }

    fn system2(mut res: BundledResources) {
        res.counter.0 += res.config.max_entities as i32;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems((system1, system2).chain());
    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 100);
}

// =============================================================================
// CATEGORY 8: Commands Tests
// =============================================================================

#[test]
fn commands_spawn_entity() {
    let mut world = World::new();

    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world);

    commands.spawn((Position { x: 10.0, y: 20.0 },));

    state.apply(&mut world);

    let mut query_state = SystemState::<Query<&Position>>::new(&mut world);
    let query = query_state.get(&world);

    assert_eq!(query.iter().count(), 1);
    let pos = query.single().unwrap();
    assert!((pos.x - 10.0).abs() < f32::EPSILON);
}

#[test]
fn commands_entity_insert() {
    let mut world = World::new();

    let entity = world.spawn((Position { x: 0.0, y: 0.0 },)).id();

    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world);

    commands.entity(entity).insert(Velocity { x: 1.0, y: 2.0 });

    state.apply(&mut world);

    let vel = world.get::<Velocity>(entity).unwrap();
    assert!((vel.x - 1.0).abs() < f32::EPSILON);
}

#[test]
fn commands_entity_despawn() {
    let mut world = World::new();

    let entity = world.spawn((Position { x: 0.0, y: 0.0 },)).id();

    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world);

    commands.entity(entity).despawn();

    state.apply(&mut world);

    assert!(world.get_entity(entity).is_err());
}

#[test]
fn commands_insert_resource() {
    let mut world = World::new();

    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world);

    commands.insert_resource(Counter(42));

    state.apply(&mut world);

    assert_eq!(world.resource::<Counter>().0, 42);
}

#[test]
fn commands_in_system() {
    let mut world = World::new();

    fn spawner_system(mut commands: Commands) {
        commands.spawn((Position { x: 5.0, y: 5.0 }, Player));
        commands.spawn((Position { x: 10.0, y: 10.0 }, Enemy));
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(spawner_system);
    schedule.run(&mut world);

    let mut player_state = SystemState::<Query<&Position, With<Player>>>::new(&mut world);
    let mut enemy_state = SystemState::<Query<&Position, With<Enemy>>>::new(&mut world);

    let player_query = player_state.get(&world);
    let enemy_query = enemy_state.get(&world);

    assert_eq!(player_query.iter().count(), 1);
    assert_eq!(enemy_query.iter().count(), 1);
}

// =============================================================================
// CATEGORY 9: Local/NonSend Resource Tests
// =============================================================================

#[test]
fn local_state_persists() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    fn system_with_local(mut local: Local<i32>, mut counter: ResMut<Counter>) {
        *local += 1;
        counter.0 = *local;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(system_with_local);

    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 1);

    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 2);

    schedule.run(&mut world);
    assert_eq!(world.resource::<Counter>().0, 3);
}

#[test]
fn local_default_value() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    fn system_with_local(local: Local<Vec<i32>>, mut counter: ResMut<Counter>) {
        counter.0 = local.len() as i32;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(system_with_local);
    schedule.run(&mut world);

    // Vec::default() is empty
    assert_eq!(world.resource::<Counter>().0, 0);
}

#[test]
fn local_accumulator() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    fn accumulator_system(mut buffer: Local<Vec<i32>>, mut counter: ResMut<Counter>) {
        buffer.push(counter.0);
        counter.0 = buffer.len() as i32;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(accumulator_system);

    schedule.run(&mut world);
    schedule.run(&mut world);
    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 3);
}

/// NonSend resource (like raylib handles)
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

#[test]
fn non_send_resource_insert_and_access() {
    let mut world = World::new();

    world.insert_non_send_resource(NonSendHandle::new(42));

    let handle = world.non_send_resource::<NonSendHandle>();
    assert_eq!(handle.value, 42);
}

#[test]
fn non_send_resource_mut() {
    let mut world = World::new();

    world.insert_non_send_resource(NonSendHandle::new(0));

    {
        let mut handle = world.non_send_resource_mut::<NonSendHandle>();
        handle.value = 100;
    }

    let handle = world.non_send_resource::<NonSendHandle>();
    assert_eq!(handle.value, 100);
}

#[test]
fn non_send_in_system() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.insert_non_send_resource(NonSendHandle::new(50));

    fn use_non_send(handle: NonSend<NonSendHandle>, mut counter: ResMut<Counter>) {
        counter.0 = handle.value;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(use_non_send);
    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 50);
}

#[test]
fn non_send_mut_in_system() {
    let mut world = World::new();
    world.insert_non_send_resource(NonSendHandle::new(0));

    fn mutate_non_send(mut handle: NonSendMut<NonSendHandle>) {
        handle.value = 999;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(mutate_non_send);
    schedule.run(&mut world);

    let handle = world.non_send_resource::<NonSendHandle>();
    assert_eq!(handle.value, 999);
}

// =============================================================================
// CATEGORY 10: Edge Cases & Compatibility Tests
// =============================================================================

#[test]
fn entity_generation_after_despawn() {
    let mut world = World::new();

    let entity1 = world.spawn((Health(100),)).id();
    world.despawn(entity1);

    let entity2 = world.spawn((Health(50),)).id();

    // New entity should be valid
    assert!(world.get_entity(entity2).is_ok());
    assert_eq!(world.get::<Health>(entity2).unwrap().0, 50);
}

#[test]
fn query_empty_world() {
    let mut world = World::new();

    let mut state = SystemState::<Query<&Position>>::new(&mut world);
    let query = state.get(&world);

    assert_eq!(query.iter().count(), 0);
}

#[test]
fn resource_replace() {
    let mut world = World::new();

    world.insert_resource(Counter(1));
    world.insert_resource(Counter(2));

    // Later insert should replace
    assert_eq!(world.resource::<Counter>().0, 2);
}

#[test]
fn observer_entity_target() {
    // Observer triggered on specific entity
    let mut world = World::new();

    let received_entities = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received_entities.clone();

    let target_entity = world.spawn((Health(100),)).id();

    world.add_observer(move |trigger: On<DamageEvent>| {
        received_clone.lock().unwrap().push(trigger.event().entity);
    });
    world.flush();

    world.trigger(DamageEvent {
        entity: target_entity,
        amount: 10,
    });

    let received = received_entities.lock().unwrap();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], target_entity);
}

#[test]
fn schedule_initialize() {
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let mut schedule = Schedule::default();
    schedule.add_systems(increment_counter);

    // Initialize prepares the schedule
    schedule
        .initialize(&mut world)
        .expect("Failed to initialize schedule");
    schedule.run(&mut world);

    assert_eq!(world.resource::<Counter>().0, 1);
}

// =============================================================================
// CATEGORY 11: Observer Persistence Tests
// =============================================================================
// These tests verify that observers survive entity cleanup operations when
// marked with a persistence component. This is critical for the engine's
// scene switching functionality.

#[derive(Event, Debug, Clone)]
struct PersistenceTestEvent(i32);

#[test]
fn observer_without_persistent_is_despawned_by_cleanup() {
    // This test demonstrates the problem: observers spawned as entities
    // can be accidentally despawned by scene cleanup queries like:
    // Query<Entity, Without<Persistent>>
    let mut world = World::new();

    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();

    // Spawn observer WITHOUT Persistent marker
    let observer = Observer::new(move |_trigger: On<PersistenceTestEvent>| {
        *counter_clone.lock().unwrap() += 1;
    });
    let observer_entity = world.spawn(observer).id();
    world.flush();

    // Verify observer works before cleanup
    world.trigger(PersistenceTestEvent(1));
    assert_eq!(
        *counter.lock().unwrap(),
        1,
        "Observer should fire before cleanup"
    );

    // Simulate scene cleanup: despawn all entities without Persistent
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // Verify observer entity was despawned
    assert!(
        world.get_entity(observer_entity).is_err(),
        "Observer entity should be despawned by cleanup"
    );

    // Trigger event again - observer should NOT fire (it was despawned)
    world.trigger(PersistenceTestEvent(2));
    assert_eq!(
        *counter.lock().unwrap(),
        1,
        "Observer should NOT fire after being despawned"
    );
}

#[test]
fn observer_with_persistent_survives_cleanup() {
    // This test shows the solution: mark observers with Persistent
    // so they survive scene cleanup
    let mut world = World::new();

    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();

    // Spawn observer WITH Persistent marker
    let observer = Observer::new(move |_trigger: On<PersistenceTestEvent>| {
        *counter_clone.lock().unwrap() += 1;
    });
    let observer_entity = world.spawn((observer, Persistent)).id();
    world.flush();

    // Verify observer works before cleanup
    world.trigger(PersistenceTestEvent(1));
    assert_eq!(
        *counter.lock().unwrap(),
        1,
        "Observer should fire before cleanup"
    );

    // Simulate scene cleanup: despawn all entities without Persistent
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // Verify observer entity was NOT despawned
    assert!(
        world.get_entity(observer_entity).is_ok(),
        "Observer entity with Persistent should survive cleanup"
    );

    // Trigger event again - observer SHOULD still fire
    world.trigger(PersistenceTestEvent(2));
    assert_eq!(
        *counter.lock().unwrap(),
        2,
        "Observer with Persistent should still fire after cleanup"
    );
}

#[test]
fn multiple_observers_mixed_persistence() {
    // Test that only persistent observers survive when mixed
    let mut world = World::new();

    let persistent_counter = Arc::new(Mutex::new(0));
    let non_persistent_counter = Arc::new(Mutex::new(0));
    let pc = persistent_counter.clone();
    let npc = non_persistent_counter.clone();

    // Spawn persistent observer
    let persistent_observer = Observer::new(move |_trigger: On<PersistenceTestEvent>| {
        *pc.lock().unwrap() += 1;
    });
    world.spawn((persistent_observer, Persistent));

    // Spawn non-persistent observer
    let non_persistent_observer = Observer::new(move |_trigger: On<PersistenceTestEvent>| {
        *npc.lock().unwrap() += 1;
    });
    world.spawn(non_persistent_observer);

    // Also spawn some regular entities to ensure they're cleaned up too
    world.spawn((Position { x: 0.0, y: 0.0 },));
    world.spawn((Position { x: 1.0, y: 1.0 }, Velocity { x: 1.0, y: 1.0 }));

    world.flush();

    // Both observers should fire before cleanup
    world.trigger(PersistenceTestEvent(1));
    assert_eq!(*persistent_counter.lock().unwrap(), 1);
    assert_eq!(*non_persistent_counter.lock().unwrap(), 1);

    // Simulate scene cleanup
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // Trigger again after cleanup
    world.trigger(PersistenceTestEvent(2));

    // Only persistent observer should have incremented
    assert_eq!(
        *persistent_counter.lock().unwrap(),
        2,
        "Persistent observer should fire twice"
    );
    assert_eq!(
        *non_persistent_counter.lock().unwrap(),
        1,
        "Non-persistent observer should only fire once (before cleanup)"
    );
}

#[test]
fn commands_trigger_works_with_persistent_observer() {
    // Verify that Commands::trigger works correctly with persistent observers
    // This is the pattern used in the engine's input system
    let mut world = World::new();

    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();

    // Spawn observer with Persistent (engine pattern)
    let observer = Observer::new(move |_trigger: On<PersistenceTestEvent>| {
        *counter_clone.lock().unwrap() += 1;
    });
    world.spawn((observer, Persistent));
    world.flush();

    // Simulate scene cleanup BEFORE using Commands::trigger
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // Use Commands to trigger (like the input system does)
    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world);
    commands.trigger(PersistenceTestEvent(42));
    state.apply(&mut world);

    assert_eq!(
        *counter.lock().unwrap(),
        1,
        "Commands::trigger should work with persistent observer after cleanup"
    );
}

// =============================================================================
// CATEGORY 12: Registered System Persistence Tests
// =============================================================================
// These tests verify that registered systems (via world.register_system()) are
// stored as entities in bevy_ecs 0.18+. This means they can be accidentally
// despawned by scene cleanup queries unless marked with Persistent.
// This was a breaking change discovered when upgrading from 0.17 to 0.18.

fn persistence_test_system(mut counter: ResMut<Counter>) {
    counter.0 += 1;
}

fn persistence_test_system_with_input(amount: In<i32>, mut counter: ResMut<Counter>) {
    counter.0 += *amount;
}

#[test]
fn registered_system_is_an_entity() {
    // In bevy_ecs 0.18+, register_system() creates an entity to hold the system.
    // This test verifies that behavior.
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(persistence_test_system);

    // SystemId has an .entity() method that returns the underlying Entity
    let system_entity = system_id.entity();

    // The entity should exist in the world
    assert!(
        world.get_entity(system_entity).is_ok(),
        "Registered system should be stored as an entity"
    );

    // The system should still work
    world.run_system(system_id).unwrap();
    assert_eq!(world.resource::<Counter>().0, 1);
}

#[test]
fn registered_system_without_persistent_is_despawned_by_cleanup() {
    // This test demonstrates the problem: registered systems are entities
    // and can be accidentally despawned by scene cleanup queries like:
    // Query<Entity, Without<Persistent>>
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(persistence_test_system);
    let system_entity = system_id.entity();

    // Verify system works before cleanup
    world.run_system(system_id).unwrap();
    assert_eq!(
        world.resource::<Counter>().0,
        1,
        "System should work before cleanup"
    );

    // Simulate scene cleanup: despawn all entities without Persistent
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // Verify system entity was despawned
    assert!(
        world.get_entity(system_entity).is_err(),
        "System entity should be despawned by cleanup"
    );

    // Attempting to run the system should now fail
    let result = world.run_system(system_id);
    assert!(
        result.is_err(),
        "Running a despawned system should return an error"
    );
}

#[test]
fn registered_system_with_persistent_survives_cleanup() {
    // This test shows the solution: mark registered system entities with Persistent
    // so they survive scene cleanup
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(persistence_test_system);
    let system_entity = system_id.entity();

    // Mark the system entity as Persistent
    world.entity_mut(system_entity).insert(Persistent);

    // Verify system works before cleanup
    world.run_system(system_id).unwrap();
    assert_eq!(
        world.resource::<Counter>().0,
        1,
        "System should work before cleanup"
    );

    // Simulate scene cleanup: despawn all entities without Persistent
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // Verify system entity was NOT despawned
    assert!(
        world.get_entity(system_entity).is_ok(),
        "System entity with Persistent should survive cleanup"
    );

    // System should still work after cleanup
    world.run_system(system_id).unwrap();
    assert_eq!(
        world.resource::<Counter>().0,
        2,
        "System with Persistent should still work after cleanup"
    );
}

#[test]
fn registered_system_with_input_survives_cleanup_when_persistent() {
    // Test that systems with In<T> input also work correctly with Persistent
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(persistence_test_system_with_input);
    let system_entity = system_id.entity();

    // Mark the system entity as Persistent
    world.entity_mut(system_entity).insert(Persistent);

    // Verify system works before cleanup
    world.run_system_with(system_id, 10).unwrap();
    assert_eq!(
        world.resource::<Counter>().0,
        10,
        "System should work before cleanup"
    );

    // Simulate scene cleanup
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // System should still work after cleanup
    world.run_system_with(system_id, 5).unwrap();
    assert_eq!(
        world.resource::<Counter>().0,
        15,
        "System with input should still work after cleanup"
    );
}

#[test]
fn multiple_registered_systems_mixed_persistence() {
    // Test that only persistent systems survive when mixed
    let mut world = World::new();
    world.insert_resource(Counter(0));

    // Register two systems
    let persistent_system_id = world.register_system(persistence_test_system);
    let non_persistent_system_id = world.register_system(persistence_test_system);

    // Mark only one as Persistent
    world
        .entity_mut(persistent_system_id.entity())
        .insert(Persistent);

    // Both should work before cleanup
    world.run_system(persistent_system_id).unwrap();
    world.run_system(non_persistent_system_id).unwrap();
    assert_eq!(
        world.resource::<Counter>().0,
        2,
        "Both systems should work before cleanup"
    );

    // Simulate scene cleanup
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // Persistent system should still work
    world.run_system(persistent_system_id).unwrap();
    assert_eq!(
        world.resource::<Counter>().0,
        3,
        "Persistent system should still work"
    );

    // Non-persistent system should fail
    let result = world.run_system(non_persistent_system_id);
    assert!(
        result.is_err(),
        "Non-persistent system should fail after cleanup"
    );
}

#[test]
fn commands_run_system_works_with_persistent_system() {
    // Verify that Commands::run_system works correctly with persistent systems
    // This is the pattern used in the engine's menu selection observer
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(persistence_test_system);

    // Mark the system entity as Persistent (engine pattern)
    world.entity_mut(system_id.entity()).insert(Persistent);

    // Simulate scene cleanup BEFORE using Commands::run_system
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // Use Commands to run system (like the menu selection observer does)
    let mut state = SystemState::<Commands>::new(&mut world);
    let mut commands = state.get_mut(&mut world);
    commands.run_system(system_id);
    state.apply(&mut world);

    assert_eq!(
        world.resource::<Counter>().0,
        1,
        "Commands::run_system should work with persistent system after cleanup"
    );
}

#[test]
fn system_entity_can_have_additional_components() {
    // Test that we can add other components to the system entity if needed
    let mut world = World::new();
    world.insert_resource(Counter(0));

    let system_id = world.register_system(persistence_test_system);
    let system_entity = system_id.entity();

    // Add multiple components to the system entity
    world
        .entity_mut(system_entity)
        .insert((Persistent, Name("switch_scene".to_string())));

    // Verify components are present
    assert!(world.entity(system_entity).contains::<Persistent>());
    assert!(world.entity(system_entity).contains::<Name>());

    // System should still work
    world.run_system(system_id).unwrap();
    assert_eq!(world.resource::<Counter>().0, 1);

    // Simulate cleanup
    let entities_to_despawn: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();

    for entity in entities_to_despawn {
        world.despawn(entity);
    }

    // System with extra components should survive and work
    world.run_system(system_id).unwrap();
    assert_eq!(world.resource::<Counter>().0, 2);
}
