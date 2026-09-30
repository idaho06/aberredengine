//! Integration tests for Rust `CollisionRule` dispatch.
//!
//! Runs the collision pipeline end to end in an ECS world:
//! `rebuild_collision_rule_index` -> `collision_detector` ->
//! `rust_collision_observer`, then checks which rule callbacks fired and with
//! which arguments (group matching, entity ordering, contact sides, and
//! lowest-entity-wins when several rules cover the same pair).

use bevy_ecs::prelude::*;

use aberred_core::components::boxcollider::BoxCollider;
use aberred_core::components::collision::{BoxSides, CollisionRule};
use aberred_core::components::group::Group;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::signals::Signals;
use aberred_core::resources::collision_rule_index::CollisionRuleIndex;
use aberred_core::systems::GameCtx;
use aberred_core::systems::collision_detector::collision_detector;
use aberred_core::systems::collision_rule_index::rebuild_collision_rule_index;
use aberred_core::systems::rust_collision::rust_collision_observer;
use aberred_core::testing::insert_game_ctx_resources;

/// `GameCtx`'s resources plus the `CollisionRuleIndex` the observer reads.
fn make_world() -> World {
    let mut world = World::new();
    insert_game_ctx_resources(&mut world);
    world.insert_resource(CollisionRuleIndex::default());
    world
}

fn tick_collision_detector(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_collision_rule_index.before(collision_detector));
    schedule.add_systems(collision_detector);
    schedule.run(world);
}

#[test]
fn collision_rule_callback_fires_on_matching_groups() {
    let mut world = make_world();

    fn on_collision(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        if let Ok(mut signals) = ctx.signals.get_mut(ent_a) {
            signals.set_flag("collided");
        }
    }

    let a = world
        .spawn((
            Group::new("ball"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((
        Group::new("brick"),
        MapPosition::new(5.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    world.spawn((CollisionRule::rust("ball", "brick", on_collision),));

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let signals = world.get::<Signals>(a).unwrap();
    assert!(signals.has_flag("collided"));
}

#[test]
fn collision_rule_callback_not_fired_on_non_matching_groups() {
    let mut world = make_world();

    fn on_collision(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        if let Ok(mut signals) = ctx.signals.get_mut(ent_a) {
            signals.set_flag("should_not_fire");
        }
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
    // Rule is for "ball" vs "brick", not "player" vs "enemy"
    world.spawn((CollisionRule::rust("ball", "brick", on_collision),));

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let signals = world.get::<Signals>(a).unwrap();
    assert!(!signals.has_flag("should_not_fire"));
}

#[test]
fn collision_rule_entities_ordered_correctly_when_groups_swapped() {
    let mut world = make_world();

    // Callback expects entity_a to be "ball" (group_a of the rule).
    // It sets a flag on entity_a to prove ordering is correct.
    fn on_collision(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        // ent_a should be ball (group_a of rule)
        if let Ok(group) = ctx.groups.get(ent_a)
            && group.name() == "ball"
            && let Ok(mut signals) = ctx.signals.get_mut(ent_a)
        {
            signals.set_flag("ball_is_first");
        }
    }

    // Spawn "brick" first so it gets a lower Entity id.
    // The collision detector will report (brick, ball) but the rule
    // defines group_a="ball", so the observer must reorder them.
    let brick = world
        .spawn((
            Group::new("brick"),
            MapPosition::new(5.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    let ball = world
        .spawn((
            Group::new("ball"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((CollisionRule::rust("ball", "brick", on_collision),));

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let ball_signals = world.get::<Signals>(ball).unwrap();
    assert!(ball_signals.has_flag("ball_is_first"));
    // brick should NOT have the flag
    let brick_signals = world.get::<Signals>(brick).unwrap();
    assert!(!brick_signals.has_flag("ball_is_first"));
}

#[test]
fn collision_rule_sides_passed_to_callback() {
    let mut world = make_world();

    // rect_a is at (0,0) 10x10, rect_b is at (8,0) 10x10
    // → rect_a's right side collides, rect_b's left side collides
    fn on_collision(
        ent_a: Entity,
        _ent_b: Entity,
        sides_a: &BoxSides,
        sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        use aberred_core::components::collision::BoxSide;
        let has_right_a = sides_a.iter().any(|s| matches!(s, BoxSide::Right));
        let has_left_b = sides_b.iter().any(|s| matches!(s, BoxSide::Left));
        if has_right_a
            && has_left_b
            && let Ok(mut signals) = ctx.signals.get_mut(ent_a)
        {
            signals.set_flag("sides_correct");
        }
    }

    let a = world
        .spawn((
            Group::new("ball"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((
        Group::new("brick"),
        MapPosition::new(8.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));
    world.spawn((CollisionRule::rust("ball", "brick", on_collision),));

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let signals = world.get::<Signals>(a).unwrap();
    assert!(signals.has_flag("sides_correct"));
}

/// When multiple rules cover the same group pair, first-match is
/// deterministic (lowest `Entity` wins), not query-iteration order. Per
/// `.claude/context/system-order.md`, `Entity`'s `Ord` does NOT correlate
/// with spawn order (its niche encoding stores `!index`), so this test
/// determines which of the two rules has the lower id *after* spawning
/// both, rather than assuming spawn order predicts it.
#[test]
fn collision_rule_same_pair_multiple_rules_lowest_entity_wins() {
    let mut world = make_world();

    fn on_collision_first(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        if let Ok(mut signals) = ctx.signals.get_mut(ent_a) {
            signals.set_flag("first_rule_fired");
        }
    }

    fn on_collision_second(
        ent_a: Entity,
        _ent_b: Entity,
        _sides_a: &BoxSides,
        _sides_b: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        if let Ok(mut signals) = ctx.signals.get_mut(ent_a) {
            signals.set_flag("second_rule_fired");
        }
    }

    let a = world
        .spawn((
            Group::new("ball"),
            MapPosition::new(0.0, 0.0),
            BoxCollider::new(10.0, 10.0),
            Signals::default(),
        ))
        .id();
    world.spawn((
        Group::new("brick"),
        MapPosition::new(5.0, 0.0),
        BoxCollider::new(10.0, 10.0),
    ));

    let rule_1 = world
        .spawn(CollisionRule::rust("ball", "brick", on_collision_first))
        .id();
    let rule_2 = world
        .spawn(CollisionRule::rust("ball", "brick", on_collision_second))
        .id();
    let first_rule_has_lower_entity = rule_1 < rule_2;

    world.add_observer(rust_collision_observer);
    world.flush();

    tick_collision_detector(&mut world);

    let (expected, unexpected) = if first_rule_has_lower_entity {
        ("first_rule_fired", "second_rule_fired")
    } else {
        ("second_rule_fired", "first_rule_fired")
    };
    let signals = world.get::<Signals>(a).unwrap();
    assert!(signals.has_flag(expected));
    assert!(!signals.has_flag(unexpected));
}
