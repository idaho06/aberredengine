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

/// Rotate a 2D vector by `angle_degrees`.
fn rotate(v: Vector2, angle_degrees: f32) -> Vector2 {
    let rad = angle_degrees * std::f32::consts::PI / 180.0;
    let (sin, cos) = rad.sin_cos();
    Vector2 {
        x: v.x * cos - v.y * sin,
        y: v.x * sin + v.y * cos,
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
    roots: Query<
        (
            Entity,
            &MapPosition,
            Option<&Rotation>,
            Option<&Scale>,
            &Children,
        ),
        Without<ChildOf>,
    >,
    children_query: Query<
        (
            &MapPosition,
            Option<&Rotation>,
            Option<&Scale>,
            Option<&Children>,
        ),
        With<ChildOf>,
    >,
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
    children_query: &Query<
        (
            &MapPosition,
            Option<&Rotation>,
            Option<&Scale>,
            Option<&Children>,
        ),
        With<ChildOf>,
    >,
    globals: &mut Query<&mut GlobalTransform2D>,
    commands: &mut Commands,
) {
    for child_entity in children.iter() {
        let Ok((pos, rot, scale, maybe_grandchildren)) = children_query.get(child_entity) else {
            continue;
        };

        let local_rot = rot.map(|r| r.degrees).unwrap_or(0.0);
        let local_scale = scale.map(|s| s.scale).unwrap_or(Vector2 { x: 1.0, y: 1.0 });

        // Scale the child's local offset by the parent's scale, then rotate
        let scaled_offset = Vector2 {
            x: pos.pos.x * parent_gt.scale.x,
            y: pos.pos.y * parent_gt.scale.y,
        };
        let rotated_offset = rotate(scaled_offset, parent_gt.rotation_degrees);

        let child_gt = GlobalTransform2D {
            position: Vector2 {
                x: parent_gt.position.x + rotated_offset.x,
                y: parent_gt.position.y + rotated_offset.y,
            },
            rotation_degrees: parent_gt.rotation_degrees + local_rot,
            scale: Vector2 {
                x: parent_gt.scale.x * local_scale.x,
                y: parent_gt.scale.y * local_scale.y,
            },
        };

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
