//! Index of collision rules by unordered group pair.
//!
//! A collision observer scans only the (small) bucket of rules covering the
//! colliding pair's groups instead of every rule entity, keyed by an
//! unordered pair normalized so `(a, b)` and `(b, a)` share a bucket.
//!
//! [`RuleIndex<T>`] is generic over the rule component ([`RuleGroups`]), so
//! `aberred-lua` indexes its own rule component with the same type and the
//! same rebuild system,
//! [`rebuild_rule_index`](crate::systems::collision_rule_index::rebuild_rule_index).
//! The index is rebuilt from scratch whenever a rule changes (rules are few
//! and changes are rare -- spawn/despawn on scene switch -- so a full rebuild
//! is simpler than incremental maintenance). Each bucket is sorted by
//! `Entity` so "first match wins" is deterministic instead of
//! query-iteration order.

use std::marker::PhantomData;

use bevy_ecs::prelude::{Component, Entity, Resource};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::components::collision::CollisionRule;

/// A rule component matched by a pair of group names.
pub trait RuleGroups: Component {
    /// The rule's `(group_a, group_b)`.
    fn groups(&self) -> (&str, &str);
}

/// Normalizes an unordered group-name pair so `(a, b)` and `(b, a)` produce
/// the same key. Borrows rather than allocates, so a per-`CollisionEvent`
/// lookup costs no heap allocation.
pub(crate) fn normalize_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

/// `T` rule entities by unordered group pair.
///
/// Nested (lesser group -> greater group -> rules) rather than keyed by a
/// `(String, String)` tuple: std's `HashMap` can't look a tuple of `String`s
/// up by a tuple of `&str`s, but each level alone looks up by `&str`.
#[derive(Resource)]
pub struct RuleIndex<T: RuleGroups> {
    buckets: FxHashMap<String, FxHashMap<String, SmallVec<[Entity; 2]>>>,
    rule: PhantomData<fn() -> T>,
}

/// Index of [`CollisionRule`] entities, read by
/// [`rust_collision_observer`](crate::systems::rust_collision::rust_collision_observer).
pub type CollisionRuleIndex = RuleIndex<CollisionRule>;

impl<T: RuleGroups> Default for RuleIndex<T> {
    fn default() -> Self {
        Self {
            buckets: FxHashMap::default(),
            rule: PhantomData,
        }
    }
}

impl<T: RuleGroups> RuleIndex<T> {
    /// Whether no rule is indexed.
    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// Rule entities covering the given (unordered) group pair, sorted by
    /// `Entity` for deterministic first-match.
    pub fn bucket(&self, ga: &str, gb: &str) -> Option<&[Entity]> {
        let (lo, hi) = normalize_pair(ga, gb);
        self.buckets.get(lo)?.get(hi).map(|v| v.as_slice())
    }

    /// Clears and refills from `rules`, sorting each resulting sub-bucket by
    /// `Entity` so "first match wins" is deterministic.
    pub(crate) fn rebuild<'a>(&mut self, rules: impl Iterator<Item = (Entity, &'a T)>) {
        self.buckets.clear();
        for (entity, rule) in rules {
            let (group_a, group_b) = rule.groups();
            let (lo, hi) = normalize_pair(group_a, group_b);
            self.buckets
                .entry(lo.to_owned())
                .or_default()
                .entry(hi.to_owned())
                .or_default()
                .push(entity);
        }
        for entities in self
            .buckets
            .values_mut()
            .flat_map(|inner| inner.values_mut())
        {
            entities.sort_unstable();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_pair_is_order_independent() {
        assert_eq!(
            normalize_pair("ball", "brick"),
            normalize_pair("brick", "ball")
        );
    }

    #[test]
    fn is_empty_true_by_default() {
        let index = CollisionRuleIndex::default();
        assert!(index.is_empty());
    }

    #[test]
    fn bucket_missing_key_is_none() {
        let index = CollisionRuleIndex::default();
        assert!(index.bucket("a", "b").is_none());
    }

    #[test]
    fn bucket_found_regardless_of_query_order() {
        let mut index = CollisionRuleIndex::default();
        let e = Entity::from_bits(1);
        fn noop(
            _: Entity,
            _: Entity,
            _: &crate::components::collision::BoxSides,
            _: &crate::components::collision::BoxSides,
            _: &mut crate::systems::GameCtx,
        ) {
        }
        let rule = CollisionRule::rust("a", "b", noop);
        index.rebuild(std::iter::once((e, &rule)));
        assert!(!index.is_empty());
        assert_eq!(index.bucket("a", "b").unwrap(), &[e]);
        assert_eq!(index.bucket("b", "a").unwrap(), &[e]);
    }
}
