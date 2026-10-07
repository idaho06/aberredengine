//! Index of collision rules by unordered group pair.
//!
//! A collision observer scans only the (small) bucket of rules covering the
//! colliding pair's groups instead of every rule entity, keyed by an
//! unordered pair normalized so `(a, b)` and `(b, a)` share a bucket.
//!
//! [`CollisionRuleIndex`] is rebuilt by
//! [`rebuild_rule_index`](crate::systems::collision_rule_index::rebuild_rule_index)
//! from scratch whenever a rule changes (rules are few
//! and changes are rare -- spawn/despawn on scene switch -- so a full rebuild
//! is simpler than incremental maintenance). Each bucket is sorted by
//! `Entity` so "first match wins" is deterministic instead of
//! query-iteration order.

use bevy_ecs::prelude::{Entity, Query, Resource};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::components::collision::{CollisionRule, match_groups};

/// Normalizes an unordered group-name pair so `(a, b)` and `(b, a)` produce
/// the same key. Borrows rather than allocates, so a per-`Overlapping`
/// lookup costs no heap allocation.
pub(crate) fn normalize_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

/// [`CollisionRule`] entities by unordered group pair, read by
/// [`collision_rule_observer`](crate::systems::collision_rule::collision_rule_observer).
///
/// Nested (lesser group -> greater group -> rules) rather than keyed by a
/// `(String, String)` tuple: std's `HashMap` can't look a tuple of `String`s
/// up by a tuple of `&str`s, but each level alone looks up by `&str`.
#[derive(Resource, Default)]
pub struct CollisionRuleIndex {
    buckets: FxHashMap<String, FxHashMap<String, SmallVec<[Entity; 2]>>>,
}

impl CollisionRuleIndex {
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

    /// The first rule covering the colliding pair `(a, b)` in groups
    /// `(ga, gb)`, as `(rule_entity, ent_a, ent_b)` with `ent_a`/`ent_b`
    /// ordered to match the rule's `group_a`/`group_b`.
    ///
    /// Scans the pair's bucket in `Entity` order, skipping any entity that
    /// stopped being a rule (or despawned) since the last rebuild.
    pub(crate) fn find_match(
        &self,
        rules: &Query<&CollisionRule>,
        a: Entity,
        b: Entity,
        ga: &str,
        gb: &str,
    ) -> Option<(Entity, Entity, Entity)> {
        self.bucket(ga, gb)?.iter().find_map(|&rule_entity| {
            let rule = rules.get(rule_entity).ok()?;
            let (ent_a, ent_b) = match_groups(&rule.group_a, &rule.group_b, a, b, ga, gb)?;
            Some((rule_entity, ent_a, ent_b))
        })
    }

    /// Clears and refills from `rules`, sorting each resulting sub-bucket by
    /// `Entity` so "first match wins" is deterministic.
    pub(crate) fn rebuild<'a>(&mut self, rules: impl Iterator<Item = (Entity, &'a CollisionRule)>) {
        self.buckets.clear();
        for (entity, rule) in rules {
            let (lo, hi) = normalize_pair(&rule.group_a, &rule.group_b);
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
        let rule = CollisionRule::new("a", "b");
        index.rebuild(std::iter::once((e, &rule)));
        assert!(!index.is_empty());
        assert_eq!(index.bucket("a", "b").unwrap(), &[e]);
        assert_eq!(index.bucket("b", "a").unwrap(), &[e]);
    }
}
