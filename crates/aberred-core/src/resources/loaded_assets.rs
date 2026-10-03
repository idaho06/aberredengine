//! [`LoadedAssets`]: the assets the render and audio threads report as loaded.

use bevy_ecs::prelude::Resource;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::protocol::asset_kind::{AssetChange, AssetKind};

/// The keys the render and audio threads have reported as loaded, per
/// [`AssetKind`]. Every change comes from one reply, mapped by
/// `LogicMsg::asset_change`/`AudioMessage::asset_change`: a successful load
/// adds a key, a removal, unload or rename takes it away. A failed load
/// changes nothing.
#[derive(Resource, Debug, Default)]
pub struct LoadedAssets {
    keys: FxHashMap<AssetKind, FxHashSet<String>>,
}

impl LoadedAssets {
    /// Applies one reply's change.
    pub fn apply(&mut self, change: AssetChange) {
        match change {
            AssetChange::Loaded { kind, key } => {
                self.keys.entry(kind).or_default().insert(key);
            }
            AssetChange::Removed { kind, key } => {
                if let Some(keys) = self.keys.get_mut(&kind) {
                    keys.remove(&key);
                }
            }
            AssetChange::Renamed {
                kind,
                old_key,
                new_key,
            } => {
                if let Some(keys) = self.keys.get_mut(&kind)
                    && keys.remove(&old_key)
                {
                    keys.insert(new_key);
                }
            }
            AssetChange::Cleared(kind) => {
                self.keys.remove(&kind);
            }
        }
    }

    /// Whether `key` is loaded.
    pub fn contains(&self, kind: AssetKind, key: &str) -> bool {
        self.keys.get(&kind).is_some_and(|keys| keys.contains(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded(kind: AssetKind, key: &str) -> AssetChange {
        AssetChange::Loaded {
            kind,
            key: key.into(),
        }
    }

    #[test]
    fn applies_loads_removals_renames_and_clears_per_kind() {
        let mut assets = LoadedAssets::default();
        assets.apply(loaded(AssetKind::Texture, "player"));
        assets.apply(loaded(AssetKind::Sound, "player"));
        assert!(assets.contains(AssetKind::Texture, "player"));
        assert!(!assets.contains(AssetKind::Font, "player"));

        assets.apply(AssetChange::Renamed {
            kind: AssetKind::Texture,
            old_key: "player".into(),
            new_key: "hero".into(),
        });
        assert!(!assets.contains(AssetKind::Texture, "player"));
        assert!(assets.contains(AssetKind::Texture, "hero"));

        assets.apply(AssetChange::Removed {
            kind: AssetKind::Texture,
            key: "hero".into(),
        });
        assert!(!assets.contains(AssetKind::Texture, "hero"));
        assert!(
            assets.contains(AssetKind::Sound, "player"),
            "other kinds untouched"
        );

        assets.apply(loaded(AssetKind::Sound, "jump"));
        assets.apply(AssetChange::Cleared(AssetKind::Sound));
        assert!(!assets.contains(AssetKind::Sound, "player"));
        assert!(!assets.contains(AssetKind::Sound, "jump"));
    }

    #[test]
    fn renaming_an_unloaded_key_is_a_no_op() {
        let mut assets = LoadedAssets::default();
        assets.apply(AssetChange::Renamed {
            kind: AssetKind::Font,
            old_key: "missing".into(),
            new_key: "other".into(),
        });
        assert!(!assets.contains(AssetKind::Font, "other"));
    }
}
