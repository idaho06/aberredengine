//! Rust-based phase state machine system.
//!
//! This module provides the system for processing [`Phase`](crate::components::phase::Phase) components:
//!
//! - [`phase_system`] – runs Rust callbacks for phase enter/update/exit
//!
//! Callbacks receive `&mut `[`GameCtx`](crate::systems::GameCtx) for full ECS access.
//!
//! # System Flow
//!
//! Each frame, for each entity with a `Phase` component:
//!
//! 1. If `needs_enter_callback` is set, call on_enter for current phase
//! 2. If `next` is set (transition requested):
//!    - Swap phases, reset time
//!    - Call on_exit for old phase (phase.current is now the new phase)
//!    - Call on_enter for new phase
//! 3. Call on_update for current phase
//! 4. Increment `time_in_phase` by delta
//! 5. Apply any transitions returned by callbacks
//!
//! # Callback Signatures
//!
//! ```ignore
//! fn my_enter(entity: Entity, ctx: &mut GameCtx, input: &InputState) -> Option<String>;
//! fn my_update(entity: Entity, ctx: &mut GameCtx, input: &InputState, dt: f32) -> Option<String>;
//! fn my_exit(entity: Entity, ctx: &mut GameCtx);
//! ```
//!
//! # Related
//!
//! - [`crate::components::phase::Phase`] – the phase component
//! - `aberred_lua::systems::luaphase` – Lua equivalent

use bevy_ecs::prelude::*;

use crate::components::phase::Phase;
use crate::resources::input::InputState;
use crate::systems::GameCtx;

use super::phase_core::{PhaseRunner, apply_callback_transitions, run_phase_callbacks};

struct RustPhaseRunner<'a, 'w, 's> {
    ctx: &'a mut GameCtx<'w, 's>,
    input: &'a InputState,
}

impl<'a, 'w, 's> PhaseRunner<crate::components::phase::PhaseCallbackFns>
    for RustPhaseRunner<'a, 'w, 's>
{
    fn call_enter(
        &mut self,
        entity: Entity,
        _phase: &Phase,
        callbacks: &crate::components::phase::PhaseCallbackFns,
    ) -> Option<String> {
        callbacks
            .on_enter
            .and_then(|enter_fn| enter_fn(entity, self.ctx, self.input))
    }

    fn call_update(
        &mut self,
        entity: Entity,
        _phase: &Phase,
        callbacks: &crate::components::phase::PhaseCallbackFns,
        delta: f32,
    ) -> Option<String> {
        callbacks
            .on_update
            .and_then(|update_fn| update_fn(entity, self.ctx, self.input, delta))
    }

    fn call_exit(
        &mut self,
        entity: Entity,
        _phase: &Phase,
        callbacks: &crate::components::phase::PhaseCallbackFns,
    ) {
        if let Some(exit_fn) = callbacks.on_exit {
            exit_fn(entity, self.ctx);
        }
    }
}

/// Process Rust-based phase state machines.
///
/// This system:
/// 1. Collects all phase entities to avoid borrow conflicts
/// 2. Runs Rust callbacks (enter/update/exit) via function pointers
/// 3. Handles phase transitions requested by callbacks or external code
///
/// Entities are processed individually (not iterated) so that [`GameCtx`]
/// queries can be passed to callbacks without conflicting with the phase query.
#[allow(clippy::too_many_arguments)]
pub fn phase_system(
    mut phase_query: Query<(Entity, &mut Phase)>,
    mut ctx: GameCtx,
    input: Res<InputState>,
    mut callback_transitions: Local<Vec<(Entity, String)>>,
    mut phase_entities: Local<Vec<Entity>>,
) {
    callback_transitions.clear();
    phase_entities.clear();

    let delta = ctx.world_time.delta;
    let mut runner = RustPhaseRunner {
        ctx: &mut ctx,
        input: &input,
    };

    run_phase_callbacks(
        &mut phase_query,
        delta,
        &mut callback_transitions,
        &mut phase_entities,
        &mut runner,
    );

    apply_callback_transitions(&mut phase_query, &mut callback_transitions);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::phase::PhaseCallbackFns;
    use crate::components::signals::Signals;
    use crate::events::input::InputAction;
    use crate::protocol::audio::AudioCmd;
    use crate::resources::worldsignals::WorldSignals;
    use crate::resources::worldtime::WorldTime;
    use crate::testing::{approx_eq, insert_game_ctx_resources};
    use bevy_ecs::system::{RunSystemOnce, SystemState};

    fn tick_phases(world: &mut World) {
        world
            .run_system_once(phase_system)
            .expect("phase_system should run");
    }

    /// `GameCtx`'s resources plus `InputState` (read by `phase_system`),
    /// with a fixed per-tick delta.
    fn make_phase_world(delta: f32) -> World {
        let mut world = World::new();
        insert_game_ctx_resources(&mut world);
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        world.insert_resource(InputState::default());
        world
    }

    fn simple_two_phase_map() -> rustc_hash::FxHashMap<String, PhaseCallbackFns> {
        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: None,
                on_update: None,
                on_exit: None,
            },
        );
        phases.insert(
            "moving".into(),
            PhaseCallbackFns {
                on_enter: None,
                on_update: None,
                on_exit: None,
            },
        );
        phases
    }

    #[test]
    fn phase_calls_on_enter_on_first_frame() {
        let mut world = make_phase_world(0.016);

        fn enter_fn(entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
            if let Ok(mut signals) = ctx.signals.get_mut(entity) {
                signals.set_flag("entered");
            }
            None
        }

        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: Some(enter_fn),
                on_update: None,
                on_exit: None,
            },
        );

        let entity = world
            .spawn((Phase::new("idle", phases), Signals::default()))
            .id();

        tick_phases(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(signals.has_flag("entered"));
    }

    #[test]
    fn phase_on_enter_not_called_twice() {
        let mut world = make_phase_world(0.016);

        fn enter_fn(entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
            if let Ok(mut signals) = ctx.signals.get_mut(entity) {
                let count = signals.get_scalar("enter_count").unwrap_or(0.0);
                signals.set_scalar("enter_count", count + 1.0);
            }
            None
        }

        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: Some(enter_fn),
                on_update: None,
                on_exit: None,
            },
        );

        let entity = world
            .spawn((Phase::new("idle", phases), Signals::default()))
            .id();

        tick_phases(&mut world);
        tick_phases(&mut world);
        tick_phases(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(approx_eq(signals.get_scalar("enter_count").unwrap(), 1.0));
    }

    #[test]
    fn phase_calls_on_update_every_frame() {
        let mut world = make_phase_world(0.016);

        fn update_fn(
            entity: Entity,
            ctx: &mut GameCtx,
            _input: &InputState,
            _dt: f32,
        ) -> Option<String> {
            if let Ok(mut signals) = ctx.signals.get_mut(entity) {
                let count = signals.get_scalar("update_count").unwrap_or(0.0);
                signals.set_scalar("update_count", count + 1.0);
            }
            None
        }

        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: None,
                on_update: Some(update_fn),
                on_exit: None,
            },
        );

        let entity = world
            .spawn((Phase::new("idle", phases), Signals::default()))
            .id();

        tick_phases(&mut world);
        tick_phases(&mut world);
        tick_phases(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(approx_eq(signals.get_scalar("update_count").unwrap(), 3.0));
    }

    #[test]
    fn phase_transition_via_update_return() {
        let mut world = make_phase_world(0.016);

        fn update_fn(
            _entity: Entity,
            _ctx: &mut GameCtx,
            _input: &InputState,
            _dt: f32,
        ) -> Option<String> {
            Some("moving".into())
        }

        let mut phases = simple_two_phase_map();
        phases.get_mut("idle").unwrap().on_update = Some(update_fn);

        let entity = world.spawn((Phase::new("idle", phases),)).id();

        // First tick: on_update returns "moving", which gets stored in phase.next
        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        // After first tick, the transition is pending (stored in next)
        assert_eq!(phase.next.as_deref(), Some("moving"));

        // Second tick: the pending transition is processed
        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        assert_eq!(phase.current, "moving");
        assert_eq!(phase.previous.as_deref(), Some("idle"));
    }

    #[test]
    fn phase_transition_via_external_next() {
        let mut world = make_phase_world(0.016);

        let entity = world
            .spawn((Phase::new("idle", simple_two_phase_map()),))
            .id();

        // Externally request a transition
        world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        assert_eq!(phase.current, "moving");
        assert_eq!(phase.previous.as_deref(), Some("idle"));
    }

    #[test]
    fn phase_on_enter_return_is_applied_on_next_frame() {
        let mut world = make_phase_world(0.016);

        fn enter_fn(_entity: Entity, _ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
            Some("moving".into())
        }

        let mut phases = simple_two_phase_map();
        phases.get_mut("idle").unwrap().on_enter = Some(enter_fn);

        let entity = world.spawn((Phase::new("idle", phases),)).id();

        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        assert_eq!(phase.current, "idle");
        assert_eq!(phase.next.as_deref(), Some("moving"));

        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        assert_eq!(phase.current, "moving");
        assert_eq!(phase.previous.as_deref(), Some("idle"));
    }

    #[test]
    fn phase_on_exit_called_on_transition() {
        let mut world = make_phase_world(0.016);

        fn exit_fn(entity: Entity, ctx: &mut GameCtx) {
            if let Ok(mut signals) = ctx.signals.get_mut(entity) {
                signals.set_flag("exited_idle");
            }
        }

        let mut phases = simple_two_phase_map();
        phases.get_mut("idle").unwrap().on_exit = Some(exit_fn);

        let entity = world
            .spawn((Phase::new("idle", phases), Signals::default()))
            .id();

        // Request transition
        world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

        tick_phases(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(signals.has_flag("exited_idle"));
    }

    #[test]
    fn phase_on_enter_called_on_transition() {
        let mut world = make_phase_world(0.016);

        fn enter_fn(entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
            if let Ok(mut signals) = ctx.signals.get_mut(entity) {
                signals.set_flag("entered_moving");
            }
            None
        }

        let mut phases = simple_two_phase_map();
        phases.get_mut("moving").unwrap().on_enter = Some(enter_fn);

        let entity = world
            .spawn((Phase::new("idle", phases), Signals::default()))
            .id();

        // Request transition
        world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

        tick_phases(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(signals.has_flag("entered_moving"));
    }

    #[test]
    fn phase_callback_return_takes_precedence_after_external_transition() {
        let mut world = make_phase_world(0.016);

        fn update_fn(
            _entity: Entity,
            _ctx: &mut GameCtx,
            _input: &InputState,
            _dt: f32,
        ) -> Option<String> {
            Some("attacking".into())
        }

        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: None,
                on_update: None,
                on_exit: None,
            },
        );
        phases.insert(
            "moving".into(),
            PhaseCallbackFns {
                on_enter: None,
                on_update: Some(update_fn),
                on_exit: None,
            },
        );
        phases.insert(
            "attacking".into(),
            PhaseCallbackFns {
                on_enter: None,
                on_update: None,
                on_exit: None,
            },
        );

        let entity = world.spawn((Phase::new("idle", phases),)).id();
        world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        assert_eq!(phase.current, "moving");
        assert_eq!(phase.next.as_deref(), Some("attacking"));
        assert_eq!(phase.previous.as_deref(), Some("idle"));

        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        assert_eq!(phase.current, "attacking");
        assert_eq!(phase.previous.as_deref(), Some("moving"));
    }

    #[test]
    fn phase_time_in_phase_resets_on_transition() {
        let mut world = make_phase_world(0.5);

        let entity = world
            .spawn((Phase::new("idle", simple_two_phase_map()),))
            .id();

        // Run a couple frames to accumulate time
        tick_phases(&mut world);
        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        assert!(approx_eq(phase.time_in_phase, 1.0));

        // Request transition
        world.get_mut::<Phase>(entity).unwrap().next = Some("moving".into());

        tick_phases(&mut world);

        let phase = world.get::<Phase>(entity).unwrap();
        // time_in_phase was reset to 0 at transition, then incremented by delta (0.5)
        assert!(approx_eq(phase.time_in_phase, 0.5));
    }

    #[test]
    fn phase_update_receives_delta_time() {
        let mut world = make_phase_world(0.25);

        fn update_fn(
            entity: Entity,
            ctx: &mut GameCtx,
            _input: &InputState,
            dt: f32,
        ) -> Option<String> {
            if let Ok(mut signals) = ctx.signals.get_mut(entity) {
                signals.set_scalar("received_dt", dt);
            }
            None
        }

        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: None,
                on_update: Some(update_fn),
                on_exit: None,
            },
        );

        let entity = world
            .spawn((Phase::new("idle", phases), Signals::default()))
            .id();

        tick_phases(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(approx_eq(signals.get_scalar("received_dt").unwrap(), 0.25));
    }

    #[test]
    fn phase_callback_can_set_world_signal() {
        let mut world = make_phase_world(0.016);

        fn enter_fn(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
            ctx.world_signals.set_flag("game_started");
            None
        }

        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: Some(enter_fn),
                on_update: None,
                on_exit: None,
            },
        );

        world.spawn((Phase::new("idle", phases),));

        tick_phases(&mut world);

        let world_signals = world.resource::<WorldSignals>();
        assert!(world_signals.has_flag("game_started"));
    }

    #[test]
    fn phase_callback_can_write_audio() {
        let mut world = make_phase_world(0.016);

        fn enter_fn(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
            ctx.audio.write(AudioCmd::PlayFx {
                id: "phase_start".into(),
            });
            None
        }

        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: Some(enter_fn),
                on_update: None,
                on_exit: None,
            },
        );

        world.spawn((Phase::new("idle", phases),));

        tick_phases(&mut world);

        // Flip message buffers so they become readable
        world.resource_mut::<Messages<AudioCmd>>().update();

        let mut state = SystemState::<MessageReader<AudioCmd>>::new(&mut world);
        let mut reader = state
            .get_mut(&mut world)
            .expect("Audio command reader should fetch");
        let cmds: Vec<_> = reader.read().collect();
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], AudioCmd::PlayFx { id } if id == "phase_start"));
    }

    #[test]
    fn phase_callback_receives_input_state() {
        let mut world = make_phase_world(0.016);

        let mut input = InputState::default();
        input.action_mut(InputAction::Action1).active = true;
        input.action_mut(InputAction::Action1).just_pressed = true;
        world.insert_resource(input);

        fn update_fn(
            entity: Entity,
            ctx: &mut GameCtx,
            input: &InputState,
            _dt: f32,
        ) -> Option<String> {
            if input.action(InputAction::Action1).active
                && let Ok(mut signals) = ctx.signals.get_mut(entity)
            {
                signals.set_flag("input_received");
            }
            None
        }

        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbackFns {
                on_enter: None,
                on_update: Some(update_fn),
                on_exit: None,
            },
        );

        let entity = world
            .spawn((Phase::new("idle", phases), Signals::default()))
            .id();

        tick_phases(&mut world);

        let signals = world.get::<Signals>(entity).unwrap();
        assert!(signals.has_flag("input_received"));
    }
}
