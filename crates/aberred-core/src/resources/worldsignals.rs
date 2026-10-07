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
//! - Systems and observers (e.g. of [`Collided`](crate::events::collision::Collided)) take it as `Res<WorldSignals>`/`ResMut<WorldSignals>`
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

/// Stack buffer size for a `"group_count:{name}"` key (no heap allocation per tick).
const GROUP_KEY_BUF_LEN: usize = 64;

/// Longest group name (in bytes) that fits a group-count key. Longer names are refused by
/// [`TrackedGroups::add_group`](crate::resources::group::TrackedGroups::add_group) and
/// ignored by [`WorldSignals::set_group_count`] / [`WorldSignals::get_group_count`].
pub const MAX_GROUP_NAME_LEN: usize = GROUP_KEY_BUF_LEN - sk::GROUP_COUNT_PREFIX.len();

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

/// Read access shared by the live [`WorldSignals`] resource and its read-only
/// [`SignalSnapshot`] copy, so code that only reads signals works with either.
///
/// Render-side callbacks receive a `&SignalSnapshot`; logic-side systems read
/// `Res<WorldSignals>`. Both answer these calls identically.
pub trait SignalsRead {
    /// Get a scalar signal by key.
    fn get_scalar(&self, key: &str) -> Option<f32>;
    /// Get an integer signal by key.
    fn get_integer(&self, key: &str) -> Option<i32>;
    /// Get a string signal by key.
    fn get_string(&self, key: &str) -> Option<&str>;
    /// Check whether a flag is set.
    fn has_flag(&self, key: &str) -> bool;
    /// Get a registered entity by key.
    fn get_entity(&self, key: &str) -> Option<Entity>;
    /// Get the live entity count of a tracked group.
    fn get_group_count(&self, group_name: &str) -> Option<i32>;
}

impl SignalsRead for SignalSnapshot {
    fn get_scalar(&self, key: &str) -> Option<f32> {
        self.scalars.get(key).copied()
    }
    fn get_integer(&self, key: &str) -> Option<i32> {
        self.integers.get(key).copied()
    }
    fn get_string(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(String::as_str)
    }
    fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
    fn get_entity(&self, key: &str) -> Option<Entity> {
        self.entities.get(key).copied().map(Entity::from_bits)
    }
    fn get_group_count(&self, group_name: &str) -> Option<i32> {
        // The snapshot stores the integer signal `as u32`; `as i32` inverts that
        // exactly, so both sides answer the same for every value.
        self.group_counts.get(group_name).map(|&count| count as i32)
    }
}

impl SignalsRead for WorldSignals {
    fn get_scalar(&self, key: &str) -> Option<f32> {
        WorldSignals::get_scalar(self, key)
    }
    fn get_integer(&self, key: &str) -> Option<i32> {
        WorldSignals::get_integer(self, key)
    }
    fn get_string(&self, key: &str) -> Option<&str> {
        WorldSignals::get_string(self, key)
    }
    fn has_flag(&self, key: &str) -> bool {
        WorldSignals::has_flag(self, key)
    }
    fn get_entity(&self, key: &str) -> Option<Entity> {
        WorldSignals::get_entity(self, key)
    }
    fn get_group_count(&self, group_name: &str) -> Option<i32> {
        WorldSignals::get_group_count(self, group_name)
    }
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
    ///
    /// Only marks the domain dirty if the value actually changed.
    pub fn set_scalar(&mut self, key: impl Into<String>, value: f32) {
        let key = key.into();
        if self.scalars.get(&key) == Some(&value) {
            return;
        }
        self.scalars.insert(key, value);
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
    ///
    /// Only marks the domain (and the group-count side effect) dirty if the
    /// value actually changed.
    pub fn set_integer(&mut self, key: impl Into<String>, value: i32) {
        let key = key.into();
        if self.integers.get(&key) == Some(&value) {
            return;
        }
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
    /// Get a group count by group name. Returns `None` if not tracked, or if the name is
    /// longer than [`MAX_GROUP_NAME_LEN`].
    pub fn get_group_count(&self, group_name: &str) -> Option<i32> {
        use std::fmt::Write;
        if group_name.len() > MAX_GROUP_NAME_LEN {
            return None;
        }
        let mut buf = arrayvec::ArrayString::<GROUP_KEY_BUF_LEN>::new();
        let _ = write!(buf, "{}{}", sk::GROUP_COUNT_PREFIX, group_name);
        self.integers.get(buf.as_str()).copied()
    }

    /// Set a group count by the name of the group.
    ///
    /// Only updates if the value changed to avoid unnecessary dirty marking.
    /// Uses a stack buffer to avoid heap allocation. Names longer than
    /// [`MAX_GROUP_NAME_LEN`] bytes are ignored (a truncated key would make every long
    /// name share one count); `TrackedGroups::add_group` already refused them with a warning.
    pub fn set_group_count(&mut self, group_name: &str, count: i32) {
        use std::fmt::Write;
        if group_name.len() > MAX_GROUP_NAME_LEN {
            return;
        }
        let mut buf = arrayvec::ArrayString::<GROUP_KEY_BUF_LEN>::new();
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
    ///
    /// Only marks the domain dirty if the value actually changed.
    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if self.strings.get(&key) == Some(&value) {
            return;
        }
        self.strings.insert(key, value);
        self.strings_dirty = true;
    }
    /// Get a string signal by key.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(String::as_str)
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
    pub fn remove_scalar(&mut self, key: &str) -> Option<f32> {
        let result = self.scalars.remove(key);
        if result.is_some() {
            self.scalars_dirty = true;
        }
        result
    }
    /// Remove an integer signal by key.
    pub fn remove_integer(&mut self, key: &str) -> Option<i32> {
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
    ///
    /// Only marks the domain dirty if the flag was not already present.
    pub fn set_flag(&mut self, key: impl Into<String>) {
        if self.flags.insert(key.into()) {
            self.flags_dirty = true;
        }
    }
    /// Remove a flag (make it false/absent). Returns whether it was present.
    pub fn remove_flag(&mut self, key: &str) -> bool {
        let removed = self.flags.remove(key);
        if removed {
            self.flags_dirty = true;
        }
        removed
    }
    /// Check whether a flag is present/true.
    pub fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
    /// Consume a one-shot event flag: returns whether it was set, and clears it.
    ///
    /// The same operation as [`remove_flag`](Self::remove_flag), named for
    /// the `if signals.take_flag("event") { ... }` idiom.
    pub fn take_flag(&mut self, key: &str) -> bool {
        self.remove_flag(key)
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

    /// Request a switch to the scene `name`: sets [`sk::SCENE`] and the
    /// [`sk::SWITCH_SCENE`] flag.
    ///
    /// The switch happens when the active scene driver polls the flag
    /// ([`scene_switch_poll`](crate::systems::scene_dispatch::scene_switch_poll)
    /// for `add_scene()` games, the Lua plugin's update for Lua games).
    pub fn request_scene(&mut self, name: impl Into<String>) {
        self.set_string(sk::SCENE, name);
        self.set_flag(sk::SWITCH_SCENE);
    }
    /// Request that the game quit: sets the [`sk::QUIT_GAME`] flag, which the
    /// engine turns into a `Quitting` state transition on the next poll.
    pub fn request_quit(&mut self) {
        self.set_flag(sk::QUIT_GAME);
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
    pub fn get_entity(&self, key: &str) -> Option<Entity> {
        self.entities.get(key).copied()
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
    /// Lua's `EntityCmd::Despawn` calls this immediately, so a later command in
    /// the same drained batch (e.g. `engine.clone` of that key) already sees the
    /// key gone. Every other despawn is covered by
    /// [`remove_dead_entity_registrations`](Self::remove_dead_entity_registrations),
    /// which the engine runs at the end of each sim tick.
    pub fn remove_entity_registrations_for(&mut self, entity: Entity) {
        if self.entities.is_empty() {
            return;
        }
        let before = self.entities.len();
        self.entities.retain(|_, e| *e != entity);
        if self.entities.len() != before {
            self.entities_dirty = true;
        }
    }

    /// Remove every entity registration whose [`Entity`] fails `is_alive`.
    ///
    /// Returns `true` if anything was removed. Backs the engine's per-tick
    /// `prune_dead_entity_registrations` sweep, so a registration never
    /// resolves to a despawned entity, whichever code path despawned it.
    pub fn remove_dead_entity_registrations(&mut self, is_alive: impl Fn(Entity) -> bool) -> bool {
        if self.entities.is_empty() {
            return false;
        }
        let before = self.entities.len();
        self.entities.retain(|_, e| is_alive(*e));
        let removed = self.entities.len() != before;
        if removed {
            self.entities_dirty = true;
        }
        removed
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
    ///
    /// `snapshot()` clears the flags, so they only tell the caller whether it was
    /// the first to look: a consumer that must see every write (the Lua signal
    /// cache, via `LuaRuntime::sync_signals`) calls `snapshot()` unconditionally,
    /// which is O(1) when nothing changed (six bool checks plus one `Arc::clone`).
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
    use crate::testing::approx_eq;

    // --- Scalars ---

    /// The same reads must give the same answers on the live resource and its snapshot.
    fn assert_signal_reads(signals: &impl SignalsRead, player: Entity) {
        assert_eq!(signals.get_scalar("speed"), Some(1.5));
        assert_eq!(signals.get_integer("score"), Some(7));
        assert_eq!(signals.get_string("scene"), Some("menu"));
        assert!(signals.has_flag("paused"));
        assert_eq!(signals.get_entity("player"), Some(player));
        assert_eq!(signals.get_group_count("enemies"), Some(3));
        assert_eq!(signals.get_group_count("debt"), Some(-2));

        assert_eq!(signals.get_scalar("missing"), None);
        assert_eq!(signals.get_integer("missing"), None);
        assert_eq!(signals.get_string("missing"), None);
        assert!(!signals.has_flag("missing"));
        assert_eq!(signals.get_entity("missing"), None);
        assert_eq!(signals.get_group_count("missing"), None);
    }

    #[test]
    fn world_signals_and_snapshot_share_one_read_api() {
        let mut ws = WorldSignals::default();
        let player = Entity::from_bits(42);
        ws.set_scalar("speed", 1.5);
        ws.set_integer("score", 7);
        ws.set_string("scene", "menu");
        ws.set_flag("paused");
        ws.set_entity("player", player);
        ws.set_group_count("enemies", 3);
        ws.set_group_count("debt", -2);

        assert_signal_reads(&ws, player);
        assert_signal_reads(&*ws.snapshot(), player);
    }

    // Removing an absent key must not dirty the domain, or `snapshot()` would
    // rebuild (and hand Lua a fresh Arc) for no change.
    #[test]
    fn removals_return_value_and_mark_dirty_only_when_present() {
        let mut ws = WorldSignals::default();
        let e = Entity::from_bits(42);
        ws.set_scalar("s", 1.5);
        ws.set_integer("i", 7);
        ws.set_string("t", "hi");
        ws.set_flag("f");
        ws.set_entity("e", e);
        ws.snapshot();
        assert_eq!(ws.get_string("t"), Some("hi"));
        assert_eq!(ws.get_entity("e"), Some(e));

        assert_eq!(ws.remove_scalar("missing"), None);
        assert_eq!(ws.remove_integer("missing"), None);
        assert_eq!(ws.remove_string("missing"), None);
        assert!(!ws.remove_flag("missing"));
        assert_eq!(ws.remove_entity("missing"), None);
        assert!(!ws.is_dirty(), "removing absent keys must not dirty");

        assert_eq!(ws.remove_scalar("s"), Some(1.5));
        assert_eq!(ws.remove_integer("i"), Some(7));
        assert_eq!(ws.remove_string("t").as_deref(), Some("hi"));
        assert!(ws.remove_flag("f"));
        assert_eq!(ws.remove_entity("e"), Some(e));
        assert!(ws.scalars_dirty && ws.integers_dirty && ws.strings_dirty);
        assert!(ws.flags_dirty && ws.entities_dirty);

        let snap = ws.snapshot();
        assert!(snap.scalars.is_empty() && snap.integers.is_empty() && snap.strings.is_empty());
        assert!(snap.flags.is_empty() && snap.entities.is_empty());
    }

    #[test]
    fn test_set_scalar_marks_dirty_only_when_changed() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("speed", 42.0);
        ws.snapshot(); // clear dirty
        ws.set_scalar("speed", 42.0); // same value, should not mark dirty
        assert!(!ws.scalars_dirty);
        ws.set_scalar("speed", 43.0); // changed value, should mark dirty
        assert!(ws.scalars_dirty);
    }

    // --- Integers ---

    #[test]
    fn test_set_integer_marks_dirty_only_when_changed() {
        let mut ws = WorldSignals::default();
        ws.set_integer("score", 100);
        ws.snapshot(); // clear dirty
        ws.set_integer("score", 100); // same value, should not mark dirty
        assert!(!ws.integers_dirty);
        ws.set_integer("score", 200); // changed value, should mark dirty
        assert!(ws.integers_dirty);
    }

    #[test]
    fn test_set_integer_group_count_side_effect_only_fires_on_change() {
        let mut ws = WorldSignals::default();
        let key = format!("{}enemy", sk::GROUP_COUNT_PREFIX);
        ws.set_integer(key.clone(), 5);
        ws.snapshot(); // clear dirty

        ws.set_integer(key.clone(), 5); // same value, group_counts untouched
        assert!(!ws.integers_dirty);
        assert!(!ws.group_counts_dirty);
        assert_eq!(ws.get_group_count("enemy"), Some(5));

        ws.set_integer(key, 6); // changed value, group_counts updated
        assert!(ws.integers_dirty);
        assert!(ws.group_counts_dirty);
        assert_eq!(ws.get_group_count("enemy"), Some(6));
    }

    // --- Strings ---

    #[test]
    fn test_set_string_marks_dirty_only_when_changed() {
        let mut ws = WorldSignals::default();
        ws.set_string("scene", "menu");
        ws.snapshot(); // clear dirty
        ws.set_string("scene", "menu"); // same value, should not mark dirty
        assert!(!ws.strings_dirty);
        ws.set_string("scene", "game"); // changed value, should mark dirty
        assert!(ws.strings_dirty);
    }

    // --- Flags ---

    #[test]
    fn test_set_flag_marks_dirty_only_when_new() {
        let mut ws = WorldSignals::default();
        ws.set_flag("paused");
        ws.snapshot(); // clear dirty
        ws.set_flag("paused"); // already present, should not mark dirty
        assert!(!ws.flags_dirty);
    }

    #[test]
    fn test_take_flag_marks_dirty_only_when_present() {
        let mut ws = WorldSignals::default();
        ws.set_flag("fire");
        ws.snapshot(); // clear dirty
        assert!(!ws.take_flag("nope")); // absent — should not dirty
        assert!(!ws.flags_dirty);
        assert!(ws.take_flag("fire")); // present — should dirty
        assert!(ws.flags_dirty);
        assert!(!ws.has_flag("fire"));
    }

    #[test]
    fn test_toggle_flag_marks_dirty() {
        let mut ws = WorldSignals::default();
        ws.set_flag("x");
        ws.snapshot(); // clear dirty
        ws.toggle_flag("x"); // present → remove
        assert!(ws.flags_dirty && !ws.has_flag("x"));
        ws.snapshot(); // clear dirty
        ws.toggle_flag("x"); // absent → insert
        assert!(ws.flags_dirty && ws.has_flag("x"));
    }

    // --- Scene / quit requests ---

    #[test]
    fn request_scene_sets_the_target_and_the_switch_flag() {
        let mut ws = WorldSignals::default();
        ws.request_scene("lvl");
        assert_eq!(ws.get_string(sk::SCENE), Some("lvl"));
        assert!(ws.has_flag(sk::SWITCH_SCENE));
    }

    #[test]
    fn request_quit_sets_the_quit_flag() {
        let mut ws = WorldSignals::default();
        ws.request_quit();
        assert!(ws.has_flag(sk::QUIT_GAME));
    }

    // --- Entities ---

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
            Some(entity_b),
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

        assert_eq!(ws.get_entity("cursor"), Some(entity_b));
        assert!(!ws.entities_dirty);
    }

    #[test]
    fn test_remove_dead_entity_registrations_drops_dead_keeps_live() {
        let mut ws = WorldSignals::default();
        let dead = Entity::from_bits(1);
        let live = Entity::from_bits(2);
        ws.set_entity("enemy", dead);
        ws.set_entity("enemy_alias", dead);
        ws.set_entity("player", live);
        ws.entities_dirty = false;

        let removed = ws.remove_dead_entity_registrations(|e| e == live);

        assert!(removed);
        assert!(ws.get_entity("enemy").is_none());
        assert!(ws.get_entity("enemy_alias").is_none());
        assert_eq!(ws.get_entity("player"), Some(live));
        assert!(ws.entities_dirty);
    }

    #[test]
    fn test_remove_dead_entity_registrations_all_live_is_clean() {
        let mut ws = WorldSignals::default();
        let live = Entity::from_bits(2);
        ws.set_entity("player", live);
        ws.entities_dirty = false;

        let removed = ws.remove_dead_entity_registrations(|_| true);

        assert!(!removed);
        assert_eq!(ws.get_entity("player"), Some(live));
        assert!(!ws.entities_dirty);
    }

    #[test]
    fn test_remove_entity_registrations_for_empty_registry() {
        let mut ws = WorldSignals::default();
        let entity_a = Entity::from_bits(1);
        ws.entities_dirty = false;

        ws.remove_entity_registrations_for(entity_a);

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
            Some(entity_b),
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

        assert_eq!(ws.get_entity("player"), Some(entity_a));
        assert_eq!(ws.get_entity("cursor"), Some(entity_b));
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
    fn over_long_group_names_are_ignored_not_merged() {
        let too_long_a = "a".repeat(MAX_GROUP_NAME_LEN + 1);
        let too_long_b = "b".repeat(MAX_GROUP_NAME_LEN + 1);
        let mut ws = WorldSignals::default();
        ws.set_group_count(&too_long_a, 5);
        assert_eq!(
            ws.get_group_count(&too_long_a),
            None,
            "over-long name is not stored"
        );
        assert_eq!(
            ws.get_group_count(&too_long_b),
            None,
            "a different over-long name must not read another's count"
        );
        assert!(ws.get_integers().is_empty(), "no truncated key was written");

        let at_limit = "c".repeat(MAX_GROUP_NAME_LEN);
        ws.set_group_count(&at_limit, 7);
        assert_eq!(
            ws.get_group_count(&at_limit),
            Some(7),
            "a name at the limit still works"
        );
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
        ws.remove_integer(&format!("{}enemy", sk::GROUP_COUNT_PREFIX));
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
