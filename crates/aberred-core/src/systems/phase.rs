//! Phase state machine system.
//!
//! [`phase_system`] applies [`Phase`] transitions once per sim tick and triggers
//! [`PhaseEntered`] / [`PhaseExited`]. It runs at the start of the tick, so a
//! transition requested at any point in tick N applies in tick N+1.
//!
//! # System Flow
//!
//! Each run, for each entity with a `Phase` component:
//!
//! 1. On the first run, trigger `PhaseEntered` for the initial phase.
//! 2. If `next` is set: swap phases and reset `time_in_phase`, then trigger
//!    `PhaseExited` for the old phase and `PhaseEntered` for the new one.
//! 3. Add the tick's delta to `time_in_phase`.
//!
//! Observers run after the system, so they see the post-swap `current`: for a
//! transition, the new phase in both events. The initial `PhaseEntered` sees a
//! `current` that already moved on when `next` was set before the first run, so
//! observers read the phase from the event's `name`. A `next` that an observer
//! sets applies on the following run.
//!
//! # Related
//!
//! - [`crate::components::phase::Phase`] – the phase component
//! - [`crate::events::phase`] – the triggered events
//! - `aberred_lua::systems::luaphase` – Lua equivalent

use std::sync::Arc;

use bevy_ecs::prelude::*;

use crate::components::phase::Phase;
use crate::events::phase::{PhaseEntered, PhaseExited};
use crate::resources::worldtime::WorldTime;

/// Apply requested [`Phase`] transitions and trigger [`PhaseEntered`] /
/// [`PhaseExited`]; see the [module docs](self).
pub fn phase_system(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut Phase)>,
    mut commands: Commands,
) {
    for (entity, mut phase) in &mut query {
        if phase.begin() {
            commands.trigger(PhaseEntered {
                entity,
                name: phase.current.as_str().into(),
                previous: None,
            });
        }
        if let Some(old) = phase.apply_next() {
            let old: Arc<str> = old.into();
            let new: Arc<str> = phase.current.as_str().into();
            commands.trigger(PhaseExited {
                entity,
                name: Arc::clone(&old),
                next: Arc::clone(&new),
            });
            commands.trigger(PhaseEntered {
                entity,
                name: new,
                previous: Some(old),
            });
        }
        phase.time_in_phase += world_time.delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::phase::{PhaseEntered, PhaseExited};
    use crate::resources::worldtime::WorldTime;
    use crate::testing::approx_eq;
    use bevy_ecs::system::RunSystemOnce;

    /// Phase events in trigger order, each with the `Phase::current` its
    /// observer saw: `entered:<name>:<previous>@<current>` /
    /// `exited:<name>:<next>@<current>`.
    #[derive(Resource, Default)]
    struct Log(Vec<String>);

    fn record_entered(ev: On<PhaseEntered>, phases: Query<&Phase>, mut log: ResMut<Log>) {
        let current = &phases.get(ev.entity).unwrap().current;
        let previous = ev.previous.as_deref().unwrap_or("-");
        log.0
            .push(format!("entered:{}:{previous}@{current}", ev.name));
    }

    fn record_exited(ev: On<PhaseExited>, phases: Query<&Phase>, mut log: ResMut<Log>) {
        let current = &phases.get(ev.entity).unwrap().current;
        log.0
            .push(format!("exited:{}:{}@{current}", ev.name, ev.next));
    }

    /// World with a fixed per-tick delta and global recording observers.
    fn make_phase_world(delta: f32) -> World {
        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        world.init_resource::<Log>();
        world.add_observer(record_entered);
        world.add_observer(record_exited);
        world
    }

    fn tick_phases(world: &mut World) {
        world
            .run_system_once(phase_system)
            .expect("phase_system should run");
    }

    fn take_log(world: &mut World) -> Vec<String> {
        std::mem::take(&mut world.resource_mut::<Log>().0)
    }

    #[test]
    fn enters_initial_phase_on_first_run_only() {
        let mut world = make_phase_world(0.1);
        world.spawn(Phase::new("idle"));

        tick_phases(&mut world);
        assert_eq!(take_log(&mut world), ["entered:idle:-@idle"]);

        tick_phases(&mut world);
        assert!(take_log(&mut world).is_empty(), "enter fires only once");
    }

    #[test]
    fn next_triggers_exit_then_enter_with_names() {
        let mut world = make_phase_world(0.1);
        let entity = world.spawn(Phase::new("idle")).id();
        tick_phases(&mut world);
        take_log(&mut world);

        world.get_mut::<Phase>(entity).unwrap().next = Some("run".into());
        tick_phases(&mut world);

        assert_eq!(
            take_log(&mut world),
            ["exited:idle:run@run", "entered:run:idle@run"],
            "exit then enter; both observers see the post-swap current"
        );
        let phase = world.get::<Phase>(entity).unwrap();
        assert_eq!(phase.current, "run");
        assert_eq!(phase.previous.as_deref(), Some("idle"));
        assert!(phase.next.is_none());
    }

    #[test]
    fn next_set_before_first_run_enters_initial_phase_first() {
        let mut world = make_phase_world(0.1);
        let mut phase = Phase::new("idle");
        phase.next = Some("run".into());
        world.spawn(phase);

        tick_phases(&mut world);
        assert_eq!(
            take_log(&mut world),
            [
                "entered:idle:-@run",
                "exited:idle:run@run",
                "entered:run:idle@run"
            ]
        );
    }

    #[test]
    fn time_in_phase_accumulates_and_resets_on_swap() {
        let mut world = make_phase_world(0.25);
        let entity = world.spawn(Phase::new("idle")).id();

        tick_phases(&mut world);
        tick_phases(&mut world);
        assert!(approx_eq(
            world.get::<Phase>(entity).unwrap().time_in_phase,
            0.5
        ));

        world.get_mut::<Phase>(entity).unwrap().next = Some("run".into());
        tick_phases(&mut world);
        assert!(
            approx_eq(world.get::<Phase>(entity).unwrap().time_in_phase, 0.25),
            "reset on swap, then this run's delta"
        );
    }

    #[test]
    fn next_set_by_entered_observer_applies_on_following_run() {
        let mut world = make_phase_world(0.1);
        world.add_observer(|ev: On<PhaseEntered>, mut phases: Query<&mut Phase>| {
            if &*ev.name == "idle" {
                phases.get_mut(ev.entity).unwrap().next = Some("run".into());
            }
        });
        let entity = world.spawn(Phase::new("idle")).id();

        tick_phases(&mut world);
        assert_eq!(take_log(&mut world), ["entered:idle:-@idle"]);
        let phase = world.get::<Phase>(entity).unwrap();
        assert_eq!(phase.current, "idle");
        assert_eq!(phase.next.as_deref(), Some("run"));

        tick_phases(&mut world);
        assert_eq!(
            take_log(&mut world),
            ["exited:idle:run@run", "entered:run:idle@run"]
        );
    }

    #[test]
    fn per_entity_observer_sees_only_its_entity() {
        #[derive(Resource, Default)]
        struct Seen(Vec<Entity>);

        let mut world = make_phase_world(0.1);
        world.init_resource::<Seen>();
        let watched = world
            .spawn(Phase::new("idle"))
            .observe(|ev: On<PhaseEntered>, mut seen: ResMut<Seen>| seen.0.push(ev.entity))
            .id();
        world.spawn(Phase::new("idle"));

        tick_phases(&mut world);
        assert_eq!(world.resource::<Seen>().0, [watched]);
    }
}
