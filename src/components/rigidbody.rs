//! Kinematic body component with multiple named acceleration forces.
//!
//! The [`RigidBody`] component stores velocity and multiple named acceleration
//! forces for an entity. Each force can be individually enabled/disabled,
//! allowing game logic to toggle forces like gravity, wind, or motor thrust
//! independently.
//!
//! The `frozen` flag allows temporarily disabling all movement calculations,
//! useful when an entity's position is controlled externally (e.g., ball stuck
//! to paddle).

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;
use rustc_hash::FxHashMap;

/// A named acceleration force that can be toggled on/off.
#[derive(Clone, Copy, Debug)]
pub struct AccelerationForce {
    /// The acceleration vector in world units per second squared.
    pub value: Vector2,
    /// Whether this force is currently active.
    pub enabled: bool,
}

impl AccelerationForce {
    /// Create a new enabled acceleration force.
    pub fn new(value: Vector2) -> Self {
        Self {
            value,
            enabled: true,
        }
    }

    /// Create a new acceleration force with specified enabled state.
    pub fn with_enabled(value: Vector2, enabled: bool) -> Self {
        Self { value, enabled }
    }
}

/// Kinematic body storing velocity and multiple named acceleration forces.
///
/// Intended to be updated by input/physics systems and consumed by movement
/// systems to update [`MapPosition`](super::mapposition::MapPosition).
///
/// # Fields
/// - `velocity` - Current velocity in world units per second
/// - `forces` - Named acceleration forces that can be individually toggled
/// - `friction` - Velocity damping factor (0.0 = no friction, higher = more drag)
/// - `max_speed` - Optional maximum speed clamp
/// - `frozen` - When true, movement system skips all calculations for this entity
///
/// # Example
/// ```ignore
/// let mut rb = RigidBody::with_physics(5.0, Some(300.0));
/// rb.add_force("gravity", Vector2 { x: 0.0, y: 980.0 });
/// rb.add_force("wind", Vector2 { x: 50.0, y: 0.0 });
/// rb.add_force("motor", Vector2 { x: 0.0, y: -500.0 });
///
/// // Disable gravity when on ground
/// rb.set_force_enabled("gravity", false);
///
/// // Freeze position (e.g., ball stuck to paddle)
/// rb.frozen = true;
/// ```
#[derive(Component, Clone, Debug)]
pub struct RigidBody {
    /// Current velocity in world units per second.
    pub velocity: Vector2,
    /// Named acceleration forces. The total acceleration is the sum of all enabled forces.
    pub forces: FxHashMap<String, AccelerationForce>,
    /// Velocity damping factor. Applied as: velocity *= (1 - friction * delta).
    /// Typical values: 0.0 (no friction) to 10.0 (heavy drag).
    pub friction: f32,
    /// Optional maximum speed. If set, velocity magnitude is clamped to this value.
    pub max_speed: Option<f32>,
    /// When true, movement system skips all physics calculations for this entity.
    /// Position can still be modified externally (e.g., by StuckTo system).
    pub frozen: bool,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self::new()
    }
}

impl RigidBody {
    /// Create a RigidBody with zero velocity and no forces.
    pub fn new() -> Self {
        Self {
            velocity: Vector2 { x: 0.0, y: 0.0 },
            forces: FxHashMap::default(),
            friction: 0.0,
            max_speed: None,
            frozen: false,
        }
    }

    /// Create a RigidBody with physics parameters configured.
    ///
    /// # Arguments
    /// * `friction` - Velocity damping (0.0 = none, ~5.0 = responsive, ~10.0 = heavy)
    /// * `max_speed` - Optional velocity magnitude limit
    pub fn with_physics(friction: f32, max_speed: Option<f32>) -> Self {
        Self {
            velocity: Vector2 { x: 0.0, y: 0.0 },
            forces: FxHashMap::default(),
            friction,
            max_speed,
            frozen: false,
        }
    }

    /// Add or update a named acceleration force (enabled by default).
    pub fn add_force(&mut self, name: &str, value: Vector2) {
        self.forces
            .insert(name.to_string(), AccelerationForce::new(value));
    }

    /// Add or update a named acceleration force with specified enabled state.
    pub fn add_force_with_state(&mut self, name: &str, value: Vector2, enabled: bool) {
        self.forces.insert(
            name.to_string(),
            AccelerationForce::with_enabled(value, enabled),
        );
    }

    /// Remove a named force entirely.
    pub fn remove_force(&mut self, name: &str) {
        self.forces.remove(name);
    }

    /// Enable or disable a specific force by name.
    /// Returns false if the force doesn't exist.
    pub fn set_force_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(force) = self.forces.get_mut(name) {
            force.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Check if a force exists and is enabled.
    pub fn is_force_enabled(&self, name: &str) -> bool {
        self.forces.get(name).map(|f| f.enabled).unwrap_or(false)
    }

    /// Update the value of an existing force.
    /// Returns false if the force doesn't exist.
    pub fn set_force_value(&mut self, name: &str, value: Vector2) -> bool {
        if let Some(force) = self.forces.get_mut(name) {
            force.value = value;
            true
        } else {
            false
        }
    }

    /// Get the value of a force by name.
    pub fn get_force(&self, name: &str) -> Option<&AccelerationForce> {
        self.forces.get(name)
    }

    /// Calculate the total acceleration from all enabled forces.
    pub fn total_acceleration(&self) -> Vector2 {
        let mut total = Vector2 { x: 0.0, y: 0.0 };
        for force in self.forces.values() {
            if force.enabled {
                total += force.value;
            }
        }
        total
    }

    /// Set the velocity of the RigidBody.
    pub fn set_velocity(&mut self, velocity: Vector2) {
        self.velocity = velocity;
    }

    /// Get the current velocity.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn velocity(&self) -> Vector2 {
        self.velocity
    }

    /// Translate the RigidBody velocity by a delta vector.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.velocity.x += dx;
        self.velocity.y += dy;
    }

    /// Freeze the rigid body, preventing movement system from updating it.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Unfreeze the rigid body, allowing movement system to update it.
    pub fn unfreeze(&mut self) {
        self.frozen = false;
    }
}
