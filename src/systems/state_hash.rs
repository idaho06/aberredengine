//! Per-tick deterministic world-state hash (determinism roadmap phase 05,
//! `docs/plans/determinism-05-replays.md`).
//!
//! [`hash_world_state`] is called directly from `logic_thread_main`'s loop
//! (and equally from `TestWorld`-driven tests) right after `run_sim_tick` —
//! not as a `SimSet::Bookkeeping` schedule system — so it's reachable
//! identically from every `TickInput` source with no schedule hook needed in
//! the headless test harness. Callers gate how often this actually runs
//! (`checkpoint_countdown` in `logic_thread_main`,
//! `src/engine_app/logic_thread.rs`); this function itself always does a
//! full-world pass.
//!
//! **Blind spot:** `AppState` (`Box<dyn Any>`) is not hashed — v1 scope cut,
//! see `docs/plans/determinism-05-replays.md`'s Open Questions. A game whose
//! deterministic behavior depends solely on `AppState` contents (never
//! reflected into a hashed component/resource) will not have that dependency
//! caught by replay divergence tests.

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::phase::{Phase, PhaseCallbackFns};
use crate::components::position2d::{Position2D, PositionSpace};
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::timer::{Timer, TimerCallback};
use crate::components::ttl::Ttl;
use crate::components::tween::{Easing, LoopMode, Tween, TweenValue};
use crate::protocol::replay::ReplayHasher;
use crate::resources::sim_rng::SimRng;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

fn hash_vector2(h: &mut ReplayHasher, v: Vector2) {
    h.write_f32(v.x);
    h.write_f32(v.y);
}

fn hash_easing(h: &mut ReplayHasher, easing: Easing) {
    let tag: u8 = match easing {
        Easing::Linear => 0,
        Easing::QuadIn => 1,
        Easing::QuadOut => 2,
        Easing::QuadInOut => 3,
        Easing::CubicIn => 4,
        Easing::CubicOut => 5,
        Easing::CubicInOut => 6,
    };
    h.write_u8(tag);
}

fn hash_loop_mode(h: &mut ReplayHasher, loop_mode: LoopMode) {
    let tag: u8 = match loop_mode {
        LoopMode::Once => 0,
        LoopMode::Loop => 1,
        LoopMode::PingPong => 2,
    };
    h.write_u8(tag);
}

/// Extends [`TweenValue`] with a hash body, implemented for the four
/// concrete `Tween<T>` instantiations (`MapPosition`/`ScreenPosition` via
/// the blanket [`Position2D`] impl, plus `Rotation`/`Scale`) so
/// `hash_tweens::<T>` stays generic instead of one bespoke function per `T`.
trait HashableTweenValue: TweenValue {
    fn hash_into(&self, h: &mut ReplayHasher);
}

impl<S: PositionSpace + std::fmt::Debug> HashableTweenValue for Position2D<S> {
    fn hash_into(&self, h: &mut ReplayHasher) {
        hash_vector2(h, self.pos);
    }
}

impl HashableTweenValue for Rotation {
    fn hash_into(&self, h: &mut ReplayHasher) {
        h.write_f32(self.degrees);
    }
}

impl HashableTweenValue for Scale {
    fn hash_into(&self, h: &mut ReplayHasher) {
        hash_vector2(h, self.scale);
    }
}

/// Hash `Option<&T>` behind a one-byte presence tag -- the same
/// tag-then-value shape every hashed component follows, factored out so
/// `hash_world_state` doesn't hand-repeat the `match { Some => .., None =>
/// h.write_u8(0) }` boilerplate once per component type.
fn hash_optional<T>(
    h: &mut ReplayHasher,
    value: Option<&T>,
    f: impl FnOnce(&mut ReplayHasher, &T),
) {
    match value {
        Some(v) => {
            h.write_u8(1);
            f(h, v);
        }
        None => h.write_u8(0),
    }
}

fn hash_tween<T: HashableTweenValue>(h: &mut ReplayHasher, e: EntityRef) {
    hash_optional(h, e.get::<Tween<T>>(), |h, t| {
        t.from.hash_into(h);
        t.to.hash_into(h);
        h.write_f32(t.duration);
        hash_easing(h, t.easing);
        hash_loop_mode(h, t.loop_mode);
        h.write_bool(t.playing);
        h.write_f32(t.time);
        h.write_bool(t.forward);
    });
}

/// Hash a set of `String` keys in sorted order, writing each key then its
/// value (via `write_value`). Used for `WorldSignals`' `FxHashMap` fields,
/// whose iteration order is not itself stable across runs.
fn hash_sorted_map<'a, V>(
    h: &mut ReplayHasher,
    map: impl Iterator<Item = (&'a String, V)>,
    mut write_value: impl FnMut(&mut ReplayHasher, V),
) {
    let mut entries: Vec<(&String, V)> = map.collect();
    entries.sort_by_key(|(k, _)| *k);
    h.write_u64(entries.len() as u64);
    for (key, value) in entries {
        h.write_str(key);
        write_value(h, value);
    }
}

fn hash_sorted_set<'a>(h: &mut ReplayHasher, set: impl Iterator<Item = &'a String>) {
    let mut entries: Vec<&String> = set.collect();
    entries.sort();
    h.write_u64(entries.len() as u64);
    for key in entries {
        h.write_str(key);
    }
}

/// Full-world deterministic hash: entities (sorted by `Entity::to_bits()`)
/// with a fixed, hand-assigned per-component list, then resources in a
/// fixed order. Every f32 is hashed via `.to_bits()` — never
/// epsilon-compared, matching the bit-exact scope of the whole determinism
/// roadmap (same build, same arch).
pub(crate) fn hash_world_state(world: &World) -> u64 {
    let mut h = ReplayHasher::new();

    // Collect the EntityRefs themselves (Copy) rather than just Entity ids
    // -- sorting these directly means the per-component hashing loop below
    // reuses the same archetype-location lookup iter_entities() already
    // did, instead of re-resolving each entity a second time via
    // world.entity(entity).
    let mut entities: Vec<EntityRef> = world.iter_entities().collect();
    entities.sort_by_key(|e| e.id().to_bits());

    h.write_u64(entities.len() as u64);
    for e in entities {
        h.write_u64(e.id().to_bits());

        hash_optional(&mut h, e.get::<MapPosition>(), |h, c| {
            hash_vector2(h, c.pos);
        });
        hash_optional(&mut h, e.get::<ScreenPosition>(), |h, c| {
            hash_vector2(h, c.pos);
        });
        hash_optional(&mut h, e.get::<RigidBody>(), |h, c| {
            hash_vector2(h, c.velocity);
            h.write_f32(c.friction);
            hash_optional(h, c.max_speed.as_ref(), |h, s| h.write_f32(*s));
            h.write_bool(c.frozen);
            // Forces are keyed by name (FxHashMap) -- hash sorted.
            hash_sorted_map(
                h,
                c.forces.iter(),
                |h, force: &crate::components::rigidbody::AccelerationForce| {
                    hash_vector2(h, force.value);
                    h.write_bool(force.enabled);
                },
            );
        });
        hash_optional(&mut h, e.get::<Rotation>(), |h, c| {
            h.write_f32(c.degrees);
        });
        hash_optional(&mut h, e.get::<Scale>(), |h, c| {
            hash_vector2(h, c.scale);
        });
        hash_optional(&mut h, e.get::<BoxCollider>(), |h, c| {
            hash_vector2(h, c.size);
            hash_vector2(h, c.offset);
            hash_vector2(h, c.origin);
        });
        hash_optional(&mut h, e.get::<Signals>(), |h, c| {
            hash_sorted_map(h, c.scalars.iter(), |h, v: &f32| h.write_f32(*v));
            hash_sorted_map(h, c.integers.iter(), |h, v: &i32| h.write_i32(*v));
            hash_sorted_set(h, c.flags.iter());
            hash_sorted_map(h, c.strings.iter(), |h, v: &String| h.write_str(v));
        });
        hash_optional(&mut h, e.get::<Phase<PhaseCallbackFns>>(), |h, c| {
            h.write_str(&c.current);
            h.write_f32(c.time_in_phase);
        });
        hash_optional(&mut h, e.get::<Timer<TimerCallback>>(), |h, c| {
            h.write_f32(c.duration);
            h.write_f32(c.elapsed);
        });
        hash_optional(&mut h, e.get::<Ttl>(), |h, c| {
            h.write_f32(c.remaining);
        });
        hash_optional(&mut h, e.get::<Animation>(), |h, c| {
            h.write_str(&c.animation_key);
            h.write_u64(c.frame_index as u64);
        });
        hash_tween::<MapPosition>(&mut h, e);
        hash_tween::<ScreenPosition>(&mut h, e);
        hash_tween::<Rotation>(&mut h, e);
        hash_tween::<Scale>(&mut h, e);
        hash_optional(&mut h, e.get::<GlobalTransform2D>(), |h, c| {
            hash_vector2(h, c.position);
            h.write_f32(c.rotation_degrees);
            hash_vector2(h, c.scale);
        });
        hash_optional(&mut h, e.get::<Group>(), |h, c| {
            h.write_str(&c.0);
        });
    }

    // Resources, fixed order. AppState deliberately excluded -- see this
    // module's doc comment.
    let signals = world.resource::<WorldSignals>();
    hash_sorted_map(&mut h, signals.scalars.iter(), |h, v: &f32| h.write_f32(*v));
    hash_sorted_map(&mut h, signals.integers.iter(), |h, v: &i32| {
        h.write_i32(*v)
    });
    hash_sorted_map(&mut h, signals.strings.iter(), |h, v: &String| {
        h.write_str(v)
    });
    hash_sorted_set(&mut h, signals.flags.iter());
    hash_sorted_map(&mut h, signals.entities.iter(), |h, v: &Entity| {
        h.write_u64(v.to_bits())
    });

    h.write_u64(world.resource::<WorldTime>().frame_count);
    h.write_u64(world.resource::<SimRng>().0.get_seed());

    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestWorld;

    #[test]
    fn hash_changes_when_map_position_moves() {
        let mut tw = TestWorld::builder().deterministic(1).build().unwrap();
        let entity = tw.world.spawn(MapPosition::new(0.0, 0.0)).id();
        let h1 = hash_world_state(&tw.world);
        tw.world
            .entity_mut(entity)
            .get_mut::<MapPosition>()
            .unwrap()
            .pos
            .x = 5.0;
        let h2 = hash_world_state(&tw.world);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_is_stable_for_unchanged_world() {
        let tw = TestWorld::builder().deterministic(1).build().unwrap();
        let h1 = hash_world_state(&tw.world);
        let h2 = hash_world_state(&tw.world);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_changes_when_sim_rng_advances() {
        let mut tw = TestWorld::builder().deterministic(1).build().unwrap();
        let h1 = hash_world_state(&tw.world);
        tw.world.resource_mut::<SimRng>().0.f32();
        let h2 = hash_world_state(&tw.world);
        assert_ne!(h1, h2);
    }
}
