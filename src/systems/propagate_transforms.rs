//! Transform propagation for parent-child entity hierarchies.
//!
//! Computes [`GlobalTransform2D`] for every entity participating in a hierarchy
//! (root parents with [`Children`] and descendants with [`ChildOf`]).
//!
//! # Schedule position
//!
//! Should run **after** all systems that mutate local transforms (movement,
//! tweens) and **before** collision detection and rendering so that downstream
//! systems see up-to-date world positions.

use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::*;
use raylib::math::Vector2;

use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;

type RootsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MapPosition,
        Option<&'static Rotation>,
        Option<&'static Scale>,
        &'static Children,
    ),
    Without<ChildOf>,
>;

type ChildrenQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static MapPosition,
        Option<&'static Rotation>,
        Option<&'static Scale>,
        Option<&'static Children>,
    ),
    With<ChildOf>,
>;

/// Rotate a 2D vector by `angle_degrees`.
pub(crate) fn rotate(v: Vector2, angle_degrees: f32) -> Vector2 {
    let rad = angle_degrees * std::f32::consts::PI / 180.0;
    let (sin, cos) = rad.sin_cos();
    Vector2 {
        x: v.x * cos - v.y * sin,
        y: v.x * sin + v.y * cos,
    }
}

/// Compose a child [`GlobalTransform2D`] from a parent world transform and the
/// child's local position, rotation, and scale.
fn compose_child_transform(
    parent_gt: &GlobalTransform2D,
    local_pos: Vector2,
    local_rot: f32,
    local_scale: Vector2,
) -> GlobalTransform2D {
    let scaled_offset = Vector2 {
        x: local_pos.x * parent_gt.scale.x,
        y: local_pos.y * parent_gt.scale.y,
    };
    let rotated_offset = rotate(scaled_offset, parent_gt.rotation_degrees);
    GlobalTransform2D {
        position: Vector2 {
            x: parent_gt.position.x + rotated_offset.x,
            y: parent_gt.position.y + rotated_offset.y,
        },
        rotation_degrees: parent_gt.rotation_degrees + local_rot,
        scale: Vector2 {
            x: parent_gt.scale.x * local_scale.x,
            y: parent_gt.scale.y * local_scale.y,
        },
    }
}

/// Propagate transforms from root parents down through the hierarchy.
///
/// For each root entity (has [`Children`] but no [`ChildOf`]):
/// 1. Compute its [`GlobalTransform2D`] from local components.
/// 2. Recursively traverse children, composing transforms at each level.
///
/// Entities that already have a `GlobalTransform2D` are updated in place.
/// Entities missing the component get it inserted via deferred [`Commands`]
/// (visible next frame).
pub fn propagate_transforms(
    roots: RootsQuery,
    children_query: ChildrenQuery,
    mut globals: Query<&mut GlobalTransform2D>,
    mut commands: Commands,
) {
    for (root_entity, pos, rot, scale, children) in roots.iter() {
        let root_gt = GlobalTransform2D {
            position: pos.pos,
            rotation_degrees: rot.map(|r| r.degrees).unwrap_or(0.0),
            scale: scale.map(|s| s.scale).unwrap_or(Vector2 { x: 1.0, y: 1.0 }),
        };

        // Update or insert root's GlobalTransform2D
        if let Ok(mut gt) = globals.get_mut(root_entity) {
            *gt = root_gt;
        } else {
            commands.entity(root_entity).insert(root_gt);
        }

        // Recurse into children
        propagate_children(
            &root_gt,
            children,
            &children_query,
            &mut globals,
            &mut commands,
        );
    }
}

fn propagate_children(
    parent_gt: &GlobalTransform2D,
    children: &Children,
    children_query: &ChildrenQuery,
    globals: &mut Query<&mut GlobalTransform2D>,
    commands: &mut Commands,
) {
    for child_entity in children.iter() {
        let Ok((pos, rot, scale, maybe_grandchildren)) = children_query.get(child_entity) else {
            continue;
        };

        let local_rot = rot.map(|r| r.degrees).unwrap_or(0.0);
        let local_scale = scale.map(|s| s.scale).unwrap_or(Vector2 { x: 1.0, y: 1.0 });

        let child_gt = compose_child_transform(parent_gt, pos.pos, local_rot, local_scale);

        if let Ok(mut gt) = globals.get_mut(child_entity) {
            *gt = child_gt;
        } else {
            commands.entity(child_entity).insert(child_gt);
        }

        // Recurse into grandchildren
        if let Some(grandchildren) = maybe_grandchildren {
            propagate_children(&child_gt, grandchildren, children_query, globals, commands);
        }
    }
}

/// Remove stale [`GlobalTransform2D`] from entities that are no longer part
/// of any hierarchy.
///
/// When a root entity loses its last child, Bevy removes [`Children`] but
/// leaves its [`GlobalTransform2D`] in place. [`propagate_transforms`] stops
/// updating it, so `resolve_world_pos` returns the frozen world position
/// instead of the live [`MapPosition`]. This system removes the orphaned
/// component so that standalone entities always resolve to [`MapPosition`].
///
/// Must run **after** [`propagate_transforms`] and **before** collision
/// detection.
#[allow(clippy::type_complexity)]
pub fn cleanup_orphaned_global_transforms(
    mut commands: Commands,
    query: Query<Entity, (With<GlobalTransform2D>, Without<Children>, Without<ChildOf>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).remove::<GlobalTransform2D>();
    }
}

/// [`EntityCommand`] that computes the correct initial [`GlobalTransform2D`]
/// for a newly spawned child entity.
///
/// Queue it via [`EntityCommands::queue`] immediately after giving an entity
/// its [`ChildOf`] component. It reads the parent's current
/// [`GlobalTransform2D`] via [`world_scope`] and composes it with the child's
/// local [`MapPosition`], [`Rotation`], and [`Scale`] to produce the correct
/// world-space transform on the very first frame the entity exists — avoiding
/// the one-frame flash at world origin caused by `GlobalTransform2D::default()`.
///
/// If the parent has no [`GlobalTransform2D`] yet, the command attempts to
/// synthesize one from the parent's local transform when the parent is a
/// standalone root. If the parent is itself a child and still lacks a global
/// transform, the command is a no-op and [`propagate_transforms`] will insert
/// the component on the next frame.
pub struct ComputeInitialGlobalTransform;

impl bevy_ecs::system::EntityCommand for ComputeInitialGlobalTransform {
    fn apply(self, mut entity: bevy_ecs::world::EntityWorldMut<'_>) {
        // Resolve parent — bail if entity has no ChildOf
        let Some(parent_entity) = entity.get::<ChildOf>().map(|c| c.parent()) else {
            return;
        };

        // Extract child's local components. All are Copy so the borrows end
        // immediately and don't conflict with the world_scope call below.
        let pos = entity
            .get::<MapPosition>()
            .map(|p| p.pos)
            .unwrap_or(Vector2 { x: 0.0, y: 0.0 });
        let local_rot = entity.get::<Rotation>().map(|r| r.degrees).unwrap_or(0.0);
        let local_scale = entity
            .get::<Scale>()
            .map(|s| s.scale)
            .unwrap_or(Vector2 { x: 1.0, y: 1.0 });

        // Read the parent's world transform. world_scope gives temporary &mut
        // World access; reading a *different* entity is safe.
        //
        // Prefer an existing GlobalTransform2D. If the parent is a standalone
        // root without one yet, synthesize it from local components so the
        // first child attached to a previously-standalone entity still renders
        // correctly on its first frame.
        let Some(parent_gt) = entity.world_scope(|world| {
            world
                .get::<GlobalTransform2D>(parent_entity)
                .copied()
                .or_else(|| {
                    if world.get::<ChildOf>(parent_entity).is_some() {
                        return None;
                    }

                    let parent_pos = world.get::<MapPosition>(parent_entity)?.pos;
                    let parent_rot = world
                        .get::<Rotation>(parent_entity)
                        .map(|r| r.degrees)
                        .unwrap_or(0.0);
                    let parent_scale = world
                        .get::<Scale>(parent_entity)
                        .map(|s| s.scale)
                        .unwrap_or(Vector2 { x: 1.0, y: 1.0 });

                    Some(GlobalTransform2D {
                        position: parent_pos,
                        rotation_degrees: parent_rot,
                        scale: parent_scale,
                    })
                })
        }) else {
            // Parent still has no resolvable world transform — leave child
            // without GT; propagate_transforms will handle both next frame.
            return;
        };

        entity.insert(compose_child_transform(
            &parent_gt,
            pos,
            local_rot,
            local_scale,
        ));
    }
}
