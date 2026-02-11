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

use std::sync::Arc;

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
use crate::resources::lua_runtime::LuaRuntime;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::worldsignals::WorldSignals;
use crate::{components::dynamictext::DynamicText, game::load_texture_from_text};
use bevy_ecs::prelude::*;
use log::{info, debug, error, warn};
use raylib::prelude::Vector2;

/// Spawns entities for newly added [`Menu`] components.
///
/// For each menu item, spawns a text entity (either [`DynamicText`] or a
/// static sprite) and positions it in world or screen space. Also spawns
/// the cursor entity if configured.
///
/// When `visible_count` is set, only positions items within the visible window
/// and spawns "..." indicator entities for scrolling.
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
        let selected_color = menu.selected_color;
        let selected_index = menu.selected_index;
        let use_screen_space = menu.use_screen_space;
        let origin = menu.origin;
        let item_spacing = menu.item_spacing;
        let visible_count = menu.visible_count;
        let scroll_offset = menu.scroll_offset;

        debug!(
            "menu_spawn_system: Spawning menu entity {:?} with {} items",
            entity,
            menu.items.len()
        );

        // Calculate visible range
        let visible_end = if let Some(vc) = visible_count {
            (scroll_offset + vc).min(menu.items.len())
        } else {
            menu.items.len()
        };

        // Spawn DynamicText or Sprite for each menu item
        for (i, menu_item) in menu.items.iter_mut().enumerate() {
            let mut ecmd = commands.spawn_empty();
            if menu_item.dynamic_text {
                // Dynamic text will be updated each frame
                // Use selected_color for the initially selected item
                let color = if i == selected_index {
                    selected_color
                } else {
                    normal_color
                };
                ecmd.insert(DynamicText::new(
                    &menu_item.label,
                    font_string.clone(),
                    font_size,
                    color,
                ));
                debug!(
                    "menu_spawn_system: Spawned DynamicText for menu item id={}",
                    menu_item.id
                );
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
                    tex_key: Arc::from(key),
                    width,
                    height,
                    offset: Vector2 { x: 0.0, y: 0.0 },
                    origin: Vector2 { x: 0.0, y: 0.0 },
                    flip_h: false,
                    flip_v: false,
                });
                debug!(
                    "menu_spawn_system: Spawned Sprite for menu item id={}, size=({}, {})",
                    menu_item.id, width, height
                );
            }

            // For world-space, add ZIndex to ALL items (needed when they become visible)
            if !use_screen_space {
                ecmd.insert(ZIndex(23.0));
            }

            // Only add position component for visible items
            let is_visible = i >= scroll_offset && i < visible_end;
            if is_visible {
                // Calculate position within visible viewport
                let viewport_index = i - scroll_offset;
                let pos = Vector2 {
                    x: origin.x,
                    y: origin.y + (viewport_index as f32) * item_spacing,
                };
                if use_screen_space {
                    ecmd.insert(ScreenPosition { pos });
                } else {
                    ecmd.insert(MapPosition { pos });
                }
            }
            // Non-visible items don't get position component, so render system skips them

            let text_entity = ecmd.id();
            ecmd.insert(Group::new(&format!("menu_{}", entity.to_string())));
            menu_item.entity = Some(text_entity);
            debug!(
                "menu_spawn_system: Menu item id={} assigned entity {:?} (visible={})",
                menu_item.id, text_entity, is_visible
            );
        } // end for each menu item

        // Spawn "..." indicators if visible_count is set
        if let Some(vc) = visible_count {
            // Top indicator (shown when scroll_offset > 0)
            let mut top_cmd = commands.spawn(DynamicText::new(
                "...",
                font_string.clone(),
                font_size,
                normal_color,
            ));
            top_cmd.insert(Group::new(&format!("menu_{}", entity.to_string())));
            // For world-space, add ZIndex always (needed when indicator becomes visible)
            if !use_screen_space {
                top_cmd.insert(ZIndex(23.0));
            }
            let top_indicator = top_cmd.id();
            // Position only if needed (scroll_offset > 0)
            if scroll_offset > 0 {
                let pos = Vector2 {
                    x: origin.x,
                    y: origin.y - item_spacing,
                };
                if use_screen_space {
                    commands
                        .entity(top_indicator)
                        .insert(ScreenPosition { pos });
                } else {
                    commands.entity(top_indicator).insert(MapPosition { pos });
                }
            }
            menu.top_indicator_entity = Some(top_indicator);

            // Bottom indicator (shown when more items below)
            let mut bottom_cmd = commands.spawn(DynamicText::new(
                "...",
                font_string.clone(),
                font_size,
                normal_color,
            ));
            bottom_cmd.insert(Group::new(&format!("menu_{}", entity.to_string())));
            // For world-space, add ZIndex always (needed when indicator becomes visible)
            if !use_screen_space {
                bottom_cmd.insert(ZIndex(23.0));
            }
            let bottom_indicator = bottom_cmd.id();
            // Position only if needed (visible_end < items.len())
            if visible_end < menu.items.len() {
                let pos = Vector2 {
                    x: origin.x,
                    y: origin.y + (vc as f32) * item_spacing,
                };
                if use_screen_space {
                    commands
                        .entity(bottom_indicator)
                        .insert(ScreenPosition { pos });
                } else {
                    commands
                        .entity(bottom_indicator)
                        .insert(MapPosition { pos });
                }
            }
            menu.bottom_indicator_entity = Some(bottom_indicator);
        }

        // Add a signals component to the menu entity for state tracking
        commands
            .entity(entity)
            .insert(Signals::default().with_flag("waiting_selection"));
        debug!(
            "menu_spawn_system: Added Signals component to menu entity {:?}",
            entity
        );

        // Spawn cursor entity if needed
        if let Some(cursor_entity) = menu.cursor_entity {
            debug!(
                "menu_spawn_system: Spawning cursor entity {:?} for menu {:?}",
                cursor_entity, entity
            );
            // Position cursor at selected item's viewport position
            let selected_viewport_index = menu.selected_index.saturating_sub(scroll_offset);
            let cursor_position = Vector2 {
                x: origin.x,
                y: origin.y + (selected_viewport_index as f32) * item_spacing,
            };
            if use_screen_space {
                commands.entity(cursor_entity).insert(ScreenPosition {
                    pos: cursor_position,
                });
            } else {
                commands.entity(cursor_entity).insert(MapPosition {
                    pos: cursor_position,
                });
                commands.entity(cursor_entity).insert(ZIndex(23.0));
            }
            debug!(
                "menu_spawn_system: Positioned cursor entity {:?} at {:?}",
                cursor_entity, cursor_position
            );
        }
        debug!(
            "menu_spawn_system: Spawned menu entity {:?} with {} items (visible_count={:?})",
            entity,
            menu.items.len(),
            visible_count
        );
    }
}

/// Despawns a specific menu entity and its related entities.
///
/// Removes menu item entities, cursor entity, indicator entities, and the
/// menu entity itself. Called via `world.run_system_with(system_id, entity)`.
///
/// # Parameters
///
/// - `target` - The menu entity to despawn
pub fn menu_despawn(
    In(target): In<Entity>,
    mut commands: Commands,
    query: Query<&Menu>,
    mut texture_store: ResMut<TextureStore>,
) {
    let Ok(menu) = query.get(target) else {
        warn!(
            "menu_despawn: Entity {:?} not found or has no Menu component",
            target
        );
        return;
    };

    // Despawn menu item entities and clean up textures
    for item in menu.items.iter() {
        // Remove texture if it exists (only non-dynamic items have textures)
        let texture_key = format!("menu_{}", item.id);
        texture_store.remove(&texture_key);

        if let Some(item_entity) = item.entity {
            commands.entity(item_entity).despawn();
        }
    }

    // Despawn indicator entities if applicable
    if let Some(top_entity) = menu.top_indicator_entity {
        commands.entity(top_entity).despawn();
    }
    if let Some(bottom_entity) = menu.bottom_indicator_entity {
        commands.entity(bottom_entity).despawn();
    }

    // Despawn cursor entity if applicable
    if let Some(cursor_entity) = menu.cursor_entity {
        commands.entity(cursor_entity).despawn();
    }

    // Finally despawn the menu entity itself
    commands.entity(target).despawn();
}

/// Handles input events to navigate menus and confirm selections.
///
/// Responds to secondary direction inputs (arrow keys) to move selection
/// and action buttons to confirm. Triggers [`MenuSelectionEvent`] when
/// an item is selected.
///
/// When `visible_count` is set, navigation is bounded (no wrap-around) and
/// scrolling occurs when selection moves outside the visible window.
pub fn menu_controller_observer(
    trigger: On<InputEvent>,
    mut query: Query<(Entity, &mut Menu, &mut Signals)>,
    mut dynamic_text_query: Query<&mut DynamicText>,
    mut commands: Commands,
    mut audio_cmds: MessageWriter<AudioCmd>,
) {
    for (entity, mut menu, mut signals) in query.iter_mut() {
        debug!(
            "menu_controller_observer: Handling input for menu entity {:?}",
            entity
        );
        if !menu.active {
            debug!(
                "menu_controller_observer: Menu entity {:?} is not active, skipping",
                entity
            );
            continue;
        }
        let event = trigger.event();
        if !event.pressed {
            debug!("menu_controller_observer: Input event is a release, skipping");
            continue; // Only handle key press, not release
        }

        let mut changed_selection = false;
        let mut needs_reposition = false;
        let old_selected_index = menu.selected_index;

        match event.action {
            InputAction::SecondaryDirectionUp => {
                if !menu.items.is_empty() {
                    if menu.visible_count.is_some() {
                        // Bounded navigation (no wrap-around when scrolling enabled)
                        if menu.selected_index > 0 {
                            menu.selected_index -= 1;
                            // Scroll up if selection above visible window
                            if menu.selected_index < menu.scroll_offset {
                                menu.scroll_offset = menu.selected_index;
                                needs_reposition = true;
                            }
                            changed_selection = true;
                        }
                    } else {
                        // Original wrap-around behavior
                        menu.selected_index =
                            (menu.selected_index + menu.items.len() - 1) % menu.items.len();
                        changed_selection = true;
                    }
                }
            }
            InputAction::SecondaryDirectionDown => {
                if !menu.items.is_empty() {
                    if let Some(visible_count) = menu.visible_count {
                        // Bounded navigation (no wrap-around when scrolling enabled)
                        if menu.selected_index < menu.items.len() - 1 {
                            menu.selected_index += 1;
                            // Scroll down if selection below visible window
                            if menu.selected_index >= menu.scroll_offset + visible_count {
                                menu.scroll_offset = menu.selected_index - visible_count + 1;
                                needs_reposition = true;
                            }
                            changed_selection = true;
                        }
                    } else {
                        // Original wrap-around behavior
                        menu.selected_index = (menu.selected_index + 1) % menu.items.len();
                        changed_selection = true;
                    }
                }
            }
            InputAction::Action1 | InputAction::Action2 => {
                if let Some(item) = menu.items.get(menu.selected_index) {
                    let selected_id = item.id.clone();
                    debug!(
                        "menu_controller_observer: Selection confirmed! item_id={}, triggering MenuSelectionEvent",
                        selected_id
                    );
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

        // Reposition items if scrolling occurred
        if needs_reposition {
            reposition_menu_items(&mut commands, &menu);
        }

        // Update cursor position and colors if applicable
        if changed_selection {
            // Update colors for old and new selected items (only for DynamicText)
            if let Some(old_item) = menu.items.get(old_selected_index) {
                if let Some(entity) = old_item.entity {
                    if let Ok(mut text) = dynamic_text_query.get_mut(entity) {
                        text.color = menu.normal_color;
                    }
                }
            }
            if let Some(new_item) = menu.items.get(menu.selected_index) {
                if let Some(entity) = new_item.entity {
                    if let Ok(mut text) = dynamic_text_query.get_mut(entity) {
                        text.color = menu.selected_color;
                    }
                }
            }

            if let Some(cursor_entity) = menu.cursor_entity {
                // Calculate cursor position based on visible viewport
                let viewport_index = menu.selected_index.saturating_sub(menu.scroll_offset);
                let cursor_position = Vector2 {
                    x: menu.origin.x,
                    y: menu.origin.y + (viewport_index as f32) * menu.item_spacing,
                };
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

/// Repositions menu items and indicators after scrolling.
///
/// Items within the visible window get position components added/updated,
/// while items outside the window have their position components removed.
fn reposition_menu_items(commands: &mut Commands, menu: &Menu) {
    let visible_count = menu.visible_count.unwrap_or(menu.items.len());
    let visible_end = (menu.scroll_offset + visible_count).min(menu.items.len());

    // Reposition all menu items
    for (i, item) in menu.items.iter().enumerate() {
        if let Some(entity) = item.entity {
            let is_visible = i >= menu.scroll_offset && i < visible_end;

            if is_visible {
                // Add/update position component
                let viewport_index = i - menu.scroll_offset;
                let new_pos = Vector2 {
                    x: menu.origin.x,
                    y: menu.origin.y + (viewport_index as f32) * menu.item_spacing,
                };
                if menu.use_screen_space {
                    commands
                        .entity(entity)
                        .insert(ScreenPosition { pos: new_pos });
                } else {
                    commands.entity(entity).insert(MapPosition { pos: new_pos });
                }
            } else {
                // Remove position component to hide (render system skips)
                if menu.use_screen_space {
                    commands.entity(entity).remove::<ScreenPosition>();
                } else {
                    commands.entity(entity).remove::<MapPosition>();
                }
            }
        }
    }

    // Update indicators
    let show_top = menu.scroll_offset > 0;
    let show_bottom = visible_end < menu.items.len();

    if let Some(top_entity) = menu.top_indicator_entity {
        if show_top {
            let pos = Vector2 {
                x: menu.origin.x,
                y: menu.origin.y - menu.item_spacing,
            };
            if menu.use_screen_space {
                commands.entity(top_entity).insert(ScreenPosition { pos });
            } else {
                commands.entity(top_entity).insert(MapPosition { pos });
            }
        } else {
            if menu.use_screen_space {
                commands.entity(top_entity).remove::<ScreenPosition>();
            } else {
                commands.entity(top_entity).remove::<MapPosition>();
            }
        }
    }

    if let Some(bottom_entity) = menu.bottom_indicator_entity {
        if show_bottom {
            let pos = Vector2 {
                x: menu.origin.x,
                y: menu.origin.y + (visible_count as f32) * menu.item_spacing,
            };
            if menu.use_screen_space {
                commands
                    .entity(bottom_entity)
                    .insert(ScreenPosition { pos });
            } else {
                commands.entity(bottom_entity).insert(MapPosition { pos });
            }
        } else {
            if menu.use_screen_space {
                commands.entity(bottom_entity).remove::<ScreenPosition>();
            } else {
                commands.entity(bottom_entity).remove::<MapPosition>();
            }
        }
    }
}

/// Executes the action associated with a selected menu item.
///
/// If the menu has an `on_select_callback`, invokes the Lua callback with a
/// context table containing `menu_id`, `item_id`, and `item_index`. When a
/// callback is set, `MenuActions` are ignored (callback takes full control).
///
/// Otherwise, looks up the [`MenuAction`] for the selected item and performs it:
/// - [`MenuAction::SetScene`] – triggers scene switch
/// - [`MenuAction::QuitGame`] – transitions to quitting state
/// - [`MenuAction::ShowSubMenu`] – displays a sub-menu (TODO)
/// - [`MenuAction::Noop`] – does nothing
pub fn menu_selection_observer(
    trigger: On<MenuSelectionEvent>,
    mut commands: Commands,
    menus: Query<(&Menu, Option<&MenuActions>)>,
    mut world_signals: ResMut<WorldSignals>,
    mut next_game_state: ResMut<NextGameState>,
    systems_store: Res<SystemsStore>,
    lua_runtime: NonSend<LuaRuntime>,
) {
    let event = trigger.event();
    debug!(
        "menu_selection_observer: Received MenuSelectionEvent for menu {:?}, item_id={}",
        event.menu, event.item_id
    );

    let Ok((menu, menu_actions_opt)) = menus.get(event.menu) else {
        warn!(
            "menu_selection_observer: Menu entity {:?} not found",
            event.menu
        );
        return;
    };

    // If menu has a Lua callback, invoke it
    if let Some(ref callback_name) = menu.on_select_callback {
        if lua_runtime.has_function(callback_name) {
            // Build context table
            let ctx = lua_runtime.lua().create_table().unwrap();
            ctx.set("menu_id", event.menu.to_bits()).unwrap();
            ctx.set("item_id", event.item_id.clone()).unwrap();

            // Find item index
            let item_index = menu
                .items
                .iter()
                .position(|item| item.id == event.item_id)
                .unwrap_or(0);
            ctx.set("item_index", item_index).unwrap();

            if let Err(e) = lua_runtime.call_function::<_, ()>(callback_name, ctx) {
                error!(target: "lua", "Error in menu callback '{}': {}", callback_name, e);
            }
        } else {
            warn!(target: "lua", "menu callback '{}' not found", callback_name);
        }
        return; // Callback handles everything, skip MenuActions
    }

    // Fallback to existing MenuActions logic
    let Some(menu_actions) = menu_actions_opt else {
        warn!(
            "menu_selection_observer: No MenuActions found for menu entity {:?}, item_id {:?}",
            event.menu, event.item_id
        );
        return;
    };

    debug!(
        "menu_selection_observer: Found MenuActions, looking up action for item_id={}",
        event.item_id
    );
    match menu_actions.get(&event.item_id) {
        MenuAction::SetScene(scene_name) => {
            info!(
                "menu_selection_observer: SetScene action found, scene_name={}",
                scene_name
            );
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
