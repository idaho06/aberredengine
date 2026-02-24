//! Rust-based phase state machine system.
//!
//! This module provides the system for processing [`Phase`](crate::components::phase::Phase) components:
//!
//! - [`phase_system`] – runs Rust callbacks for phase enter/update/exit
//! - [`PhaseCtx`] – bundled ECS access passed to phase callbacks
//!
//! Unlike the Lua-based [`luaphase`](super::luaphase) system, this system calls
//! Rust function pointers directly, with no Lua runtime involvement.
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
//! fn my_enter(entity: Entity, ctx: &mut PhaseCtx, input: &InputState) -> Option<String>;
//! fn my_update(entity: Entity, ctx: &mut PhaseCtx, input: &InputState, dt: f32) -> Option<String>;
//! fn my_exit(entity: Entity, ctx: &mut PhaseCtx);
//! ```
//!
//! # Related
//!
//! - [`crate::components::phase::Phase`] – the phase component
//! - [`crate::systems::luaphase`] – Lua equivalent

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::phase::Phase;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::events::audio::AudioCmd;
use crate::resources::input::InputState;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

/// Bundled ECS access passed to Rust phase callbacks.
///
/// Mirrors [`TimerCtx`](crate::systems::timer::TimerCtx), providing full
/// query and resource access so that phase callbacks can read/write any
/// entity's components and interact with engine resources.
///
/// # Usage in callbacks
///
/// ```ignore
/// fn my_enter(entity: Entity, ctx: &mut PhaseCtx, input: &InputState) -> Option<String> {
///     if let Ok(mut rb) = ctx.rigid_bodies.get_mut(entity) {
///         rb.velocity = Vector2::zero();
///     }
///     ctx.audio.write(AudioCmd::PlayFx { id: "start".into() });
///     None
/// }
/// ```
#[derive(SystemParam)]
pub struct PhaseCtx<'w, 's> {
    /// ECS command buffer for spawning, despawning, inserting/removing components.
    pub commands: Commands<'w, 's>,
    // Mutable queries
    /// Mutable access to entity positions (world-space).
    pub positions: Query<'w, 's, &'static mut MapPosition>,
    /// Mutable access to rigid bodies (velocity, friction, forces).
    pub rigid_bodies: Query<'w, 's, &'static mut RigidBody>,
    /// Mutable access to per-entity signals.
    pub signals: Query<'w, 's, &'static mut Signals>,
    /// Mutable access to animation state.
    pub animations: Query<'w, 's, &'static mut Animation>,
    /// Mutable access to per-entity shaders.
    pub shaders: Query<'w, 's, &'static mut EntityShader>,
    // Read-only queries
    /// Read-only access to entity groups.
    pub groups: Query<'w, 's, &'static Group>,
    /// Read-only access to screen-space positions.
    pub screen_positions: Query<'w, 's, &'static ScreenPosition>,
    /// Read-only access to box colliders.
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    /// Read-only access to world-space transforms (from parent-child hierarchy).
    pub global_transforms: Query<'w, 's, &'static GlobalTransform2D>,
    /// Read-only access to StuckTo relationships.
    pub stuckto: Query<'w, 's, &'static StuckTo>,
    /// Read-only access to rotation.
    pub rotations: Query<'w, 's, &'static Rotation>,
    /// Read-only access to scale.
    pub scales: Query<'w, 's, &'static Scale>,
    /// Read-only access to sprites.
    pub sprites: Query<'w, 's, &'static Sprite>,
    // Resources
    /// Mutable access to global world signals.
    pub world_signals: ResMut<'w, WorldSignals>,
    /// Writer for audio commands (play sounds/music).
    pub audio: MessageWriter<'w, AudioCmd>,
    /// Read-only access to world time (delta, elapsed, time_scale).
    pub world_time: Res<'w, WorldTime>,
}

/// Process Rust-based phase state machines.
///
/// This system:
/// 1. Collects all phase entities to avoid borrow conflicts
/// 2. Runs Rust callbacks (enter/update/exit) via function pointers
/// 3. Handles phase transitions requested by callbacks or external code
///
/// Entities are processed individually (not iterated) so that `PhaseCtx`
/// queries can be passed to callbacks without conflicting with the phase query.
#[allow(clippy::too_many_arguments)]
pub fn phase_system(
    mut phase_query: Query<(Entity, &mut Phase)>,
    mut ctx: PhaseCtx,
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

            if let Some(enter_fn) = on_enter_fn {
                if let Some(next) = enter_fn(entity, &mut ctx, &input) {
                    callback_transitions.push((entity, next));
                }
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
            if let Some(enter_fn) = enter_fn {
                if let Some(next) = enter_fn(entity, &mut ctx, &input) {
                    callback_transitions.push((entity, next));
                }
            }
        }

        // --- Update callback ---
        let update_fn = {
            let Ok((_, phase)) = phase_query.get(entity) else {
                continue;
            };
            phase.current_callbacks().and_then(|cbs| cbs.on_update)
        };

        if let Some(update_fn) = update_fn {
            if let Some(next) = update_fn(entity, &mut ctx, &input, delta) {
                callback_transitions.push((entity, next));
            }
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
