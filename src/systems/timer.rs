//! Rust timer systems.
//!
//! This module provides systems for processing [`Timer`](crate::components::timer::Timer) components:
//!
//! - [`update_timers`] – updates timer elapsed time and emits events when they expire
//! - [`timer_observer`] – observer that calls Rust callbacks when timer events fire
//! - [`TimerCtx`] – bundled ECS access passed to timer callbacks
//!
//! # System Flow
//!
//! Each frame:
//!
//! 1. `update_timers` accumulates delta time on all Timer components
//! 2. When `elapsed >= duration`, emits `TimerEvent` and resets timer
//! 3. `timer_observer` receives events and calls the Rust callback
//! 4. Callback executes with full ECS access through `TimerCtx`
//!
//! # Callback Signature
//!
//! ```ignore
//! fn my_callback(entity: Entity, ctx: &mut TimerCtx, input: &InputState) {
//!     // Full ECS access: queries, commands, resources
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::components::timer::Timer`] – the timer component
//! - [`crate::events::timer::TimerEvent`] – event emitted on expiration
//! - [`crate::systems::luatimer`] – Lua equivalent

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::entityshader::EntityShader;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::components::timer::Timer;
use crate::events::audio::AudioCmd;
use crate::events::timer::TimerEvent;
use crate::resources::input::InputState;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

/// Bundled ECS access passed to Rust timer callbacks.
///
/// Mirrors the system parameters that [`lua_timer_observer`](crate::systems::luatimer::lua_timer_observer)
/// uses, providing full query and resource access so that Rust timer callbacks
/// can read/write any entity's components and interact with engine resources.
///
/// # Usage in callbacks
///
/// ```ignore
/// fn my_callback(entity: Entity, ctx: &mut TimerCtx, input: &InputState) {
///     // Modify the timer entity's velocity
///     if let Ok(mut rb) = ctx.rigid_bodies.get_mut(entity) {
///         rb.velocity = Vector2::zero();
///     }
///     // Play a sound
///     ctx.audio.write(AudioCmd::PlayFx { id: "boom".into() });
///     // Set a global signal
///     ctx.world_signals.set_flag("game_over");
///     // Despawn an entity
///     ctx.commands.entity(entity).despawn();
/// }
/// ```
#[derive(SystemParam)]
pub struct TimerCtx<'w, 's> {
    /// ECS command buffer for spawning, despawning, inserting/removing components.
    pub commands: Commands<'w, 's>,
    // Mutable queries (most commonly needed in callbacks)
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
    for (entity, mut timer) in query.iter_mut() {
        timer.elapsed += world_time.delta;
        if timer.elapsed >= timer.duration {
            commands.trigger(TimerEvent {
                entity,
                callback: timer.callback,
            });
            timer.reset();
        }
    }
}

/// Observer that handles Rust timer events by calling the callback function.
///
/// When a [`TimerEvent`](crate::events::timer::TimerEvent) is triggered:
///
/// 1. Extracts the entity and callback from the event
/// 2. Calls the callback with `(entity, &mut TimerCtx, &InputState)`
/// 3. The callback can use `TimerCtx` to interact with the ECS
pub fn timer_observer(trigger: On<TimerEvent>, input: Res<InputState>, mut ctx: TimerCtx) {
    let event = trigger.event();
    (event.callback)(event.entity, &mut ctx, &input);
}
