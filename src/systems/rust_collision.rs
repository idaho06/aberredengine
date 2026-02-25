//! Rust collision observer and callback dispatch.
//!
//! This module provides the Rust-native collision handling:
//!
//! - [`CollisionCtx`] – bundled ECS access passed to collision callbacks
//! - [`rust_collision_observer`] – receives [`CollisionEvent`](crate::events::collision::CollisionEvent)s
//!   and dispatches to [`CollisionRule`](crate::components::collision::CollisionRule) callbacks
//!
//! # Collision Flow
//!
//! 1. [`collision_detector`](crate::systems::collision_detector::collision_detector) detects overlaps
//!    and emits `CollisionEvent`s
//! 2. `rust_collision_observer` looks up matching Rust collision rules by
//!    [`Group`](crate::components::group::Group) names
//! 3. For each match, computes collision sides and calls the Rust callback
//!
//! # Callback Signature
//!
//! ```ignore
//! fn my_collision(
//!     a: Entity,
//!     b: Entity,
//!     sides_a: &BoxSides,
//!     sides_b: &BoxSides,
//!     ctx: &mut CollisionCtx,
//! ) {
//!     // Full ECS access via ctx
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – pure Rust collision detection
//! - [`crate::components::collision::CollisionRule`] – defines Rust collision handlers
//! - [`crate::components::collision::CollisionCallback`] – callback type alias
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision::CollisionEvent`] – emitted on each collision

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::collision::{get_colliding_sides, CollisionRule};
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::events::audio::AudioCmd;
use crate::events::collision::CollisionEvent;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

/// Bundled ECS access passed to Rust collision callbacks.
///
/// Mirrors [`TimerCtx`](crate::systems::timer::TimerCtx) and
/// [`PhaseCtx`](crate::systems::phase::PhaseCtx) field-for-field,
/// providing full query and resource access so that collision callbacks
/// can read/write any entity's components and interact with engine resources.
///
/// # Usage in callbacks
///
/// ```ignore
/// fn on_ball_brick(
///     ball: Entity,
///     brick: Entity,
///     sides_a: &BoxSides,
///     sides_b: &BoxSides,
///     ctx: &mut CollisionCtx,
/// ) {
///     // Reflect the ball
///     if let Ok(mut rb) = ctx.rigid_bodies.get_mut(ball) {
///         rb.velocity.y = -rb.velocity.y;
///     }
///     // Despawn the brick
///     ctx.commands.entity(brick).despawn();
///     // Play a sound
///     ctx.audio.write(AudioCmd::PlayFx { id: "hit".into() });
/// }
/// ```
#[derive(SystemParam)]
pub struct CollisionCtx<'w, 's> {
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

/// Observer that handles Rust collision rules.
///
/// When a [`CollisionEvent`] is triggered:
///
/// 1. Looks up [`Group`] names for both entities (returns early if missing)
/// 2. Queries all [`CollisionRule`] entities for a matching rule
/// 3. Computes collision sides via [`get_colliding_sides`]
/// 4. Calls the matched callback with `(ent_a, ent_b, &sides_a, &sides_b, &mut ctx)`
pub fn rust_collision_observer(
    trigger: On<CollisionEvent>,
    rules: Query<&CollisionRule>,
    mut ctx: CollisionCtx,
) {
    if rules.is_empty() {
        return;
    }

    let a = trigger.event().a;
    let b = trigger.event().b;

    let ga = if let Ok(group) = ctx.groups.get(a) {
        group.name()
    } else {
        return;
    };
    let gb = if let Ok(group) = ctx.groups.get(b) {
        group.name()
    } else {
        return;
    };

    for rule in rules.iter() {
        if let Some((ent_a, ent_b)) = rule.match_and_order(a, b, ga, gb) {
            // Resolve world positions using GlobalTransform2D when available
            let pos_a = ctx
                .positions
                .get(ent_a)
                .ok()
                .map(|p| {
                    ctx.global_transforms
                        .get(ent_a)
                        .ok()
                        .map_or(p.pos, |gt| gt.position)
                });
            let pos_b = ctx
                .positions
                .get(ent_b)
                .ok()
                .map(|p| {
                    ctx.global_transforms
                        .get(ent_b)
                        .ok()
                        .map_or(p.pos, |gt| gt.position)
                });

            // Compute collider rectangles for side detection
            let rect_a = ctx
                .box_colliders
                .get(ent_a)
                .ok()
                .and_then(|c| pos_a.map(|pos| c.as_rectangle(pos)));
            let rect_b = ctx
                .box_colliders
                .get(ent_b)
                .ok()
                .and_then(|c| pos_b.map(|pos| c.as_rectangle(pos)));

            let (sides_a, sides_b) = match (rect_a, rect_b) {
                (Some(ra), Some(rb)) => get_colliding_sides(&ra, &rb).unwrap_or_default(),
                _ => Default::default(),
            };

            let callback = rule.callback;
            callback(ent_a, ent_b, &sides_a, &sides_b, &mut ctx);
            return;
        }
    }
}
