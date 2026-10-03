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
#[derive(Resource, Debug, Default)]
pub struct PendingAssets {
    /// Outstanding load count per key, per kind. A key with no outstanding
    /// load has no entry.
    loads: FxHashMap<AssetKind, FxHashMap<String, u32>>,
}

impl PendingAssets {
    /// Records one load of `key` sent to its loading thread.
    pub fn queue(&mut self, kind: AssetKind, key: &str) {
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
}
