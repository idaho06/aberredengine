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

use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

/// Immutable snapshot of world signals for Lua read access.
///
/// This struct is wrapped in `Arc` for cheap sharing with the Lua runtime.
/// Instead of cloning all signal maps on every Lua callback, we create
/// a snapshot once when signals change and share it via Arc.
#[derive(Debug, Clone, Default)]
pub struct SignalSnapshot {
    /// Floating-point numeric signals.
    pub scalars: FxHashMap<String, f32>,
    /// Integer numeric signals.
    pub integers: FxHashMap<String, i32>,
    /// String signals.
    pub strings: FxHashMap<String, String>,
    /// Presence-only boolean flags.
    pub flags: FxHashSet<String>,
    /// Group entity counts (derived from integers with "group_count:" prefix).
    pub group_counts: FxHashMap<String, u32>,
    /// Entity IDs as u64 (from Entity::to_bits()).
    pub entities: FxHashMap<String, u64>,
}

/// Global signal storage for cross-system communication.
///
/// Provides maps for scalars, integers, strings, and flags accessible from
/// any system without entity queries.
///
/// # Snapshot System
///
/// For efficient sharing with the Lua runtime, `WorldSignals` maintains a
/// cached [`SignalSnapshot`] wrapped in `Arc`. When signals are modified,
/// the `dirty` flag is set. Call [`snapshot()`](Self::snapshot) to get an
/// up-to-date Arc that can be cheaply cloned and shared with Lua callbacks.
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
    /// Cached snapshot for Lua runtime (rebuilt when dirty).
    snapshot: Arc<SignalSnapshot>,
    /// Whether the snapshot needs to be rebuilt.
    dirty: bool,
}

impl Default for WorldSignals {
    fn default() -> Self {
        Self {
            scalars: FxHashMap::default(),
            integers: FxHashMap::default(),
            strings: FxHashMap::default(),
            flags: FxHashSet::default(),
            entities: FxHashMap::default(),
            snapshot: Arc::new(SignalSnapshot::default()),
            dirty: false,
        }
    }
}
impl WorldSignals {
    /// Set a floating-point signal value.
    pub fn set_scalar(&mut self, key: impl Into<String>, value: f32) {
        self.scalars.insert(key.into(), value);
        self.mark_dirty();
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
        self.integers.insert(key.into(), value);
        self.mark_dirty();
    }
    /// Get an integer signal by key.
    pub fn get_integer(&self, key: &str) -> Option<i32> {
        self.integers.get(key).copied()
    }
    /// Get a group count by the name of the group.
    pub fn get_group_count(&self, group_name: &str) -> Option<i32> {
        let key = format!("group_count:{}", group_name);
        self.get_integer(&key)
    }
    /// Remove all integer signals whose keys start with a given prefix.
    pub fn clear_integer_prefix(&mut self, prefix: &str) {
        let keys_to_remove: Vec<String> = self
            .integers
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        if !keys_to_remove.is_empty() {
            for key in keys_to_remove {
                self.integers.remove(&key);
            }
            self.mark_dirty();
        }
    }
    /// Remove integer signals for group counting.
    pub fn clear_group_counts(&mut self) {
        self.clear_integer_prefix("group_count:");
    }
    /// Set a string signal value.
    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.strings.insert(key.into(), value.into());
        self.mark_dirty();
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
            self.mark_dirty();
        }
        result
    }
    /// Read-only view of all integer signals.
    pub fn get_integers(&self) -> &FxHashMap<String, i32> {
        &self.integers
    }
    /// Mark a flag as present/true.
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.flags.insert(key.into());
        self.mark_dirty();
    }
    /// Remove a flag (make it false/absent).
    pub fn clear_flag(&mut self, key: &str) {
        if self.flags.remove(key) {
            self.mark_dirty();
        }
    }
    /// Check whether a flag is present/true.
    pub fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
    /// Read-only view of all flags.
    pub fn get_flags(&self) -> &FxHashSet<String> {
        &self.flags
    }
    /// Get an entity by key.
    pub fn get_entity(&self, key: &str) -> Option<&Entity> {
        self.entities.get(key)
    }
    /// Set an entity by key.
    pub fn set_entity(&mut self, key: impl Into<String>, entity: Entity) {
        self.entities.insert(key.into(), entity);
        self.mark_dirty();
    }
    /// Remove an entity by key. Returns the removed entity if it existed.
    pub fn remove_entity(&mut self, key: &str) -> Option<Entity> {
        let result = self.entities.remove(key);
        if result.is_some() {
            self.mark_dirty();
        }
        result
    }

    /// Read-only view of all scalar signals (for caching).
    pub fn scalars(&self) -> &FxHashMap<String, f32> {
        &self.scalars
    }

    /// Read-only view of all integer signals (for caching).
    pub fn integers(&self) -> &FxHashMap<String, i32> {
        &self.integers
    }

    /// Read-only view of all string signals (for caching).
    pub fn strings(&self) -> &FxHashMap<String, String> {
        &self.strings
    }

    /// Read-only view of all flags (for caching).
    pub fn flags(&self) -> &FxHashSet<String> {
        &self.flags
    }

    /// Get a map of group counts (for caching).
    /// Returns a map from group name to count.
    pub fn group_counts(&self) -> FxHashMap<String, u32> {
        self.integers
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix("group_count:")
                    .map(|group| (group.to_string(), *v as u32))
            })
            .collect()
    }

    /// Mark signals as modified, requiring a snapshot rebuild.
    ///
    /// Called internally by all mutation methods.
    #[inline]
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Get or create an up-to-date snapshot for sharing with Lua.
    ///
    /// If signals have been modified since the last snapshot, this rebuilds
    /// the snapshot by cloning all signal maps. Otherwise, it returns
    /// a cheap `Arc::clone` of the existing snapshot.
    ///
    /// # Performance
    ///
    /// - If clean: O(1) - just increments Arc refcount
    /// - If dirty: O(n) - clones all maps once
    ///
    /// This amortizes the cost of cloning across multiple Lua callbacks
    /// that run between signal modifications.
    pub fn snapshot(&mut self) -> Arc<SignalSnapshot> {
        if self.dirty {
            self.snapshot = Arc::new(SignalSnapshot {
                scalars: self.scalars.clone(),
                integers: self.integers.clone(),
                strings: self.strings.clone(),
                flags: self.flags.clone(),
                group_counts: self.group_counts(),
                entities: self
                    .entities
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_bits()))
                    .collect(),
            });
            self.dirty = false;
        }
        Arc::clone(&self.snapshot)
    }
}
