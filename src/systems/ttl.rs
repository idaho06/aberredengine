//! TTL (Time-to-live) system.
//!
//! This module provides the [`ttl_system`] that decrements TTL timers and
//! despawns entities when their time runs out.
//!
//! # System Flow
//!
//! Each frame:
//!
//! 1. `ttl_system` iterates all entities with [`Ttl`](crate::components::ttl::Ttl)
//! 2. Decrements `remaining` by `delta * time_scale`
//! 3. When `remaining <= 0`, despawns the entity
//!
//! # Time Scaling
//!
//! The countdown respects [`WorldTime::time_scale`](crate::resources::worldtime::WorldTime),
//! so slow-motion effects affect TTL duration.

use bevy_ecs::prelude::*;

use crate::components::ttl::Ttl;
use crate::resources::worldtime::WorldTime;

/// Decrements TTL and despawns entities when it reaches zero.
///
/// This system runs each frame and:
/// - Subtracts `delta * time_scale` from all [`Ttl`](crate::components::ttl::Ttl) components
/// - Despawns any entity whose TTL reaches zero or below
///
/// # Performance
///
/// Uses a simple query over all Ttl components. For games with many temporary
/// entities (bullets, particles), consider entity pooling instead.
pub fn ttl_system(
    world_time: Res<WorldTime>,
    mut query: Query<(Entity, &mut Ttl)>,
    mut commands: Commands,
) {
    let dt = world_time.delta; // delta is already scaled by time_scale
    for (entity, mut ttl) in query.iter_mut() {
        ttl.remaining -= dt;
        if ttl.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}
