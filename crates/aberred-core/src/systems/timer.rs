//! Timer system.
//!
//! [`update_timers`] advances every [`Timer`] once per sim tick and triggers a
//! [`TimerFired`] event on each timer that reaches its duration.
//!
//! # Related
//!
//! - [`crate::components::timer::Timer`] – the timer component
//! - [`crate::events::timer::TimerFired`] – event triggered on expiration
//! - `aberred_lua::systems::luatimer` – Lua equivalent

use bevy_ecs::prelude::*;

use crate::components::timer::Timer;
use crate::events::timer::TimerFired;
use crate::resources::worldtime::WorldTime;

/// Advance all [`Timer`] components and trigger [`TimerFired`] when they expire.
///
/// Accumulates delta time on each timer and fires when `elapsed >= duration`,
/// at most once per tick. The timer resets by subtracting duration, keeping the
/// overshoot for consistent periodic timing.
pub fn update_timers(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut Timer)>,
    mut commands: Commands,
) {
    for (entity, mut timer) in query.iter_mut() {
        timer.elapsed += world_time.delta;
        if timer.elapsed >= timer.duration {
            commands.trigger(TimerFired { entity });
            timer.reset();
        }
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
}
