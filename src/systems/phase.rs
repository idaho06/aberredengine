//! Phase state machine systems.
//!
//! This module provides systems for processing [`Phase`] components:
//!
//! - [`phase_change_detector`] – detects pending transitions, runs `on_exit`/`on_enter`
//!   callbacks, emits [`PhaseChangeEvent`], and updates time tracking
//! - [`phase_update_system`] – runs `on_update` callbacks each frame and collects
//!   transition requests
//!
//! # System Ordering
//!
//! These systems should run in order:
//! 1. `phase_change_detector` – process any pending transitions from last frame
//! 2. `phase_update_system` – run per-frame logic and collect new transition requests
//!
//! # Example Schedule Setup
//!
//! ```ignore
//! schedule.add_systems(phase_change_detector);
//! schedule.add_systems(phase_update_system);
//! ```
//!
//! # Related
//!
//! - [`Phase`](crate::components::phase::Phase) – the state machine component
//! - [`PhaseChangeEvent`] – event emitted on transitions

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::boxcollider::BoxCollider;
use crate::components::collision::CollisionRule;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::phase::{Phase, PhaseCallback, PhaseContext};
use crate::components::rigidbody::RigidBody;
use crate::components::scale::Scale;
use crate::components::signals::Signals;
use crate::components::stuckto::StuckTo;
use crate::events::audio::AudioCmd;
use crate::events::phase::PhaseChangeEvent;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

/// Bundled system parameters for phase callback execution.
///
/// This [`SystemParam`] aggregates the queries and resources needed to construct
/// a [`PhaseContext`] for callback invocation.
#[derive(SystemParam)]
pub struct PhaseRunnerContext<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub groups: Query<'w, 's, &'static Group>,
    pub collision_rules: Query<'w, 's, &'static CollisionRule>,
    pub positions: Query<'w, 's, &'static mut MapPosition>,
    pub rigid_bodies: Query<'w, 's, &'static mut RigidBody>,
    pub stuck_tos: Query<'w, 's, &'static mut StuckTo>,
    pub scales: Query<'w, 's, &'static Scale>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub signals: Query<'w, 's, &'static mut Signals>,
    pub world_signals: ResMut<'w, WorldSignals>,
    pub world_time: Res<'w, WorldTime>,
    pub audio_cmds: MessageWriter<'w, AudioCmd>,
}

/// Detect and process phase transitions.
///
/// This system iterates over all entities with a [`Phase`] component and:
///
/// 1. If `phase.next` is set:
///    - Swaps `current` with `next`, storing old phase in `previous`
///    - Runs the `on_exit` callback for the old phase (with `old_time_in_phase`)
///    - Emits a [`PhaseChangeEvent`]
///    - Runs the `on_enter` callback for the new phase (with `time = 0.0`)
///    - Resets `time_in_phase` to 0.0
/// 2. Otherwise, increments `time_in_phase` by the frame delta
///
/// # System Ordering
///
/// Should run **before** [`phase_update_system`] each frame.
pub fn phase_change_detector(
    mut query: Query<(Entity, &mut Phase)>,
    time: Res<WorldTime>,
    mut context: PhaseRunnerContext,
) {
    // Vec to avoid borrowing issues. Here we collect all the transitions
    let mut transitions: Vec<(
        Entity,
        String,
        f32,
        Option<PhaseCallback>,
        Option<PhaseCallback>,
    )> = Vec::new();

    for (entity, mut phase) in query.iter_mut() {
        if let Some(next_phase) = phase.next.take() {
            let previous_phase = std::mem::replace(&mut phase.current, next_phase.clone());
            phase.previous = Some(previous_phase.clone());
            let old_time_in_phase = phase.time_in_phase;
            phase.time_in_phase = 0.0;

            let on_exit: Option<PhaseCallback> = phase.on_exit.get(&previous_phase).copied();
            let on_enter: Option<PhaseCallback> = phase.on_enter.get(&next_phase).copied();

            transitions.push((entity, previous_phase, old_time_in_phase, on_exit, on_enter));
        } else {
            phase.time_in_phase += time.delta;
        }
    }

    for (entity, previous_phase, old_time_in_phase, on_exit, on_enter) in transitions {
        if let Some(callback) = on_exit {
            callback(
                entity,
                old_time_in_phase,
                Some(previous_phase.clone()),
                &mut PhaseContext {
                    commands: &mut context.commands,
                    groups: &context.groups,
                    positions: &mut context.positions,
                    rigid_bodies: &mut context.rigid_bodies,
                    box_colliders: &context.box_colliders,
                    stuck_tos: &mut context.stuck_tos,
                    scales: &context.scales,
                    signals: &mut context.signals,
                    world_signals: &mut context.world_signals,
                    world_time: &context.world_time,
                    audio_cmds: &mut context.audio_cmds,
                },
            );
        }
        context.commands.trigger(PhaseChangeEvent { entity });
        if let Some(callback) = on_enter {
            callback(
                entity,
                0.0, // time_in_phase is 0 at enter
                Some(previous_phase.clone()),
                &mut PhaseContext {
                    commands: &mut context.commands,
                    groups: &context.groups,
                    positions: &mut context.positions,
                    rigid_bodies: &mut context.rigid_bodies,
                    box_colliders: &context.box_colliders,
                    stuck_tos: &mut context.stuck_tos,
                    scales: &context.scales,
                    signals: &mut context.signals,
                    world_signals: &mut context.world_signals,
                    world_time: &context.world_time,
                    audio_cmds: &mut context.audio_cmds,
                },
            );
        }
    }
}

/// Run per-frame `on_update` callbacks for entities in their current phase.
///
/// For each entity with a [`Phase`] component, this system:
///
/// 1. Looks up the `on_update` callback registered for the current phase
/// 2. Invokes the callback with the entity, `time_in_phase`, and `previous` phase
/// 3. If the callback returns `Some(next_phase)`, queues a transition
///
/// Transitions are applied by setting `phase.next`, which will be processed
/// by [`phase_change_detector`] on the next frame.
///
/// # System Ordering
///
/// Should run **after** [`phase_change_detector`] each frame.
pub fn phase_update_system(
    mut query: Query<(Entity, &mut Phase)>,
    mut context: PhaseRunnerContext,
) {
    let mut phase_changes: Vec<(Entity, String)> = Vec::new();

    for (entity, phase) in query.iter() {
        if let Some(callback) = phase.on_update.get(&phase.current) {
            if let Some(next) = callback(
                entity,
                phase.time_in_phase,
                phase.previous.clone(),
                &mut PhaseContext {
                    commands: &mut context.commands,
                    groups: &context.groups,
                    positions: &mut context.positions,
                    rigid_bodies: &mut context.rigid_bodies,
                    box_colliders: &context.box_colliders,
                    stuck_tos: &mut context.stuck_tos,
                    scales: &context.scales,
                    signals: &mut context.signals,
                    world_signals: &mut context.world_signals,
                    world_time: &context.world_time,
                    audio_cmds: &mut context.audio_cmds,
                },
            ) {
                phase_changes.push((entity, next));
            }
        }
    }

    for (entity, next) in phase_changes {
        if let Ok((_, mut phase)) = query.get_mut(entity) {
            phase.next = Some(next);
        }
    }
}
