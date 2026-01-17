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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
            self.velocity = self.velocity.normalized() * new_speed;
        } else {
            eprintln!("[WARN] RigidBody::set_speed called with zero velocity - operation ignored");
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

    fn vec_approx_eq(a: Vector2, b: Vector2) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y)
    }

    // ==================== ACCELERATION FORCE TESTS ====================

    #[test]
    fn test_acceleration_force_new() {
        let force = AccelerationForce::new(Vector2 { x: 10.0, y: 20.0 });
        assert!(approx_eq(force.value.x, 10.0));
        assert!(approx_eq(force.value.y, 20.0));
        assert!(force.enabled); // enabled by default
    }

    #[test]
    fn test_acceleration_force_with_enabled_true() {
        let force = AccelerationForce::with_enabled(Vector2 { x: 5.0, y: 5.0 }, true);
        assert!(force.enabled);
    }

    #[test]
    fn test_acceleration_force_with_enabled_false() {
        let force = AccelerationForce::with_enabled(Vector2 { x: 5.0, y: 5.0 }, false);
        assert!(!force.enabled);
    }

    // ==================== RIGIDBODY CONSTRUCTOR TESTS ====================

    #[test]
    fn test_rigidbody_new() {
        let rb = RigidBody::new();
        assert!(vec_approx_eq(rb.velocity, Vector2 { x: 0.0, y: 0.0 }));
        assert!(rb.forces.is_empty());
        assert!(approx_eq(rb.friction, 0.0));
        assert!(rb.max_speed.is_none());
        assert!(!rb.frozen);
    }

    #[test]
    fn test_rigidbody_default() {
        let rb = RigidBody::default();
        assert!(vec_approx_eq(rb.velocity, Vector2 { x: 0.0, y: 0.0 }));
        assert!(rb.forces.is_empty());
    }

    #[test]
    fn test_rigidbody_with_physics() {
        let rb = RigidBody::with_physics(5.0, Some(300.0));
        assert!(approx_eq(rb.friction, 5.0));
        assert_eq!(rb.max_speed, Some(300.0));
        assert!(vec_approx_eq(rb.velocity, Vector2 { x: 0.0, y: 0.0 }));
        assert!(!rb.frozen);
    }

    #[test]
    fn test_rigidbody_with_physics_no_max_speed() {
        let rb = RigidBody::with_physics(10.0, None);
        assert!(approx_eq(rb.friction, 10.0));
        assert!(rb.max_speed.is_none());
    }

    // ==================== FORCE MANAGEMENT TESTS ====================

    #[test]
    fn test_add_force() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vector2 { x: 0.0, y: 980.0 });
        assert_eq!(rb.forces.len(), 1);
        let force = rb.get_force("gravity").unwrap();
        assert!(approx_eq(force.value.y, 980.0));
        assert!(force.enabled);
    }

    #[test]
    fn test_add_force_overwrites() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vector2 { x: 0.0, y: 100.0 });
        rb.add_force("gravity", Vector2 { x: 0.0, y: 200.0 });
        assert_eq!(rb.forces.len(), 1);
        let force = rb.get_force("gravity").unwrap();
        assert!(approx_eq(force.value.y, 200.0));
    }

    #[test]
    fn test_add_force_with_state_enabled() {
        let mut rb = RigidBody::new();
        rb.add_force_with_state("wind", Vector2 { x: 50.0, y: 0.0 }, true);
        let force = rb.get_force("wind").unwrap();
        assert!(force.enabled);
    }

    #[test]
    fn test_add_force_with_state_disabled() {
        let mut rb = RigidBody::new();
        rb.add_force_with_state("wind", Vector2 { x: 50.0, y: 0.0 }, false);
        let force = rb.get_force("wind").unwrap();
        assert!(!force.enabled);
    }

    #[test]
    fn test_remove_force() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vector2 { x: 0.0, y: 980.0 });
        assert_eq!(rb.forces.len(), 1);
        rb.remove_force("gravity");
        assert!(rb.forces.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_force() {
        let mut rb = RigidBody::new();
        rb.remove_force("nonexistent"); // should not panic
        assert!(rb.forces.is_empty());
    }

    #[test]
    fn test_set_force_enabled() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vector2 { x: 0.0, y: 980.0 });
        assert!(rb.is_force_enabled("gravity"));

        let result = rb.set_force_enabled("gravity", false);
        assert!(result);
        assert!(!rb.is_force_enabled("gravity"));

        let result = rb.set_force_enabled("gravity", true);
        assert!(result);
        assert!(rb.is_force_enabled("gravity"));
    }

    #[test]
    fn test_set_force_enabled_nonexistent() {
        let mut rb = RigidBody::new();
        let result = rb.set_force_enabled("nonexistent", true);
        assert!(!result);
    }

    #[test]
    fn test_is_force_enabled_nonexistent() {
        let rb = RigidBody::new();
        assert!(!rb.is_force_enabled("nonexistent"));
    }

    #[test]
    fn test_set_force_value() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vector2 { x: 0.0, y: 100.0 });

        let result = rb.set_force_value("gravity", Vector2 { x: 0.0, y: 200.0 });
        assert!(result);
        let force = rb.get_force("gravity").unwrap();
        assert!(approx_eq(force.value.y, 200.0));
    }

    #[test]
    fn test_set_force_value_nonexistent() {
        let mut rb = RigidBody::new();
        let result = rb.set_force_value("nonexistent", Vector2 { x: 0.0, y: 0.0 });
        assert!(!result);
    }

    #[test]
    fn test_get_force() {
        let mut rb = RigidBody::new();
        rb.add_force("test", Vector2 { x: 1.0, y: 2.0 });
        let force = rb.get_force("test");
        assert!(force.is_some());
        assert!(approx_eq(force.unwrap().value.x, 1.0));
    }

    #[test]
    fn test_get_force_nonexistent() {
        let rb = RigidBody::new();
        assert!(rb.get_force("nonexistent").is_none());
    }

    // ==================== TOTAL ACCELERATION TESTS ====================

    #[test]
    fn test_total_acceleration_empty() {
        let rb = RigidBody::new();
        let total = rb.total_acceleration();
        assert!(vec_approx_eq(total, Vector2 { x: 0.0, y: 0.0 }));
    }

    #[test]
    fn test_total_acceleration_single_force() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vector2 { x: 0.0, y: 980.0 });
        let total = rb.total_acceleration();
        assert!(vec_approx_eq(total, Vector2 { x: 0.0, y: 980.0 }));
    }

    #[test]
    fn test_total_acceleration_multiple_forces() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vector2 { x: 0.0, y: 100.0 });
        rb.add_force("wind", Vector2 { x: 50.0, y: 0.0 });
        rb.add_force("thrust", Vector2 { x: 0.0, y: -30.0 });
        let total = rb.total_acceleration();
        assert!(vec_approx_eq(total, Vector2 { x: 50.0, y: 70.0 }));
    }

    #[test]
    fn test_total_acceleration_disabled_forces_excluded() {
        let mut rb = RigidBody::new();
        rb.add_force("gravity", Vector2 { x: 0.0, y: 100.0 });
        rb.add_force_with_state("wind", Vector2 { x: 50.0, y: 0.0 }, false);
        let total = rb.total_acceleration();
        assert!(vec_approx_eq(total, Vector2 { x: 0.0, y: 100.0 }));
    }

    #[test]
    fn test_total_acceleration_all_disabled() {
        let mut rb = RigidBody::new();
        rb.add_force_with_state("gravity", Vector2 { x: 0.0, y: 100.0 }, false);
        rb.add_force_with_state("wind", Vector2 { x: 50.0, y: 0.0 }, false);
        let total = rb.total_acceleration();
        assert!(vec_approx_eq(total, Vector2 { x: 0.0, y: 0.0 }));
    }

    // ==================== VELOCITY TESTS ====================

    #[test]
    fn test_set_velocity() {
        let mut rb = RigidBody::new();
        rb.set_velocity(Vector2 { x: 100.0, y: 200.0 });
        assert!(vec_approx_eq(rb.velocity, Vector2 { x: 100.0, y: 200.0 }));
    }

    #[test]
    fn test_velocity_getter() {
        let mut rb = RigidBody::new();
        rb.velocity = Vector2 { x: 50.0, y: 75.0 };
        let vel = rb.velocity();
        assert!(vec_approx_eq(vel, Vector2 { x: 50.0, y: 75.0 }));
    }

    #[test]
    fn test_translate() {
        let mut rb = RigidBody::new();
        rb.velocity = Vector2 { x: 10.0, y: 20.0 };
        rb.translate(5.0, -3.0);
        assert!(vec_approx_eq(rb.velocity, Vector2 { x: 15.0, y: 17.0 }));
    }

    // ==================== FREEZE/UNFREEZE TESTS ====================

    #[test]
    fn test_freeze() {
        let mut rb = RigidBody::new();
        assert!(!rb.frozen);
        rb.freeze();
        assert!(rb.frozen);
    }

    #[test]
    fn test_unfreeze() {
        let mut rb = RigidBody::new();
        rb.frozen = true;
        rb.unfreeze();
        assert!(!rb.frozen);
    }

    // ==================== SET SPEED TESTS ====================

    #[test]
    fn test_set_speed_maintains_direction() {
        let mut rb = RigidBody::new();
        rb.velocity = Vector2 { x: 3.0, y: 4.0 }; // magnitude = 5
        rb.set_speed(10.0);
        // Direction should be preserved: (0.6, 0.8) * 10 = (6, 8)
        assert!(approx_eq(rb.velocity.x, 6.0));
        assert!(approx_eq(rb.velocity.y, 8.0));
        assert!(approx_eq(rb.velocity.length(), 10.0));
    }

    #[test]
    fn test_set_speed_with_zero_velocity() {
        let mut rb = RigidBody::new();
        rb.velocity = Vector2 { x: 0.0, y: 0.0 };
        rb.set_speed(10.0);
        // Should be no-op when velocity is zero
        assert!(vec_approx_eq(rb.velocity, Vector2 { x: 0.0, y: 0.0 }));
    }

    #[test]
    fn test_set_speed_to_zero() {
        let mut rb = RigidBody::new();
        rb.velocity = Vector2 { x: 3.0, y: 4.0 };
        rb.set_speed(0.0);
        assert!(approx_eq(rb.velocity.length(), 0.0));
    }

    #[test]
    fn test_set_speed_negative_direction() {
        let mut rb = RigidBody::new();
        rb.velocity = Vector2 { x: -3.0, y: -4.0 }; // magnitude = 5
        rb.set_speed(10.0);
        // Direction preserved: (-0.6, -0.8) * 10 = (-6, -8)
        assert!(approx_eq(rb.velocity.x, -6.0));
        assert!(approx_eq(rb.velocity.y, -8.0));
    }
}
