//! Ruled collision pairs touching across ticks.
//!
//! [`CollisionContacts`] remembers which `(rule, a, b)` contacts matched a
//! [`CollisionRule`](crate::components::collision::CollisionRule) last tick,
//! so a collision observer can tell a contact that just started from one
//! that stays, and the end of the tick can report the contacts that stopped.

use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::FxHashSet;

/// One ruled contact: the matched rule entity and the two touching entities,
/// with `a`/`b` in the rule's `group_a`/`group_b` order.
///
/// Orders by `rule`, then `a`, then `b`; `Entity`'s order agrees with
/// `Entity::to_bits` (pinned by the `entity_ord_agrees_with_to_bits` bevy
/// contract test), so sorted contacts have a reproducible order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Contact {
    /// The matched rule entity.
    pub rule: Entity,
    /// The entity in the rule's `group_a`.
    pub a: Entity,
    /// The entity in the rule's `group_b`.
    pub b: Entity,
}

/// The ruled contacts of the previous tick and of the current one.
#[derive(Resource, Default)]
pub struct CollisionContacts {
    previous: FxHashSet<Contact>,
    current: FxHashSet<Contact>,
}

impl CollisionContacts {
    /// Records that `rule` matched the touching pair `a`/`b` this tick.
    /// Called by the engine's rule observers; a game calling it fakes a
    /// contact.
    /// Returns `true` when the contact just started (it was not touching
    /// last tick).
    #[doc(hidden)]
    pub fn begin(&mut self, rule: Entity, a: Entity, b: Entity) -> bool {
        let contact = Contact { rule, a, b };
        self.current.insert(contact);
        !self.previous.contains(&contact)
    }

    /// Ends the tick: returns the contacts that touched last tick but not
    /// this one, sorted, and makes this tick's contacts the previous ones.
    /// Called by the engine's ended systems; a game calling it swallows the
    /// ended events.
    #[doc(hidden)]
    pub fn end_tick(&mut self) -> Vec<Contact> {
        let mut ended: Vec<_> = self.previous.difference(&self.current).copied().collect();
        ended.sort_unstable();
        std::mem::swap(&mut self.previous, &mut self.current);
        self.current.clear();
        ended
    }

    /// Whether no contact touched last tick or this one, so
    /// [`end_tick`](Self::end_tick) has nothing to report or rotate.
    pub fn is_empty(&self) -> bool {
        self.previous.is_empty() && self.current.is_empty()
    }

    /// Forgets every contact without reporting any as ended.
    pub fn clear(&mut self) {
        self.previous.clear();
        self.current.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn entities(n: usize) -> Vec<Entity> {
        let mut world = World::new();
        (0..n).map(|_| world.spawn_empty().id()).collect()
    }

    fn contact(rule: Entity, a: Entity, b: Entity) -> Contact {
        Contact { rule, a, b }
    }

    #[test]
    fn begin_reports_a_start_once_while_the_pair_stays() {
        let e = entities(3);
        let mut contacts = CollisionContacts::default();

        assert!(contacts.begin(e[0], e[1], e[2]));
        assert!(contacts.end_tick().is_empty());
        assert!(!contacts.begin(e[0], e[1], e[2]));
        assert!(contacts.end_tick().is_empty());
    }

    #[test]
    fn end_tick_reports_a_vanished_pair_once() {
        let e = entities(3);
        let mut contacts = CollisionContacts::default();
        contacts.begin(e[0], e[1], e[2]);
        contacts.end_tick();

        assert_eq!(contacts.end_tick(), vec![contact(e[0], e[1], e[2])]);
        assert!(contacts.end_tick().is_empty());
    }

    #[test]
    fn re_entry_after_an_end_starts_again() {
        let e = entities(3);
        let mut contacts = CollisionContacts::default();
        contacts.begin(e[0], e[1], e[2]);
        contacts.end_tick();
        contacts.end_tick();

        assert!(contacts.begin(e[0], e[1], e[2]));
    }

    #[test]
    fn end_tick_sorts_by_to_bits_whatever_the_insertion_order() {
        // Enough contacts that hash-set iteration order is not sorted by chance.
        let e = entities(8);
        let mut contacts = CollisionContacts::default();
        for rule in e.iter().rev() {
            for a in e.iter().rev() {
                contacts.begin(*rule, *a, e[0]);
            }
        }
        contacts.end_tick();

        let ended = contacts.end_tick();
        assert_eq!(ended.len(), 64);
        assert!(ended.is_sorted_by_key(|c| (c.rule.to_bits(), c.a.to_bits(), c.b.to_bits())));
    }

    #[test]
    fn clear_forgets_contacts_without_reporting_them() {
        let e = entities(3);
        let mut contacts = CollisionContacts::default();
        contacts.begin(e[0], e[1], e[2]);
        contacts.end_tick();
        contacts.begin(e[0], e[1], e[2]);

        contacts.clear();

        assert!(contacts.end_tick().is_empty());
        assert!(contacts.begin(e[0], e[1], e[2]));
    }

    #[test]
    fn is_empty_until_no_contact_touched_last_tick_or_this_one() {
        let e = entities(3);
        let mut contacts = CollisionContacts::default();
        assert!(contacts.is_empty());

        contacts.begin(e[0], e[1], e[2]);
        assert!(!contacts.is_empty(), "touching this tick");
        contacts.end_tick();
        assert!(!contacts.is_empty(), "touched last tick");
        contacts.end_tick();
        assert!(contacts.is_empty());
    }

    #[test]
    fn different_rules_on_the_same_pair_are_distinct_contacts() {
        let e = entities(4);
        let mut contacts = CollisionContacts::default();
        contacts.begin(e[0], e[2], e[3]);
        contacts.end_tick();

        assert!(contacts.begin(e[1], e[2], e[3]));
        assert_eq!(contacts.end_tick(), vec![contact(e[0], e[2], e[3])]);
    }
}
