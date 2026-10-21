//! Emitted particle marker component.
//!
//! Entities with the [`EmittedParticle`] component were spawned by a
//! [`ParticleEmitter`](crate::components::particle_emitter::ParticleEmitter).
//! Use this to filter or query specifically for particle emitter output.

use bevy_ecs::prelude::{Component, Entity};

/// Stores the [`Entity`] of the [`ParticleEmitter`](crate::components::particle_emitter::ParticleEmitter) that spawned this particle.
#[derive(Component, Clone, Debug)]
pub struct EmittedParticle(pub Entity);
