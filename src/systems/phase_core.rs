use bevy_ecs::prelude::*;

use crate::components::phase::Phase;

pub(crate) trait PhaseRunner<C> {
    fn call_enter(&mut self, entity: Entity, phase: &Phase<C>, callbacks: &C) -> Option<String>;

    fn call_update(
        &mut self,
        entity: Entity,
        phase: &Phase<C>,
        callbacks: &C,
        delta: f32,
    ) -> Option<String>;

    fn call_exit(&mut self, entity: Entity, phase: &Phase<C>, callbacks: &C);
}

pub(crate) fn run_phase_callbacks<C, R>(
    phase_query: &mut Query<(Entity, &mut Phase<C>)>,
    delta: f32,
    callback_transitions: &mut Vec<(Entity, String)>,
    runner: &mut R,
) where
    C: Send + Sync + 'static,
    R: PhaseRunner<C>,
{
    let entities: Vec<Entity> = phase_query.iter().map(|(entity, _)| entity).collect();

    for entity in entities {
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
