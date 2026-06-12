//! Global signal storage resource.
//!
//! The [`WorldSignals`] resource provides a world-wide signal map for
//! cross-system communication. Unlike per-entity
//! [`Signals`](crate::components::signals::Signals), these signals are
//! global and accessible from any system.
//!
//! # Use Cases
//!
//! - Storing the current scene name (`"scene"`)
//! - Global flags like `"game_paused"` or `"switch_scene"`
//! - Game state values (score, lives, high score)
//! - Entity references for quick lookup (`"player"`, `"ball"`)
//! - Group entity counts (published by [`update_group_counts_system`](crate::systems::group::update_group_counts_system))
//!
//! # Integration with Other Systems
//!
//! - [`Phase`](crate::components::phase::Phase) callbacks receive `WorldSignals` via [`PhaseContext`](crate::components::phase::PhaseContext)
//! - [`CollisionRule`](crate::components::collision::CollisionRule) callbacks access it via [`CollisionContext`](crate::components::collision::CollisionContext)
//! - [`SignalBinding`](crate::components::signalbinding::SignalBinding) binds UI text to world signal values
//! - [`TrackedGroups`](crate::resources::group::TrackedGroups) + group system publish entity counts here
//!
//! # Example
//!
//! ```ignore
//! // In game setup
//! world_signals.set_string("scene", "menu");
//! world_signals.set_integer("score", 0);
//! world_signals.set_integer("lives", 3);
//!
//! // In phase callback
//! if let Some(0) = ctx.world_signals.get_group_count("ball") {
//!     return Some("lose_life".into());
//! }
//! ```

use crate::resources::signal_keys as sk;
use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

/// Immutable snapshot of world signals for Lua read access.
///
/// This struct is wrapped in `Arc` for cheap sharing with the Lua runtime.
/// Instead of cloning all signal maps on every Lua callback, we create
/// a snapshot once when signals change and share it via Arc.
///
/// Each domain field is itself an `Arc`, so rebuilding the outer snapshot
/// after a partial update (e.g. only integers changed) costs only Arc
/// refcount bumps for the unchanged domains.
#[derive(Debug, Clone, Default)]
pub struct SignalSnapshot {
    /// Floating-point numeric signals.
    pub scalars: Arc<FxHashMap<String, f32>>,
    /// Integer numeric signals.
    pub integers: Arc<FxHashMap<String, i32>>,
    /// String signals.
    pub strings: Arc<FxHashMap<String, String>>,
    /// Presence-only boolean flags.
    pub flags: Arc<FxHashSet<String>>,
    /// Group entity counts (derived from integers with "group_count:" prefix).
    pub group_counts: Arc<FxHashMap<String, u32>>,
    /// Entity IDs as u64 (from Entity::to_bits()).
    pub entities: Arc<FxHashMap<String, u64>>,
}

/// Global signal storage for cross-system communication.
///
/// Provides maps for scalars, integers, strings, and flags accessible from
/// any system without entity queries.
///
/// # Snapshot System
///
/// For efficient sharing with the Lua runtime, `WorldSignals` maintains a
/// cached [`SignalSnapshot`] wrapped in `Arc`. Per-domain dirty flags track
/// which signal domains have changed. Call [`snapshot()`](Self::snapshot) to
/// get an up-to-date Arc; only dirty domains are re-cloned each call.
#[derive(Debug, Clone, Resource)]
pub struct WorldSignals {
    /// Floating-point numeric signals addressed by string keys.
    pub scalars: FxHashMap<String, f32>,
    /// Integer numeric signals addressed by string keys.
    pub integers: FxHashMap<String, i32>,
    /// String signals addressed by string keys.
    pub strings: FxHashMap<String, String>,
    /// Presence-only boolean flags; a key being present means "true".
    pub flags: FxHashSet<String>,
    /// Map of entities of interest for the current game state.
    pub entities: FxHashMap<String, Entity>,
    /// Group counts maintained in parallel with the `"group_count:"` integer entries.
    group_counts: FxHashMap<String, u32>,

    /// Per-domain cached Arcs for the snapshot.
    scalars_arc: Arc<FxHashMap<String, f32>>,
    integers_arc: Arc<FxHashMap<String, i32>>,
    strings_arc: Arc<FxHashMap<String, String>>,
    flags_arc: Arc<FxHashSet<String>>,
    group_counts_arc: Arc<FxHashMap<String, u32>>,
    entities_arc: Arc<FxHashMap<String, u64>>,

    /// Per-domain dirty bits; set when the corresponding live map changes.
    scalars_dirty: bool,
    integers_dirty: bool,
    strings_dirty: bool,
    flags_dirty: bool,
    group_counts_dirty: bool,
    entities_dirty: bool,

    /// Assembled snapshot (rebuilt when any domain is dirty).
    snapshot: Arc<SignalSnapshot>,
}

impl Default for WorldSignals {
    fn default() -> Self {
        Self {
            scalars: FxHashMap::default(),
            integers: FxHashMap::default(),
            strings: FxHashMap::default(),
            flags: FxHashSet::default(),
            entities: FxHashMap::default(),
            group_counts: FxHashMap::default(),

            scalars_arc: Arc::new(FxHashMap::default()),
            integers_arc: Arc::new(FxHashMap::default()),
            strings_arc: Arc::new(FxHashMap::default()),
            flags_arc: Arc::new(FxHashSet::default()),
            group_counts_arc: Arc::new(FxHashMap::default()),
            entities_arc: Arc::new(FxHashMap::default()),

            scalars_dirty: false,
            integers_dirty: false,
            strings_dirty: false,
            flags_dirty: false,
            group_counts_dirty: false,
            entities_dirty: false,

            snapshot: Arc::new(SignalSnapshot::default()),
        }
    }
}
impl WorldSignals {
    /// Set a floating-point signal value.
    pub fn set_scalar(&mut self, key: impl Into<String>, value: f32) {
        self.scalars.insert(key.into(), value);
        self.scalars_dirty = true;
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
    pub fn set_integer(&mut self, key: impl Into<String>, value: i32) {
        let key = key.into();
        if let Some(group_name) = key.strip_prefix(sk::GROUP_COUNT_PREFIX) {
            self.group_counts
                .insert(group_name.to_string(), value as u32);
            self.group_counts_dirty = true;
        }
        self.integers.insert(key, value);
        self.integers_dirty = true;
    }
    /// Get an integer signal by key.
    pub fn get_integer(&self, key: &str) -> Option<i32> {
        self.integers.get(key).copied()
    }
    /// Read-only view of all integer signals.
    pub fn get_integers(&self) -> &FxHashMap<String, i32> {
        &self.integers
    }
    /// Get a group count by group name. Returns `None` if not tracked.
    pub fn get_group_count(&self, group_name: &str) -> Option<i32> {
        use std::fmt::Write;
        let mut buf = arrayvec::ArrayString::<64>::new();
        let _ = write!(buf, "{}{}", sk::GROUP_COUNT_PREFIX, group_name);
        self.integers.get(buf.as_str()).copied()
    }

    /// Set a group count by the name of the group.
    ///
    /// Only updates if the value changed to avoid unnecessary dirty marking.
    /// Uses a stack buffer to avoid heap allocation. Group names must not
    /// exceed 51 characters (64 - 13 for "group_count:" prefix).
    pub fn set_group_count(&mut self, group_name: &str, count: i32) {
        use std::fmt::Write;
        let mut buf = arrayvec::ArrayString::<64>::new();
        let _ = write!(buf, "{}{}", sk::GROUP_COUNT_PREFIX, group_name);
        let current = self.integers.get(buf.as_str()).copied();
        if current != Some(count) {
            self.integers.insert(buf.to_string(), count);
            self.group_counts
                .insert(group_name.to_string(), count as u32);
            self.integers_dirty = true;
            self.group_counts_dirty = true;
        }
    }
    /// Remove all integer signals whose keys start with a given prefix.
    pub fn clear_integer_prefix(&mut self, prefix: &str) {
        // Collect group names to remove before mutating integers (borrow-checker split).
        // Only the suffix strings are collected, not the full keys.
        let group_names: Vec<String> = self
            .integers
            .keys()
            .filter(|k| k.starts_with(prefix))
            .filter_map(|k| k.strip_prefix(sk::GROUP_COUNT_PREFIX).map(str::to_string))
            .collect();
        let before = self.integers.len();
        self.integers.retain(|k, _| !k.starts_with(prefix));
        if self.integers.len() != before {
            self.integers_dirty = true;
            for name in &group_names {
                self.group_counts.remove(name.as_str());
            }
            if !group_names.is_empty() {
                self.group_counts_dirty = true;
            }
        }
    }
    /// Remove integer signals for group counting.
    pub fn clear_group_counts(&mut self) {
        self.clear_integer_prefix(sk::GROUP_COUNT_PREFIX);
    }
    /// Set a string signal value.
    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.strings.insert(key.into(), value.into());
        self.strings_dirty = true;
    }
    /// Get a string signal by key.
    /// It's recommended to clone the String if you need ownership.
    pub fn get_string(&self, key: &str) -> Option<&String> {
        self.strings.get(key)
    }
    /// Remove a string signal by key.
    pub fn remove_string(&mut self, key: &str) -> Option<String> {
        let result = self.strings.remove(key);
        if result.is_some() {
            self.strings_dirty = true;
        }
        result
    }
    /// Remove a scalar signal by key.
    pub fn clear_scalar(&mut self, key: &str) -> Option<f32> {
        let result = self.scalars.remove(key);
        if result.is_some() {
            self.scalars_dirty = true;
        }
        result
    }
    /// Remove an integer signal by key.
    pub fn clear_integer(&mut self, key: &str) -> Option<i32> {
        let result = self.integers.remove(key);
        if result.is_some() {
            self.integers_dirty = true;
            if let Some(group_name) = key.strip_prefix(sk::GROUP_COUNT_PREFIX) {
                self.group_counts.remove(group_name);
                self.group_counts_dirty = true;
            }
        }
        result
    }
    /// Mark a flag as present/true.
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.flags.insert(key.into());
        self.flags_dirty = true;
    }
    /// Remove a flag (make it false/absent).
    pub fn clear_flag(&mut self, key: &str) {
        if self.flags.remove(key) {
            self.flags_dirty = true;
        }
    }
    /// Check whether a flag is present/true.
    pub fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
    /// Remove a flag and return whether it was present.
    ///
    /// Equivalent to `has_flag` + `clear_flag` in a single hash-set lookup.
    pub fn take_flag(&mut self, key: &str) -> bool {
        if self.flags.remove(key) {
            self.flags_dirty = true;
            true
        } else {
            false
        }
    }
    /// Toggle a flag: remove it if present, add it if absent.
    ///
    /// Always marks the snapshot dirty since the state always changes.
    pub fn toggle_flag(&mut self, key: &str) {
        if !self.flags.remove(key) {
            self.flags.insert(key.to_string());
        }
        self.flags_dirty = true;
    }
    /// Read-only view of all flags.
    pub fn get_flags(&self) -> &FxHashSet<String> {
        &self.flags
    }
    /// Read-only view of all string signals.
    pub fn get_strings(&self) -> &FxHashMap<String, String> {
        &self.strings
    }
    /// Get an entity by key.
    pub fn get_entity(&self, key: &str) -> Option<&Entity> {
        self.entities.get(key)
    }
    /// Set an entity by key.
    pub fn set_entity(&mut self, key: impl Into<String>, entity: Entity) {
        self.entities.insert(key.into(), entity);
        self.entities_dirty = true;
    }
    /// Remove an entity by key. Returns the removed entity if it existed.
    pub fn remove_entity(&mut self, key: &str) -> Option<Entity> {
        let result = self.entities.remove(key);
        if result.is_some() {
            self.entities_dirty = true;
        }
        result
    }

    /// Remove all entity registrations pointing at `entity`.
    ///
    /// Called when an entity is despawned so stale registry entries (used by
    /// `engine.clone`) don't outlive the entity they reference.
    pub fn remove_entity_registrations_for(&mut self, entity: Entity) {
        let before = self.entities.len();
        self.entities.retain(|_, e| *e != entity);
        if self.entities.len() != before {
            self.entities_dirty = true;
        }
    }

    /// Remove all entity registrations whose [`Entity`] is not in `persistent_entities`.
    ///
    /// Called during scene transitions to mirror the entity despawn logic:
    /// non-persistent entities are despawned, so their registrations must be cleared too.
    /// Registrations for persistent entities are preserved unchanged.
    pub fn clear_non_persistent_entities(&mut self, persistent_entities: &FxHashSet<Entity>) {
        let before = self.entities.len();
        self.entities
            .retain(|_, entity| persistent_entities.contains(entity));
        if self.entities.len() != before {
            self.entities_dirty = true;
        }
    }

    /// Get a map of group counts (for caching).
    /// Returns a map from group name to count.
    pub fn group_counts(&self) -> FxHashMap<String, u32> {
        self.group_counts.clone()
    }

    /// Returns true if any signal domain has been modified since the last snapshot.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.scalars_dirty
            || self.integers_dirty
            || self.strings_dirty
            || self.flags_dirty
            || self.group_counts_dirty
            || self.entities_dirty
    }

    /// Get or create an up-to-date snapshot for sharing with Lua.
    ///
    /// Only dirty signal domains are re-cloned; clean domains reuse their
    /// cached `Arc`. The outer `SignalSnapshot` is rebuilt whenever any
    /// domain changed (cost: six `Arc::clone` calls).
    ///
    /// # Performance
    ///
    /// - If clean: O(1) — one `Arc::clone`
    /// - If dirty: O(n) for each modified domain only; unchanged domains are O(1)
    pub fn snapshot(&mut self) -> Arc<SignalSnapshot> {
        let mut any_dirty = false;

        if self.scalars_dirty {
            self.scalars_arc = Arc::new(self.scalars.clone());
            self.scalars_dirty = false;
            any_dirty = true;
        }
        if self.integers_dirty {
            self.integers_arc = Arc::new(self.integers.clone());
            self.integers_dirty = false;
            any_dirty = true;
        }
        if self.group_counts_dirty {
            self.group_counts_arc = Arc::new(self.group_counts.clone());
            self.group_counts_dirty = false;
            any_dirty = true;
        }
        if self.strings_dirty {
            self.strings_arc = Arc::new(self.strings.clone());
            self.strings_dirty = false;
            any_dirty = true;
        }
        if self.flags_dirty {
            self.flags_arc = Arc::new(self.flags.clone());
            self.flags_dirty = false;
            any_dirty = true;
        }
        if self.entities_dirty {
            self.entities_arc = Arc::new(
                self.entities
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_bits()))
                    .collect(),
            );
            self.entities_dirty = false;
            any_dirty = true;
        }

        if any_dirty {
            self.snapshot = Arc::new(SignalSnapshot {
                scalars: Arc::clone(&self.scalars_arc),
                integers: Arc::clone(&self.integers_arc),
                strings: Arc::clone(&self.strings_arc),
                flags: Arc::clone(&self.flags_arc),
                group_counts: Arc::clone(&self.group_counts_arc),
                entities: Arc::clone(&self.entities_arc),
            });
        }
        Arc::clone(&self.snapshot)
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
    fn test_default_all_empty() {
        let ws = WorldSignals::default();
        assert!(ws.scalars.is_empty());
        assert!(ws.integers.is_empty());
        assert!(ws.strings.is_empty());
        assert!(ws.flags.is_empty());
        assert!(ws.entities.is_empty());
        assert!(!ws.is_dirty());
    }

    // --- Scalars ---

    #[test]
    fn test_set_and_get_scalar() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("speed", 42.0);
        assert_eq!(ws.get_scalar("speed"), Some(42.0));
    }

    #[test]
    fn test_get_scalars_view() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("speed", 42.0);
        assert_eq!(ws.get_scalars().len(), 1);
    }

    #[test]
    fn test_scalar_missing_returns_none() {
        let ws = WorldSignals::default();
        assert_eq!(ws.get_scalar("nope"), None);
    }

    #[test]
    fn test_clear_scalar() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("x", 1.0);
        let removed = ws.clear_scalar("x");
        assert!(approx_eq(removed.unwrap(), 1.0));
        assert_eq!(ws.get_scalar("x"), None);
    }

    #[test]
    fn test_clear_scalar_nonexistent() {
        let mut ws = WorldSignals::default();
        assert_eq!(ws.clear_scalar("nope"), None);
    }

    // --- Integers ---

    #[test]
    fn test_set_and_get_integer() {
        let mut ws = WorldSignals::default();
        ws.set_integer("score", 100);
        assert_eq!(ws.get_integer("score"), Some(100));
    }

    #[test]
    fn test_get_integers_view() {
        let mut ws = WorldSignals::default();
        ws.set_integer("score", 100);
        assert_eq!(ws.get_integers().len(), 1);
    }

    #[test]
    fn test_integer_missing_returns_none() {
        let ws = WorldSignals::default();
        assert_eq!(ws.get_integer("nope"), None);
    }

    #[test]
    fn test_clear_integer() {
        let mut ws = WorldSignals::default();
        ws.set_integer("lives", 3);
        let removed = ws.clear_integer("lives");
        assert_eq!(removed, Some(3));
        assert_eq!(ws.get_integer("lives"), None);
    }

    // --- Strings ---

    #[test]
    fn test_set_and_get_string() {
        let mut ws = WorldSignals::default();
        ws.set_string("scene", "menu");
        assert_eq!(ws.get_string("scene").map(|s| s.as_str()), Some("menu"));
    }

    #[test]
    fn test_get_strings_view() {
        let mut ws = WorldSignals::default();
        ws.set_string("scene", "menu");
        assert_eq!(ws.get_strings().len(), 1);
    }

    #[test]
    fn test_string_missing_returns_none() {
        let ws = WorldSignals::default();
        assert_eq!(ws.get_string("nope"), None);
    }

    #[test]
    fn test_remove_string() {
        let mut ws = WorldSignals::default();
        ws.set_string("scene", "menu");
        let removed = ws.remove_string("scene");
        assert_eq!(removed.as_deref(), Some("menu"));
        assert_eq!(ws.get_string("scene"), None);
    }

    #[test]
    fn test_remove_string_nonexistent() {
        let mut ws = WorldSignals::default();
        assert_eq!(ws.remove_string("nope"), None);
    }

    // --- Flags ---

    #[test]
    fn test_set_and_has_flag() {
        let mut ws = WorldSignals::default();
        ws.set_flag("paused");
        assert!(ws.has_flag("paused"));
    }

    #[test]
    fn test_get_flags_view() {
        let mut ws = WorldSignals::default();
        ws.set_flag("paused");
        assert_eq!(ws.get_flags().len(), 1);
    }

    #[test]
    fn test_clear_flag() {
        let mut ws = WorldSignals::default();
        ws.set_flag("paused");
        ws.clear_flag("paused");
        assert!(!ws.has_flag("paused"));
    }

    #[test]
    fn test_clear_flag_nonexistent() {
        let mut ws = WorldSignals::default();
        ws.clear_flag("nope"); // should not panic
        assert!(!ws.has_flag("nope"));
    }

    #[test]
    fn test_take_flag_present() {
        let mut ws = WorldSignals::default();
        ws.set_flag("fire");
        assert!(ws.take_flag("fire"));
        assert!(!ws.has_flag("fire"));
    }

    #[test]
    fn test_take_flag_absent() {
        let mut ws = WorldSignals::default();
        assert!(!ws.take_flag("nope"));
    }

    #[test]
    fn test_take_flag_marks_dirty_only_when_present() {
        let mut ws = WorldSignals::default();
        ws.set_flag("fire");
        ws.snapshot(); // clear dirty
        ws.take_flag("nope"); // absent — should not dirty
        assert!(!ws.flags_dirty);
        ws.take_flag("fire"); // present — should dirty
        assert!(ws.flags_dirty);
    }

    #[test]
    fn test_toggle_flag_absent_sets_it() {
        let mut ws = WorldSignals::default();
        ws.toggle_flag("x");
        assert!(ws.has_flag("x"));
    }

    #[test]
    fn test_toggle_flag_present_clears_it() {
        let mut ws = WorldSignals::default();
        ws.set_flag("x");
        ws.toggle_flag("x");
        assert!(!ws.has_flag("x"));
    }

    #[test]
    fn test_toggle_flag_twice_restores() {
        let mut ws = WorldSignals::default();
        ws.toggle_flag("x");
        ws.toggle_flag("x");
        assert!(!ws.has_flag("x"));
    }

    #[test]
    fn test_toggle_flag_marks_dirty() {
        let mut ws = WorldSignals::default();
        ws.set_flag("x");
        ws.snapshot(); // clear dirty
        ws.toggle_flag("x"); // present → remove
        assert!(ws.flags_dirty);
        ws.snapshot(); // clear dirty
        ws.toggle_flag("x"); // absent → insert
        assert!(ws.flags_dirty);
    }

    // --- Entities ---

    #[test]
    fn test_set_and_get_entity() {
        let mut ws = WorldSignals::default();
        let entity = Entity::from_bits(42);
        ws.set_entity("player", entity);
        assert_eq!(ws.get_entity("player"), Some(&entity));
    }

    #[test]
    fn test_remove_entity() {
        let mut ws = WorldSignals::default();
        let entity = Entity::from_bits(42);
        ws.set_entity("player", entity);
        let removed = ws.remove_entity("player");
        assert_eq!(removed, Some(entity));
        assert_eq!(ws.get_entity("player"), None);
    }

    #[test]
    fn test_remove_entity_nonexistent() {
        let mut ws = WorldSignals::default();
        assert_eq!(ws.remove_entity("nope"), None);
    }

    #[test]
    fn test_remove_entity_registrations_for() {
        let mut ws = WorldSignals::default();
        let entity_a = Entity::from_bits(1);
        let entity_b = Entity::from_bits(2);
        ws.set_entity("player", entity_a);
        ws.set_entity("cursor", entity_b);
        ws.entities_dirty = false;

        ws.remove_entity_registrations_for(entity_a);

        assert!(
            ws.get_entity("player").is_none(),
            "registration pointing at the removed entity should be gone"
        );
        assert_eq!(
            ws.get_entity("cursor"),
            Some(&entity_b),
            "registration pointing at a different entity should be kept"
        );
        assert!(ws.entities_dirty);
    }

    #[test]
    fn test_remove_entity_registrations_for_no_match() {
        let mut ws = WorldSignals::default();
        let entity_a = Entity::from_bits(1);
        let entity_b = Entity::from_bits(2);
        ws.set_entity("cursor", entity_b);
        ws.entities_dirty = false;

        ws.remove_entity_registrations_for(entity_a);

        assert_eq!(ws.get_entity("cursor"), Some(&entity_b));
        assert!(!ws.entities_dirty);
    }

    #[test]
    fn test_clear_non_persistent_entities_removes_non_persistent() {
        let mut ws = WorldSignals::default();
        let entity_a = Entity::from_bits(1);
        let entity_b = Entity::from_bits(2);
        ws.set_entity("player", entity_a);
        ws.set_entity("cursor", entity_b);

        // Only entity_b is "persistent"
        let persistent = FxHashSet::from_iter([entity_b]);
        ws.clear_non_persistent_entities(&persistent);

        assert!(
            ws.get_entity("player").is_none(),
            "non-persistent registration should be removed"
        );
        assert_eq!(
            ws.get_entity("cursor"),
            Some(&entity_b),
            "persistent registration should be kept"
        );
    }

    #[test]
    fn test_clear_non_persistent_entities_empty_set_clears_all() {
        let mut ws = WorldSignals::default();
        ws.set_entity("player", Entity::from_bits(1));
        ws.set_entity("cursor", Entity::from_bits(2));

        ws.clear_non_persistent_entities(&FxHashSet::default());

        assert!(ws.get_entity("player").is_none());
        assert!(ws.get_entity("cursor").is_none());
    }

    #[test]
    fn test_clear_non_persistent_entities_all_persistent_keeps_all() {
        let mut ws = WorldSignals::default();
        let entity_a = Entity::from_bits(1);
        let entity_b = Entity::from_bits(2);
        ws.set_entity("player", entity_a);
        ws.set_entity("cursor", entity_b);

        let persistent = FxHashSet::from_iter([entity_a, entity_b]);
        ws.clear_non_persistent_entities(&persistent);

        assert_eq!(ws.get_entity("player"), Some(&entity_a));
        assert_eq!(ws.get_entity("cursor"), Some(&entity_b));
    }

    #[test]
    fn test_clear_non_persistent_entities_marks_dirty_only_when_changed() {
        let mut ws = WorldSignals::default();
        let entity_a = Entity::from_bits(1);
        ws.set_entity("player", entity_a);
        ws.snapshot(); // clear dirty flag
        assert!(!ws.entities_dirty);

        // Nothing removed — should stay clean
        let persistent = FxHashSet::from_iter([entity_a]);
        ws.clear_non_persistent_entities(&persistent);
        assert!(
            !ws.entities_dirty,
            "should not mark dirty when nothing was removed"
        );

        // Remove entity — should mark dirty
        ws.clear_non_persistent_entities(&FxHashSet::default());
        assert!(
            ws.entities_dirty,
            "should mark dirty when an entry was removed"
        );
    }

    // --- Group counts ---

    #[test]
    fn test_set_and_get_group_count() {
        let mut ws = WorldSignals::default();
        ws.set_group_count("enemy", 5);
        assert_eq!(ws.get_group_count("enemy"), Some(5));
    }

    #[test]
    fn test_set_group_count_noop_same_value() {
        let mut ws = WorldSignals::default();
        ws.set_group_count("enemy", 5);
        // Clear dirty flag via snapshot
        ws.snapshot();
        ws.set_group_count("enemy", 5); // same value, should not mark dirty
        assert!(!ws.is_dirty());
    }

    #[test]
    fn test_set_group_count_marks_dirty_on_change() {
        let mut ws = WorldSignals::default();
        ws.set_group_count("enemy", 5);
        ws.snapshot();
        ws.set_group_count("enemy", 6);
        assert!(ws.is_dirty());
    }

    #[test]
    fn test_clear_group_counts() {
        let mut ws = WorldSignals::default();
        ws.set_group_count("enemy", 5);
        ws.set_group_count("bullet", 10);
        ws.set_integer("score", 100); // non-group integer
        ws.clear_group_counts();
        assert_eq!(ws.get_group_count("enemy"), None);
        assert_eq!(ws.get_group_count("bullet"), None);
        assert_eq!(ws.get_integer("score"), Some(100)); // preserved
    }

    #[test]
    fn test_clear_integer_prefix() {
        let mut ws = WorldSignals::default();
        ws.set_integer("prefix_a", 1);
        ws.set_integer("prefix_b", 2);
        ws.set_integer("other", 3);
        ws.clear_integer_prefix("prefix_");
        assert_eq!(ws.get_integer("prefix_a"), None);
        assert_eq!(ws.get_integer("prefix_b"), None);
        assert_eq!(ws.get_integer("other"), Some(3));
    }

    #[test]
    fn test_group_counts_extraction() {
        let mut ws = WorldSignals::default();
        ws.set_group_count("enemy", 5);
        ws.set_group_count("bullet", 10);
        let counts = ws.group_counts();
        assert_eq!(counts.get("enemy"), Some(&5u32));
        assert_eq!(counts.get("bullet"), Some(&10u32));
        assert_eq!(counts.len(), 2);
    }

    // --- Snapshot system ---

    #[test]
    fn test_snapshot_after_mutation() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("x", 1.0);
        ws.set_integer("n", 42);
        ws.set_string("s", "hello");
        ws.set_flag("f");
        let snap = ws.snapshot();
        assert_eq!(snap.scalars.get("x").copied(), Some(1.0));
        assert_eq!(snap.integers.get("n").copied(), Some(42));
        assert_eq!(snap.strings.get("s").map(|s| s.as_str()), Some("hello"));
        assert!(snap.flags.contains("f"));
    }

    #[test]
    fn test_snapshot_without_mutation_returns_same_arc() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("x", 1.0);
        let snap1 = ws.snapshot();
        let snap2 = ws.snapshot();
        assert!(Arc::ptr_eq(&snap1, &snap2));
    }

    #[test]
    fn test_snapshot_rebuilds_after_mutation() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("x", 1.0);
        let snap1 = ws.snapshot();
        ws.set_scalar("x", 2.0);
        let snap2 = ws.snapshot();
        assert!(!Arc::ptr_eq(&snap1, &snap2));
        assert!(approx_eq(snap2.scalars["x"], 2.0));
    }

    #[test]
    fn test_snapshot_group_counts() {
        let mut ws = WorldSignals::default();
        ws.set_group_count("enemy", 3);
        let snap = ws.snapshot();
        assert_eq!(snap.group_counts.get("enemy"), Some(&3u32));
    }

    #[test]
    fn test_snapshot_entities() {
        let mut ws = WorldSignals::default();
        let entity = Entity::from_bits(7);
        ws.set_entity("player", entity);
        let snap = ws.snapshot();
        assert_eq!(snap.entities.get("player"), Some(&entity.to_bits()));
    }

    #[test]
    fn test_snapshot_unchanged_domain_arc_reused() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("x", 1.0);
        ws.set_integer("n", 1);
        let snap1 = ws.snapshot();

        // Only scalars change — integers arc should be pointer-equal
        ws.set_scalar("x", 2.0);
        let snap2 = ws.snapshot();

        assert!(
            Arc::ptr_eq(&snap1.integers, &snap2.integers),
            "integers arc should be reused when only scalars changed"
        );
        assert!(
            !Arc::ptr_eq(&snap1.scalars, &snap2.scalars),
            "scalars arc should be rebuilt"
        );
    }

    #[test]
    fn test_clear_integer_syncs_group_counts() {
        let mut ws = WorldSignals::default();
        ws.set_group_count("enemy", 5);
        ws.clear_integer(&format!("{}enemy", sk::GROUP_COUNT_PREFIX));
        assert_eq!(ws.get_group_count("enemy"), None);
        let snap = ws.snapshot();
        assert_eq!(
            snap.group_counts.get("enemy"),
            None,
            "clear_integer on a group_count key must remove it from the snapshot"
        );
    }

    #[test]
    fn test_set_integer_syncs_group_counts() {
        let mut ws = WorldSignals::default();
        ws.set_integer(format!("{}enemy", sk::GROUP_COUNT_PREFIX), 7);
        let snap = ws.snapshot();
        assert_eq!(
            snap.group_counts.get("enemy"),
            Some(&7u32),
            "set_integer on a group_count key must be visible in the snapshot"
        );
    }
}
