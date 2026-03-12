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
//! - [`crate::systems::luaphase`] – Lua equivalent

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
) {
    callback_transitions.clear();

    let delta = ctx.world_time.delta;
    let mut runner = RustPhaseRunner {
        ctx: &mut ctx,
        input: &input,
    };

    run_phase_callbacks(
        &mut phase_query,
        delta,
        &mut callback_transitions,
        &mut runner,
    );

    apply_callback_transitions(&mut phase_query, &mut callback_transitions);
}
