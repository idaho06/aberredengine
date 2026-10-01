//! Rust timer systems.
//!
//! This module provides systems for processing [`Timer`](crate::components::timer::Timer) components:
//!
//! - [`update_timers`] – updates timer elapsed time and emits events when they expire
//! - [`timer_observer`] – observer that calls Rust callbacks when timer events fire
//!
//! Callbacks receive `&mut `[`GameCtx`](crate::systems::GameCtx) for full ECS access.
//!
//! # System Flow
//!
//! Each frame:
//!
//! 1. `update_timers` accumulates delta time on all Timer components
//! 2. When `elapsed >= duration`, emits `TimerEvent` and resets timer
//! 3. `timer_observer` receives events and calls the Rust callback
//! 4. Callback executes with full ECS access through `GameCtx`
//!
//! # Callback Signature
//!
//! ```ignore
//! fn my_callback(entity: Entity, ctx: &mut GameCtx, input: &InputState) {
//!     // Full ECS access: queries, commands, resources
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::components::timer::Timer`] – the timer component
//! - [`crate::events::timer::TimerEvent`] – event emitted on expiration
//! - `aberred_lua::systems::luatimer` – Lua equivalent

use bevy_ecs::prelude::*;

use crate::components::timer::{Timer, TimerCallback};
use crate::events::timer::TimerEvent;
use crate::resources::input::InputState;
use crate::resources::worldtime::WorldTime;
use crate::systems::GameCtx;

use super::timer_core::{TimerRunner, run_timer_update};

struct RustTimerRunner<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
}

impl<'a, 'w, 's> TimerRunner<TimerCallback> for RustTimerRunner<'a, 'w, 's> {
    fn on_fire(&mut self, entity: Entity, callback: &TimerCallback) {
        self.commands.trigger(TimerEvent {
            entity,
            callback: *callback,
        });
    }
}

/// Update all Rust timer components and emit events when they expire.
///
/// Accumulates delta time on each [`Timer`](crate::components::timer::Timer)
/// and triggers a [`TimerEvent`](crate::events::timer::TimerEvent) when
/// `elapsed >= duration`. The timer resets by subtracting duration, allowing for
/// consistent periodic timing.
pub fn update_timers(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut Timer)>,
    mut commands: Commands,
) {
    let delta = world_time.delta;
    let mut runner = RustTimerRunner {
        commands: &mut commands,
    };
    run_timer_update(delta, &mut query, &mut runner);
}

/// Observer that handles Rust timer events by calling the callback function.
///
/// When a [`TimerEvent`](crate::events::timer::TimerEvent) is triggered:
///
/// 1. Extracts the entity and callback from the event
/// 2. Calls the callback with `(entity, &mut GameCtx, &InputState)`
/// 3. The callback can use [`GameCtx`] to interact with the ECS
pub fn timer_observer(trigger: On<TimerEvent>, input: Res<InputState>, mut ctx: GameCtx) {
    let event = trigger.event();
    (event.callback)(event.entity, &mut ctx, &input);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::signals::Signals;
    use crate::events::input::InputAction;
    use crate::protocol::audio::AudioCmd;
    use crate::resources::worldsignals::WorldSignals;
    use crate::testing::{approx_eq, insert_game_ctx_resources};
    use bevy_ecs::system::{RunSystemOnce, SystemState};

    /// `GameCtx`'s resources plus `InputState` (read by `timer_observer`),
    /// with a fixed per-tick delta.
    fn world_with_delta(delta: f32) -> World {
        let mut world = World::new();
        insert_game_ctx_resources(&mut world);
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        world.insert_resource(InputState::default());
        world
    }

    fn tick_timers(world: &mut World) {
        world
            .run_system_once(update_timers)
            .expect("update_timers should run");
    }

    #[test]
    fn rust_timer_accumulates_time() {
        let mut world = world_with_delta(0.3);

        fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
        let entity = world.spawn((Timer::rust(1.0, noop),)).id();

        tick_timers(&mut world);

        let timer = world.get::<Timer>(entity).unwrap();
        assert!(approx_eq(timer.elapsed, 0.3));
    }

    #[test]
    fn rust_timer_fires_event_when_expired() {
        let mut world = world_with_delta(1.0);

        fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
        let entity = world.spawn((Timer::rust(0.5, noop),)).id();

        let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
        let fired_entity = std::sync::Arc::new(std::sync::Mutex::new(None));
        let fired_clone = fired.clone();
        let entity_clone = fired_entity.clone();

        world.add_observer(move |trigger: On<TimerEvent>| {
            *fired_clone.lock().unwrap() = true;
            *entity_clone.lock().unwrap() = Some(trigger.event().entity);
        });
        world.flush();

        tick_timers(&mut world);

        assert!(*fired.lock().unwrap());
        assert_eq!(*fired_entity.lock().unwrap(), Some(entity));
    }

    #[test]
    fn rust_timer_resets_after_firing() {
        let mut world = world_with_delta(0.6);

        fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
        let entity = world.spawn((Timer::rust(0.5, noop),)).id();

        world.add_observer(|_trigger: On<TimerEvent>| {});
        world.flush();

        tick_timers(&mut world);

        let timer = world.get::<Timer>(entity).unwrap();
        // Timer should have reset: 0.6 - 0.5 = 0.1
        assert!(approx_eq(timer.elapsed, 0.1));
    }

    #[test]
    fn rust_timer_does_not_fire_before_duration() {
        let mut world = world_with_delta(0.3);

        fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
        world.spawn((Timer::rust(1.0, noop),));

        let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
        let fired_clone = fired.clone();

        world.add_observer(move |_trigger: On<TimerEvent>| {
            *fired_clone.lock().unwrap() = true;
        });
        world.flush();

        tick_timers(&mut world);

        assert!(!*fired.lock().unwrap());
    }

    #[test]
    fn rust_timer_observer_calls_callback() {
        let mut world = world_with_delta(1.0);

        fn set_flag(entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
            if let Ok(mut signals) = ctx.signals.get_mut(entity) {
                signals.set_flag("timer_fired");
            }
        }

        let entity = world
            .spawn((Timer::rust(0.5, set_flag), Signals::default()))
            .id();

        // Register the real timer_observer so the callback gets invoked
        world.add_observer(timer_observer);
        world.flush();

        tick_timers(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(signals.has_flag("timer_fired"));
    }

    #[test]
    fn rust_timer_observer_can_write_audio() {
        let mut world = world_with_delta(1.0);

        fn play_sound(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
            ctx.audio.write(AudioCmd::PlayFx {
                id: "explosion".into(),
            });
        }

        world.spawn((Timer::rust(0.5, play_sound),));

        world.add_observer(timer_observer);
        world.flush();

        tick_timers(&mut world);

        // Flip message buffers so they become readable
        world.resource_mut::<Messages<AudioCmd>>().update();

        // Read messages via SystemState<MessageReader>
        let mut state = SystemState::<MessageReader<AudioCmd>>::new(&mut world);
        let mut reader = state
            .get_mut(&mut world)
            .expect("Audio command reader should fetch");
        let cmds: Vec<_> = reader.read().collect();
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], AudioCmd::PlayFx { id } if id == "explosion"));
    }

    #[test]
    fn rust_timer_observer_can_set_world_signal() {
        let mut world = world_with_delta(1.0);

        fn set_signal(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
            ctx.world_signals.set_flag("game_over");
        }

        world.spawn((Timer::rust(0.5, set_signal),));

        world.add_observer(timer_observer);
        world.flush();

        tick_timers(&mut world);

        let world_signals = world.resource::<WorldSignals>();
        assert!(world_signals.has_flag("game_over"));
    }

    #[test]
    fn rust_timer_observer_receives_input_state() {
        let mut world = world_with_delta(1.0);

        let mut input = InputState::default();
        input.action_mut(InputAction::Action1).active = true;
        input.action_mut(InputAction::Action1).just_pressed = true;
        world.insert_resource(input);

        fn check_input(entity: Entity, ctx: &mut GameCtx, input: &InputState) {
            // Verify input is passed through — set a signal if action_1 is pressed
            if input.action(InputAction::Action1).active
                && let Ok(mut signals) = ctx.signals.get_mut(entity)
            {
                signals.set_flag("input_received");
            }
        }

        let entity = world
            .spawn((Timer::rust(0.5, check_input), Signals::default()))
            .id();

        world.add_observer(timer_observer);
        world.flush();

        tick_timers(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(signals.has_flag("input_received"));
    }

    #[test]
    fn rust_timer_fires_across_multiple_ticks() {
        // Verify elapsed accumulates correctly over multiple ticks before firing.
        // duration=0.8, delta=0.3 per tick: ticks 1+2 no fire, tick 3 fires.
        let fired_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        let fired_clone = fired_count.clone();

        fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
        let mut world = world_with_delta(0.3);
        world.spawn((Timer::rust(0.8, noop),));

        world.add_observer(move |_trigger: On<TimerEvent>| {
            *fired_clone.lock().unwrap() += 1;
        });
        world.flush();

        tick_timers(&mut world); // elapsed=0.3
        assert_eq!(*fired_count.lock().unwrap(), 0);

        tick_timers(&mut world); // elapsed=0.6
        assert_eq!(*fired_count.lock().unwrap(), 0);

        tick_timers(&mut world); // elapsed=0.9 >= 0.8, fires, resets to 0.1
        assert_eq!(*fired_count.lock().unwrap(), 1);
    }

    #[test]
    fn rust_timer_multiple_entities_fire_independently() {
        // Short-duration timer fires; long-duration timer does not.
        let fired_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        let fired_clone = fired_count.clone();

        fn noop(_: Entity, _: &mut GameCtx, _: &InputState) {}
        let mut world = world_with_delta(1.0);
        world.spawn((Timer::rust(0.5, noop),)); // fires (1.0 >= 0.5)
        world.spawn((Timer::rust(2.0, noop),)); // does not fire (1.0 < 2.0)

        world.add_observer(move |_trigger: On<TimerEvent>| {
            *fired_clone.lock().unwrap() += 1;
        });
        world.flush();

        tick_timers(&mut world);

        assert_eq!(*fired_count.lock().unwrap(), 1);
    }

    #[test]
    fn rust_timer_callback_receives_correct_entity() {
        // Verify the entity passed to the callback is the timer's own entity,
        // not another entity that happens to have Signals.
        let mut world = world_with_delta(1.0);

        fn mark_self(entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
            if let Ok(mut signals) = ctx.signals.get_mut(entity) {
                signals.set_flag("fired");
            }
        }

        let bystander = world.spawn(Signals::default()).id();
        let timer_entity = world
            .spawn((Timer::rust(0.5, mark_self), Signals::default()))
            .id();

        world.add_observer(timer_observer);
        world.flush();

        tick_timers(&mut world);

        assert!(
            world
                .get::<Signals>(timer_entity)
                .unwrap()
                .has_flag("fired")
        );
        assert!(!world.get::<Signals>(bystander).unwrap().has_flag("fired"));
    }
}
