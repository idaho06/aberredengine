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
//!    - Call on_exit for old phase
//!    - Swap phases, reset time
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
//! - [`crate::systems::luaphase`] – Lua equivalent

use bevy_ecs::prelude::*;

use crate::components::phase::Phase;
use crate::resources::input::InputState;
use crate::systems::GameCtx;

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
) {
    callback_transitions.clear();

    // Collect entity IDs to avoid borrowing phase_query during callbacks.
    let entities: Vec<Entity> = phase_query.iter().map(|(e, _)| e).collect();

    let delta = ctx.world_time.delta;

    for entity in entities {
        // --- Initial enter callback ---
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

            let on_enter_fn = {
                let Ok((_, phase)) = phase_query.get(entity) else {
                    continue;
                };
                phase
                    .current_callbacks()
                    .and_then(|cbs| cbs.on_enter)
            };

            if let Some(enter_fn) = on_enter_fn
                && let Some(next) = enter_fn(entity, &mut ctx, &input)
            {
                callback_transitions.push((entity, next));
            }
        }

        // --- Pending transition ---
        let pending = {
            let Ok((_, mut phase)) = phase_query.get_mut(entity) else {
                continue;
            };
            phase.next.take()
        };

        if let Some(next_phase) = pending {
            // Get exit callback for old phase
            let exit_fn = {
                let Ok((_, phase)) = phase_query.get(entity) else {
                    continue;
                };
                phase.current_callbacks().and_then(|cbs| cbs.on_exit)
            };

            // Call on_exit for old phase
            if let Some(exit_fn) = exit_fn {
                exit_fn(entity, &mut ctx);
            }

            // Swap phases
            let old_phase = {
                let Ok((_, mut phase)) = phase_query.get_mut(entity) else {
                    continue;
                };
                let old = std::mem::replace(&mut phase.current, next_phase);
                phase.previous = Some(old.clone());
                phase.time_in_phase = 0.0;
                old
            };
            let _ = old_phase;

            // Get enter callback for new phase
            let enter_fn = {
                let Ok((_, phase)) = phase_query.get(entity) else {
                    continue;
                };
                phase.current_callbacks().and_then(|cbs| cbs.on_enter)
            };

            // Call on_enter for new phase
            if let Some(enter_fn) = enter_fn
                && let Some(next) = enter_fn(entity, &mut ctx, &input)
            {
                callback_transitions.push((entity, next));
            }
        }

        // --- Update callback ---
        let update_fn = {
            let Ok((_, phase)) = phase_query.get(entity) else {
                continue;
            };
            phase.current_callbacks().and_then(|cbs| cbs.on_update)
        };

        if let Some(update_fn) = update_fn
            && let Some(next) = update_fn(entity, &mut ctx, &input, delta)
        {
            callback_transitions.push((entity, next));
        }

        // --- Increment time ---
        if let Ok((_, mut phase)) = phase_query.get_mut(entity) {
            phase.time_in_phase += delta;
        }
    }

    // Apply callback return transitions (take precedence, matching LuaPhase behavior)
    for (entity, next_phase) in callback_transitions.drain(..) {
        if let Ok((_, mut phase)) = phase_query.get_mut(entity) {
            phase.next = Some(next_phase);
        }
    }
}
