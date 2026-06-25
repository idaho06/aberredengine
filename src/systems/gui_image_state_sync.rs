//! Syncs a `GuiImage`'s rendered atlas cell to its current interaction state.
//!
//! [`gui_image_state_sync_system`] reads each `GuiImage` entity's
//! `GuiInteractable.state` (resolved earlier in the frame by
//! `gui_hit_test_system`) and writes the matching atlas offset into the
//! co-located `Sprite.offset` — mirrors `GuiButtonSkin`'s per-state
//! nine-patch resolution (`resolve_button_patch`, `systems/render/mod.rs`),
//! but for a plain `Sprite` rather than a read-only nine-patch lookup, since
//! `Sprite` itself must be mutated post-spawn (no render-time indirection
//! available for sprites the way `Panel` rendering has for buttons).

use bevy_ecs::prelude::*;
use raylib::math::Vector2;

use crate::components::guiimage::GuiImage;
use crate::components::guiinteractable::{GuiInteractable, GuiWidgetState};
use crate::components::sprite::Sprite;

/// Resolves the atlas offset for `image` at the given `state`, falling back
/// to `image.offset` (the `Normal`/base offset) for any unset per-state
/// offset — same "only normal required" convention as `GuiButtonSkin`.
pub(crate) fn resolve_image_offset(image: &GuiImage, state: GuiWidgetState) -> Vector2 {
    match state {
        GuiWidgetState::Normal => image.offset,
        GuiWidgetState::Hovered => image.offset_hover.unwrap_or(image.offset),
        GuiWidgetState::Pressed => image.offset_pressed.unwrap_or(image.offset),
        GuiWidgetState::Disabled => image.offset_disabled.unwrap_or(image.offset),
    }
}

/// Writes the resolved per-state atlas offset into each `GuiImage`'s
/// co-located `Sprite.offset`, but only when it actually differs from the
/// current value — `gui_hit_test_system` writes `GuiInteractable.state`
/// unconditionally every frame, so skipping a no-op write here avoids
/// needlessly bumping `Sprite`'s change-detection tick every frame for
/// widgets that never change visual state.
pub fn gui_image_state_sync_system(mut query: Query<(&GuiImage, &GuiInteractable, &mut Sprite)>) {
    for (image, interactable, mut sprite) in &mut query {
        let resolved = resolve_image_offset(image, interactable.state);
        if sprite.offset != resolved {
            sprite.offset = resolved;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use crate::components::screenposition::ScreenPosition;
    use crate::components::zindex::ZIndex;

    fn tick<M>(world: &mut World, system: impl IntoSystem<(), (), M>) {
        world
            .run_system_once(system)
            .expect("system should run without error");
    }

    fn test_sprite(offset: Vector2) -> Sprite {
        Sprite {
            tex_key: "item_sword".into(),
            width: 32.0,
            height: 32.0,
            offset,
            origin: Vector2::new(0.0, 0.0),
            flip_h: false,
            flip_v: false,
        }
    }

    #[test]
    fn resolve_image_offset_falls_back_to_base_offset_when_unset() {
        let image = GuiImage::new(32.0, 32.0, "item_sword", 10.0, 20.0);
        assert_eq!(resolve_image_offset(&image, GuiWidgetState::Normal), Vector2::new(10.0, 20.0));
        assert_eq!(resolve_image_offset(&image, GuiWidgetState::Hovered), Vector2::new(10.0, 20.0));
        assert_eq!(resolve_image_offset(&image, GuiWidgetState::Pressed), Vector2::new(10.0, 20.0));
        assert_eq!(resolve_image_offset(&image, GuiWidgetState::Disabled), Vector2::new(10.0, 20.0));
    }

    #[test]
    fn resolve_image_offset_uses_explicit_per_state_offset_when_set() {
        let image = GuiImage::new(32.0, 32.0, "item_sword", 10.0, 20.0)
            .with_offset_hover(40.0, 0.0)
            .with_offset_pressed(80.0, 0.0)
            .with_offset_disabled(120.0, 0.0);
        assert_eq!(resolve_image_offset(&image, GuiWidgetState::Normal), Vector2::new(10.0, 20.0));
        assert_eq!(resolve_image_offset(&image, GuiWidgetState::Hovered), Vector2::new(40.0, 0.0));
        assert_eq!(resolve_image_offset(&image, GuiWidgetState::Pressed), Vector2::new(80.0, 0.0));
        assert_eq!(resolve_image_offset(&image, GuiWidgetState::Disabled), Vector2::new(120.0, 0.0));
    }

    #[test]
    fn gui_image_state_sync_system_writes_resolved_offset_to_sprite() {
        let mut world = World::new();
        let image = GuiImage::new(32.0, 32.0, "item_sword", 0.0, 0.0).with_offset_hover(64.0, 0.0);
        let mut interactable = GuiInteractable::new(32.0, 32.0);
        interactable.state = GuiWidgetState::Hovered;
        world.spawn((
            image,
            interactable,
            test_sprite(Vector2::new(0.0, 0.0)),
            ScreenPosition::new(0.0, 0.0),
            ZIndex(0.0),
        ));

        tick(&mut world, gui_image_state_sync_system);

        let sprite = world
            .query::<&Sprite>()
            .iter(&world)
            .next()
            .expect("entity with Sprite should exist");
        assert_eq!(sprite.offset, Vector2::new(64.0, 0.0));
    }

    #[test]
    fn gui_image_state_sync_system_does_not_touch_sprite_when_offset_unchanged() {
        let mut world = World::new();
        let image = GuiImage::new(32.0, 32.0, "item_sword", 5.0, 5.0);
        let interactable = GuiInteractable::new(32.0, 32.0);
        world.spawn((
            image,
            interactable,
            test_sprite(Vector2::new(5.0, 5.0)),
            ScreenPosition::new(0.0, 0.0),
            ZIndex(0.0),
        ));

        // Prime change-detection ticks, then run again — a second run with
        // an already-equal offset must not mark Sprite as Changed.
        tick(&mut world, gui_image_state_sync_system);
        world.clear_trackers();
        tick(&mut world, gui_image_state_sync_system);

        let mut changed_query = world.query_filtered::<(), Changed<Sprite>>();
        assert_eq!(
            changed_query.iter(&world).count(),
            0,
            "Sprite must not be marked Changed when the resolved offset already matches"
        );
    }
}
