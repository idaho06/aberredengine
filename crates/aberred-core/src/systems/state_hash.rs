//! Per-tick deterministic world-state hash.
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
//! **Blind spot:** `AppState` (`Box<dyn Any>`) is not hashed. A game whose
//! deterministic behavior depends solely on `AppState` contents (never
//! reflected into a hashed component/resource) will not have that dependency
//! caught by replay divergence tests.

use crate::math::Vec2;
use bevy_ecs::prelude::*;

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
use crate::components::timer::Timer;
use crate::components::ttl::Ttl;
use crate::components::tween::{Easing, LoopMode, Tween, TweenValue};
use crate::protocol::replay::ReplayHasher;
use crate::resources::sim_rng::SimRng;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

fn hash_vector2(h: &mut ReplayHasher, v: Vec2) {
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
pub fn hash_world_state(world: &World) -> u64 {
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
        hash_optional(&mut h, e.get::<Timer>(), |h, c| {
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

    /// Minimal `World` carrying just the resources `hash_world_state` reads
    /// unconditionally (`WorldSignals`/`WorldTime`/`SimRng`) -- not the full
    /// `TestWorld` harness, which lives in the facade crate
    /// (`aberredengine::test_support`) and cannot be a dependency of
    /// `aberred-core`.
    fn minimal_world(seed: u64) -> World {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world.insert_resource(WorldTime::default());
        world.insert_resource(SimRng::from_seed(seed));
        world
    }

    /// A world with one entity carrying every component `hash_world_state` hashes, plus
    /// populated WorldSignals, so each mutation below starts from the same full baseline.
    fn full_world() -> (World, Entity) {
        let mut world = minimal_world(7);
        let mut rb = RigidBody::with_physics(0.5, Some(9.0));
        rb.velocity = Vec2::new(1.0, 2.0);
        rb.add_force_with_state("gravity", Vec2::new(0.0, 10.0), true);
        let mut signals = Signals::default();
        signals.set_scalar("s", 1.0);
        signals.set_integer("i", 1);
        signals.set_flag("f");
        signals.set_string("n", "a");
        let mut phases = rustc_hash::FxHashMap::default();
        phases.insert("idle".to_string(), PhaseCallbackFns::default());
        phases.insert("run".to_string(), PhaseCallbackFns::default());
        let e = world
            .spawn((
                MapPosition::new(1.0, 2.0),
                ScreenPosition::new(3.0, 4.0),
                rb,
                Rotation { degrees: 10.0 },
                Scale::new(1.0, 1.0),
                BoxCollider::new(8.0, 8.0),
                signals,
                Phase::new("idle", phases),
                Timer::new(1.0),
                Ttl::new(5.0),
                Animation::new("walk"),
                GlobalTransform2D {
                    position: Vec2::new(1.0, 2.0),
                    rotation_degrees: 0.0,
                    scale: Vec2::ONE,
                },
                Group::new("enemy"),
            ))
            .id();
        world.entity_mut(e).insert((
            Tween::new(MapPosition::new(0.0, 0.0), MapPosition::new(5.0, 5.0), 1.0),
            Tween::new(
                ScreenPosition::new(0.0, 0.0),
                ScreenPosition::new(5.0, 5.0),
                1.0,
            ),
            Tween::new(Rotation { degrees: 0.0 }, Rotation { degrees: 90.0 }, 1.0),
            Tween::new(Scale::new(1.0, 1.0), Scale::new(2.0, 2.0), 1.0),
        ));
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_scalar("ws", 1.0);
        ws.set_integer("wi", 1);
        ws.set_string("wn", "a");
        ws.set_flag("wf");
        ws.set_entity("we", e);
        (world, e)
    }

    #[test]
    fn every_hashed_field_changes_the_hash() {
        type Mutation = fn(&mut World, Entity);
        let mutations: &[(&str, Mutation)] = &[
            ("MapPosition", |w, e| {
                w.get_mut::<MapPosition>(e).unwrap().pos.x += 1.0
            }),
            ("ScreenPosition", |w, e| {
                w.get_mut::<ScreenPosition>(e).unwrap().pos.y += 1.0
            }),
            ("RigidBody.velocity", |w, e| {
                w.get_mut::<RigidBody>(e).unwrap().velocity.x += 1.0
            }),
            ("RigidBody.friction", |w, e| {
                w.get_mut::<RigidBody>(e).unwrap().friction = 0.9
            }),
            ("RigidBody.max_speed", |w, e| {
                w.get_mut::<RigidBody>(e).unwrap().max_speed = None
            }),
            ("RigidBody.frozen", |w, e| {
                w.get_mut::<RigidBody>(e).unwrap().frozen = true
            }),
            ("RigidBody.force value", |w, e| {
                w.get_mut::<RigidBody>(e)
                    .unwrap()
                    .set_force_value("gravity", Vec2::new(0.0, 11.0));
            }),
            ("RigidBody.force enabled", |w, e| {
                w.get_mut::<RigidBody>(e)
                    .unwrap()
                    .set_force_enabled("gravity", false);
            }),
            ("Rotation", |w, e| {
                w.get_mut::<Rotation>(e).unwrap().degrees += 1.0
            }),
            ("Scale", |w, e| {
                w.get_mut::<Scale>(e).unwrap().scale.x += 1.0
            }),
            ("BoxCollider.size", |w, e| {
                w.get_mut::<BoxCollider>(e).unwrap().size.x += 1.0
            }),
            ("BoxCollider.offset", |w, e| {
                w.get_mut::<BoxCollider>(e).unwrap().offset.x += 1.0
            }),
            ("BoxCollider.origin", |w, e| {
                w.get_mut::<BoxCollider>(e).unwrap().origin.x += 1.0
            }),
            ("Signals.scalar", |w, e| {
                w.get_mut::<Signals>(e).unwrap().set_scalar("s", 2.0)
            }),
            ("Signals.integer", |w, e| {
                w.get_mut::<Signals>(e).unwrap().set_integer("i", 2)
            }),
            ("Signals.flag", |w, e| {
                w.get_mut::<Signals>(e).unwrap().set_flag("g")
            }),
            ("Signals.string", |w, e| {
                w.get_mut::<Signals>(e).unwrap().set_string("n", "b")
            }),
            ("Phase.current", |w, e| {
                w.get_mut::<Phase<PhaseCallbackFns>>(e).unwrap().current = "run".to_string();
            }),
            ("Phase.time_in_phase", |w, e| {
                w.get_mut::<Phase<PhaseCallbackFns>>(e)
                    .unwrap()
                    .time_in_phase = 0.5;
            }),
            ("Timer.duration", |w, e| {
                w.get_mut::<Timer>(e).unwrap().duration = 2.0
            }),
            ("Timer.elapsed", |w, e| {
                w.get_mut::<Timer>(e).unwrap().elapsed = 0.5
            }),
            ("Ttl", |w, e| w.get_mut::<Ttl>(e).unwrap().remaining = 4.0),
            ("Animation.key", |w, e| {
                w.get_mut::<Animation>(e).unwrap().animation_key = "run".into()
            }),
            ("Animation.frame", |w, e| {
                w.get_mut::<Animation>(e).unwrap().frame_index = 1
            }),
            ("Tween<MapPosition>.to", |w, e| {
                w.get_mut::<Tween<MapPosition>>(e).unwrap().to = MapPosition::new(6.0, 5.0);
            }),
            ("Tween<MapPosition>.time", |w, e| {
                w.get_mut::<Tween<MapPosition>>(e).unwrap().time = 0.1
            }),
            ("Tween<MapPosition>.playing", |w, e| {
                w.get_mut::<Tween<MapPosition>>(e).unwrap().playing = false;
            }),
            ("Tween<MapPosition>.forward", |w, e| {
                w.get_mut::<Tween<MapPosition>>(e).unwrap().forward = false;
            }),
            ("Tween<MapPosition>.easing", |w, e| {
                w.get_mut::<Tween<MapPosition>>(e).unwrap().easing = Easing::CubicIn;
            }),
            ("Tween<MapPosition>.loop_mode", |w, e| {
                w.get_mut::<Tween<MapPosition>>(e).unwrap().loop_mode = LoopMode::PingPong;
            }),
            ("Tween<ScreenPosition>.from", |w, e| {
                w.get_mut::<Tween<ScreenPosition>>(e).unwrap().from = ScreenPosition::new(1.0, 0.0);
            }),
            ("Tween<Rotation>.duration", |w, e| {
                w.get_mut::<Tween<Rotation>>(e).unwrap().duration = 2.0
            }),
            ("Tween<Scale>.to", |w, e| {
                w.get_mut::<Tween<Scale>>(e).unwrap().to = Scale::new(3.0, 2.0);
            }),
            ("GlobalTransform2D.position", |w, e| {
                w.get_mut::<GlobalTransform2D>(e).unwrap().position.x += 1.0;
            }),
            ("GlobalTransform2D.rotation", |w, e| {
                w.get_mut::<GlobalTransform2D>(e).unwrap().rotation_degrees = 5.0;
            }),
            ("GlobalTransform2D.scale", |w, e| {
                w.get_mut::<GlobalTransform2D>(e).unwrap().scale.x = 2.0;
            }),
            ("Group", |w, e| {
                w.get_mut::<Group>(e).unwrap().0 = "ally".into()
            }),
            ("component removed", |w, e| {
                w.entity_mut(e).remove::<Ttl>();
            }),
            ("extra entity", |w, _| {
                w.spawn_empty();
            }),
            ("WorldSignals.scalar", |w, _| {
                w.resource_mut::<WorldSignals>().set_scalar("ws", 2.0)
            }),
            ("WorldSignals.integer", |w, _| {
                w.resource_mut::<WorldSignals>().set_integer("wi", 2)
            }),
            ("WorldSignals.string", |w, _| {
                w.resource_mut::<WorldSignals>().set_string("wn", "b")
            }),
            ("WorldSignals.flag", |w, _| {
                w.resource_mut::<WorldSignals>().set_flag("wg")
            }),
            ("WorldSignals.entity", |w, _| {
                let other = w.spawn_empty().id();
                w.resource_mut::<WorldSignals>().set_entity("we", other);
                w.despawn(other);
            }),
            ("WorldTime.frame_count", |w, _| {
                w.resource_mut::<WorldTime>().frame_count += 1
            }),
        ];

        let (world, _) = full_world();
        let baseline = hash_world_state(&world);
        for (name, mutate) in mutations {
            let (mut world, e) = full_world();
            mutate(&mut world, e);
            assert_ne!(
                hash_world_state(&world),
                baseline,
                "{name} must change the hash"
            );
        }
    }

    #[test]
    fn hash_is_independent_of_hash_map_insertion_order() {
        let keys: Vec<String> = (0..32).map(|i| format!("k{i}")).collect();
        let build = |order: &[String]| {
            let mut world = minimal_world(1);
            let mut signals = Signals::default();
            let mut ws = WorldSignals::default();
            for k in order {
                signals.set_scalar(k.clone(), 1.0);
                signals.set_flag(k.clone());
                ws.set_integer(k.clone(), 1);
                ws.set_flag(k.clone());
            }
            world.insert_resource(ws);
            world.spawn(signals);
            hash_world_state(&world)
        };
        let reversed: Vec<String> = keys.iter().rev().cloned().collect();
        assert_eq!(build(&keys), build(&reversed));
    }

    #[test]
    fn string_boundaries_are_part_of_the_hash() {
        let with_flags = |flags: &[&str]| {
            let mut world = minimal_world(1);
            for f in flags {
                world.resource_mut::<WorldSignals>().set_flag(*f);
            }
            hash_world_state(&world)
        };
        assert_ne!(with_flags(&["ab", "c"]), with_flags(&["a", "bc"]));
    }

    #[test]
    fn hash_is_stable_for_unchanged_world() {
        let world = minimal_world(1);
        let h1 = hash_world_state(&world);
        let h2 = hash_world_state(&world);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_changes_when_sim_rng_advances() {
        let mut world = minimal_world(1);
        let h1 = hash_world_state(&world);
        world.resource_mut::<SimRng>().0.f32();
        let h2 = hash_world_state(&world);
        assert_ne!(h1, h2);
    }
}
