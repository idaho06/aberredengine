//! Menu systems.
//!
//! This module provides systems for interactive menus:
//! - [`menu_spawn_system`] – spawns menu item entities when a [`Menu`] is added
//! - [`menu_despawn`] – despawns menu entities and their items
//! - [`menu_controller_observer`] – handles input to navigate and select items
//! - [`menu_selection_observer`] – performs actions when items are selected
//!
//! Callbacks receive `&mut `[`GameCtx`](crate::systems::GameCtx) for full ECS access.

use std::sync::Arc;

use crate::components::dynamictext::DynamicText;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::menu::{Menu, MenuAction, MenuActions};
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::events::input::{InputAction, InputEvent};
use crate::events::menu::MenuSelectionEvent;
use crate::protocol::audio::AudioCmd;
use crate::protocol::render_assets::RenderAssetCmd;
use crate::resources::fontmetrics::{FontMetricsStore, FontMetricsWarnCache};
use crate::resources::gamestate::GameStates::Quitting;
use crate::resources::gamestate::NextGameState;
use crate::resources::signal_keys as sk;
use crate::resources::systemsstore as hook_keys;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturedims::TextureDimsStore;
use crate::systems::GameCtx;
use bevy_ecs::prelude::*;
use log::{debug, warn};
use crate::math::Vec2;

/// Z-index applied to menu elements (world-space or screen-space) so they render
/// above other entities at the default z=0. World-space and screen-space menus
/// never share a draw buffer or sort together, so reusing one constant for both
/// is safe.
const MENU_Z_INDEX: f32 = 23.0;

/// Inserts the [`ZIndex`] every menu element needs to render, in either space.
/// Single source of truth for this requirement, so every menu element type
/// uses the same render layer.
fn insert_menu_zindex(ecmd: &mut EntityCommands) {
    ecmd.insert(ZIndex(MENU_Z_INDEX));
}

/// Inserts [`ScreenPosition`] or [`MapPosition`] depending on `use_screen_space`.
fn set_menu_position(ecmd: &mut EntityCommands, use_screen_space: bool, pos: Vec2) {
    if use_screen_space {
        ecmd.insert(ScreenPosition::from_vec(pos));
    } else {
        ecmd.insert(MapPosition::from_vec(pos));
    }
}

/// Removes [`ScreenPosition`] or [`MapPosition`] depending on `use_screen_space`.
fn clear_menu_position(ecmd: &mut EntityCommands, use_screen_space: bool) {
    if use_screen_space {
        ecmd.remove::<ScreenPosition>();
    } else {
        ecmd.remove::<MapPosition>();
    }
}

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
    font_metrics: Res<FontMetricsStore>,
    mut warn_cache: ResMut<FontMetricsWarnCache>,
    mut render_asset_cmd_writer: MessageWriter<RenderAssetCmd>,
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
                // Static text sprite: measure via FontMetricsStore (CPU-side),
                // queue rasterization for process_render_asset_cmds
                // (render-destined).
                let Some(metrics) = font_metrics.0.get(&font_string) else {
                    if warn_cache.warn_once(&font_string) {
                        warn!(
                            "menu_spawn_system: skipping menu item '{}' because font '{}' has no cached metrics",
                            menu_item.id, font_string
                        );
                    }
                    continue;
                };
                let size = metrics.measure_text(&menu_item.label, font_size, 1.0);
                let key = format!("menu_{}", menu_item.id);
                render_asset_cmd_writer.write(RenderAssetCmd::RasterizeText {
                    key: key.clone(),
                    font_key: font_string.clone(),
                    text: menu_item.label.clone(),
                    font_size,
                    spacing: 1.0,
                    color: normal_color,
                });
                ecmd.insert(Sprite {
                    tex_key: Arc::from(key),
                    width: size.x,
                    height: size.y,
                    offset: Vec2 { x: 0.0, y: 0.0 },
                    origin: Vec2 { x: 0.0, y: 0.0 },
                    flip_h: false,
                    flip_v: false,
                });
                debug!(
                    "menu_spawn_system: queued RasterizeText for menu item id={}, size=({}, {})",
                    menu_item.id, size.x, size.y
                );
            }

            // Add to ALL items, visible or not (needed once they become visible).
            insert_menu_zindex(&mut ecmd);

            // Only add position component for visible items
            let is_visible = i >= scroll_offset && i < visible_end;
            if is_visible {
                // Calculate position within visible viewport
                let viewport_index = i - scroll_offset;
                let pos = Vec2 {
                    x: origin.x,
                    y: origin.y + (viewport_index as f32) * item_spacing,
                };
                set_menu_position(&mut ecmd, use_screen_space, pos);
            }
            // Non-visible items don't get position component, so render system skips them

            let text_entity = ecmd.id();
            ecmd.insert(Group::new(format!("menu_{}", entity)));
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
            top_cmd.insert(Group::new(format!("menu_{}", entity)));
            // Add always (needed once the indicator becomes visible).
            insert_menu_zindex(&mut top_cmd);
            let top_indicator = top_cmd.id();
            // Position only if needed (scroll_offset > 0)
            if scroll_offset > 0 {
                let pos = Vec2 {
                    x: origin.x,
                    y: origin.y - item_spacing,
                };
                set_menu_position(&mut commands.entity(top_indicator), use_screen_space, pos);
            }
            menu.top_indicator_entity = Some(top_indicator);

            // Bottom indicator (shown when more items below)
            let mut bottom_cmd = commands.spawn(DynamicText::new(
                "...",
                font_string.clone(),
                font_size,
                normal_color,
            ));
            bottom_cmd.insert(Group::new(format!("menu_{}", entity)));
            // Add always (needed once the indicator becomes visible).
            insert_menu_zindex(&mut bottom_cmd);
            let bottom_indicator = bottom_cmd.id();
            // Position only if needed (visible_end < items.len())
            if visible_end < menu.items.len() {
                let pos = Vec2 {
                    x: origin.x,
                    y: origin.y + (vc as f32) * item_spacing,
                };
                set_menu_position(
                    &mut commands.entity(bottom_indicator),
                    use_screen_space,
                    pos,
                );
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
            let cursor_position = Vec2 {
                x: origin.x,
                y: origin.y + (selected_viewport_index as f32) * item_spacing,
            };
            let mut cursor_cmd = commands.entity(cursor_entity);
            set_menu_position(&mut cursor_cmd, use_screen_space, cursor_position);
            insert_menu_zindex(&mut cursor_cmd);
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
    mut render_asset_writer: MessageWriter<RenderAssetCmd>,
    mut texture_dims: ResMut<TextureDimsStore>,
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
        // Remove the rasterized label texture if it exists (only non-dynamic
        // items have one). Queued as a RenderAssetCmd: TextureStore
        // is render-world-only, so even the GL-free `remove()` bookkeeping
        // can't be called from logic-side cleanup. The dims mirror is
        // logic-owned and dropped directly.
        let texture_key = format!("menu_{}", item.id);
        texture_dims.remove(&texture_key);
        render_asset_writer.write(RenderAssetCmd::RemoveTexture { key: texture_key });

        if let Some(item_entity) = item.entity {
            commands.entity(item_entity).try_despawn();
        }
    }

    // Despawn indicator entities if applicable
    if let Some(top_entity) = menu.top_indicator_entity {
        commands.entity(top_entity).try_despawn();
    }
    if let Some(bottom_entity) = menu.bottom_indicator_entity {
        commands.entity(bottom_entity).try_despawn();
    }

    // Despawn cursor entity if applicable
    if let Some(cursor_entity) = menu.cursor_entity {
        commands.entity(cursor_entity).try_despawn();
    }

    // Finally despawn the menu entity itself
    commands.entity(target).try_despawn();
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
            InputAction::SecondaryDirectionUp if !menu.items.is_empty() => {
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
            InputAction::SecondaryDirectionDown if !menu.items.is_empty() => {
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
            if let Some(old_item) = menu.items.get(old_selected_index)
                && let Some(entity) = old_item.entity
                && let Ok(mut text) = dynamic_text_query.get_mut(entity)
            {
                text.color = menu.normal_color;
            }
            if let Some(new_item) = menu.items.get(menu.selected_index)
                && let Some(entity) = new_item.entity
                && let Ok(mut text) = dynamic_text_query.get_mut(entity)
            {
                text.color = menu.selected_color;
            }

            if let Some(cursor_entity) = menu.cursor_entity {
                // Calculate cursor position based on visible viewport
                let viewport_index = menu.selected_index.saturating_sub(menu.scroll_offset);
                let cursor_position = Vec2 {
                    x: menu.origin.x,
                    y: menu.origin.y + (viewport_index as f32) * menu.item_spacing,
                };
                set_menu_position(
                    &mut commands.entity(cursor_entity),
                    menu.use_screen_space,
                    cursor_position,
                );
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
                let new_pos = Vec2 {
                    x: menu.origin.x,
                    y: menu.origin.y + (viewport_index as f32) * menu.item_spacing,
                };
                set_menu_position(&mut commands.entity(entity), menu.use_screen_space, new_pos);
            } else {
                // Remove position component to hide (render system skips)
                clear_menu_position(&mut commands.entity(entity), menu.use_screen_space);
            }
        }
    }

    // Update indicators
    let show_top = menu.scroll_offset > 0;
    let show_bottom = visible_end < menu.items.len();

    if let Some(top_entity) = menu.top_indicator_entity {
        if show_top {
            let pos = Vec2 {
                x: menu.origin.x,
                y: menu.origin.y - menu.item_spacing,
            };
            set_menu_position(&mut commands.entity(top_entity), menu.use_screen_space, pos);
        } else {
            clear_menu_position(&mut commands.entity(top_entity), menu.use_screen_space);
        }
    }

    if let Some(bottom_entity) = menu.bottom_indicator_entity {
        if show_bottom {
            let pos = Vec2 {
                x: menu.origin.x,
                y: menu.origin.y + (visible_count as f32) * menu.item_spacing,
            };
            set_menu_position(
                &mut commands.entity(bottom_entity),
                menu.use_screen_space,
                pos,
            );
        } else {
            clear_menu_position(&mut commands.entity(bottom_entity), menu.use_screen_space);
        }
    }
}

/// Executes the action associated with a selected menu item.
///
/// Priority chain: Rust callback → [`MenuActions`].
///
/// If the menu has an `on_rust_callback`, invokes it with the menu entity, item ID,
/// item index, and full ECS access via [`GameCtx`](crate::systems::GameCtx).
///
/// Otherwise, looks up the [`MenuAction`] for the selected item and performs it:
/// - [`MenuAction::SetScene`] – triggers scene switch
/// - [`MenuAction::QuitGame`] – transitions to quitting state
/// - [`MenuAction::ShowSubMenu`] – displays a sub-menu (TODO)
/// - [`MenuAction::Noop`] – does nothing
///
/// The facade's `aberredengine::systems::menu` module shadows this with a
/// variant that checks `on_select_callback` (Lua) first, under
/// `#[cfg(feature = "lua")]` -- `aberred-core` cannot name `LuaRuntime`.
pub fn menu_selection_observer(
    trigger: On<MenuSelectionEvent>,
    menus: Query<(&Menu, Option<&MenuActions>)>,
    mut next_game_state: ResMut<NextGameState>,
    systems_store: Res<SystemsStore>,
    mut ctx: GameCtx,
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

    // Priority 1: Rust callback
    if let Some(cb) = menu.on_rust_callback {
        let item_index = menu
            .items
            .iter()
            .position(|item| item.id == event.item_id)
            .unwrap_or(0);
        cb(event.menu, &event.item_id, item_index, &mut ctx);
        return;
    }

    // Priority 2: MenuActions
    dispatch_menu_action(
        menu_actions_opt,
        event,
        &mut ctx,
        &mut next_game_state,
        &systems_store,
    );
}

/// `pub` (not `pub(crate)`) so the facade's Lua-priority
/// `menu_selection_observer` (which cannot live in core -- it names
/// `LuaRuntime`) can still reach `MenuActions` dispatch for its priority-3
/// fallback.
pub fn dispatch_menu_action(
    menu_actions_opt: Option<&MenuActions>,
    event: &MenuSelectionEvent,
    ctx: &mut GameCtx,
    next_game_state: &mut ResMut<NextGameState>,
    systems_store: &Res<SystemsStore>,
) {
    let Some(menu_actions) = menu_actions_opt else {
        warn!(
            "menu_selection_observer: No MenuActions found for item_id {:?}",
            event.item_id
        );
        return;
    };

    debug!(
        "menu_selection_observer: Found MenuActions, looking up action for item_id={}",
        event.item_id
    );
    match menu_actions.get(&event.item_id) {
        MenuAction::SetScene(scene_name) => {
            debug!(
                "menu_selection_observer: SetScene action found, scene_name={}",
                scene_name
            );
            ctx.world_signals.set_string(sk::SCENE, scene_name.clone());
            // Startup validation only requires a switch_scene hook for Lua / scene-manager /
            // custom-hook games, so a Rust-only game can reach this without one: log, don't panic.
            match systems_store.get(hook_keys::SWITCH_SCENE) {
                Some(switch_scene) => ctx.commands.run_system(*switch_scene),
                None => log::error!(
                    "menu action SetScene('{scene_name}'): no switch_scene system is registered \
                     (use Lua, the scene manager, or EngineBuilder::on_switch_scene); ignoring"
                ),
            }
        }
        MenuAction::ShowSubMenu(submenu_name) => {
            ctx.world_signals
                .set_string("show_submenu", submenu_name.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use crate::math::{Color, Vec2};
    use crate::resources::gamestate::{GameStates, NextGameStates};
    use crate::resources::worldsignals::WorldSignals;

    use crate::resources::fontmetrics::test_support::lowercase_alphabet_metrics;

    fn new_test_world() -> World {
        let mut world = World::new();

        let mut store = FontMetricsStore::default();
        store
            .0
            .insert("test_font".to_string(), lowercase_alphabet_metrics());
        world.insert_resource(store);
        world.insert_resource(FontMetricsWarnCache::default());
        world.insert_resource(Messages::<RenderAssetCmd>::default());
        world
    }

    #[derive(Resource, Default)]
    struct Selections(Vec<(Entity, String)>);

    fn record_selection(trigger: On<MenuSelectionEvent>, mut s: ResMut<Selections>) {
        let e = trigger.event();
        s.0.push((e.menu, e.item_id.clone()));
    }

    /// Spawns `menu`, runs `menu_spawn_system`, and wires the input controller.
    fn spawn_menu(menu: Menu) -> (World, Entity) {
        let mut world = new_test_world();
        world.insert_resource(Messages::<AudioCmd>::default());
        world.init_resource::<Selections>();
        world.add_observer(menu_controller_observer);
        world.add_observer(record_selection);
        let e = world.spawn(menu).id();
        world.run_system_once(menu_spawn_system).unwrap();
        (world, e)
    }

    fn press(world: &mut World, action: InputAction) {
        world.trigger(InputEvent { action, pressed: true });
        world.flush();
    }

    fn five_items() -> Menu {
        Menu::new(
            &[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D"), ("e", "E")],
            Vec2::new(100.0, 50.0),
            "test_font",
            12.0,
            10.0,
            true,
        )
    }

    fn item(world: &World, menu: Entity, i: usize) -> Entity {
        world.get::<Menu>(menu).unwrap().items[i].entity.unwrap()
    }

    fn screen_pos(world: &World, e: Entity) -> Option<Vec2> {
        world.get::<ScreenPosition>(e).map(|p| p.pos)
    }

    fn visible_items(world: &World, menu: Entity) -> Vec<Option<Vec2>> {
        (0..world.get::<Menu>(menu).unwrap().items.len())
            .map(|i| screen_pos(world, item(world, menu, i)))
            .collect()
    }

    fn played_sounds(world: &mut World) -> usize {
        world.resource_mut::<Messages<AudioCmd>>().drain().count()
    }

    #[test]
    fn spawn_positions_only_the_visible_window_and_the_indicators() {
        let (world, menu) = spawn_menu(five_items().with_visible_count(2));
        assert_eq!(
            visible_items(&world, menu),
            [Some(Vec2::new(100.0, 50.0)), Some(Vec2::new(100.0, 60.0)), None, None, None]
        );
        let m = world.get::<Menu>(menu).unwrap();
        let (top, bottom) = (m.top_indicator_entity.unwrap(), m.bottom_indicator_entity.unwrap());
        assert_eq!(screen_pos(&world, top), None, "nothing above the first item");
        assert_eq!(screen_pos(&world, bottom), Some(Vec2::new(100.0, 70.0)), "more items below");
        for e in (0..5).map(|i| item(&world, menu, i)).chain([top, bottom]) {
            assert_eq!(world.get::<ZIndex>(e).unwrap().0, MENU_Z_INDEX, "hidden ones too");
            assert_eq!(world.get::<Group>(e).unwrap().0, format!("menu_{menu}"));
        }
        assert!(world.get::<Signals>(menu).unwrap().has_flag("waiting_selection"));
        assert_eq!(world.get::<DynamicText>(item(&world, menu, 0)).unwrap().color, Color::YELLOW);
        assert_eq!(world.get::<DynamicText>(item(&world, menu, 1)).unwrap().color, Color::WHITE);
    }

    #[test]
    fn world_space_menu_uses_map_position() {
        let mut menu = five_items();
        menu.use_screen_space = false;
        let (world, menu) = spawn_menu(menu);
        let first = item(&world, menu, 0);
        assert_eq!(world.get::<MapPosition>(first).unwrap().pos, Vec2::new(100.0, 50.0));
        assert!(world.get::<ScreenPosition>(first).is_none());
    }

    #[test]
    fn navigation_wraps_around_without_visible_count() {
        let (mut world, menu) = spawn_menu(five_items());
        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 4, "up from first wraps to last");
        press(&mut world, InputAction::SecondaryDirectionDown);
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 0, "down from last wraps to first");
    }

    #[test]
    fn scrolling_navigation_is_bounded_and_moves_the_window() {
        let (mut world, menu) = spawn_menu(
            five_items()
                .with_visible_count(2)
                .with_selection_sound("blip"),
        );

        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 0, "no wrap at the top");
        assert_eq!(played_sounds(&mut world), 0, "no change, no sound");

        press(&mut world, InputAction::SecondaryDirectionDown); // index 1, still in window
        press(&mut world, InputAction::SecondaryDirectionDown); // index 2, window scrolls to 1..=2
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!((m.selected_index, m.scroll_offset), (2, 1));
        let (top, bottom) = (m.top_indicator_entity.unwrap(), m.bottom_indicator_entity.unwrap());
        assert_eq!(
            visible_items(&world, menu),
            [None, Some(Vec2::new(100.0, 50.0)), Some(Vec2::new(100.0, 60.0)), None, None]
        );
        assert_eq!(screen_pos(&world, top), Some(Vec2::new(100.0, 40.0)), "items above now");
        assert_eq!(screen_pos(&world, bottom), Some(Vec2::new(100.0, 70.0)));
        assert_eq!(played_sounds(&mut world), 2, "one sound per selection change");

        for _ in 0..5 {
            press(&mut world, InputAction::SecondaryDirectionDown);
        }
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!((m.selected_index, m.scroll_offset), (4, 3), "stops at the last item");
        let bottom = m.bottom_indicator_entity.unwrap();
        assert_eq!(screen_pos(&world, bottom), None, "nothing below the last item");
    }

    #[test]
    fn selection_change_recolors_items_and_moves_the_cursor() {
        let mut world = new_test_world();
        let cursor = world.spawn_empty().id();
        let menu_c = five_items().with_cursor(cursor);
        // spawn_menu builds its own world, so wire this one by hand.
        world.insert_resource(Messages::<AudioCmd>::default());
        world.init_resource::<Selections>();
        world.add_observer(menu_controller_observer);
        let menu = world.spawn(menu_c).id();
        world.run_system_once(menu_spawn_system).unwrap();
        assert_eq!(screen_pos(&world, cursor), Some(Vec2::new(100.0, 50.0)));

        press(&mut world, InputAction::SecondaryDirectionDown);
        let color = |w: &World, i| w.get::<DynamicText>(item(w, menu, i)).unwrap().color;
        assert_eq!((color(&world, 0), color(&world, 1)), (Color::WHITE, Color::YELLOW));
        assert_eq!(screen_pos(&world, cursor), Some(Vec2::new(100.0, 60.0)));
    }

    #[test]
    fn confirming_deactivates_the_menu_and_triggers_selection_once() {
        let (mut world, menu) = spawn_menu(five_items());
        press(&mut world, InputAction::SecondaryDirectionDown);
        world.trigger(InputEvent {
            action: InputAction::Action1,
            pressed: false,
        });
        world.flush();
        assert!(world.resource::<Selections>().0.is_empty(), "releases are ignored");

        press(&mut world, InputAction::Action1);
        assert_eq!(world.resource::<Selections>().0, [(menu, "b".to_string())]);
        let signals = world.get::<Signals>(menu).unwrap();
        assert!(!signals.has_flag("waiting_selection"));
        assert_eq!(signals.get_string("selected_item").map(String::as_str), Some("b"));
        assert!(!world.get::<Menu>(menu).unwrap().active);

        press(&mut world, InputAction::Action2);
        press(&mut world, InputAction::SecondaryDirectionDown);
        assert_eq!(world.resource::<Selections>().0.len(), 1, "an inactive menu ignores input");
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 1);
    }

    #[test]
    fn menu_despawn_removes_all_menu_entities_and_label_textures() {
        let mut world = new_test_world();
        let cursor = world.spawn_empty().id();
        let bystander = world.spawn_empty().id();
        let menu = world
            .spawn(
                Menu::new(&[("play", "play"), ("quit", "quit")], Vec2::ZERO, "test_font", 12.0, 10.0, true)
                    .with_dynamic_text(false)
                    .with_cursor(cursor)
                    .with_visible_count(1),
            )
            .id();
        world.run_system_once(menu_spawn_system).unwrap();
        let mut dims = TextureDimsStore::default();
        dims.insert("menu_play", 10, 10);
        dims.insert("menu_quit", 10, 10);
        dims.insert("unrelated", 10, 10);
        world.insert_resource(dims);
        let m = world.get::<Menu>(menu).unwrap().clone();
        let spawned: Vec<Entity> = m
            .items
            .iter()
            .filter_map(|i| i.entity)
            .chain([m.top_indicator_entity.unwrap(), m.bottom_indicator_entity.unwrap(), cursor, menu])
            .collect();
        world.resource_mut::<Messages<RenderAssetCmd>>().clear();

        world.run_system_once_with(menu_despawn, menu).unwrap();

        for e in spawned {
            assert!(world.get_entity(e).is_err(), "{e:?} despawned");
        }
        assert!(world.get_entity(bystander).is_ok());
        let dims = world.resource::<TextureDimsStore>();
        assert!(dims.get("menu_play").is_none() && dims.get("menu_quit").is_none());
        assert!(dims.get("unrelated").is_some());
        let removed: Vec<String> = world
            .resource_mut::<Messages<RenderAssetCmd>>()
            .drain()
            .filter_map(|c| match c {
                RenderAssetCmd::RemoveTexture { key } => Some(key),
                _ => None,
            })
            .collect();
        assert_eq!(removed, ["menu_play", "menu_quit"]);
    }

    #[test]
    fn menu_despawn_of_a_non_menu_entity_does_nothing() {
        let mut world = new_test_world();
        world.insert_resource(TextureDimsStore::default());
        let plain = world.spawn_empty().id();
        world.run_system_once_with(menu_despawn, plain).unwrap();
        assert!(world.get_entity(plain).is_ok());
        assert_eq!(world.resource_mut::<Messages<RenderAssetCmd>>().drain().count(), 0);
    }

    fn record_rust_callback(_menu: Entity, id: &str, index: usize, ctx: &mut GameCtx) {
        ctx.world_signals.set_string("rust_cb", format!("{id}:{index}"));
    }

    fn mark_switch_scene(mut ws: ResMut<WorldSignals>) {
        ws.set_flag("switch_hook_ran");
    }

    /// World wired for `menu_selection_observer`, with a registered switch_scene hook.
    fn selection_world() -> World {
        let mut world = World::new();
        crate::testing::insert_game_ctx_resources(&mut world);
        world.init_resource::<NextGameState>();
        let mut store = SystemsStore::default();
        store.insert(hook_keys::SWITCH_SCENE, world.register_system(mark_switch_scene));
        world.insert_resource(store);
        world.add_observer(menu_selection_observer);
        world
    }

    fn select(world: &mut World, menu: Entity, item: &str) {
        world.trigger(MenuSelectionEvent { menu, item_id: item.to_string() });
        world.flush();
    }

    fn actions() -> MenuActions {
        MenuActions::new()
            .with("play", MenuAction::SetScene("level01".into()))
            .with("options", MenuAction::ShowSubMenu("options_menu".into()))
            .with("quit", MenuAction::QuitGame)
    }

    fn three_items() -> Menu {
        Menu::new(&[("play", "P"), ("options", "O"), ("quit", "Q")], Vec2::ZERO, "f", 12.0, 10.0, true)
    }

    #[test]
    fn selection_prefers_the_rust_callback_over_menu_actions() {
        let mut world = selection_world();
        let menu = world
            .spawn((three_items().with_on_rust_callback(record_rust_callback), actions()))
            .id();
        select(&mut world, menu, "options");
        let ws = world.resource::<WorldSignals>();
        assert_eq!(ws.get_string("rust_cb").map(String::as_str), Some("options:1"));
        assert!(ws.get_string("show_submenu").is_none(), "MenuActions not consulted");
    }

    #[test]
    fn selection_dispatches_each_menu_action() {
        let mut world = selection_world();
        let menu = world.spawn((three_items(), actions())).id();

        select(&mut world, menu, "play");
        let ws = world.resource::<WorldSignals>();
        assert_eq!(ws.get_string(sk::SCENE).map(String::as_str), Some("level01"));
        assert!(ws.has_flag("switch_hook_ran"), "switch_scene hook was run");

        select(&mut world, menu, "options");
        assert_eq!(
            world.resource::<WorldSignals>().get_string("show_submenu").map(String::as_str),
            Some("options_menu")
        );

        assert_eq!(world.resource::<NextGameState>().get(), &NextGameStates::Unchanged);
        select(&mut world, menu, "quit");
        assert_eq!(
            world.resource::<NextGameState>().get(),
            &NextGameStates::Pending(GameStates::Quitting)
        );
    }

    #[test]
    fn selection_without_an_action_or_actions_component_is_a_noop() {
        let mut world = selection_world();
        let with_actions = world.spawn((three_items(), MenuActions::new())).id();
        let without = world.spawn(three_items()).id();
        select(&mut world, with_actions, "play");
        select(&mut world, without, "play");
        let ws = world.resource::<WorldSignals>();
        assert!(ws.get_string(sk::SCENE).is_none() && !ws.has_flag("switch_hook_ran"));
        assert_eq!(world.resource::<NextGameState>().get(), &NextGameStates::Unchanged);
    }

    #[test]
    fn set_scene_without_a_switch_scene_hook_logs_instead_of_panicking() {
        // A Rust-only game with no Lua, no scene manager and no custom switch hook.
        let mut world = World::new();
        crate::testing::insert_game_ctx_resources(&mut world);
        world.init_resource::<NextGameState>();
        world.insert_resource(SystemsStore::default());
        world.add_observer(menu_selection_observer);
        let menu = world.spawn((three_items(), actions())).id();

        select(&mut world, menu, "play");

        assert_eq!(
            world.resource::<WorldSignals>().get_string(sk::SCENE).map(String::as_str),
            Some("level01"),
            "the requested scene is still recorded"
        );
    }

    #[test]
    fn static_label_queues_rasterize_text_and_sizes_sprite() {
        let mut world = new_test_world();
        world.spawn(
            Menu::new(
                &[("ok", "ok")],
                Vec2::ZERO,
                "test_font",
                20.0,
                4.0,
                false,
            )
            .with_dynamic_text(false),
        );

        world
            .run_system_once(menu_spawn_system)
            .expect("system should run");
        world.resource_mut::<Messages<RenderAssetCmd>>().update();

        let mut reader_state =
            bevy_ecs::system::SystemState::<MessageReader<RenderAssetCmd>>::new(&mut world);
        let cmds: Vec<RenderAssetCmd> = {
            let mut reader = reader_state
                .get_mut(&mut world)
                .expect("render asset reader should fetch");
            reader.read().cloned().collect()
        };
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            RenderAssetCmd::RasterizeText {
                key,
                font_key,
                text,
                font_size,
                ..
            } => {
                assert_eq!(key, "menu_ok");
                assert_eq!(font_key, "test_font");
                assert_eq!(text, "ok");
                assert_eq!(*font_size, 20.0_f32);
            }
            other => panic!("expected RasterizeText, got {other:?}"),
        }

        let mut sprite_query = world.query::<&Sprite>();
        let sprite = sprite_query
            .iter(&world)
            .next()
            .expect("sprite should be spawned");
        // "ok" @ scale 1.0, spacing 1.0: 2*10*1.0 + (2-1)*1.0 = 21.0
        assert_eq!(sprite.width, 21.0);
        assert_eq!(sprite.height, 20.0);
        assert_eq!(&*sprite.tex_key, "menu_ok");
    }

    #[test]
    fn missing_font_metrics_skips_item_and_queues_nothing() {
        let mut world = new_test_world();
        world.spawn(
            Menu::new(
                &[("ok", "ok")],
                Vec2::ZERO,
                "missing_font",
                20.0,
                4.0,
                false,
            )
            .with_dynamic_text(false),
        );

        world
            .run_system_once(menu_spawn_system)
            .expect("system should run");
        world.resource_mut::<Messages<RenderAssetCmd>>().update();

        let mut reader_state =
            bevy_ecs::system::SystemState::<MessageReader<RenderAssetCmd>>::new(&mut world);
        let cmds: Vec<RenderAssetCmd> = {
            let mut reader = reader_state
                .get_mut(&mut world)
                .expect("render asset reader should fetch");
            reader.read().cloned().collect()
        };
        assert!(cmds.is_empty());

        let mut sprite_query = world.query::<&Sprite>();
        assert!(sprite_query.iter(&world).next().is_none());
    }
}
