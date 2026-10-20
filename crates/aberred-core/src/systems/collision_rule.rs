//! Rule-matched collision events.
//!
//! [`collision_rule_observer`] receives each [`BoxOverlap`], finds the
//! [`CollisionRule`] covering the pair's groups, and triggers
//! [`CollisionOngoing`] on that rule's entity, preceded by
//! [`CollisionStarted`] when the contact is new. [`collision_ended_system`]
//! triggers [`CollisionEnded`] for the contacts that stopped this tick.
//!
//! # Collision Flow
//!
//! 1. [`collision_detector`](crate::systems::collision_detector::collision_detector) detects overlaps
//!    and triggers `BoxOverlap` events
//! 2. `collision_rule_observer` looks up the matching rule by
//!    [`Group`] names
//! 3. It computes the contact sides, records the contact in
//!    [`CollisionContacts`], and triggers `CollisionStarted` (new contacts
//!    only) and `CollisionOngoing` on the rule entity
//! 4. After detection, `collision_ended_system` triggers `CollisionEnded` for
//!    each contact that did not touch this tick
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – collision detection
//! - [`crate::components::collision::CollisionRule`] – group-pair rule
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision`] – `BoxOverlap`, `CollisionOngoing` and the contact events

use bevy_ecs::prelude::*;

use crate::components::collision::CollisionRule;
use crate::components::group::Group;
use crate::events::collision::{BoxOverlap, CollisionEnded, CollisionOngoing, CollisionStarted};
use crate::resources::collision_contacts::CollisionContacts;
use crate::resources::collision_rule_index::CollisionRuleIndex;
use crate::systems::collision::{ColliderRects, compute_sides, resolve_groups};

/// Observer that turns a [`BoxOverlap`] into a [`CollisionOngoing`] on the
/// matching [`CollisionRule`] entity.
///
/// 1. Looks up [`Group`] names for both entities (returns early if missing)
/// 2. Scans the [`CollisionRuleIndex`] bucket for the pair's groups for a
///    matching rule (deterministic lowest-`Entity`-first if more than one
///    covers the pair)
/// 3. Computes contact sides via [`compute_sides`]
/// 4. Records the contact in [`CollisionContacts`]; a new one triggers
///    [`CollisionStarted`] first
/// 5. Triggers `CollisionOngoing` with `a`/`b` in the rule's group order
pub fn collision_rule_observer(
    trigger: On<BoxOverlap>,
    rules: Query<&CollisionRule>,
    index: Res<CollisionRuleIndex>,
    groups: Query<&Group>,
    rects: ColliderRects,
    mut contacts: ResMut<CollisionContacts>,
    mut commands: Commands,
) {
    if index.is_empty() {
        return;
    }

    let BoxOverlap { a, b } = *trigger.event();

    let Some((ga, gb)) = resolve_groups(&groups, a, b) else {
        return;
    };

    let Some((rule, ent_a, ent_b)) = index.find_match(&rules, a, b, ga, gb) else {
        return;
    };

    let (sides_a, sides_b) = compute_sides(rects.rect(ent_a), rects.rect(ent_b));

    if contacts.begin(rule, ent_a, ent_b) {
        commands.trigger(CollisionStarted {
            rule,
            a: ent_a,
            b: ent_b,
            sides_a: sides_a.clone(),
            sides_b: sides_b.clone(),
        });
    }
    commands.trigger(CollisionOngoing {
        rule,
        a: ent_a,
        b: ent_b,
        sides_a,
        sides_b,
    });
}

/// Run condition: some ruled contact touched last tick or this one, so
/// [`collision_ended_system`] has something to report or rotate.
pub fn has_rule_contacts(contacts: Res<CollisionContacts>) -> bool {
    !contacts.is_empty()
}

/// Triggers [`CollisionEnded`] for each [`CollisionContacts`] contact that
/// touched last tick but not this one, in `(rule, a, b)` order.
///
/// Runs after [`collision_detector`](crate::systems::collision_detector::collision_detector),
/// once this tick's [`collision_rule_observer`] calls have recorded the
/// contacts that still touch.
pub fn collision_ended_system(mut contacts: ResMut<CollisionContacts>, mut commands: Commands) {
    for contact in contacts.end_tick() {
        commands.trigger(CollisionEnded {
            rule: contact.rule,
            a: contact.a,
            b: contact.b,
        });
    }
}
