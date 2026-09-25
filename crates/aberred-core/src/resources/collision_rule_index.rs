//! Index of collision rules by unordered group pair.
//!
//! Both collision observers ([`rust_collision_observer`](crate::systems::rust_collision::rust_collision_observer),
//! [`lua_collision_observer`](crate::systems::lua_collision::lua_collision_observer))
//! used to linearly scan every rule entity for every `CollisionEvent`. This
//! resource pre-filters that scan down to the (small) bucket of rules
//! actually covering the colliding pair's groups, keyed by an unordered pair
//! normalized so `(a, b)` and `(b, a)` collide to the same bucket.
//!
//! Rebuilt from scratch by
//! [`rebuild_collision_rule_index`](crate::systems::collision_rule_index::rebuild_collision_rule_index)
//! whenever `CollisionRule`/`LuaCollisionRule` entities change (rules are
//! few and changes are rare -- spawn/despawn on scene switch -- so a full
//! rebuild is simpler than incremental maintenance and not worth the extra
//! bookkeeping). Each bucket is sorted by `Entity` so "first match wins"
//! becomes deterministic instead of query-iteration order.

use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

/// Normalizes an unordered group-name pair so `(a, b)` and `(b, a)` produce
/// the same key. Borrows rather than allocates, so a per-`CollisionEvent`
/// lookup costs no heap allocation.
pub(crate) fn normalize_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Rule entities bucketed by normalized group pair, for one rule kind
/// (Rust or Lua). Not generic over the rule's callback payload -- a plain
/// non-generic map is all either kind needs, and keeping this as its own
/// small type (rather than a bare `FxHashMap` field) is what lets
/// [`CollisionRuleIndex`] hold two instances without hand-duplicating
/// `is_empty`/`bucket` for each.
///
/// Nested (lesser group -> greater group -> rules) rather than keyed by a
/// `(String, String)` tuple: std's `HashMap` can't look a tuple of `String`s
/// up by a tuple of `&str`s, but each level alone looks up by `&str`.
#[derive(Default)]
pub struct RuleBuckets(FxHashMap<String, FxHashMap<String, SmallVec<[Entity; 2]>>>);

impl RuleBuckets {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn bucket(&self, ga: &str, gb: &str) -> Option<&SmallVec<[Entity; 2]>> {
        let (lo, hi) = normalize_pair(ga, gb);
        self.0.get(lo)?.get(hi)
    }

    /// Clears and refills from `rules`, sorting each resulting sub-bucket by
    /// `Entity` so "first match wins" is deterministic.
    ///
    /// `pub`, not `pub(crate)`: the facade's Lua-aware
    /// `aberredengine::systems::collision_rule_index::rebuild_collision_rule_index`
    /// calls this directly on `CollisionRuleIndex::lua`, since that variant
    /// (indexing `LuaCollisionRule`) cannot live in `aberred-core`.
    pub fn rebuild<'a>(
        &mut self,
        rules: impl Iterator<Item = (Entity, &'a String, &'a String)>,
    ) {
        self.0.clear();
        for (entity, group_a, group_b) in rules {
            let (lo, hi) = normalize_pair(group_a, group_b);
            self.0
                .entry(lo.to_owned())
                .or_default()
                .entry(hi.to_owned())
                .or_default()
                .push(entity);
        }
        for entities in self.0.values_mut().flat_map(|inner| inner.values_mut()) {
            entities.sort_unstable();
        }
    }
}

/// Maps an unordered group pair to the rule entities covering it. Two
/// [`RuleBuckets`] fields (rather than a generic type param) keep this one
/// resource and avoid monomorphizing the rebuild system; `lua` is
/// `#[cfg(feature = "lua")]`.
#[derive(Resource, Default)]
pub struct CollisionRuleIndex {
    /// `pub`, not `pub(crate)`: the facade's Lua-aware `rebuild_collision_rule_index`
    /// (which cannot live in `aberred-core` -- it names `LuaCollisionRule`)
    /// writes this field directly.
    #[cfg(feature = "lua")]
    pub lua: RuleBuckets,
    pub rust: RuleBuckets,
}

impl CollisionRuleIndex {
    /// Whether there are no rules registered at all (either kind) -- cheaper
    /// equivalent of the old `rules.is_empty()` early-return check.
    pub fn is_empty(&self) -> bool {
        self.rust.is_empty() && self.lua_is_empty()
    }

    #[cfg(feature = "lua")]
    fn lua_is_empty(&self) -> bool {
        self.lua.is_empty()
    }

    #[cfg(not(feature = "lua"))]
    fn lua_is_empty(&self) -> bool {
        true
    }

    /// Rust rule entities covering the given (unordered) group pair, sorted
    /// by `Entity` for deterministic first-match.
    pub fn rust_bucket(&self, ga: &str, gb: &str) -> Option<&SmallVec<[Entity; 2]>> {
        self.rust.bucket(ga, gb)
    }

    /// Lua rule entities covering the given (unordered) group pair, sorted
    /// by `Entity` for deterministic first-match.
    #[cfg(feature = "lua")]
    pub fn lua_bucket(&self, ga: &str, gb: &str) -> Option<&SmallVec<[Entity; 2]>> {
        self.lua.bucket(ga, gb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_pair_is_order_independent() {
        assert_eq!(normalize_pair("ball", "brick"), normalize_pair("brick", "ball"));
    }

    #[test]
    fn is_empty_true_by_default() {
        let index = CollisionRuleIndex::default();
        assert!(index.is_empty());
    }

    #[test]
    fn rust_bucket_missing_key_is_none() {
        let index = CollisionRuleIndex::default();
        assert!(index.rust_bucket("a", "b").is_none());
    }

    #[test]
    fn rust_bucket_found_regardless_of_query_order() {
        let mut index = CollisionRuleIndex::default();
        let e = Entity::from_bits(1);
        let (a, b) = ("a".to_string(), "b".to_string());
        index.rust.rebuild(std::iter::once((e, &a, &b)));
        assert!(!index.is_empty());
        assert_eq!(index.rust_bucket("a", "b").unwrap().as_slice(), &[e]);
        assert_eq!(index.rust_bucket("b", "a").unwrap().as_slice(), &[e]);
    }
}
