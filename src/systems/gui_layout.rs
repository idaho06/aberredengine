//! GUI child layout resolution.
//!
//! Derives a GUI child entity's [`ScreenPosition`] from its parent's
//! `ScreenPosition` plus its authored [`GuiOffset`], every frame, top-down.
//!
//! `ChildOf` is used for lifecycle only (cascade despawn) — not positioning.
//! `ScreenPosition` is a derived value for GUI children, recomputed here each
//! frame, the same way `Sprite`/`Animation` derive visual state from
//! authored components rather than being authored directly.
//!
//! Visibility model: a `GuiWindow` has no `visible` flag — showing/hiding is
//! presence/absence of `ScreenPosition`. When a parent lacks `ScreenPosition`
//! (hidden, or hidden upstream this same pass), this system removes
//! `ScreenPosition` from every descendant, computed top-down within one
//! execution so a multi-level subtree resolves in one pass rather than one
//! extra frame per nesting level.
//!
//! v1 GUI is translate-only (no rigid rotate/scale of children — see
//! `docs/gui-system-architecture.md`'s "Why a custom system" section), so
//! this is plain `Vector2` addition rather than `transform_compose`'s
//! scale/rotate/translate composition.

use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::*;
use raylib::math::Vector2;

use crate::components::guioffset::GuiOffset;
use crate::components::screenposition::ScreenPosition;

type RootsQuery<'w, 's> = Query<
    'w,
    's,
    (Option<&'static ScreenPosition>, &'static Children),
    Without<GuiOffset>,
>;

type GuiChildrenQuery<'w, 's> = Query<
    'w,
    's,
    (&'static GuiOffset, Option<&'static Children>),
    With<ChildOf>,
>;

/// Resolve every GUI child's `ScreenPosition` from its parent's
/// `ScreenPosition` + `GuiOffset`, top-down. Should run after any system
/// that mutates `ScreenPosition` (e.g. `tween_system::<ScreenPosition>`) and
/// before rendering/hit-testing.
pub fn gui_layout_system(
    roots: RootsQuery,
    gui_children: GuiChildrenQuery,
    // `With<GuiOffset>` keeps this disjoint from `roots`'s `Without<GuiOffset>`
    // read of `ScreenPosition` — without it Bevy sees both queries as
    // potentially aliasing the same entity's `ScreenPosition`.
    mut screen_positions: Query<&mut ScreenPosition, With<GuiOffset>>,
    mut commands: Commands,
) {
    crate::tracy::tracy_span!("gui_layout_system");
    for (parent_screen_pos, children) in roots.iter() {
        let parent_pos = parent_screen_pos.map(|p| p.pos());
        layout_children(
            parent_pos,
            children,
            &gui_children,
            &mut screen_positions,
            &mut commands,
        );
    }
}

fn layout_children(
    parent_pos: Option<Vector2>,
    children: &Children,
    gui_children: &GuiChildrenQuery,
    screen_positions: &mut Query<&mut ScreenPosition, With<GuiOffset>>,
    commands: &mut Commands,
) {
    for child_entity in children.iter() {
        // Not a GUI child (no GuiOffset) — leave it alone; it's managed by
        // whatever else attached `ChildOf` to it.
        let Ok((offset, maybe_grandchildren)) = gui_children.get(child_entity) else {
            continue;
        };

        let new_pos = parent_pos.map(|p| p + offset.0);

        if let Some(pos) = new_pos {
            if let Ok(mut screen_pos) = screen_positions.get_mut(child_entity) {
                screen_pos.set_pos(pos);
            } else {
                commands
                    .entity(child_entity)
                    .insert(ScreenPosition::from_vec(pos));
            }
        } else if screen_positions.get(child_entity).is_ok() {
            // Only queue a removal when the component is actually still
            // present — otherwise an already-hidden subtree would re-queue
            // a no-op `remove` every frame for as long as it stays hidden.
            commands.entity(child_entity).remove::<ScreenPosition>();
        }

        if let Some(grandchildren) = maybe_grandchildren {
            layout_children(new_pos, grandchildren, gui_children, screen_positions, commands);
        }
    }
}
