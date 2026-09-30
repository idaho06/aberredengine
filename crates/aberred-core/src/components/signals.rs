//! Lightweight per-entity signal storage for cross-system communication.
//!
//! The [`Signals`] component provides four small maps you can use to share
//! numeric, string, and boolean state between systems without introducing tight
//! coupling:
//! - floating-point scalars (`scalars`)
//! - 32-bit integers (`integers`)
//! - string values (`strings`)
//! - boolean flags (`flags`)
//!
//! Keys are `String`s, allowing you to standardize on a small set of names
//! across your game (e.g. "hp", "is_running"). Accessors are provided to set,
//! query, and read views of each collection.
//!
//! # Entity vs World Signals
//!
//! - [`Signals`] – per-entity signals, attached to specific entities
//! - [`WorldSignals`](crate::resources::worldsignals::WorldSignals) – global signals accessible from any system
//!
//! Use entity signals for per-entity state (health, sticky flag) and world
//! signals for global state (score, scene name, tracked entity counts).
//!
//! # Integration with Other Components
//!
//! - [`AnimationController`](super::animation::AnimationController) – reads signals for animation rule conditions
//! - [`Phase`](super::phase::Phase) – callbacks can read/write signals via [`PhaseContext`](super::phase::PhaseContext)
//! - [`CollisionRule`](super::collision::CollisionRule) – callbacks access signals via [`CollisionContext`](super::collision::CollisionContext)
//!
//! # Example
//!
//! ```rust
//! use aberred_core::components::signals::Signals;
//!
//! let mut s = Signals::default();
//! s.set_scalar("hp", 100.0);
//! s.set_integer("coins", 5);
//! s.set_flag("is_running");
//!
//! assert_eq!(s.get_scalar("hp"), Some(100.0));
//! assert!(s.has_flag("is_running"));
//! ```

use bevy_ecs::prelude::Component;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone, Component, Default)]
/// Bag-of-signals component used by systems to exchange simple values.
///
/// This component is intended to be attached to an entity and updated by
/// various systems. Consider clearing or normalizing your signals in a
/// dedicated system each tick if they represent transient state.
pub struct Signals {
    /// Floating-point numeric signals addressed by string keys.
    pub scalars: FxHashMap<String, f32>,
    /// Integer numeric signals addressed by string keys.
    pub integers: FxHashMap<String, i32>,
    /// Presence-only boolean flags; a key being present means "true".
    pub flags: FxHashSet<String>,
    /// String signals addressed by string keys.
    pub strings: FxHashMap<String, String>,
}

impl Signals {
    /// Set a floating-point signal value.
    pub fn set_scalar(&mut self, key: impl Into<String>, value: f32) {
        self.scalars.insert(key.into(), value);
    }
    /// Set a scalar without allocating when the key already exists.
    ///
    /// Updates the existing value in place if present; otherwise allocates the
    /// key once on first insert. Prefer this in per-frame hot paths with fixed
    /// `&'static str` keys (e.g. `movement`).
    pub fn update_scalar(&mut self, key: &str, value: f32) {
        if let Some(slot) = self.scalars.get_mut(key) {
            *slot = value;
        } else {
            self.scalars.insert(key.to_string(), value);
        }
    }
    /// Get a floating-point signal by key.
    pub fn get_scalar(&self, key: &str) -> Option<f32> {
        self.scalars.get(key).copied()
    }
    /// Remove a scalar signal by key.
    pub fn clear_scalar(&mut self, key: &str) -> Option<f32> {
        self.scalars.remove(key)
    }
    /// Read-only view of all scalar signals.
    pub fn get_scalars(&self) -> &FxHashMap<String, f32> {
        &self.scalars
    }
    /// Set an integer signal value.
    pub fn set_integer(&mut self, key: impl Into<String>, value: i32) {
        self.integers.insert(key.into(), value);
    }
    /// Get an integer signal by key.
    pub fn get_integer(&self, key: &str) -> Option<i32> {
        self.integers.get(key).copied()
    }
    /// Remove an integer signal by key.
    pub fn clear_integer(&mut self, key: &str) -> Option<i32> {
        self.integers.remove(key)
    }
    /// Read-only view of all integer signals.
    pub fn get_integers(&self) -> &FxHashMap<String, i32> {
        &self.integers
    }
    /// Create a Signals with a single flag set.
    pub fn with_flag(mut self, key: impl Into<String>) -> Self {
        self.set_flag(key);
        self
    }
    /// Mark a flag as present/true.
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.flags.insert(key.into());
    }
    /// Mark a flag present without allocating when it is already set.
    ///
    /// Allocates the key only on the first insert. Prefer this in per-frame hot
    /// paths with fixed `&'static str` keys (e.g. `movement`).
    pub fn ensure_flag(&mut self, key: &str) {
        if !self.flags.contains(key) {
            self.flags.insert(key.to_string());
        }
    }
    /// Remove a flag (make it false/absent).
    pub fn clear_flag(&mut self, key: &str) {
        self.flags.remove(key);
    }
    /// Check whether a flag is present/true.
    pub fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
    /// Remove a flag and return whether it was present.
    pub fn take_flag(&mut self, key: &str) -> bool {
        self.flags.remove(key)
    }
    /// Toggle a flag: remove it if present, add it if absent.
    pub fn toggle_flag(&mut self, key: &str) {
        if !self.flags.remove(key) {
            self.flags.insert(key.to_string());
        }
    }
    /// Read-only view of all flags.
    pub fn get_flags(&self) -> &FxHashSet<String> {
        &self.flags
    }
    /// Set a string signal value.
    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.strings.insert(key.into(), value.into());
    }
    /// Get a string signal by key.
    pub fn get_string(&self, key: &str) -> Option<&String> {
        self.strings.get(key)
    }
    /// Remove a string signal by key.
    pub fn remove_string(&mut self, key: &str) -> Option<String> {
        self.strings.remove(key)
    }
    /// Read-only view of all string signals.
    pub fn get_strings(&self) -> &FxHashMap<String, String> {
        &self.strings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_scalar_inserts_then_updates_in_place() {
        let mut s = Signals::default();
        s.update_scalar("speed_sq", 1.0);
        assert_eq!(s.get_scalar("speed_sq"), Some(1.0));

        // Repeated updates must overwrite the value without growing the map
        // (steady-state hot path stays allocation-free: same key reused).
        let ptr_before = s.scalars.get_key_value("speed_sq").unwrap().0.as_ptr();
        for v in 0..100 {
            s.update_scalar("speed_sq", v as f32);
        }
        let ptr_after = s.scalars.get_key_value("speed_sq").unwrap().0.as_ptr();
        assert_eq!(s.scalars.len(), 1);
        assert_eq!(s.get_scalar("speed_sq"), Some(99.0));
        assert_eq!(
            ptr_before, ptr_after,
            "the key String must be reused across updates (no per-call realloc)"
        );
    }

    #[test]
    fn ensure_flag_sets_and_is_idempotent() {
        let mut s = Signals::default();
        s.ensure_flag("moving");
        assert!(s.has_flag("moving"));

        // Re-asserting an already-present flag keeps the existing key (no realloc).
        let ptr_before = s.flags.get("moving").unwrap().as_ptr();
        s.ensure_flag("moving");
        let ptr_after = s.flags.get("moving").unwrap().as_ptr();
        assert_eq!(s.flags.len(), 1);
        assert_eq!(
            ptr_before, ptr_after,
            "the flag String must be reused when already present"
        );
    }

    #[test]
    fn take_flag_reports_and_clears() {
        let mut s = Signals::default().with_flag("is_running");
        assert!(s.take_flag("is_running"));
        assert!(!s.has_flag("is_running"));
        assert!(!s.take_flag("is_running"));
    }

    #[test]
    fn toggle_flag_flips_presence() {
        let mut s = Signals::default();
        s.toggle_flag("is_running");
        assert!(s.has_flag("is_running"));
        s.toggle_flag("is_running");
        assert!(!s.has_flag("is_running"));
    }
}
