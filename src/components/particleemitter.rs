//! Particle emitter component for spawning particle effects.
//!
//! The [`ParticleEmitter`] component enables entities to emit particles by cloning
//! template entities at configurable rates, directions, and speeds.
//!
//! # How It Works
//!
//! 1. Entity is spawned with a `ParticleEmitter` component containing:
//!    - Template entity IDs (particles to clone)
//!    - Emission shape (point or rectangle)
//!    - Emission rate and count
//!    - Direction arc and speed range
//!    - Optional TTL for spawned particles
//! 2. The `particle_emitter_system` runs each frame:
//!    - Accumulates time and emits particles when threshold reached
//!    - Supports catch-up behavior for large delta times
//!    - Clones template entities with position/velocity/rotation overrides
//!    - Preserves template's RigidBody fields (friction, forces) if present
//!
//! # Usage from Lua
//!
//! ```lua
//! -- Create a particle template first
//! engine.spawn()
//!     :with_group("particle")
//!     :with_sprite("smoke", 8, 8, 4, 4)
//!     :with_friction(2.0)
//!     :register_as("smoke_particle")
//!     :build()
//!
//! -- Create an emitter that spawns smoke particles
//! engine.spawn()
//!     :with_position(100, 100)
//!     :with_particle_emitter({
//!         templates = { "smoke_particle" },
//!         shape = "point",
//!         particles_per_emission = 3,
//!         emissions_per_second = 10,
//!         emissions_remaining = 100,
//!         arc = { -30, 30 },
//!         speed = { 50, 100 },
//!         ttl = { min = 0.5, max = 1.0 },
//!     })
//!     :build()
//! ```
//!
//! # Related
//!
//! - [`crate::systems::particleemitter::particle_emitter_system`] – system that emits particles
//! - [`crate::components::ttl::Ttl`] – time-to-live for automatic despawn

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

/// Shape of the emission area.
#[derive(Debug, Clone, Default)]
pub enum EmitterShape {
    /// Emit from a single point at the emitter's position.
    #[default]
    Point,
    /// Emit from random positions within a centered rectangle.
    Rect {
        /// Width of the rectangle.
        width: f32,
        /// Height of the rectangle.
        height: f32,
    },
}

/// TTL configuration for spawned particles.
#[derive(Debug, Clone, Default)]
pub enum TtlSpec {
    /// No TTL - particles live until manually despawned.
    #[default]
    None,
    /// Fixed TTL value for all particles.
    Fixed(f32),
    /// Random TTL within a range.
    Range {
        /// Minimum TTL in seconds.
        min: f32,
        /// Maximum TTL in seconds.
        max: f32,
    },
}

/// Particle emitter component for spawning particle effects.
///
/// Emits particles by cloning template entities with velocity, rotation,
/// and optional TTL overrides. Supports point and rectangle emission shapes,
/// configurable direction arcs, and speed ranges.
///
/// # Fields
///
/// - `templates` - Entity IDs to clone (resolved from WorldSignals keys)
/// - `shape` - Point or rectangle emission area
/// - `offset` - Offset from owner's MapPosition
/// - `particles_per_emission` - Particles spawned per emission event
/// - `emissions_per_second` - Emission frequency (0 or negative = disabled)
/// - `emissions_remaining` - Emissions left before stopping (0 = stopped)
/// - `arc_degrees` - Direction range in degrees (0° = up, normalized min/max)
/// - `speed_range` - Speed range for particles (normalized min/max)
/// - `ttl` - TTL configuration for spawned particles
/// - `time_since_emit` - Internal accumulator for emission timing
#[derive(Component, Debug, Clone)]
pub struct ParticleEmitter {
    /// Template entities to clone. Must be non-empty to emit.
    pub templates: Vec<Entity>,
    /// Emission area shape.
    pub shape: EmitterShape,
    /// Offset from owner's MapPosition.
    pub offset: Vector2,
    /// Number of particles spawned per emission event.
    pub particles_per_emission: u32,
    /// Emissions per second. If <= 0, no emissions occur.
    pub emissions_per_second: f32,
    /// Remaining emission events. When 0, emitter stops.
    pub emissions_remaining: u32,
    /// Direction arc in degrees. 0° points up. Stored as (min, max).
    pub arc_degrees: (f32, f32),
    /// Speed range for particles. Stored as (min, max).
    pub speed_range: (f32, f32),
    /// TTL configuration for spawned particles.
    pub ttl: TtlSpec,
    /// Time accumulated since last emission.
    pub time_since_emit: f32,
}

impl Default for ParticleEmitter {
    fn default() -> Self {
        Self {
            templates: Vec::new(),
            shape: EmitterShape::Point,
            offset: Vector2 { x: 0.0, y: 0.0 },
            particles_per_emission: 1,
            emissions_per_second: 10.0,
            emissions_remaining: 100,
            arc_degrees: (0.0, 360.0),
            speed_range: (50.0, 100.0),
            ttl: TtlSpec::None,
            time_since_emit: 0.0,
        }
    }
}

