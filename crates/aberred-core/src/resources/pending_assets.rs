//! [`PendingAssets`]: asset loads queued but not yet answered.

use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;

use crate::protocol::asset_kind::AssetKind;

/// Asset loads that have been sent to the render or audio thread and not yet
/// answered.
///
/// A load is added when its command is forwarded (from `AssetLoader` or a
/// raw `MessageWriter<RenderAssetCmd>`/`MessageWriter<AudioCmd>`), and
/// removed when its reply arrives, success or failure. Loading the same key
/// twice adds two entries, settled by two replies. Read it to wait for a
/// batch of loads, e.g. to show a loading screen until `is_empty()`.
///
/// The queued total is cumulative for the life of the world;
/// [`progress`](Self::progress) is the answered share of it.
#[derive(Resource, Debug, Default)]
pub struct PendingAssets {
    /// Outstanding load count per key, per kind. A key with no outstanding
    /// load has no entry.
    loads: FxHashMap<AssetKind, FxHashMap<String, u32>>,
    /// Loads ever queued.
    queued: usize,
}

impl PendingAssets {
    /// Records one load of `key` sent to its loading thread.
    pub fn queue(&mut self, kind: AssetKind, key: &str) {
        self.queued += 1;
        let keys = self.loads.entry(kind).or_default();
        match keys.get_mut(key) {
            Some(count) => *count += 1,
            None => {
                keys.insert(key.to_owned(), 1);
            }
        }
    }

    /// Settles one load of `key` when its reply arrives. A reply with no
    /// pending load is ignored.
    pub fn settle(&mut self, kind: AssetKind, key: &str) {
        let Some(keys) = self.loads.get_mut(&kind) else {
            return;
        };
        let Some(count) = keys.get_mut(key) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            keys.remove(key);
        }
    }

    /// The fraction of all queued loads that have been answered, `1.0` when
    /// nothing was ever queued.
    pub fn progress(&self) -> f32 {
        if self.queued == 0 {
            1.0
        } else {
            (self.queued - self.len()) as f32 / self.queued as f32
        }
    }

    /// Whether a load of `key` is still waiting for its reply.
    pub fn contains(&self, kind: AssetKind, key: &str) -> bool {
        self.loads
            .get(&kind)
            .is_some_and(|keys| keys.contains_key(key))
    }

    /// The number of loads still waiting for a reply.
    pub fn len(&self) -> usize {
        self.loads
            .values()
            .flat_map(|keys| keys.values())
            .map(|&count| count as usize)
            .sum()
    }

    /// Whether every queued load has been answered.
    pub fn is_empty(&self) -> bool {
        self.loads.values().all(|keys| keys.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_load_is_pending_until_settled() {
        let mut pending = PendingAssets::default();
        assert!(pending.is_empty());

        pending.queue(AssetKind::Texture, "player");
        assert!(pending.contains(AssetKind::Texture, "player"));
        assert!(!pending.contains(AssetKind::Font, "player"));
        assert_eq!(pending.len(), 1);

        pending.settle(AssetKind::Texture, "player");
        assert!(pending.is_empty());
    }

    #[test]
    fn each_queued_load_needs_its_own_reply() {
        let mut pending = PendingAssets::default();
        pending.queue(AssetKind::Sound, "jump");
        pending.queue(AssetKind::Sound, "jump");
        assert_eq!(pending.len(), 2);

        pending.settle(AssetKind::Sound, "jump");
        assert!(pending.contains(AssetKind::Sound, "jump"));
        assert_eq!(pending.len(), 1);

        pending.settle(AssetKind::Sound, "jump");
        assert!(pending.is_empty());
    }

    #[test]
    fn settling_an_unqueued_load_is_a_no_op() {
        let mut pending = PendingAssets::default();
        pending.queue(AssetKind::Font, "arcade");
        pending.settle(AssetKind::Font, "other");
        pending.settle(AssetKind::Texture, "arcade");
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn progress_is_one_when_nothing_was_queued() {
        assert_eq!(PendingAssets::default().progress(), 1.0);
    }

    #[test]
    fn progress_is_settled_over_queued() {
        let mut pending = PendingAssets::default();
        for key in ["a", "b", "c", "d"] {
            pending.queue(AssetKind::Texture, key);
        }
        pending.settle(AssetKind::Texture, "a");
        assert_eq!(pending.progress(), 0.25);

        for key in ["b", "c", "d"] {
            pending.settle(AssetKind::Texture, key);
        }
        assert_eq!(pending.progress(), 1.0);
    }

    #[test]
    fn queueing_the_same_key_twice_counts_twice() {
        let mut pending = PendingAssets::default();
        pending.queue(AssetKind::Sound, "jump");
        pending.queue(AssetKind::Sound, "jump");
        pending.settle(AssetKind::Sound, "jump");
        assert_eq!(pending.progress(), 0.5);
    }

    #[test]
    fn settling_an_unqueued_load_does_not_advance_progress() {
        let mut pending = PendingAssets::default();
        pending.queue(AssetKind::Font, "arcade");
        pending.settle(AssetKind::Font, "other");
        pending.settle(AssetKind::Texture, "arcade");
        assert_eq!(pending.progress(), 0.0);
    }

    #[test]
    fn a_later_queue_lowers_progress_again() {
        let mut pending = PendingAssets::default();
        pending.queue(AssetKind::Texture, "a");
        pending.settle(AssetKind::Texture, "a");
        assert_eq!(pending.progress(), 1.0);

        pending.queue(AssetKind::Texture, "b");
        assert_eq!(pending.progress(), 0.5);
    }
}
