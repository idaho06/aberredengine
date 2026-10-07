//! Timer system.
//!
//! [`update_timers`] advances every [`Timer`] once per sim tick and triggers a
//! [`TimerFired`] event on each timer that reaches its duration.
//!
//! # Related
//!
//! - [`crate::components::timer::Timer`] – the timer component
//! - [`crate::events::timer::TimerFired`] – event triggered on expiration
//! - `aberred_lua::systems::lua_timer_fired` – calls a Lua function when a timer fires

use bevy_ecs::change_detection::Tick;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemChangeTick;

use crate::components::timer::{Timer, TimerMode};
use crate::events::timer::TimerFired;
use crate::resources::worldtime::WorldTime;

/// Advance all [`Timer`] components and trigger [`TimerFired`] when they expire.
///
/// Accumulates delta time on each timer and fires when `elapsed >= duration`,
/// at most once per tick, then applies the timer's [`TimerMode`].
pub fn update_timers(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut Timer)>,
    mut commands: Commands,
    ticks: SystemChangeTick,
) {
    let fired_at = ticks.this_run();
    for (entity, mut timer) in query.iter_mut() {
        if timer.advance(world_time.delta) {
            commands.trigger(TimerFired { entity });
            match timer.mode {
                TimerMode::Repeat => timer.reset(),
                TimerMode::Once => {
                    commands
                        .entity(entity)
                        .queue_silenced(move |entity: EntityWorldMut| {
                            remove_fired_timer(entity, fired_at)
                        });
                }
            }
        }
    }
}

/// Removes the entity's `Timer` unless it was inserted or changed after `fired_at`.
///
/// Runs after the fired event's observers: a timer they replaced or modified has a
/// newer change tick and is kept. `queue_silenced` ignores an entity they despawned.
fn remove_fired_timer(mut entity: EntityWorldMut, fired_at: Tick) {
    let now = entity.world().read_change_tick();
    if entity
        .get_change_ticks::<Timer>()
        .is_some_and(|ticks| !ticks.is_changed(fired_at, now))
    {
        entity.remove::<Timer>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::approx_eq;
    use bevy_ecs::system::RunSystemOnce;

    /// Entities targeted by [`TimerFired`], in firing order.
    #[derive(Resource, Default)]
    struct Fired(Vec<Entity>);

    fn record_fire(ev: On<TimerFired>, mut fired: ResMut<Fired>) {
        fired.0.push(ev.entity);
    }

    /// Minimal world for `update_timers`: a fixed per-tick delta plus [`Fired`].
    fn world_with_delta(delta: f32) -> World {
        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        world.init_resource::<Fired>();
        world
    }

    /// Same as [`world_with_delta`], with a global [`record_fire`] observer.
    fn observed_world(delta: f32) -> World {
        let mut world = world_with_delta(delta);
        world.add_observer(record_fire);
        world.flush();
        world
    }

    fn tick_timers(world: &mut World) {
        world
            .run_system_once(update_timers)
            .expect("update_timers should run");
    }

    fn fired(world: &World) -> &[Entity] {
        &world.resource::<Fired>().0
    }

    #[test]
    fn timer_fires_targeting_its_entity() {
        let mut world = observed_world(1.0);
        let entity = world.spawn(Timer::new(0.5)).id();

        tick_timers(&mut world);

        assert_eq!(fired(&world), [entity]);
    }

    #[test]
    fn per_entity_observer_fires_only_for_its_entity() {
        let mut world = world_with_delta(1.0);
        let observed = world.spawn(Timer::new(0.5)).observe(record_fire).id();
        world.spawn(Timer::new(0.5)); // another timer, not observed
        world.flush();

        tick_timers(&mut world);

        assert_eq!(fired(&world), [observed]);
    }

    #[test]
    fn timer_does_not_fire_before_duration() {
        let mut world = observed_world(0.3);
        world.spawn(Timer::new(1.0));

        tick_timers(&mut world);

        assert!(fired(&world).is_empty());
    }

    #[test]
    fn timer_fires_across_multiple_ticks() {
        // duration=0.8, delta=0.3 per tick: ticks 1+2 no fire, tick 3 fires.
        let mut world = observed_world(0.3);
        let entity = world.spawn(Timer::new(0.8)).id();

        tick_timers(&mut world); // elapsed=0.3
        tick_timers(&mut world); // elapsed=0.6
        assert!(fired(&world).is_empty());

        tick_timers(&mut world); // elapsed=0.9 >= 0.8, fires, resets to 0.1
        assert_eq!(fired(&world), [entity]);
        assert!(approx_eq(world.get::<Timer>(entity).unwrap().elapsed, 0.1));
    }

    #[test]
    fn timers_fire_independently() {
        let mut world = observed_world(1.0);
        let short = world.spawn(Timer::new(0.5)).id(); // fires (1.0 >= 0.5)
        world.spawn(Timer::new(2.0)); // does not fire (1.0 < 2.0)

        tick_timers(&mut world);

        assert_eq!(fired(&world), [short]);
    }

    #[test]
    fn timer_fires_at_most_once_per_tick_with_large_delta() {
        // delta covers four durations, but the timer fires once and keeps the
        // overshoot: no catch-up within a tick.
        let mut world = observed_world(2.0);
        let entity = world.spawn(Timer::new(0.5)).id();

        tick_timers(&mut world);

        assert_eq!(fired(&world), [entity]);
        assert!(approx_eq(world.get::<Timer>(entity).unwrap().elapsed, 1.5));
    }

    #[test]
    fn once_timer_fires_once_then_loses_its_timer() {
        let mut world = observed_world(1.0);
        let entity = world.spawn(Timer::once(0.5)).id();

        tick_timers(&mut world);
        tick_timers(&mut world);

        assert_eq!(fired(&world), [entity]);
        assert!(world.get::<Timer>(entity).is_none());
        assert!(world.get_entity(entity).is_ok(), "the entity survives");
    }

    #[test]
    fn once_timer_observer_sees_the_timer_and_may_despawn() {
        let mut world = world_with_delta(1.0);
        world.add_observer(
            |ev: On<TimerFired>, timers: Query<&Timer>, mut commands: Commands| {
                assert_eq!(timers.get(ev.entity).unwrap().mode, TimerMode::Once);
                commands.entity(ev.entity).despawn();
            },
        );
        world.flush();
        let entity = world.spawn(Timer::once(0.5)).id();

        tick_timers(&mut world); // the removal after a despawn must not panic

        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn once_timer_observer_can_rearm_a_new_timer() {
        let mut world = world_with_delta(1.0);
        world.add_observer(|ev: On<TimerFired>, mut commands: Commands| {
            commands.entity(ev.entity).insert(Timer::once(2.0));
        });
        world.flush();
        let entity = world.spawn(Timer::once(0.5)).id();

        tick_timers(&mut world);

        let timer = world
            .get::<Timer>(entity)
            .expect("the re-armed timer must survive the spent timer's removal");
        assert_eq!(timer.duration, 2.0);
    }

    #[test]
    fn once_timer_rearmed_with_zero_duration_fires_next_tick() {
        let mut world = observed_world(1.0);
        world.add_observer(
            |ev: On<TimerFired>, mut rearmed: Local<bool>, mut commands: Commands| {
                if !*rearmed {
                    *rearmed = true;
                    commands.entity(ev.entity).insert(Timer::once(0.0));
                }
            },
        );
        world.flush();
        let entity = world.spawn(Timer::once(0.5)).id();

        tick_timers(&mut world);
        assert_eq!(fired(&world), [entity]);
        assert!(
            world.get::<Timer>(entity).is_some(),
            "the zero-duration re-arm must survive the spent timer's removal"
        );

        tick_timers(&mut world);
        assert_eq!(fired(&world), [entity, entity], "fires on the next tick");
        assert!(world.get::<Timer>(entity).is_none());
    }

    #[test]
    fn repeat_timer_keeps_firing() {
        let mut world = observed_world(1.0);
        let entity = world.spawn(Timer::new(0.5)).id();

        tick_timers(&mut world);
        tick_timers(&mut world);

        assert_eq!(fired(&world), [entity, entity]);
    }
}
