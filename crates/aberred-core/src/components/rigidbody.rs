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

use crate::math::Vec2;
use bevy_ecs::prelude::Component;
use rustc_hash::FxHashMap;

/// A named acceleration force that can be toggled on/off.
#[derive(Clone, Copy, Debug)]
pub struct AccelerationForce {
    /// The acceleration vector in world units per second squared.
    pub value: Vec2,
    /// Whether this force is currently active.
    pub enabled: bool,
}

impl AccelerationForce {
    /// Create a new enabled acceleration force.
    pub fn new(value: Vec2) -> Self {
        Self {
            value,
            enabled: true,
        }
    }

    /// Create a new acceleration force with specified enabled state.
    pub fn with_enabled(value: Vec2, enabled: bool) -> Self {
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
/// ```
/// # use aberred_core::components::rigidbody::RigidBody;
/// # use aberred_core::math::Vec2;
/// let mut rb = RigidBody::with_physics(5.0, Some(300.0));
/// rb.add_force("gravity", Vec2 { x: 0.0, y: 980.0 });
/// rb.add_force("wind", Vec2 { x: 50.0, y: 0.0 });
/// rb.add_force("motor", Vec2 { x: 0.0, y: -500.0 });
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
    pub velocity: Vec2,
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
            velocity: Vec2 { x: 0.0, y: 0.0 },
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
            velocity: Vec2 { x: 0.0, y: 0.0 },
            forces: FxHashMap::default(),
            friction,
            max_speed,
            frozen: false,
        }
    }

    /// Add or update a named acceleration force (enabled by default).
    pub fn add_force(&mut self, name: &str, value: Vec2) {
        self.forces
            .insert(name.to_string(), AccelerationForce::new(value));
    }

    /// Add or update a named acceleration force with specified enabled state.
    pub fn add_force_with_state(&mut self, name: &str, value: Vec2, enabled: bool) {
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
    pub fn set_force_value(&mut self, name: &str, value: Vec2) -> bool {
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
    ///
    /// `forces` is an unseeded `FxHashMap`, so this summation's iteration
    /// (and therefore f32 rounding) order is a deterministic function of
    /// `forces`' insert/remove history, not random -- safe within this
    /// engine's same-build/same-arch determinism scope once sim execution
    /// order is pinned (single-threaded schedule executor). Left as
    /// `FxHashMap` rather than restructured to a `Vec`/sorted-key summation:
    /// order-sensitive, but reproducible as-is.
    pub fn total_acceleration(&self) -> Vec2 {
        let mut total = Vec2 { x: 0.0, y: 0.0 };
        for force in self.forces.values() {
            if force.enabled {
                total += force.value;
            }
        }
        total
    }

    /// Set the velocity of the RigidBody.
    pub fn set_velocity(&mut self, velocity: Vec2) {
        self.velocity = velocity;
    }

    /// Get the current velocity.
    pub fn velocity(&self) -> Vec2 {
        self.velocity
    }

    /// Translate the RigidBody velocity by a delta vector.
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

    /// Set speed while maintaining the current direction of velocity.
    ///
    /// If the current velocity is zero, this is a no-op since there's no
    /// direction to maintain. A warning will be printed to stderr.
    ///
    /// # Arguments
    /// * `new_speed` - The desired speed (magnitude of velocity)
    pub fn set_speed(&mut self, new_speed: f32) {
        let current_speed = self.velocity.length();
        if current_speed > 0.0 {
            self.velocity = self.velocity.normalize() * new_speed;
        } else {
            log::warn!("RigidBody::set_speed called with zero velocity - operation ignored");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::vec2_approx_eq;

    #[test]
    fn add_force_is_enabled_and_overwrites_same_name() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vec2::new(0.0, 9.8));
        rb.add_force("gravity", Vec2::new(0.0, 5.0));
        assert_eq!(rb.forces.len(), 1);
        assert!(rb.is_force_enabled("gravity"));
        assert_eq!(rb.total_acceleration(), Vec2::new(0.0, 5.0));
    }

    #[test]
    fn force_mutators_report_missing_force() {
        let mut rb = RigidBody::new();
        assert!(!rb.set_force_enabled("missing", true));
        assert!(!rb.set_force_value("missing", Vec2::ONE));
        assert!(!rb.is_force_enabled("missing"));
        assert!(rb.forces.is_empty(), "mutators must not insert");

        rb.add_force("wind", Vec2::new(1.0, 0.0));
        assert!(rb.set_force_enabled("wind", false));
        assert!(!rb.is_force_enabled("wind"));
        assert!(rb.set_force_value("wind", Vec2::new(2.0, 0.0)));
        assert_eq!(rb.get_force("wind").unwrap().value, Vec2::new(2.0, 0.0));
    }

    #[test]
    fn total_acceleration_sums_enabled_forces_only() {
        let mut rb = RigidBody::new();
        assert_eq!(rb.total_acceleration(), Vec2::ZERO);

        rb.add_force("gravity", Vec2::new(0.0, 10.0));
        rb.add_force("wind", Vec2::new(5.0, 0.0));
        rb.add_force_with_state("boost", Vec2::new(100.0, 100.0), false);
        assert!(vec2_approx_eq(
            rb.total_acceleration(),
            Vec2::new(5.0, 10.0)
        ));

        rb.set_force_enabled("gravity", false);
        rb.set_force_enabled("wind", false);
        assert_eq!(rb.total_acceleration(), Vec2::ZERO);
    }

    #[test]
    fn translate_adds_to_velocity() {
        let mut rb = RigidBody::new();
        rb.velocity = Vec2::new(1.0, 2.0);
        rb.translate(3.0, -4.0);
        assert!(vec2_approx_eq(rb.velocity, Vec2::new(4.0, -2.0)));
    }

    #[test]
    fn set_speed_keeps_direction() {
        for (start, speed, expected) in [
            (Vec2::new(3.0, 4.0), 10.0, Vec2::new(6.0, 8.0)),
            (Vec2::new(-3.0, -4.0), 10.0, Vec2::new(-6.0, -8.0)),
            (Vec2::new(3.0, 4.0), 0.0, Vec2::ZERO),
        ] {
            let mut rb = RigidBody::new();
            rb.velocity = start;
            rb.set_speed(speed);
            assert!(
                vec2_approx_eq(rb.velocity, expected),
                "{start:?} -> {speed}"
            );
        }
    }

    #[test]
    fn set_speed_on_zero_velocity_is_noop() {
        let mut rb = RigidBody::new();
        rb.set_speed(10.0);
        assert_eq!(rb.velocity, Vec2::ZERO);
    }
}
