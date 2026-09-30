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
use crate::math::Vec2;

use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::systems::transform_compose::{Transform2D, compose_transform};

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

/// Compose a child [`GlobalTransform2D`] from a parent world transform and the
/// child's local position, rotation, and scale.
fn compose_child_transform(
    parent_gt: &GlobalTransform2D,
    local_pos: Vec2,
    local_rot: f32,
    local_scale: Vec2,
) -> GlobalTransform2D {
    let result = compose_transform(
        Transform2D {
            pos: parent_gt.position,
            rot_degrees: parent_gt.rotation_degrees,
            scale: parent_gt.scale,
        },
        Transform2D {
            pos: local_pos,
            rot_degrees: local_rot,
            scale: local_scale,
        },
    );
    GlobalTransform2D {
        position: result.pos,
        rotation_degrees: result.rot_degrees,
        scale: result.scale,
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
    crate::tracy::tracy_span!("propagate_transforms");
    for (root_entity, pos, rot, scale, children) in roots.iter() {
        let root_gt = GlobalTransform2D {
            position: pos.pos,
            rotation_degrees: rot.map(|r| r.degrees).unwrap_or(0.0),
            scale: scale.map(|s| s.scale).unwrap_or(Vec2 { x: 1.0, y: 1.0 }),
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
        let local_scale = scale.map(|s| s.scale).unwrap_or(Vec2 { x: 1.0, y: 1.0 });

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
/// [`GlobalTransform2D`] via [`world_scope`](bevy_ecs::world::EntityWorldMut::world_scope) and composes it with the child's
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
    type Out = ();

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
            .unwrap_or(Vec2 { x: 0.0, y: 0.0 });
        let local_rot = entity.get::<Rotation>().map(|r| r.degrees).unwrap_or(0.0);
        let local_scale = entity
            .get::<Scale>()
            .map(|s| s.scale)
            .unwrap_or(Vec2 { x: 1.0, y: 1.0 });

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
                        .unwrap_or(Vec2 { x: 1.0, y: 1.0 });

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::approx_eq;
    use bevy_ecs::system::RunSystemOnce;

    fn tick_propagate(world: &mut World) {
        world
            .run_system_once(propagate_transforms)
            .expect("propagate_transforms should run");
    }

    #[test]
    fn propagate_root_entity_without_children() {
        let mut world = World::new();

        let root = world
            .spawn((
                MapPosition::new(100.0, 50.0),
                Rotation { degrees: 45.0 },
                Scale::new(2.0, 2.0),
                GlobalTransform2D::default(),
            ))
            .id();

        // Root has no Children — propagate_transforms only processes roots WITH Children.
        // A standalone entity with GlobalTransform2D but no Children is NOT processed
        // (it's not participating in a hierarchy). This is by design.
        tick_propagate(&mut world);

        // GlobalTransform2D stays at default because the root query requires &Children.
        let gt = world.get::<GlobalTransform2D>(root).unwrap();
        assert!(
            approx_eq(gt.position.x, 0.0),
            "Standalone entity should not be processed by propagate_transforms"
        );
    }

    #[test]
    fn propagate_single_child_position_only() {
        let mut world = World::new();

        let parent = world
            .spawn((MapPosition::new(100.0, 100.0), GlobalTransform2D::default()))
            .id();

        let child = world
            .spawn((
                MapPosition::new(40.0, 0.0),
                ChildOf(parent),
                GlobalTransform2D::default(),
            ))
            .id();

        // Flush so Bevy populates Children on parent
        world.flush();

        tick_propagate(&mut world);

        let gt = world.get::<GlobalTransform2D>(child).unwrap();
        assert!(
            approx_eq(gt.position.x, 140.0),
            "Child world X: expected 140, got {}",
            gt.position.x
        );
        assert!(
            approx_eq(gt.position.y, 100.0),
            "Child world Y: expected 100, got {}",
            gt.position.y
        );
        assert!(approx_eq(gt.rotation_degrees, 0.0));
        assert!(approx_eq(gt.scale.x, 1.0));
        assert!(approx_eq(gt.scale.y, 1.0));
    }

    #[test]
    fn propagate_child_inherits_parent_rotation() {
        let mut world = World::new();

        let parent = world
            .spawn((
                MapPosition::new(100.0, 100.0),
                Rotation { degrees: 90.0 },
                GlobalTransform2D::default(),
            ))
            .id();

        let child = world
            .spawn((
                MapPosition::new(40.0, 0.0),
                ChildOf(parent),
                GlobalTransform2D::default(),
            ))
            .id();

        world.flush();
        tick_propagate(&mut world);

        let gt = world.get::<GlobalTransform2D>(child).unwrap();
        // Local offset (40, 0) rotated 90deg CW => (0, 40)
        assert!(
            approx_eq(gt.position.x, 100.0),
            "Child world X: expected 100, got {}",
            gt.position.x
        );
        assert!(
            approx_eq(gt.position.y, 140.0),
            "Child world Y: expected 140, got {}",
            gt.position.y
        );
        assert!(
            approx_eq(gt.rotation_degrees, 90.0),
            "Child world rotation: expected 90, got {}",
            gt.rotation_degrees
        );
    }

    #[test]
    fn propagate_child_inherits_parent_scale() {
        let mut world = World::new();

        let parent = world
            .spawn((
                MapPosition::new(100.0, 100.0),
                Scale::new(2.0, 2.0),
                GlobalTransform2D::default(),
            ))
            .id();

        let child = world
            .spawn((
                MapPosition::new(40.0, 0.0),
                ChildOf(parent),
                GlobalTransform2D::default(),
            ))
            .id();

        world.flush();
        tick_propagate(&mut world);

        let gt = world.get::<GlobalTransform2D>(child).unwrap();
        // Offset (40, 0) scaled by (2, 2) => (80, 0)
        assert!(
            approx_eq(gt.position.x, 180.0),
            "Child world X: expected 180, got {}",
            gt.position.x
        );
        assert!(
            approx_eq(gt.position.y, 100.0),
            "Child world Y: expected 100, got {}",
            gt.position.y
        );
        assert!(approx_eq(gt.scale.x, 2.0));
        assert!(approx_eq(gt.scale.y, 2.0));
    }

    #[test]
    fn propagate_child_inherits_rotation_and_scale() {
        let mut world = World::new();

        let parent = world
            .spawn((
                MapPosition::new(0.0, 0.0),
                Rotation { degrees: 90.0 },
                Scale::new(2.0, 1.0),
                GlobalTransform2D::default(),
            ))
            .id();

        let child = world
            .spawn((
                MapPosition::new(10.0, 0.0),
                ChildOf(parent),
                GlobalTransform2D::default(),
            ))
            .id();

        world.flush();
        tick_propagate(&mut world);

        let gt = world.get::<GlobalTransform2D>(child).unwrap();
        // Offset (10, 0) scaled by (2, 1) => (20, 0), rotated 90deg => (0, 20)
        assert!(
            approx_eq(gt.position.x, 0.0),
            "Child world X: expected 0, got {}",
            gt.position.x
        );
        assert!(
            approx_eq(gt.position.y, 20.0),
            "Child world Y: expected 20, got {}",
            gt.position.y
        );
        assert!(approx_eq(gt.rotation_degrees, 90.0));
        assert!(approx_eq(gt.scale.x, 2.0));
        assert!(approx_eq(gt.scale.y, 1.0));
    }

    #[test]
    fn propagate_chain_grandchild() {
        let mut world = World::new();

        let root = world
            .spawn((MapPosition::new(100.0, 0.0), GlobalTransform2D::default()))
            .id();

        let child = world
            .spawn((
                MapPosition::new(50.0, 0.0),
                ChildOf(root),
                GlobalTransform2D::default(),
            ))
            .id();

        let grandchild = world
            .spawn((
                MapPosition::new(10.0, 0.0),
                ChildOf(child),
                GlobalTransform2D::default(),
            ))
            .id();

        world.flush();
        tick_propagate(&mut world);

        // Root: world pos = (100, 0)
        let root_gt = world.get::<GlobalTransform2D>(root).unwrap();
        assert!(
            approx_eq(root_gt.position.x, 100.0),
            "Root X: expected 100, got {}",
            root_gt.position.x
        );

        // Child: world pos = (100 + 50, 0) = (150, 0)
        let child_gt = world.get::<GlobalTransform2D>(child).unwrap();
        assert!(
            approx_eq(child_gt.position.x, 150.0),
            "Child X: expected 150, got {}",
            child_gt.position.x
        );

        // Grandchild: world pos = (150 + 10, 0) = (160, 0)
        let gc_gt = world.get::<GlobalTransform2D>(grandchild).unwrap();
        assert!(
            approx_eq(gc_gt.position.x, 160.0),
            "Grandchild X: expected 160, got {}",
            gc_gt.position.x
        );
        assert!(approx_eq(gc_gt.position.y, 0.0));
    }

    #[test]
    fn propagate_no_crash_on_empty_world() {
        let mut world = World::new();
        // No entities at all — should not panic
        tick_propagate(&mut world);
    }

    #[test]
    fn propagate_root_with_children_gets_correct_gt() {
        let mut world = World::new();

        let parent = world
            .spawn((
                MapPosition::new(50.0, 25.0),
                Rotation { degrees: 30.0 },
                Scale::new(3.0, 2.0),
                GlobalTransform2D::default(),
            ))
            .id();

        // Need at least one child for parent to have Children component
        world.spawn((
            MapPosition::new(0.0, 0.0),
            ChildOf(parent),
            GlobalTransform2D::default(),
        ));

        world.flush();
        tick_propagate(&mut world);

        // Root's GlobalTransform2D should reflect its own local values
        let gt = world.get::<GlobalTransform2D>(parent).unwrap();
        assert!(
            approx_eq(gt.position.x, 50.0),
            "Root GT X: expected 50, got {}",
            gt.position.x
        );
        assert!(
            approx_eq(gt.position.y, 25.0),
            "Root GT Y: expected 25, got {}",
            gt.position.y
        );
        assert!(approx_eq(gt.rotation_degrees, 30.0));
        assert!(approx_eq(gt.scale.x, 3.0));
        assert!(approx_eq(gt.scale.y, 2.0));
    }

    #[test]
    fn propagate_child_with_own_rotation_and_scale() {
        let mut world = World::new();

        let parent = world
            .spawn((
                MapPosition::new(0.0, 0.0),
                Rotation { degrees: 0.0 },
                Scale::new(2.0, 2.0),
                GlobalTransform2D::default(),
            ))
            .id();

        let child = world
            .spawn((
                MapPosition::new(10.0, 0.0),
                Rotation { degrees: 45.0 },
                Scale::new(0.5, 0.5),
                ChildOf(parent),
                GlobalTransform2D::default(),
            ))
            .id();

        world.flush();
        tick_propagate(&mut world);

        let gt = world.get::<GlobalTransform2D>(child).unwrap();
        // Offset (10, 0) scaled by parent (2, 2) => (20, 0), parent rot=0 => (20, 0)
        assert!(
            approx_eq(gt.position.x, 20.0),
            "Child X: expected 20, got {}",
            gt.position.x
        );
        assert!(approx_eq(gt.position.y, 0.0));
        // World rotation = parent 0 + child 45 = 45
        assert!(approx_eq(gt.rotation_degrees, 45.0));
        // World scale = parent (2,2) * child (0.5, 0.5) = (1.0, 1.0)
        assert!(approx_eq(gt.scale.x, 1.0));
        assert!(approx_eq(gt.scale.y, 1.0));
    }

    #[test]
    fn propagate_inserts_missing_globaltransform2d_via_commands() {
        let mut world = World::new();

        let parent = world.spawn((MapPosition::new(100.0, 0.0),)).id();

        let child = world
            .spawn((MapPosition::new(10.0, 0.0), ChildOf(parent)))
            .id();

        world.flush();

        // Neither entity has GlobalTransform2D yet.
        // First tick: system inserts via Commands (deferred).
        tick_propagate(&mut world);

        // After the schedule runs, commands should have been applied.
        assert!(
            world.get::<GlobalTransform2D>(parent).is_some(),
            "Parent should have GlobalTransform2D inserted by commands"
        );
        assert!(
            world.get::<GlobalTransform2D>(child).is_some(),
            "Child should have GlobalTransform2D inserted by commands"
        );

        // The values should be correct after the commands are applied.
        // Run a second tick so the system can read the now-present components.
        tick_propagate(&mut world);

        let child_gt = world.get::<GlobalTransform2D>(child).unwrap();
        assert!(
            approx_eq(child_gt.position.x, 110.0),
            "Child world X after second tick: expected 110, got {}",
            child_gt.position.x
        );
    }

    // --- cleanup_orphaned_global_transforms ---

    #[test]
    fn cleanup_removes_gt_from_entity_with_no_children_and_no_childof() {
        let mut world = World::new();

        // Entity with a GlobalTransform2D but no hierarchy relationship
        let entity = world
            .spawn((
                MapPosition::new(10.0, 20.0),
                GlobalTransform2D {
                    position: Vec2 { x: 999.0, y: 999.0 },
                    rotation_degrees: 0.0,
                    scale: Vec2 { x: 1.0, y: 1.0 },
                },
            ))
            .id();

        // Run cleanup only (no hierarchy, so propagate_transforms does nothing)
        world
            .run_system_once(cleanup_orphaned_global_transforms)
            .expect("cleanup_orphaned_global_transforms should run");

        assert!(
            world.get::<GlobalTransform2D>(entity).is_none(),
            "GT should be removed from standalone entity with no Children and no ChildOf"
        );
    }

    #[test]
    fn cleanup_preserves_gt_on_current_hierarchy_root() {
        let mut world = World::new();

        // Parent with a child — has Children, so cleanup must leave its GT alone
        let parent = world
            .spawn((MapPosition::new(50.0, 0.0), GlobalTransform2D::default()))
            .id();
        world.spawn((MapPosition::new(0.0, 0.0), ChildOf(parent)));
        world.flush(); // Bevy populates Children on parent

        assert!(
            world.get::<Children>(parent).is_some(),
            "Parent should have Children after flush"
        );

        world
            .run_system_once(cleanup_orphaned_global_transforms)
            .expect("cleanup_orphaned_global_transforms should run");

        assert!(
            world.get::<GlobalTransform2D>(parent).is_some(),
            "GT should be preserved on entity that still has Children"
        );
    }

    #[test]
    fn cleanup_preserves_gt_on_child_entity() {
        let mut world = World::new();

        // Child entity: has both ChildOf and GT — cleanup must not touch it
        let parent = world.spawn(MapPosition::new(0.0, 0.0)).id();
        let child = world
            .spawn((
                MapPosition::new(10.0, 0.0),
                ChildOf(parent),
                GlobalTransform2D::default(),
            ))
            .id();
        world.flush();

        world
            .run_system_once(cleanup_orphaned_global_transforms)
            .expect("cleanup_orphaned_global_transforms should run");

        assert!(
            world.get::<GlobalTransform2D>(child).is_some(),
            "GT should be preserved on child entity (has ChildOf)"
        );
    }

    #[test]
    fn cleanup_does_not_affect_entity_without_gt() {
        let mut world = World::new();

        // Standalone entity with no GT — cleanup should leave it untouched
        let entity = world.spawn(MapPosition::new(5.0, 5.0)).id();

        world
            .run_system_once(cleanup_orphaned_global_transforms)
            .expect("cleanup_orphaned_global_transforms should run");

        // MapPosition should still be there, and no GT should have appeared
        assert!(world.get::<GlobalTransform2D>(entity).is_none());
        let pos = world.get::<MapPosition>(entity).unwrap();
        assert!(approx_eq(pos.pos.x, 5.0));
    }

    #[test]
    fn cleanup_removes_all_orphaned_gt_entities() {
        let mut world = World::new();

        // Spawn three standalone entities each with a stale GT
        let entities: Vec<Entity> = (0..3)
            .map(|i| {
                world
                    .spawn((
                        MapPosition::new(i as f32 * 10.0, 0.0),
                        GlobalTransform2D {
                            position: Vec2 {
                                x: 999.0 + i as f32,
                                y: 999.0,
                            },
                            rotation_degrees: 0.0,
                            scale: Vec2 { x: 1.0, y: 1.0 },
                        },
                    ))
                    .id()
            })
            .collect();

        world
            .run_system_once(cleanup_orphaned_global_transforms)
            .expect("cleanup_orphaned_global_transforms should run");

        for entity in &entities {
            assert!(
                world.get::<GlobalTransform2D>(*entity).is_none(),
                "GT should be removed from all orphaned entities"
            );
        }
    }
}
