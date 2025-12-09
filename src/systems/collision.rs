//! Collision detection and handling systems.
//!
//! This module provides two main systems:
//!
//! - [`collision_detector`] – pairwise AABB overlap checks, emits [`CollisionEvent`](crate::events::collision::CollisionEvent)
//! - [`collision_observer`] – receives collision events and dispatches to [`CollisionRule`](crate::components::collision::CollisionRule) callbacks
//!
//! # Collision Flow
//!
//! 1. `collision_detector` iterates all entity pairs with [`BoxCollider`](crate::components::boxcollider::BoxCollider) + [`MapPosition`](crate::components::mapposition::MapPosition)
//! 2. For each overlap, triggers a `CollisionEvent`
//! 3. `collision_observer` looks up matching `CollisionRule` components by [`Group`](crate::components::group::Group) names
//! 4. Invokes the rule's callback with both entities and a [`CollisionContext`](crate::components::collision::CollisionContext)
//!
//! # Defining Collision Rules
//!
//! Collision rules are defined in game code and spawned as entities:
//!
//! ```ignore
//! commands.spawn((
//!     CollisionRule::new("ball", "brick", ball_brick_callback as CollisionCallback),
//!     Group::new("collision_rules"),
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::components::collision::CollisionRule`] – defines collision handlers
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision::CollisionEvent`] – emitted on each collision

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::boxcollider::BoxCollider;
use crate::components::collision::{CollisionContext, CollisionRule};
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::events::audio::AudioCmd;
use crate::events::collision::CollisionEvent;
use crate::resources::worldsignals::WorldSignals;
// use crate::resources::worldtime::WorldTime; // Collisions are independent of time

/// Broad-phase pairwise overlap test with event emission.
///
/// Uses ECS `iter_combinations_mut()` to efficiently iterate unique pairs,
/// checks overlap, and triggers an event for each collision. Observers can
/// react to despawn, apply damage, or play sounds.
pub fn collision_detector(
    mut query: Query<(Entity, &mut MapPosition, &BoxCollider)>,
    mut commands: Commands,
) {
    // first we create a Vector of pairs of entities
    let mut pairs: Vec<(Entity, Entity)> = Vec::new();

    let mut combos = query.iter_combinations_mut();
    while let Some(
        [
            (entity_a, position_a, collider_a),
            (entity_b, position_b, collider_b),
        ],
    ) = combos.fetch_next()
    {
        /* if collider_a.overlaps(position_a.pos, collider_b, position_b.pos) {
            pairs.push((entity_a, entity_b));
        } */
        let rect_a = collider_a.as_rectangle(position_a.pos);
        let rect_b = collider_b.as_rectangle(position_b.pos);
        if rect_a.check_collision_recs(&rect_b) {
            pairs.push((entity_a, entity_b));
        }
    }

    // Trigger a CollisionEvent for each pair. Observers will run immediately when commands flush.
    for (entity_a, entity_b) in pairs {
        // println!(
        //     "Triggering CollisionEvent between {:?} and {:?}",
        //     entity_a, entity_b
        // );
        commands.trigger(CollisionEvent {
            a: entity_a,
            b: entity_b,
        });
    }
}

/// Global observer when a CollisionEvent is triggered.
///
#[derive(SystemParam)]
pub struct CollisionObserverParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub groups: Query<'w, 's, &'static Group>,
    pub rules: Query<'w, 's, &'static CollisionRule>,
    pub positions: Query<'w, 's, &'static mut MapPosition>,
    pub rigid_bodies: Query<'w, 's, &'static mut RigidBody>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub signals: Query<'w, 's, &'static mut Signals>,
    pub world_signals: ResMut<'w, WorldSignals>,
    pub audio_cmds: MessageWriter<'w, AudioCmd>,
}

pub fn collision_observer(trigger: On<CollisionEvent>, mut params: CollisionObserverParams) {
    let a = trigger.event().a;
    let b = trigger.event().b;

    //eprintln!("Collision detected: {:?} and {:?}", a, b);
    let ga = if let Ok(group) = params.groups.get(a) {
        group.name()
    } else {
        return;
    };
    let gb = if let Ok(group) = params.groups.get(b) {
        group.name()
    } else {
        return;
    };

    for rule in params.rules.iter() {
        if let Some((ent_a, ent_b)) = rule.match_and_order(a, b, ga, gb) {
            //eprintln!(
            //    "Collision rule matched for groups '{}' and '{}'",
            //    ga, gb
            //);
            let callback = rule.callback;
            let mut ctx = CollisionContext {
                commands: &mut params.commands,
                groups: &params.groups,
                positions: &mut params.positions,
                rigid_bodies: &mut params.rigid_bodies,
                box_colliders: &params.box_colliders,
                signals: &mut params.signals,
                world_signals: &mut params.world_signals,
                audio_cmds: &mut params.audio_cmds,
            };
            callback(ent_a, ent_b, &mut ctx);
            break;
        }
    }
}
