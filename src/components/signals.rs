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
//! use aberredengine::components::signals::Signals;
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

#[derive(Debug, Clone, Component)]
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

impl Default for Signals {
    fn default() -> Self {
        Self {
            scalars: FxHashMap::default(),
            integers: FxHashMap::default(),
            flags: FxHashSet::default(),
            strings: FxHashMap::default(),
        }
    }
}

impl Signals {
    /// Set a floating-point signal value.
    pub fn set_scalar(&mut self, key: impl Into<String>, value: f32) {
        self.scalars.insert(key.into(), value);
    }
    /// Get a floating-point signal by key.
    pub fn get_scalar(&self, key: &str) -> Option<f32> {
        self.scalars.get(key).copied()
    }
    /// Read-only view of all scalar signals.
    pub fn get_scalars(&self) -> &FxHashMap<String, f32> {
        &self.scalars
    }
    /// Set an integer signal value.
    #[allow(dead_code)]
    pub fn set_integer(&mut self, key: impl Into<String>, value: i32) {
        self.integers.insert(key.into(), value);
    }
    /// Get an integer signal by key.
    pub fn get_integer(&self, key: &str) -> Option<i32> {
        self.integers.get(key).copied()
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
    /// Remove a flag (make it false/absent).
    pub fn clear_flag(&mut self, key: &str) {
        self.flags.remove(key);
    }
    /// Check whether a flag is present/true.
    pub fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_all_empty() {
        let s = Signals::default();
        assert!(s.scalars.is_empty());
        assert!(s.integers.is_empty());
        assert!(s.flags.is_empty());
        assert!(s.strings.is_empty());
    }

    #[test]
    fn test_set_and_get_scalar() {
        let mut s = Signals::default();
        s.set_scalar("hp", 100.0);
        assert_eq!(s.get_scalar("hp"), Some(100.0));
    }

    #[test]
    fn test_scalar_overwrite() {
        let mut s = Signals::default();
        s.set_scalar("hp", 100.0);
        s.set_scalar("hp", 50.0);
        assert_eq!(s.get_scalar("hp"), Some(50.0));
    }

    #[test]
    fn test_scalar_missing_returns_none() {
        let s = Signals::default();
        assert_eq!(s.get_scalar("nonexistent"), None);
    }

    #[test]
    fn test_set_and_get_integer() {
        let mut s = Signals::default();
        s.set_integer("coins", 5);
        assert_eq!(s.get_integer("coins"), Some(5));
    }

    #[test]
    fn test_integer_overwrite() {
        let mut s = Signals::default();
        s.set_integer("coins", 5);
        s.set_integer("coins", 10);
        assert_eq!(s.get_integer("coins"), Some(10));
    }

    #[test]
    fn test_integer_missing_returns_none() {
        let s = Signals::default();
        assert_eq!(s.get_integer("nonexistent"), None);
    }

    #[test]
    fn test_set_and_has_flag() {
        let mut s = Signals::default();
        s.set_flag("is_running");
        assert!(s.has_flag("is_running"));
    }

    #[test]
    fn test_clear_flag() {
        let mut s = Signals::default();
        s.set_flag("is_running");
        s.clear_flag("is_running");
        assert!(!s.has_flag("is_running"));
    }

    #[test]
    fn test_clear_nonexistent_flag_is_noop() {
        let mut s = Signals::default();
        s.clear_flag("nonexistent");
        assert!(!s.has_flag("nonexistent"));
    }

    #[test]
    fn test_set_and_get_string() {
        let mut s = Signals::default();
        s.set_string("name", "player");
        assert_eq!(s.get_string("name").map(|s| s.as_str()), Some("player"));
    }

    #[test]
    fn test_string_overwrite() {
        let mut s = Signals::default();
        s.set_string("name", "player");
        s.set_string("name", "enemy");
        assert_eq!(s.get_string("name").map(|s| s.as_str()), Some("enemy"));
    }

    #[test]
    fn test_string_missing_returns_none() {
        let s = Signals::default();
        assert_eq!(s.get_string("nonexistent"), None);
    }

    #[test]
    fn test_with_flag_builder() {
        let s = Signals::default().with_flag("active");
        assert!(s.has_flag("active"));
    }

    #[test]
    fn test_get_scalars_view() {
        let mut s = Signals::default();
        s.set_scalar("a", 1.0);
        s.set_scalar("b", 2.0);
        assert_eq!(s.get_scalars().len(), 2);
    }

    #[test]
    fn test_get_integers_view() {
        let mut s = Signals::default();
        s.set_integer("a", 1);
        assert_eq!(s.get_integers().len(), 1);
    }

    #[test]
    fn test_get_flags_view() {
        let mut s = Signals::default();
        s.set_flag("x");
        s.set_flag("y");
        assert_eq!(s.get_flags().len(), 2);
    }

    #[test]
    fn test_multiple_signal_types_coexist() {
        let mut s = Signals::default();
        s.set_scalar("hp", 100.0);
        s.set_integer("coins", 5);
        s.set_flag("active");
        s.set_string("name", "test");
        assert_eq!(s.get_scalar("hp"), Some(100.0));
        assert_eq!(s.get_integer("coins"), Some(5));
        assert!(s.has_flag("active"));
        assert_eq!(s.get_string("name").map(|s| s.as_str()), Some("test"));
    }
}
