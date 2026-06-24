//! GUI interactable hover/press/click resolution.
//!
//! [`gui_hit_test_system`] resolves every visible (`ScreenPosition` present)
//! [`GuiInteractable`]'s [`GuiWidgetState`] from cursor position + the raw
//! left mouse button, picks the highest-`ZIndex` interactable under the
//! cursor as the sole interaction target (others stay `Normal`), and
//! triggers [`GuiInteractableClickEvent`] on a press-then-release-inside
//! transition. Any clickable GUI widget (`GuiButton`, `GuiImage`, future
//! widgets) goes through this same system by carrying `GuiInteractable`.
//!
//! State is a pure function of the current frame's cursor position
//! (drag-off-cancels): outside bounds is always `Normal` regardless of
//! mouse-button state; inside+down is `Pressed`; inside+up is `Hovered`. A
//! click fires only when the widget was `Pressed` last frame and the mouse
//! is released this frame while still inside bounds. `Disabled` widgets are
//! never overwritten by this resolution, but still consume the click if
//! they're the topmost hit (see `docs/gui-system-architecture.md`'s "Click
//! Consumption" section).

use bevy_ecs::prelude::*;
use raylib::math::{Rectangle, Vector2};

use crate::components::guiinteractable::{GuiInteractable, GuiWidgetState};
use crate::components::screenposition::ScreenPosition;
use crate::components::zindex::ZIndex;
use crate::events::gui_interactable::GuiInteractableClickEvent;
use crate::resources::guiinputstate::GuiInputState;
use crate::resources::input::InputState;

fn contains_point(pos: Vector2, size: Vector2, point: Vector2) -> bool {
    Rectangle::new(pos.x, pos.y, size.x, size.y).check_collision_point_rec(point)
}

/// Resolves hover/press/click state for every `GuiInteractable` with a
/// `ScreenPosition` (hidden widgets are automatically excluded, consistent
/// with the engine's "presence of `ScreenPosition`" visibility idiom).
pub fn gui_hit_test_system(
    mut interactables: Query<(Entity, &mut GuiInteractable, &ScreenPosition, &ZIndex)>,
    input: Res<InputState>,
    mut gui_input: ResMut<GuiInputState>,
    mut commands: Commands,
) {
    crate::tracy::tracy_span!("gui_hit_test_system");
    gui_input.click_consumed_this_frame = false;

    let cursor = Vector2::new(input.mouse_x, input.mouse_y);

    // Highest-ZIndex hit under the cursor wins (Disabled widgets are still
    // eligible to win — a Disabled top widget blocks/consumes clicks for
    // anything beneath it). Exact ZIndex ties are broken by lower Entity id,
    // so the winner is deterministic regardless of Bevy's (unstable,
    // archetype-dependent) query iteration order — see camera_follow.rs for
    // the same lower-Entity-id-wins-ties convention.
    let mut winner: Option<(Entity, f32)> = None;
    for (entity, interactable, pos, z) in interactables.iter() {
        if contains_point(pos.pos(), interactable.size, cursor) {
            let better = match winner {
                None => true,
                Some((we, wz)) => z.0 > wz || (z.0 == wz && entity < we),
            };
            if better {
                winner = Some((entity, z.0));
            }
        }
    }
    let winner = winner.map(|(e, _)| e);

    for (entity, mut interactable, _pos, _z) in interactables.iter_mut() {
        let is_winner = winner == Some(entity);

        if interactable.state == GuiWidgetState::Disabled {
            if is_winner {
                gui_input.click_consumed_this_frame = true;
            }
            continue;
        }

        if !is_winner {
            interactable.state = GuiWidgetState::Normal;
            continue;
        }

        let was_pressed = interactable.state == GuiWidgetState::Pressed;
        let mouse_down = input.mouse_left_button.active;
        let released = input.mouse_left_button.just_released;

        interactable.state = if mouse_down {
            GuiWidgetState::Pressed
        } else {
            GuiWidgetState::Hovered
        };

        if was_pressed && released {
            commands.trigger(GuiInteractableClickEvent { entity });
            gui_input.click_consumed_this_frame = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::guiimage::GuiImage;

    fn tick(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(gui_hit_test_system);
        schedule.run(world);
        world.flush();
    }

    fn new_world() -> World {
        let mut world = World::new();
        world.insert_resource(InputState::default());
        world.insert_resource(GuiInputState::default());
        world
    }

    fn spawn_interactable(world: &mut World, x: f32, y: f32, w: f32, h: f32, z: f32) -> Entity {
        world
            .spawn((
                GuiInteractable::new(w, h),
                ScreenPosition::new(x, y),
                ZIndex(z),
            ))
            .id()
    }

    #[test]
    fn hovered_when_cursor_inside_and_mouse_up() {
        let mut world = new_world();
        let btn = spawn_interactable(&mut world, 10.0, 10.0, 50.0, 20.0, 0.0);
        world.resource_mut::<InputState>().mouse_x = 20.0;
        world.resource_mut::<InputState>().mouse_y = 15.0;

        tick(&mut world);

        assert_eq!(
            world.get::<GuiInteractable>(btn).unwrap().state,
            GuiWidgetState::Hovered
        );
    }

    #[test]
    fn pressed_when_cursor_inside_and_mouse_down() {
        let mut world = new_world();
        let btn = spawn_interactable(&mut world, 10.0, 10.0, 50.0, 20.0, 0.0);
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 20.0;
            input.mouse_y = 15.0;
            input.mouse_left_button.active = true;
        }

        tick(&mut world);

        assert_eq!(
            world.get::<GuiInteractable>(btn).unwrap().state,
            GuiWidgetState::Pressed
        );
    }

    #[test]
    fn normal_when_cursor_outside_regardless_of_mouse_button() {
        let mut world = new_world();
        let btn = spawn_interactable(&mut world, 10.0, 10.0, 50.0, 20.0, 0.0);
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 999.0;
            input.mouse_y = 999.0;
            input.mouse_left_button.active = true;
        }

        tick(&mut world);

        assert_eq!(
            world.get::<GuiInteractable>(btn).unwrap().state,
            GuiWidgetState::Normal
        );
    }

    #[test]
    fn drag_off_cancels_press() {
        let mut world = new_world();
        let btn = spawn_interactable(&mut world, 10.0, 10.0, 50.0, 20.0, 0.0);

        // Frame 1: press inside.
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 20.0;
            input.mouse_y = 15.0;
            input.mouse_left_button.active = true;
        }
        tick(&mut world);
        assert_eq!(
            world.get::<GuiInteractable>(btn).unwrap().state,
            GuiWidgetState::Pressed
        );

        // Frame 2: drag outside while still held.
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 999.0;
            input.mouse_y = 999.0;
            // still held
            input.mouse_left_button.active = true;
        }
        tick(&mut world);
        assert_eq!(
            world.get::<GuiInteractable>(btn).unwrap().state,
            GuiWidgetState::Normal,
            "dragging off while held should cancel Pressed, not keep it latched"
        );
    }

    #[test]
    fn click_fires_on_release_while_inside() {
        let mut world = new_world();
        let btn = spawn_interactable(&mut world, 10.0, 10.0, 50.0, 20.0, 0.0);

        // Frame 1: press inside.
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 20.0;
            input.mouse_y = 15.0;
            input.mouse_left_button.active = true;
            input.mouse_left_button.just_pressed = true;
        }
        tick(&mut world);

        // Frame 2: release inside.
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_left_button.active = false;
            input.mouse_left_button.just_pressed = false;
            input.mouse_left_button.just_released = true;
        }
        tick(&mut world);

        assert_eq!(
            world.get::<GuiInteractable>(btn).unwrap().state,
            GuiWidgetState::Hovered
        );
        assert!(world.resource::<GuiInputState>().click_consumed_this_frame);
    }

    #[test]
    fn two_overlapping_buttons_only_highest_zindex_becomes_hovered() {
        let mut world = new_world();
        let low = spawn_interactable(&mut world, 0.0, 0.0, 100.0, 100.0, 5.0);
        let high = spawn_interactable(&mut world, 0.0, 0.0, 100.0, 100.0, 10.0);
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 50.0;
            input.mouse_y = 50.0;
        }

        tick(&mut world);

        assert_eq!(
            world.get::<GuiInteractable>(high).unwrap().state,
            GuiWidgetState::Hovered
        );
        assert_eq!(
            world.get::<GuiInteractable>(low).unwrap().state,
            GuiWidgetState::Normal
        );
    }

    #[test]
    fn equal_zindex_tie_broken_by_lower_entity_id_regardless_of_spawn_order() {
        let mut world = new_world();
        let first_spawned = spawn_interactable(&mut world, 0.0, 0.0, 100.0, 100.0, 5.0);
        let second_spawned = spawn_interactable(&mut world, 0.0, 0.0, 100.0, 100.0, 5.0);
        let lower_id = first_spawned.min(second_spawned);
        let higher_id = first_spawned.max(second_spawned);
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 50.0;
            input.mouse_y = 50.0;
        }

        tick(&mut world);

        assert_eq!(
            world.get::<GuiInteractable>(lower_id).unwrap().state,
            GuiWidgetState::Hovered,
            "lower Entity id should win an exact ZIndex tie"
        );
        assert_eq!(
            world.get::<GuiInteractable>(higher_id).unwrap().state,
            GuiWidgetState::Normal
        );
    }

    #[test]
    fn disabled_button_never_overwritten_by_hover() {
        let mut world = new_world();
        let btn = spawn_interactable(&mut world, 10.0, 10.0, 50.0, 20.0, 0.0);
        world.get_mut::<GuiInteractable>(btn).unwrap().state = GuiWidgetState::Disabled;
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 20.0;
            input.mouse_y = 15.0;
        }

        tick(&mut world);

        assert_eq!(
            world.get::<GuiInteractable>(btn).unwrap().state,
            GuiWidgetState::Disabled
        );
    }

    #[test]
    fn disabled_button_still_consumes_click() {
        let mut world = new_world();
        let btn = spawn_interactable(&mut world, 10.0, 10.0, 50.0, 20.0, 0.0);
        world.get_mut::<GuiInteractable>(btn).unwrap().state = GuiWidgetState::Disabled;
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 20.0;
            input.mouse_y = 15.0;
            input.mouse_left_button.active = true;
            input.mouse_left_button.just_pressed = true;
        }

        tick(&mut world);

        assert!(world.resource::<GuiInputState>().click_consumed_this_frame);
        assert_eq!(
            world.get::<GuiInteractable>(btn).unwrap().state,
            GuiWidgetState::Disabled,
            "disabled button must never transition state even while consuming the click"
        );
    }

    #[test]
    fn gui_image_hit_test_resolves_like_a_button() {
        let mut world = new_world();
        let img = world
            .spawn((
                GuiInteractable::new(50.0, 20.0),
                GuiImage::new(50.0, 20.0, "item_sword"),
                ScreenPosition::new(10.0, 10.0),
                ZIndex(0.0),
            ))
            .id();
        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_x = 20.0;
            input.mouse_y = 15.0;
            input.mouse_left_button.active = true;
            input.mouse_left_button.just_pressed = true;
        }
        tick(&mut world);
        assert_eq!(
            world.get::<GuiInteractable>(img).unwrap().state,
            GuiWidgetState::Pressed
        );

        {
            let mut input = world.resource_mut::<InputState>();
            input.mouse_left_button.active = false;
            input.mouse_left_button.just_pressed = false;
            input.mouse_left_button.just_released = true;
        }
        tick(&mut world);

        assert_eq!(
            world.get::<GuiInteractable>(img).unwrap().state,
            GuiWidgetState::Hovered
        );
        assert!(world.resource::<GuiInputState>().click_consumed_this_frame);
    }
}
