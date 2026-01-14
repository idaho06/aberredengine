//! Tracked groups resource for entity counting.
//!
//! The [`TrackedGroups`] resource defines which group names should be
//! monitored by the [`update_group_counts_system`](crate::systems::group::update_group_counts_system).
//! This keeps the engine decoupled from game-specific group names.
//!
//! # How It Works
//!
//! 1. Register groups to track via `tracked_groups.add_group("ball")`
//! 2. The group counting system queries all entities with matching [`Group`](crate::components::group::Group) components
//! 3. Entity counts are published to [`WorldSignals`](crate::resources::worldsignals::WorldSignals) as `"group_count:{name}"`
//!
//! # Integration with Phases
//!
//! Phase callbacks can check group counts to detect game state changes:
//! - No balls remaining → lose a life
//! - No bricks remaining → level complete
//!
//! # Usage
//!
//! ```ignore
//! // At scene setup, configure which groups to count:
//! tracked_groups.add_group("ball");
//! tracked_groups.add_group("brick");
//!
//! // The system will then update WorldSignals with:
//! // - "group_count:ball" → number of ball entities
//! // - "group_count:brick" → number of brick entities
//!
//! // In phase callback:
//! if let Some(0) = ctx.world_signals.get_group_count("ball") {
//!     return Some("lose_life".into());
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::systems::group::update_group_counts_system`] – the counting system
//! - [`crate::resources::worldsignals::WorldSignals`] – where counts are published
//! - [`crate::components::group::Group`] – the group tag component

use bevy_ecs::prelude::*;
use rustc_hash::FxHashSet;

/// Resource that holds the set of group names to track for entity counting.
///
/// Groups added here will have their entity counts published to
/// [`WorldSignals`](crate::resources::worldsignals::WorldSignals) by
/// [`update_group_counts_system`](crate::systems::group::update_group_counts_system).
///
/// This resource should be cleared when switching scenes to avoid stale counts.
#[derive(Debug, Clone, Resource, Default)]
pub struct TrackedGroups {
    /// The set of group names currently being tracked.
    pub groups: FxHashSet<String>,
}

impl TrackedGroups {
    /// Builder method to add a group name to track.
    ///
    /// Returns `self` for method chaining.
    // pub fn with(mut self, group_name: impl Into<String>) -> Self {
    //     self.add_group(group_name);
    //     self
    // }

    /// Adds a group name to the set of tracked groups.
    pub fn add_group(&mut self, group_name: impl Into<String>) {
        self.groups.insert(group_name.into());
    }

    /// Returns `true` if the given group name is being tracked.
    pub fn has_group(&self, group_name: impl AsRef<str>) -> bool {
        self.groups.contains(group_name.as_ref())
    }

    /// Removes a group name from tracking.
    pub fn remove_group(&mut self, group_name: impl AsRef<str>) {
        self.groups.remove(group_name.as_ref());
    }

    /// Clears all tracked group names.
    pub fn clear(&mut self) {
        self.groups.clear();
    }

    /// Returns an iterator over all tracked group names.
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.groups.iter()
    }
}
