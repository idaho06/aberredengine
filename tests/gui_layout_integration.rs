//! Integration tests for the GUI child layout system (`gui_layout_system`).
//!
//! Mirrors the harness pattern in `tests/hierarchy_integration.rs`.
//!
//! # Usage
//!
//! ```sh
//! cargo test --test gui_layout_integration
//! ```

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use raylib::math::Vector2;

use aberredengine::components::globaltransform2d::GlobalTransform2D;
use aberredengine::components::guioffset::GuiOffset;
use aberredengine::components::screenposition::ScreenPosition;
use aberredengine::systems::gui_layout::gui_layout_system;
use aberredengine::systems::propagate_transforms::propagate_transforms;

const EPSILON: f32 = 1e-4;

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < EPSILON
}

fn tick_gui_layout(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(gui_layout_system);
    schedule.run(world);
}

/// Spawn a `ScreenPosition` parent plus one `GuiOffset` child under it,
/// flushed and ready for a `tick_gui_layout` call. Returns `(parent, child)`.
fn spawn_window_and_child(
    world: &mut World,
    parent_pos: (f32, f32),
    offset: (f32, f32),
) -> (Entity, Entity) {
    let parent = world
        .spawn(ScreenPosition::new(parent_pos.0, parent_pos.1))
        .id();
    let child = world
        .spawn((GuiOffset(Vector2::new(offset.0, offset.1)), ChildOf(parent)))
        .id();
    world.flush();
    (parent, child)
}

#[test]
fn single_child_resolves_parent_plus_offset() {
    let mut world = World::new();

    let (_, child) = spawn_window_and_child(&mut world, (100.0, 100.0), (20.0, 40.0));

    tick_gui_layout(&mut world);
    world.flush();

    let pos = world.get::<ScreenPosition>(child).unwrap().pos();
    assert!(approx_eq(pos.x, 120.0));
    assert!(approx_eq(pos.y, 140.0));
}

#[test]
fn nested_grandchild_cascades_additively() {
    let mut world = World::new();

    let (_, button) = spawn_window_and_child(&mut world, (100.0, 100.0), (20.0, 40.0));
    let label = world
        .spawn((GuiOffset(Vector2::new(5.0, 5.0)), ChildOf(button)))
        .id();
    world.flush();

    tick_gui_layout(&mut world);
    world.flush();

    let button_pos = world.get::<ScreenPosition>(button).unwrap().pos();
    assert!(approx_eq(button_pos.x, 120.0));
    assert!(approx_eq(button_pos.y, 140.0));

    let label_pos = world.get::<ScreenPosition>(label).unwrap().pos();
    assert!(approx_eq(label_pos.x, 125.0));
    assert!(approx_eq(label_pos.y, 145.0));
}

#[test]
fn hiding_parent_removes_descendant_screen_position() {
    let mut world = World::new();

    let (window, button) = spawn_window_and_child(&mut world, (100.0, 100.0), (20.0, 40.0));
    let label = world
        .spawn((GuiOffset(Vector2::new(5.0, 5.0)), ChildOf(button)))
        .id();
    world.flush();

    // First tick establishes ScreenPosition on both descendants.
    tick_gui_layout(&mut world);
    world.flush();
    assert!(world.get::<ScreenPosition>(button).is_some());
    assert!(world.get::<ScreenPosition>(label).is_some());

    // Hide the window by removing its ScreenPosition.
    world.entity_mut(window).remove::<ScreenPosition>();

    tick_gui_layout(&mut world);
    world.flush();

    assert!(
        world.get::<ScreenPosition>(button).is_none(),
        "direct child of hidden window should lose ScreenPosition"
    );
    assert!(
        world.get::<ScreenPosition>(label).is_none(),
        "grandchild should also lose ScreenPosition in the same pass, not one frame later"
    );
}

#[test]
fn re_showing_parent_restores_descendant_screen_position() {
    let mut world = World::new();

    let (window, button) = spawn_window_and_child(&mut world, (100.0, 100.0), (20.0, 40.0));

    tick_gui_layout(&mut world);
    world.flush();

    world.entity_mut(window).remove::<ScreenPosition>();
    tick_gui_layout(&mut world);
    world.flush();
    assert!(world.get::<ScreenPosition>(button).is_none());

    // Re-show at a new position — no despawn/respawn needed.
    world
        .entity_mut(window)
        .insert(ScreenPosition::new(200.0, 50.0));
    tick_gui_layout(&mut world);
    world.flush();

    let pos = world.get::<ScreenPosition>(button).unwrap().pos();
    assert!(approx_eq(pos.x, 220.0));
    assert!(approx_eq(pos.y, 90.0));
}

#[test]
fn child_of_without_gui_offset_is_left_untouched() {
    let mut world = World::new();

    let parent = world.spawn(ScreenPosition::new(100.0, 100.0)).id();
    // A ChildOf relationship with no GuiOffset — not a GUI child, e.g. some
    // other hierarchy relationship. gui_layout_system must not touch it.
    let other_child = world
        .spawn((ScreenPosition::new(5.0, 5.0), ChildOf(parent)))
        .id();
    world.flush();

    tick_gui_layout(&mut world);
    world.flush();

    let pos = world.get::<ScreenPosition>(other_child).unwrap().pos();
    assert!(approx_eq(pos.x, 5.0), "non-GUI child must be left alone");
    assert!(approx_eq(pos.y, 5.0), "non-GUI child must be left alone");
}

#[test]
fn screen_space_gui_hierarchy_is_ignored_by_propagate_transforms() {
    let mut world = World::new();

    let (window, button) = spawn_window_and_child(&mut world, (100.0, 100.0), (20.0, 40.0));

    let mut schedule = Schedule::default();
    schedule.add_systems(propagate_transforms);
    schedule.run(&mut world);

    assert!(
        world.get::<GlobalTransform2D>(window).is_none(),
        "screen-space GUI root has no MapPosition, so propagate_transforms's \
         RootsQuery must not match it"
    );
    assert!(
        world.get::<GlobalTransform2D>(button).is_none(),
        "screen-space GUI child has no MapPosition, so propagate_transforms's \
         ChildrenQuery must not match it"
    );
}
