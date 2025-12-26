//! Group entity counting system.
//!
//! This module provides a system that counts entities belonging to tracked
//! groups and publishes the counts as integer signals in [`WorldSignals`].
//!
//! # Purpose
//!
//! This enables game logic (especially [`Phase`](crate::components::phase::Phase) callbacks)
//! to react to group population changes, such as:
//! - Detecting when all "ball" entities are gone → lose a life
//! - Detecting when all "brick" entities are destroyed → level complete
//!
//! # Engine-Agnostic Design
//!
//! The system does not know about specific group names. Games configure which
//! groups to track via the [`TrackedGroups`] resource, keeping the engine
//! decoupled from game-specific logic.
//!
//! # Signal Keys
//!
//! Counts are stored with the key format `"group_count:{name}"`. Use
//! `world_signals.get_group_count("name")` for convenient access.
//!
//! # Related
//!
//! - [`TrackedGroups`](crate::resources::group::TrackedGroups) – configures which groups to count
//! - [`WorldSignals`](crate::resources::worldsignals::WorldSignals) – where counts are published
//! - [`Group`](crate::components::group::Group) – the group tag component

use crate::components::group::Group;
use crate::resources::group::TrackedGroups;
use crate::resources::worldsignals::WorldSignals;
use bevy_ecs::prelude::*;

use rustc_hash::FxHashMap;

/// Counts entities for each tracked group and updates [`WorldSignals`].
///
/// For each group name registered in [`TrackedGroups`], this system counts
/// how many entities have a matching [`Group`] component and stores the
/// result as an integer signal with the key `group_count:{name}`.
///
/// Groups with zero entities are correctly reported as `0`, which is
/// essential for detecting when all entities of a group have been despawned.
///
/// # Example
///
/// ```ignore
/// // In game setup, register groups to track:
/// tracked_groups.add_group("ball");
/// tracked_groups.add_group("brick");
///
/// // Later, in game logic:
/// let ball_count = world_signals.get_integer("group_count:ball").unwrap_or(0);
/// if ball_count == 0 {
///     // Player lost a life!
/// }
/// ```
pub fn update_group_counts_system(
    query_group: Query<&Group>,
    mut world_signals: ResMut<WorldSignals>,
    tracked_groups: Res<TrackedGroups>,
) {
    // Count entities for each group in the query
    let mut counts: FxHashMap<&str, i32> = FxHashMap::default();
    for group in query_group.iter() {
        if tracked_groups.has_group(group.name()) {
            *counts.entry(group.name()).or_insert(0) += 1;
        }
    }

    // Update world signals for all tracked groups (including zeros for despawned groups)
    // set_group_count only marks dirty if value changed, and uses stack allocation
    for group_name in tracked_groups.iter() {
        let count = counts.get(group_name.as_str()).copied().unwrap_or(0);
        world_signals.set_group_count(group_name, count);
    }
}
