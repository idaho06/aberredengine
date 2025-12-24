//! Menu systems.
//!
//! This module provides systems for interactive menus:
//! - [`menu_spawn_system`] – spawns menu item entities when a [`Menu`] is added
//! - [`menu_despawn`] – despawns menu entities and their items
//! - [`menu_controller_observer`] – handles input to navigate and select items
//! - [`menu_selection_observer`] – performs actions when items are selected
//!
//! Menus can render in world-space or screen-space depending on the
//! `use_screen_space` flag.

use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::menu::{Menu, MenuAction, MenuActions};
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::events::audio::AudioCmd;
use crate::events::input::{InputAction, InputEvent};
use crate::events::menu::MenuSelectionEvent;
use crate::resources::fontstore::FontStore;
use crate::resources::gamestate::GameStates::Quitting;
use crate::resources::gamestate::NextGameState;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::worldsignals::WorldSignals;
use crate::{components::dynamictext::DynamicText, game::load_texture_from_text};
use bevy_ecs::prelude::*;
use raylib::audio;
use raylib::prelude::{Color, Font, Vector2};

/// Spawns entities for newly added [`Menu`] components.
///
/// For each menu item, spawns a text entity (either [`DynamicText`] or a
/// static sprite) and positions it in world or screen space. Also spawns
/// the cursor entity if configured.
pub fn menu_spawn_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Menu), Added<Menu>>,
    font_store: NonSend<FontStore>,
    mut texture_store: ResMut<TextureStore>,
    mut rl: NonSendMut<raylib::RaylibHandle>,
    th: NonSend<raylib::RaylibThread>,
) {
    for (entity, mut menu) in query.iter_mut() {
        // Cache immutable data before mutable iteration to satisfy borrow rules
        let font_string = menu.font.clone();
        let font_size = menu.font_size;
        let normal_color = menu.normal_color;
        let use_screen_space = menu.use_screen_space;

        // Spawn DynamicText or Sprite for each menu item if needed
        for menu_item in menu.items.iter_mut() {
            let mut ecmd = commands.spawn_empty();
            if menu_item.dynamic_text {
                // Dynamic text will be updated each frame
                ecmd.insert(DynamicText::new(
                    menu_item.label.clone(),
                    font_string.clone(),
                    font_size,
                    normal_color,
                ));
            } else {
                // Static text sprite
                let font_handle = font_store.get(&font_string).expect(&format!(
                    "menu_spawn_system: Font {} not found in FontStore",
                    font_string
                ));
                let texture_handle = load_texture_from_text(
                    &mut rl,
                    &th,
                    font_handle,
                    &menu_item.label,
                    font_size,
                    1.0,
                    normal_color,
                )
                .expect("Failed to create texture from text");
                let width = texture_handle.width as f32;
                let height = texture_handle.height as f32;
                let key = format!("menu_{}", menu_item.id);
                texture_store.insert(&key, texture_handle);
                ecmd.insert(Sprite {
                    tex_key: key.into(),
                    width,
                    height,
                    offset: Vector2 { x: 0.0, y: 0.0 },
                    origin: Vector2 { x: 0.0, y: 0.0 },
                    flip_h: false,
                    flip_v: false,
                });
            }

            if use_screen_space {
                ecmd.insert(ScreenPosition {
                    pos: menu_item.position,
                });
            } else {
                ecmd.insert(MapPosition {
                    pos: menu_item.position,
                });
                ecmd.insert(ZIndex(23));
            }
            let text_entity = ecmd.id();
            ecmd.insert(Group::new(&format!("menu_{}", entity.to_string())));
            menu_item.entity = Some(text_entity);
        } // end for each menu item

        // Add a signals component to the menu entity for state tracking
        commands
            .entity(entity)
            .insert(Signals::default().with_flag("waiting_selection"));

        // Spawn cursor entity if needed
        if let Some(cursor_entity) = menu.cursor_entity {
            let cursor_position = menu.items[menu.selected_index].position; // make sure sprite has correct origin
            if use_screen_space {
                commands.entity(cursor_entity).insert(ScreenPosition {
                    pos: cursor_position,
                });
            } else {
                commands.entity(cursor_entity).insert(MapPosition {
                    pos: cursor_position,
                });
                commands.entity(cursor_entity).insert(ZIndex(23));
            }
        }
    }
}

/// Despawns all menu-related entities.
///
/// Removes menu item entities, cursor entity, and the menu entity itself.
pub fn menu_despawn(mut commands: Commands, query: Query<(Entity, &Menu)>) {
    for (entity, menu) in query.iter() {
        // Despawn menu item entities
        for item in menu.items.iter() {
            if let Some(item_entity) = item.entity {
                commands.entity(item_entity).despawn();
            }
        }

        // Despawn cursor entity if applicable
        if let Some(cursor_entity) = menu.cursor_entity {
            commands.entity(cursor_entity).despawn();
        }

        // Finally despawn the menu entity itself
        commands.entity(entity).despawn();
    }
}

/// Handles input events to navigate menus and confirm selections.
///
/// Responds to secondary direction inputs (arrow keys) to move selection
/// and action buttons to confirm. Triggers [`MenuSelectionEvent`] when
/// an item is selected.
pub fn menu_controller_observer(
    trigger: On<InputEvent>,
    mut query: Query<(Entity, &mut Menu, &mut Signals)>,
    mut commands: Commands,
    mut audio_cmds: MessageWriter<AudioCmd>,
) {
    for (entity, mut menu, mut signals) in query.iter_mut() {
        if !menu.active {
            continue;
        }
        let event = trigger.event();
        if !event.pressed {
            continue; // Only handle key press, not release
        }

        let mut changed_selection = false;
        match event.action {
            InputAction::SecondaryDirectionUp => {
                if !menu.items.is_empty() {
                    menu.selected_index =
                        (menu.selected_index + menu.items.len() - 1) % menu.items.len();
                    changed_selection = true;
                }
            }
            InputAction::SecondaryDirectionDown => {
                if !menu.items.is_empty() {
                    menu.selected_index = (menu.selected_index + 1) % menu.items.len();
                    changed_selection = true;
                }
            }
            InputAction::Action1 | InputAction::Action2 => {
                if let Some(item) = menu.items.get(menu.selected_index) {
                    let selected_id = item.id.clone();
                    signals.clear_flag("waiting_selection");
                    menu.active = false;
                    signals.set_string("selected_item", selected_id.clone());
                    commands.trigger(MenuSelectionEvent {
                        menu: entity,
                        item_id: selected_id,
                    });
                }
            }
            _ => {}
        }

        // Update cursor position if applicable
        if changed_selection {
            // TODO: change colors of selected/unselected items
            // TODO: sounds
            // TODO: Use Tween for cursor movement
            if let Some(cursor_entity) = menu.cursor_entity {
                let cursor_position = menu.items[menu.selected_index].position;
                if menu.use_screen_space {
                    commands.entity(cursor_entity).insert(ScreenPosition {
                        pos: cursor_position,
                    });
                } else {
                    commands.entity(cursor_entity).insert(MapPosition {
                        pos: cursor_position,
                    });
                }
            }
            // Play selection change sound if configured
            if let Some(sound_key) = &menu.selection_change_sound {
                audio_cmds.write(AudioCmd::PlayFx {
                    id: sound_key.clone(),
                });
            }
        }
    }
}

/// Executes the action associated with a selected menu item.
///
/// Looks up the [`MenuAction`] for the selected item and performs it:
/// - [`MenuAction::SetScene`] – triggers scene switch
/// - [`MenuAction::QuitGame`] – transitions to quitting state
/// - [`MenuAction::ShowSubMenu`] – displays a sub-menu (TODO)
/// - [`MenuAction::Noop`] – does nothing
pub fn menu_selection_observer(
    trigger: On<MenuSelectionEvent>,
    mut commands: Commands,
    menus: Query<&MenuActions>,
    mut world_signals: ResMut<WorldSignals>,
    mut next_game_state: ResMut<NextGameState>,
    systems_store: Res<SystemsStore>,
) {
    let event = trigger.event();
    let Ok(menu_actions) = menus.get(event.menu) else {
        eprintln!(
            "menu_selection_observer: No MenuActions found for menu entity {:?}, item_id {:?}",
            event.menu, event.item_id
        );
        return;
    };
    match menu_actions.get(&event.item_id) {
        MenuAction::SetScene(scene_name) => {
            world_signals.set_string("scene", scene_name.clone());
            commands.run_system(
                systems_store
                    .get("switch_scene")
                    .expect("switch_scene system not found")
                    .clone(),
            );
        }
        MenuAction::ShowSubMenu(submenu_name) => {
            world_signals.set_string("show_submenu", submenu_name.clone());
            // TODO: trigger submenu display system
        }
        MenuAction::QuitGame => {
            next_game_state.set(Quitting);
        }
        MenuAction::Noop => {
            // Do nothing
        }
    }
}
