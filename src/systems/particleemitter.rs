//! Particle emitter system.
//!
//! This system processes [`ParticleEmitter`] components and spawns particles
//! by cloning template entities with velocity, rotation, and TTL overrides.
//!
//! # Behavior
//!
//! - Accumulates time and emits particles based on `emissions_per_second`
//! - Supports catch-up: if dt is large, may emit multiple times per frame
//! - Clones random templates from the template list
//! - Overrides position, velocity, rotation on cloned particles
//! - Preserves template's RigidBody fields (friction, max_speed, forces)
//! - Optionally inserts TTL component based on emitter configuration
//! - Stops emitting when `emissions_remaining` reaches 0
//!
//! # Coordinate System
//!
//! - 0° points up (negative Y in screen coordinates)
//! - Angles increase clockwise
//! - Y+ is down (screen coordinates)

use bevy_ecs::prelude::*;
use fastrand::Rng;
use raylib::prelude::Vector2;

// use crate::components::animation::Animation;
use crate::components::mapposition::MapPosition;
use crate::components::particleemitter::{EmitterShape, ParticleEmitter, TtlSpec};
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::ttl::Ttl;
use crate::resources::worldtime::WorldTime;

/// System that processes particle emitters and spawns particles.
///
/// Queries all entities with `ParticleEmitter` and `MapPosition`, accumulates
/// time, and spawns particles by cloning templates when thresholds are met.
///
/// # Ordering
///
/// Should run **before** `movement` so particles move on their spawn frame.
pub fn particle_emitter_system(
    mut emitter_query: Query<(&MapPosition, &mut ParticleEmitter)>,
    rigidbody_query: Query<&RigidBody>,
    // mut animation_query: Query<&mut Animation>,
    time: Res<WorldTime>,
    mut commands: Commands,
    mut rng: Local<Rng>,
) {
    let dt = time.delta; // delta is already scaled
    if dt <= 0.0 {
        return;
    }

    for (owner_pos, mut emitter) in emitter_query.iter_mut() {
        // Skip if no templates, no emissions remaining, or rate is zero/negative
        if emitter.templates.is_empty()
            || emitter.emissions_remaining == 0
            || emitter.emissions_per_second <= 0.0
        {
            continue;
        }

        let period = 1.0 / emitter.emissions_per_second;
        emitter.time_since_emit += dt;

        // Catch-up loop: emit multiple times if dt is large
        while emitter.time_since_emit >= period && emitter.emissions_remaining > 0 {
            emit_particles(
                &mut commands,
                owner_pos,
                &emitter,
                &rigidbody_query,
                // &mut animation_query,
                &mut rng,
            );
            emitter.time_since_emit -= period;
            emitter.emissions_remaining -= 1;
        }
    }
}

/// Sample a random f32 in the range [min, max].
/// If the range is smaller than EPSILON, returns min directly.
#[inline]
fn random_f32_range(rng: &mut Rng, min: f32, max: f32) -> f32 {
    let range = max - min;
    if range < f32::EPSILON {
        return min;
    }
    min + rng.f32() * range
}

/// Emit particles for a single emission event.
fn emit_particles(
    commands: &mut Commands,
    owner_pos: &MapPosition,
    emitter: &ParticleEmitter,
    rigidbody_query: &Query<&RigidBody>,
    // animation_query: &mut Query<&mut Animation>,
    rng: &mut Rng,
) {
    let base_pos = owner_pos.pos + emitter.offset;

    for _ in 0..emitter.particles_per_emission {
        // Pick a random template
        let template_idx = rng.usize(0..emitter.templates.len());
        let template = emitter.templates[template_idx];

        // Check if template still exists
        if commands.get_entity(template).is_err() {
            continue;
        }

        // Sample spawn position
        let spawn_pos = match emitter.shape {
            EmitterShape::Point => base_pos,
            EmitterShape::Rect { width, height } => {
                let dx = random_f32_range(rng, -width / 2.0, width / 2.0);
                let dy = random_f32_range(rng, -height / 2.0, height / 2.0);
                Vector2 {
                    x: base_pos.x + dx,
                    y: base_pos.y + dy,
                }
            }
        };

        // Sample angle (degrees)
        let (arc_min, arc_max) = emitter.arc_degrees;
        let angle_deg = random_f32_range(rng, arc_min, arc_max);

        // Sample speed
        let (speed_min, speed_max) = emitter.speed_range;
        let speed = random_f32_range(rng, speed_min, speed_max);

        // Convert angle to direction vector (0° = up, Y+ is down)
        let theta = angle_deg.to_radians();
        let dir = Vector2 {
            x: theta.sin(),
            y: -theta.cos(),
        };
        let velocity = Vector2 {
            x: dir.x * speed,
            y: dir.y * speed,
        };

        // Sample TTL
        let ttl_value = match &emitter.ttl {
            TtlSpec::None => None,
            TtlSpec::Fixed(v) => Some(*v),
            TtlSpec::Range { min, max } => Some(random_f32_range(rng, *min, *max)),
        };

        // Read template's RigidBody to preserve fields
        let template_rb = rigidbody_query.get(template).ok().cloned();

        // Build RigidBody: preserve template fields but override velocity
        let rb = if let Some(mut rb) = template_rb {
            rb.velocity = velocity;
            rb
        } else {
            let mut rb = RigidBody::new();
            rb.velocity = velocity;
            rb
        };

        // get Animation component to reset frame index
        /* if let Ok(mut animation) = animation_query.get_mut(template) {
            animation.reset();
        } */

        // Clone and spawn with overrides
        let mut source_commands = commands.entity(template);
        source_commands
            .clone_and_spawn()
            .insert(MapPosition::new(spawn_pos.x, spawn_pos.y))
            .insert(Rotation { degrees: angle_deg })
            .insert(rb)
            .insert_if(Ttl::new(ttl_value.unwrap_or(0.0)), || ttl_value.is_some());
    }
}
