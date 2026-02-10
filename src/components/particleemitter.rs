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

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_particle_emitter_default_templates_empty() {
        let e = ParticleEmitter::default();
        assert!(e.templates.is_empty());
    }

    #[test]
    fn test_particle_emitter_default_shape_is_point() {
        let e = ParticleEmitter::default();
        assert!(matches!(e.shape, EmitterShape::Point));
    }

    #[test]
    fn test_particle_emitter_default_values() {
        let e = ParticleEmitter::default();
        assert!(approx_eq(e.offset.x, 0.0));
        assert!(approx_eq(e.offset.y, 0.0));
        assert_eq!(e.particles_per_emission, 1);
        assert!(approx_eq(e.emissions_per_second, 10.0));
        assert_eq!(e.emissions_remaining, 100);
        assert!(approx_eq(e.arc_degrees.0, 0.0));
        assert!(approx_eq(e.arc_degrees.1, 360.0));
        assert!(approx_eq(e.speed_range.0, 50.0));
        assert!(approx_eq(e.speed_range.1, 100.0));
        assert!(approx_eq(e.time_since_emit, 0.0));
    }

    #[test]
    fn test_emitter_shape_default_is_point() {
        let shape = EmitterShape::default();
        assert!(matches!(shape, EmitterShape::Point));
    }

    #[test]
    fn test_emitter_shape_rect() {
        let shape = EmitterShape::Rect {
            width: 10.0,
            height: 20.0,
        };
        if let EmitterShape::Rect { width, height } = shape {
            assert!(approx_eq(width, 10.0));
            assert!(approx_eq(height, 20.0));
        } else {
            panic!("Expected Rect variant");
        }
    }

    #[test]
    fn test_ttl_spec_default_is_none() {
        let ttl = TtlSpec::default();
        assert!(matches!(ttl, TtlSpec::None));
    }

    #[test]
    fn test_ttl_spec_fixed() {
        let ttl = TtlSpec::Fixed(2.5);
        if let TtlSpec::Fixed(v) = ttl {
            assert!(approx_eq(v, 2.5));
        } else {
            panic!("Expected Fixed variant");
        }
    }

    #[test]
    fn test_ttl_spec_range() {
        let ttl = TtlSpec::Range {
            min: 0.5,
            max: 1.5,
        };
        if let TtlSpec::Range { min, max } = ttl {
            assert!(approx_eq(min, 0.5));
            assert!(approx_eq(max, 1.5));
        } else {
            panic!("Expected Range variant");
        }
    }
}

