//! Integration tests for `CollisionRule` dispatch.
//!
//! Runs the collision pipeline end to end in an ECS world:
//! `rebuild_rule_index::<CollisionRule>` -> `collision_detector` ->
//! `collision_rule_observer`, then checks which `Collided` events reached
//! observers and with which fields (group matching, entity ordering, contact
//! sides, and lowest-entity-wins when several rules cover the same pair).

use bevy_ecs::prelude::*;

use aberred_core::components::boxcollider::BoxCollider;
use aberred_core::components::collision::{BoxSide, CollisionRule};
use aberred_core::components::group::Group;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::events::collision::Collided;
use aberred_core::resources::collision_contacts::CollisionContacts;
use aberred_core::resources::collision_rule_index::CollisionRuleIndex;
use aberred_core::systems::collision_detector::collision_detector;
use aberred_core::systems::collision_rule::collision_rule_observer;
use aberred_core::systems::collision_rule_index::rebuild_rule_index;

/// Every `Collided` an observer received, in trigger order.
#[derive(Resource, Default)]
struct Hits(Vec<Collided>);

fn record(trigger: On<Collided>, mut hits: ResMut<Hits>) {
    hits.0.push(trigger.event().clone());
}

fn make_world() -> World {
    let mut world = World::new();
    world.insert_resource(CollisionRuleIndex::default());
    world.insert_resource(CollisionContacts::default());
    world.init_resource::<Hits>();
    world.add_observer(collision_rule_observer);
    world
}

fn tick(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_rule_index::<CollisionRule>.before(collision_detector));
    schedule.add_systems(collision_detector);
    schedule.run(world);
}

fn spawn_collider(world: &mut World, group: &str, x: f32) -> Entity {
    world
        .spawn((
            Group::new(group),
            MapPosition::new(x, 0.0),
            BoxCollider::new(10.0, 10.0),
        ))
        .id()
}

#[test]
fn rule_observer_gets_collided_with_entities_in_group_order() {
    let mut world = make_world();
    // Spawn "brick" first so the detector may report (brick, ball); the rule
    // names "ball" first, so the event must reorder them.
    let brick = spawn_collider(&mut world, "brick", 5.0);
    let ball = spawn_collider(&mut world, "ball", 0.0);
    let rule = world
        .spawn(CollisionRule::new("ball", "brick"))
        .observe(record)
        .id();

    tick(&mut world);

    let hits = &world.resource::<Hits>().0;
    assert_eq!(hits.len(), 1);
    assert_eq!((hits[0].rule, hits[0].a, hits[0].b), (rule, ball, brick));
}

#[test]
fn collided_carries_the_touching_sides() {
    let mut world = make_world();
    // a at x=0, b at x=8, both 10 wide: a's right side meets b's left side.
    spawn_collider(&mut world, "ball", 0.0);
    spawn_collider(&mut world, "brick", 8.0);
    world
        .spawn(CollisionRule::new("ball", "brick"))
        .observe(record);

    tick(&mut world);

    let hit = &world.resource::<Hits>().0[0];
    assert!(hit.sides_a.contains(&BoxSide::Right) && hit.sides_b.contains(&BoxSide::Left));
}

/// When multiple rules cover the same group pair, first-match is
/// deterministic (lowest `Entity` wins), not query-iteration order.
/// `Entity`'s `Ord` does NOT correlate with spawn order (its niche encoding
/// stores `!index`), so the test compares ids after spawning both rules.
#[test]
fn only_the_lowest_entity_rule_fires_for_a_pair() {
    let mut world = make_world();
    spawn_collider(&mut world, "ball", 0.0);
    spawn_collider(&mut world, "brick", 5.0);
    let rule_1 = world.spawn(CollisionRule::new("ball", "brick")).id();
    let rule_2 = world.spawn(CollisionRule::new("ball", "brick")).id();
    world.add_observer(record);

    tick(&mut world);

    let hits = &world.resource::<Hits>().0;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].rule, rule_1.min(rule_2));
}

#[test]
fn collided_fires_every_tick_while_overlapping() {
    let mut world = make_world();
    spawn_collider(&mut world, "ball", 0.0);
    spawn_collider(&mut world, "brick", 5.0);
    world
        .spawn(CollisionRule::new("ball", "brick"))
        .observe(record);

    for expected in 1..=3 {
        tick(&mut world);
        assert_eq!(world.resource::<Hits>().0.len(), expected);
    }
}

#[test]
fn no_collided_for_a_pair_without_a_rule() {
    let mut world = make_world();
    spawn_collider(&mut world, "player", 0.0);
    spawn_collider(&mut world, "enemy", 5.0);
    world.spawn(CollisionRule::new("ball", "brick"));
    world.add_observer(record);

    tick(&mut world);

    assert!(world.resource::<Hits>().0.is_empty());
}

#[test]
fn global_and_rule_observers_both_get_collided() {
    let mut world = make_world();
    spawn_collider(&mut world, "ball", 0.0);
    spawn_collider(&mut world, "brick", 5.0);
    world
        .spawn(CollisionRule::new("ball", "brick"))
        .observe(record);
    world.add_observer(record);

    tick(&mut world);

    assert_eq!(world.resource::<Hits>().0.len(), 2);
}
