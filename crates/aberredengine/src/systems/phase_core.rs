//! Shared phase lifecycle runner used by both the Rust and Lua phase systems.
//!
//! [`run_phase_callbacks`] owns the backend-agnostic control flow for
//! [`Phase<C>`](crate::components::phase::Phase) state machines. The concrete
//! systems supply a [`PhaseRunner`] implementation that maps the three lifecycle
//! stages to the appropriate backend: `RustPhaseRunner` calls Rust function
//! pointers directly, while `LuaPhaseRunner` resolves named Lua callbacks and
//! runs them through the scripting runtime.

use bevy_ecs::prelude::*;

use crate::components::phase::Phase;

/// Backend-specific lifecycle dispatcher for the shared phase update loop.
///
/// `C` is the callback payload stored inside [`Phase<C>`]. In the Rust phase
/// path this is [`PhaseCallbackFns`](crate::components::phase::PhaseCallbackFns),
/// while the Lua phase path uses
/// [`PhaseCallbacks`](crate::components::luaphase::PhaseCallbacks).
///
/// [`call_enter`](Self::call_enter), [`call_update`](Self::call_update), and
/// [`call_exit`](Self::call_exit) map directly to the three phase lifecycle
/// events. Returning `Some(next_phase)` from `call_enter` or `call_update`
/// requests an immediate transition to `next_phase`; returning `None` leaves the
/// entity in its current phase.
pub(crate) trait PhaseRunner<C> {
    /// Run the current phase's enter callback.
    ///
    /// Returning `Some(phase_name)` requests that the entity transition to that
    /// phase as soon as callback-requested transitions are drained.
    fn call_enter(&mut self, entity: Entity, phase: &Phase<C>, callbacks: &C) -> Option<String>;

    /// Run the current phase's per-frame update callback.
    ///
    /// Returning `Some(phase_name)` requests that the entity transition to that
    /// phase as soon as callback-requested transitions are drained.
    fn call_update(
        &mut self,
        entity: Entity,
        phase: &Phase<C>,
        callbacks: &C,
        delta: f32,
    ) -> Option<String>;

    /// Run the previous phase's exit callback after a transition swap.
    fn call_exit(&mut self, entity: Entity, phase: &Phase<C>, callbacks: &C);
}

/// Run one frame of shared phase lifecycle processing for every [`Phase<C>`] entity.
///
/// For each entity this function:
/// 1. Fires `on_enter` if `needs_enter_callback` is set.
/// 2. Applies any already-queued `phase.next` transition, including `on_exit` for
///    the old phase and `on_enter` for the new one.
/// 3. Runs the current phase's `on_update` callback.
///
/// Any phase name returned by any of the above callbacks is collected into
/// `callback_transitions` for deferred application via [`apply_callback_transitions`].
///
/// `entity_scratch` pre-collects entity IDs before the mutation-heavy loop so the
/// query is not iterated while individual entities are being re-fetched and
/// mutated.
pub(crate) fn run_phase_callbacks<C, R>(
    phase_query: &mut Query<(Entity, &mut Phase<C>)>,
    delta: f32,
    callback_transitions: &mut Vec<(Entity, String)>,
    entity_scratch: &mut Vec<Entity>,
    runner: &mut R,
) where
    C: Send + Sync + 'static,
    R: PhaseRunner<C>,
{
    entity_scratch.extend(phase_query.iter().map(|(entity, _)| entity));

    for entity in entity_scratch.iter().copied() {
        // Borrow isolation: each `get()` scope must end before a later `get_mut()`
        // on the same query, so immutable reads are wrapped in short blocks.
        let needs_enter = {
            let Ok((_, phase)) = phase_query.get(entity) else {
                continue;
            };
            phase.needs_enter_callback
        };

        if needs_enter {
            if let Ok((_, mut phase)) = phase_query.get_mut(entity) {
                phase.needs_enter_callback = false;
            }

            let enter_transition = {
                let Ok((_, phase)) = phase_query.get(entity) else {
                    continue;
                };
                phase
                    .current_callbacks()
                    .and_then(|callbacks| runner.call_enter(entity, phase, callbacks))
            };

            if let Some(next_phase) = enter_transition {
                callback_transitions.push((entity, next_phase));
            }
        }

        let pending_transition = {
            let Ok((_, mut phase)) = phase_query.get_mut(entity) else {
                continue;
            };
            phase.next.take()
        };

        if let Some(next_phase) = pending_transition {
            let old_phase = {
                let Ok((_, mut phase)) = phase_query.get_mut(entity) else {
                    continue;
                };
                let old_phase = std::mem::replace(&mut phase.current, next_phase);
                phase.previous = Some(old_phase.clone());
                phase.time_in_phase = 0.0;
                old_phase
            };

            // exit called after swap: phase.current is already the new phase,
            // callbacks are looked up by old phase name
            let Ok((_, phase)) = phase_query.get(entity) else {
                continue;
            };
            if let Some(callbacks) = phase.get_callbacks(&old_phase) {
                runner.call_exit(entity, phase, callbacks);
            }

            let enter_transition = {
                let Ok((_, phase)) = phase_query.get(entity) else {
                    continue;
                };
                phase
                    .current_callbacks()
                    .and_then(|callbacks| runner.call_enter(entity, phase, callbacks))
            };

            if let Some(next_phase) = enter_transition {
                callback_transitions.push((entity, next_phase));
            }
        }

        let update_transition = {
            let Ok((_, phase)) = phase_query.get(entity) else {
                continue;
            };
            phase
                .current_callbacks()
                .and_then(|callbacks| runner.call_update(entity, phase, callbacks, delta))
        };

        if let Some(next_phase) = update_transition {
            callback_transitions.push((entity, next_phase));
        }

        if let Ok((_, mut phase)) = phase_query.get_mut(entity) {
            phase.time_in_phase += delta;
        }
    }
}

/// Store a callback-requested phase change in [`Phase::next`](crate::components::phase::Phase::next).
///
/// Callback returns are not applied inline inside [`run_phase_callbacks`]; they are
/// queued first so the current entity-loop pass finishes before the transition is
/// picked up by the next phase-processing step.
pub(crate) fn queue_phase_transition<C>(
    phase_query: &mut Query<(Entity, &mut Phase<C>)>,
    entity: Entity,
    next_phase: String,
) where
    C: Send + Sync + 'static,
{
    if let Ok((_, mut phase)) = phase_query.get_mut(entity) {
        phase.next = Some(next_phase);
    }
}

/// Drain callback-requested transitions after the entity loop completes.
///
/// Deferring this step avoids mutating phase state in the middle of
/// [`run_phase_callbacks`], which would otherwise make callback-triggered
/// transitions re-enter the lifecycle flow during the same pass.
pub(crate) fn apply_callback_transitions<C>(
    phase_query: &mut Query<(Entity, &mut Phase<C>)>,
    callback_transitions: &mut Vec<(Entity, String)>,
) where
    C: Send + Sync + 'static,
{
    for (entity, next_phase) in callback_transitions.drain(..) {
        queue_phase_transition(phase_query, entity, next_phase);
    }
}
