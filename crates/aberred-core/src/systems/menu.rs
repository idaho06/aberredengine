//! Menu systems.
//!
//! This module provides systems for interactive menus:
//! - [`menu_spawn_system`] – spawns menu item entities when a [`Menu`] is added
//! - [`menu_despawn`] – despawns menu entities and their items
//! - [`menu_controller_observer`] – handles input to navigate and select items
//! - [`menu_selection_observer`] – performs [`MenuActions`] when items are selected
//!
//! Game code reacts to a confirmed item by observing
//! [`MenuSelected`] on the menu entity.

use std::sync::Arc;

use crate::components::dynamic_text::DynamicText;
use crate::components::group::Group;
use crate::components::menu::{
    Menu, MenuAction, MenuActions, MenuItem, MenuRebuild, MenuStep, ensure_visible, first_enabled,
    step_selection,
};
use crate::components::screen_position::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::world_position::WorldPosition;
use crate::components::z_index::ZIndex;
use crate::events::input::InputActionChanged;
use crate::events::menu::{MenuSelected, MenuSelectionChanged};
use crate::math::Vec2;
use crate::protocol::audio::AudioCmd;
use crate::protocol::render_assets::RenderAssetCmd;
use crate::resources::font_metrics::{FontMetrics, FontMetricsStore, FontMetricsWarnCache};
use crate::resources::game_state::GameState::Quitting;
use crate::resources::game_state::NextGameState;
use crate::resources::input::InputAction;
use crate::resources::next_scene::NextScene;
use crate::resources::pending_assets::PendingAssets;
use crate::resources::texture_dims::TextureDimsStore;
use bevy_ecs::prelude::*;
use log::{debug, warn};

/// Z-index applied to menu elements (world-space or screen-space) so they render
/// above other entities at the default z=0. World-space and screen-space menus
/// never share a draw buffer or sort together, so reusing one constant for both
/// is safe.
const MENU_Z_INDEX: f32 = 23.0;

/// Inserts the [`ZIndex`] every menu element needs to render, in either space.
/// Single source of truth for this requirement, so every menu element type
/// uses the same render layer.
fn insert_menu_z_index(ecmd: &mut EntityCommands) {
    ecmd.insert(ZIndex(MENU_Z_INDEX));
}

/// Inserts [`ScreenPosition`] or [`WorldPosition`] depending on `use_screen_space`.
fn set_menu_position(ecmd: &mut EntityCommands, use_screen_space: bool, pos: Vec2) {
    if use_screen_space {
        ecmd.insert(ScreenPosition::from_vec(pos));
    } else {
        ecmd.insert(WorldPosition::from_vec(pos));
    }
}

/// Removes [`ScreenPosition`] or [`WorldPosition`] depending on `use_screen_space`.
fn clear_menu_position(ecmd: &mut EntityCommands, use_screen_space: bool) {
    if use_screen_space {
        ecmd.remove::<ScreenPosition>();
    } else {
        ecmd.remove::<WorldPosition>();
    }
}

/// Name of the [`Group`] every entity spawned for `entity` joins, so a menu's
/// parts are found and despawned together.
fn menu_group_name(entity: Entity) -> String {
    format!("menu_{entity}")
}

/// Position of a menu element on `row`: `origin.x + indent` horizontally and
/// `origin.y + row * item_spacing` vertically. `row` is the viewport row of an
/// item or the cursor, -1 for the top "..." indicator and `visible_count` for
/// the bottom one; the cursor passes an `indent` of 0.0 to sit at `origin.x`.
fn menu_row_pos(origin: Vec2, row: i32, item_spacing: f32, indent: f32) -> Vec2 {
    Vec2::new(origin.x + indent, origin.y + (row as f32) * item_spacing)
}

/// The menu font resolved for measuring or rasterizing: the resolved key
/// (after the pending-asset fallback) and its cached [`FontMetrics`], `None`
/// when the resolved font has none.
fn resolve_menu_font<'a, 'k>(
    font_metrics: &'a FontMetricsStore,
    font_key: &'k str,
    pending: &PendingAssets,
    warn_cache: &mut FontMetricsWarnCache,
) -> (&'k str, Option<&'a FontMetrics>) {
    let resolved = font_metrics.resolve_key(font_key, pending, warn_cache);
    (resolved, font_metrics.0.get(resolved))
}

/// Menus `menu_spawn_system` handles: freshly added, or carrying a newly added
/// [`MenuRebuild`] marker.
type MenuSpawnQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Menu, Option<&'static mut MenuRebuild>),
    Or<(Added<Menu>, Added<MenuRebuild>)>,
>;

/// Spawns entities for newly added [`Menu`] components.
///
/// For each menu item, spawns a text entity (either [`DynamicText`] or a
/// static sprite) and positions it at `origin.x + item_indent` in world or
/// screen space. When `cursor_text` is set, spawns a [`DynamicText`] cursor
/// entity holding that text (stored in `cursor_entity`, overriding a
/// caller-supplied one) and measures `item_indent` as the width of
/// `"{cursor_text} "` in the menu font; the cursor itself sits at `origin.x`
/// on the selected row, or is left unpositioned when the selected item is
/// disabled. Without `cursor_text`, `item_indent` is 0.0 and a
/// caller-supplied cursor entity is positioned as before.
///
/// When `visible_count` is set, only positions items within the visible window
/// and spawns "..." indicator entities for scrolling, at the same indented x.
///
/// An entity carrying a [`MenuRebuild`] marker has its items replaced in
/// place first: the old item and indicator entities are despawned, the
/// replacement items inherit the current `dynamic_text` mode, the menu is
/// re-activated with the first enabled item selected and the scroll reset,
/// `item_indent` is re-measured, and the marker is removed once applied.
pub fn menu_spawn_system(
    mut commands: Commands,
    mut query: MenuSpawnQuery,
    font_metrics: Res<FontMetricsStore>,
    pending: Res<PendingAssets>,
    mut warn_cache: ResMut<FontMetricsWarnCache>,
    mut render_asset_cmd_writer: MessageWriter<RenderAssetCmd>,
    mut texture_dims: ResMut<TextureDimsStore>,
) {
    for (entity, mut menu, rebuild) in query.iter_mut() {
        let is_rebuild = rebuild.is_some();
        if let Some(mut rebuild) = rebuild {
            // Clean up from the old state: despawn the old item and indicator
            // entities and drop their label textures. Items that were never
            // spawned (`entity == None`) keep only the texture bookkeeping.
            despawn_menu_parts(
                &mut commands,
                &menu.items,
                menu.top_indicator_entity,
                menu.bottom_indicator_entity,
                &mut render_asset_cmd_writer,
                &mut texture_dims,
            );
            menu.top_indicator_entity = None;
            menu.bottom_indicator_entity = None;
            // The replacement inherits the current mode; every new item
            // starts unspawned. The marker is removed below, so its items
            // move out by value instead of being cloned.
            let dynamic_text = menu.items.first().is_none_or(|i| i.dynamic_text);
            let mut new_items = std::mem::take(&mut rebuild.items);
            for item in &mut new_items {
                item.dynamic_text = dynamic_text;
                item.entity = None;
            }
            menu.items = new_items;
            menu.selected_index = first_enabled(&menu.items).unwrap_or(0);
            menu.scroll_offset = 0;
            menu.active = true;
        }
        // Cache immutable data before mutable iteration to satisfy borrow rules
        let font_string = menu.font.clone();
        let font_size = menu.font_size;
        let normal_color = menu.normal_color;
        let selected_color = menu.selected_color;
        let disabled_color = menu.disabled_color();
        let selected_index = menu.selected_index;
        let use_screen_space = menu.use_screen_space;
        let origin = menu.origin;
        let item_spacing = menu.item_spacing;
        let visible_count = menu.visible_count;
        // Put the initial selection on screen before positioning anything; a
        // window that already shows it stays where it is (a rebuild has reset
        // the offset to 0 by now).
        if let Some(vc) = visible_count {
            menu.scroll_offset = ensure_visible(menu.selected_index, menu.scroll_offset, vc);
        }
        let scroll_offset = menu.scroll_offset;

        // A text cursor indents the items and indicators by the measured width
        // of "{cursor} " in the menu font; without a text cursor, or without
        // cached metrics for the resolved font key, the indent is 0.0.
        let item_indent = match menu.cursor_text.as_deref() {
            Some(cursor) => {
                let (cursor_font, metrics) =
                    resolve_menu_font(&font_metrics, &font_string, &pending, &mut warn_cache);
                match metrics {
                    Some(metrics) => metrics.measure_text(&format!("{cursor} "), font_size).x,
                    None => {
                        if warn_cache.warn_once(cursor_font) {
                            warn!(
                                "menu_spawn_system: item indent is 0 for menu {:?} because font '{}' has no cached metrics",
                                entity, cursor_font
                            );
                        }
                        0.0
                    }
                }
            }
            None => 0.0,
        };
        menu.item_indent = item_indent;

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
                // A disabled item gets disabled_color; otherwise the selected
                // item gets selected_color and the rest normal_color.
                let color = if !menu_item.enabled {
                    disabled_color
                } else if i == selected_index {
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
                // Resolve once so the label is measured and rasterized with
                // the same font.
                let (label_font, metrics) =
                    resolve_menu_font(&font_metrics, &font_string, &pending, &mut warn_cache);
                let Some(metrics) = metrics else {
                    if warn_cache.warn_once(label_font) {
                        warn!(
                            "menu_spawn_system: skipping menu item '{}' because font '{}' has no cached metrics",
                            menu_item.id, label_font
                        );
                    }
                    // The item gets no entity: drop the one reserved for it, so a
                    // menu with missing metrics does not collect an empty entity
                    // per item on every spawn and rebuild.
                    ecmd.despawn();
                    continue;
                };
                let size = metrics.measure_text(&menu_item.label, font_size);
                let key = format!("menu_{}", menu_item.id);
                // Static sprites are baked once, so they never recolour on
                // selection: an enabled item keeps normal_color even when it
                // is the selected index; a disabled item gets disabled_color.
                let color = if menu_item.enabled {
                    normal_color
                } else {
                    disabled_color
                };
                render_asset_cmd_writer.write(RenderAssetCmd::RasterizeText {
                    key: key.clone(),
                    font_key: label_font.to_owned(),
                    text: menu_item.label.clone(),
                    font_size,
                    spacing: metrics.spacing,
                    color,
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
            insert_menu_z_index(&mut ecmd);

            // Only add position component for visible items
            let is_visible = i >= scroll_offset && i < visible_end;
            if is_visible {
                // Calculate position within visible viewport
                let viewport_index = i - scroll_offset;
                let pos = menu_row_pos(origin, viewport_index as i32, item_spacing, item_indent);
                set_menu_position(&mut ecmd, use_screen_space, pos);
            }
            // Non-visible items don't get position component, so render system skips them

            let text_entity = ecmd.id();
            ecmd.insert(Group::new(menu_group_name(entity)));
            menu_item.entity = Some(text_entity);
            debug!(
                "menu_spawn_system: Menu item id={} assigned entity {:?} (visible={})",
                menu_item.id, text_entity, is_visible
            );
        } // end for each menu item

        // Spawns one "..." indicator on `row`, shown only when `show`: it joins
        // the menu's group and gets the z-index always (needed once it becomes
        // visible), but a position only while shown.
        let spawn_indicator = |commands: &mut Commands, row: i32, show: bool| -> Entity {
            let mut cmd = commands.spawn(DynamicText::new(
                "...",
                font_string.clone(),
                font_size,
                normal_color,
            ));
            cmd.insert(Group::new(menu_group_name(entity)));
            insert_menu_z_index(&mut cmd);
            let indicator = cmd.id();
            if show {
                set_menu_position(
                    &mut commands.entity(indicator),
                    use_screen_space,
                    menu_row_pos(origin, row, item_spacing, item_indent),
                );
            }
            indicator
        };
        // Spawn "..." indicators if visible_count is set
        if let Some(vc) = visible_count {
            let show_top = scroll_offset > 0;
            let show_bottom = visible_end < menu.items.len();
            menu.top_indicator_entity = Some(spawn_indicator(&mut commands, -1, show_top));
            menu.bottom_indicator_entity =
                Some(spawn_indicator(&mut commands, vc as i32, show_bottom));
        }

        // Add a signals component to the menu entity for state tracking
        commands
            .entity(entity)
            .insert(Signals::default().with_flag("waiting_selection"));
        debug!(
            "menu_spawn_system: Added Signals component to menu entity {:?}",
            entity
        );

        // Spawn the text cursor entity if the menu asks for one: on a first
        // spawn, or on a rebuild of a menu that has no cursor yet. A rebuild
        // of a menu that already has its cursor never respawns it, and a
        // spawned text cursor overrides a caller-supplied cursor entity
        // (which is left untouched).
        if let Some(cursor_text) = menu.cursor_text.clone()
            && (!is_rebuild || menu.cursor_entity.is_none())
        {
            let mut cursor_cmd = commands.spawn(DynamicText::new(
                cursor_text,
                font_string.clone(),
                font_size,
                selected_color,
            ));
            cursor_cmd.insert(Group::new(menu_group_name(entity)));
            let text_cursor = cursor_cmd.id();
            menu.cursor_entity = Some(text_cursor);
            debug!(
                "menu_spawn_system: Spawned text cursor entity {:?} for menu {:?}",
                text_cursor, entity
            );
        }
        // Position the cursor entity if there is one
        if let Some(cursor_entity) = menu.cursor_entity {
            let mut cursor_cmd = commands.entity(cursor_entity);
            // The cursor has a row only when the selected item is enabled; an
            // all-disabled or empty menu leaves it unpositioned.
            if menu
                .items
                .get(menu.selected_index)
                .is_some_and(|item| item.enabled)
            {
                debug!(
                    "menu_spawn_system: Positioning cursor entity {:?} for menu {:?}",
                    cursor_entity, entity
                );
                // Position cursor at selected item's viewport position, at origin.x
                let selected_viewport_index = menu.selected_index.saturating_sub(scroll_offset);
                let cursor_position =
                    menu_row_pos(origin, selected_viewport_index as i32, item_spacing, 0.0);
                set_menu_position(&mut cursor_cmd, use_screen_space, cursor_position);
                debug!(
                    "menu_spawn_system: Positioned cursor entity {:?} at {:?}",
                    cursor_entity, cursor_position
                );
            } else {
                clear_menu_position(&mut cursor_cmd, use_screen_space);
            }
            insert_menu_z_index(&mut cursor_cmd);
        }
        debug!(
            "menu_spawn_system: Spawned menu entity {:?} with {} items (visible_count={:?})",
            entity,
            menu.items.len(),
            visible_count
        );

        // A rebuild request is consumed once applied; a no-op otherwise.
        commands.entity(entity).remove::<MenuRebuild>();
    }
}

/// Despawns a menu's item and indicator entities and drops their label
/// textures. Shared by [`menu_despawn`] and the [`MenuRebuild`] path of
/// [`menu_spawn_system`]. An item whose `entity` is `None` was never spawned,
/// so only its texture bookkeeping runs.
fn despawn_menu_parts(
    commands: &mut Commands,
    items: &[MenuItem],
    top: Option<Entity>,
    bottom: Option<Entity>,
    render_asset_writer: &mut MessageWriter<RenderAssetCmd>,
    texture_dims: &mut TextureDimsStore,
) {
    for item in items {
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

    if let Some(top_entity) = top {
        commands.entity(top_entity).try_despawn();
    }
    if let Some(bottom_entity) = bottom {
        commands.entity(bottom_entity).try_despawn();
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

    // Despawn menu item and indicator entities and clean up textures
    despawn_menu_parts(
        &mut commands,
        &menu.items,
        menu.top_indicator_entity,
        menu.bottom_indicator_entity,
        &mut render_asset_writer,
        &mut texture_dims,
    );

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
/// and action buttons to confirm. Triggers [`MenuSelected`] when
/// an item is selected.
///
/// When `visible_count` is set, navigation is bounded (no wrap-around) and
/// scrolling occurs when selection moves outside the visible window.
pub fn menu_controller_observer(
    trigger: On<InputActionChanged>,
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
            InputAction::SecondaryDirectionUp | InputAction::SecondaryDirectionDown => {
                let step = if matches!(event.action, InputAction::SecondaryDirectionUp) {
                    MenuStep::Prev
                } else {
                    MenuStep::Next
                };
                // A scroll window bounds the move; without one, selection wraps.
                let wrap = menu.visible_count.is_none();
                if let Some(i) = step_selection(&menu.items, menu.selected_index, step, wrap) {
                    menu.selected_index = i;
                    changed_selection = true;
                    if let Some(vc) = menu.visible_count {
                        let new_offset = ensure_visible(i, menu.scroll_offset, vc);
                        if new_offset != menu.scroll_offset {
                            menu.scroll_offset = new_offset;
                            needs_reposition = true;
                        }
                    }
                }
            }
            InputAction::Action1 | InputAction::Action2 => {
                // A disabled or missing item is not confirmable.
                if let Some(item_id) = menu
                    .items
                    .get(menu.selected_index)
                    .filter(|item| item.enabled)
                    .map(|item| item.id.clone())
                {
                    debug!(
                        "menu_controller_observer: Selection confirmed! item_id={}, triggering MenuSelected",
                        item_id
                    );
                    signals.remove_flag("waiting_selection");
                    menu.active = false;
                    signals.set_string("selected_item", item_id.clone());
                    commands.trigger(MenuSelected {
                        entity,
                        item_id,
                        index: menu.selected_index,
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
                // Restoring normal_color is valid because selection never rests
                // on a disabled item (an all-disabled menu has no navigation).
                // A "selectable but inert" mode would have to restore
                // disabled_color() for disabled items instead.
                text.color = menu.normal_color;
            }
            if let Some(new_item) = menu.items.get(menu.selected_index)
                && let Some(entity) = new_item.entity
                && let Ok(mut text) = dynamic_text_query.get_mut(entity)
            {
                text.color = menu.selected_color;
            }

            if let Some(cursor_entity) = menu.cursor_entity {
                // Calculate cursor position based on visible viewport; the
                // cursor sits at origin.x, without the item indent.
                let viewport_index = menu.selected_index.saturating_sub(menu.scroll_offset);
                let cursor_position =
                    menu_row_pos(menu.origin, viewport_index as i32, menu.item_spacing, 0.0);
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
            if let Some(item) = menu.items.get(menu.selected_index) {
                commands.trigger(MenuSelectionChanged {
                    entity,
                    item_id: item.id.clone(),
                    index: menu.selected_index,
                    previous_index: old_selected_index,
                });
            }
        }
    }
}

/// Repositions menu items and indicators after scrolling.
///
/// Items within the visible window get position components added/updated at
/// `origin.x + item_indent`, while items outside the window have their
/// position components removed. The "..." indicators use the same indented x.
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
                let new_pos = menu_row_pos(
                    menu.origin,
                    viewport_index as i32,
                    menu.item_spacing,
                    menu.item_indent,
                );
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
            let pos = menu_row_pos(menu.origin, -1, menu.item_spacing, menu.item_indent);
            set_menu_position(&mut commands.entity(top_entity), menu.use_screen_space, pos);
        } else {
            clear_menu_position(&mut commands.entity(top_entity), menu.use_screen_space);
        }
    }

    if let Some(bottom_entity) = menu.bottom_indicator_entity {
        if show_bottom {
            let pos = menu_row_pos(
                menu.origin,
                visible_count as i32,
                menu.item_spacing,
                menu.item_indent,
            );
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

/// Executes the [`MenuAction`] associated with a selected menu item.
///
/// - [`MenuAction::RequestScene`] – requests the scene through [`NextScene`]; the
///   switch lands at the next scene switch poll, at the end of the tick
/// - [`MenuAction::RequestQuit`] – requests the quitting state through [`NextGameState`]
/// - [`MenuAction::Noop`] – does nothing
///
/// Observers of [`MenuSelected`] run in addition; don't combine
/// [`MenuActions`] with an observer for the same item.
pub fn menu_selection_observer(
    trigger: On<MenuSelected>,
    menus: Query<Option<&MenuActions>, With<Menu>>,
    mut next_scene: ResMut<NextScene>,
    mut next_game_state: ResMut<NextGameState>,
) {
    let event = trigger.event();
    let Ok(menu_actions) = menus.get(event.entity) else {
        warn!(
            "menu_selection_observer: Menu entity {:?} not found",
            event.entity
        );
        return;
    };
    // A menu without `MenuActions` is handled by `MenuSelected` observers.
    let Some(menu_actions) = menu_actions else {
        return;
    };

    debug!(
        "menu_selection_observer: item_id={} on menu {:?}",
        event.item_id, event.entity
    );
    match menu_actions.get(&event.item_id) {
        MenuAction::RequestScene(name) => next_scene.set(name),
        MenuAction::RequestQuit => {
            next_game_state.set(Quitting);
        }
        MenuAction::Noop => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Color, Vec2};
    use crate::resources::world_signals::WorldSignals;
    use bevy_ecs::message::Messages;
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::system::RunSystemOnce;

    use crate::protocol::asset_kind::AssetKind;
    use crate::resources::font_metrics::test_support::{
        default_font_metrics, lowercase_alphabet_metrics,
    };
    use crate::resources::pending_assets::PendingAssets;

    fn new_test_world() -> World {
        let mut world = World::new();

        let mut store = FontMetricsStore::default();
        store
            .0
            .insert("test_font".to_string(), lowercase_alphabet_metrics());
        world.insert_resource(store);
        world.insert_resource(FontMetricsWarnCache::default());
        world.insert_resource(PendingAssets::default());
        world.insert_resource(Messages::<RenderAssetCmd>::default());
        world.insert_resource(TextureDimsStore::default());
        world
    }

    /// A world whose `"default"` font has metrics, like the engine's.
    fn world_with_default_font() -> World {
        let mut world = new_test_world();
        world
            .resource_mut::<FontMetricsStore>()
            .0
            .insert("default".to_string(), default_font_metrics());
        world
    }

    /// Runs `menu_spawn_system` for one static-label menu using `font`;
    /// returns the queued `RasterizeText` font keys and spacings, and the label
    /// sprite width.
    fn spawn_static_label(world: &mut World, font: &str) -> (Vec<String>, Vec<f32>, f32) {
        world.spawn(
            Menu::new(&[("ok", "ok")], Vec2::ZERO, font, 20.0, 4.0, false).with_dynamic_text(false),
        );
        world
            .run_system_once(menu_spawn_system)
            .expect("system should run");
        let (font_keys, spacings) = world
            .resource_mut::<Messages<RenderAssetCmd>>()
            .drain()
            .filter_map(|cmd| match cmd {
                RenderAssetCmd::RasterizeText {
                    font_key, spacing, ..
                } => Some((font_key, spacing)),
                _ => None,
            })
            .unzip();
        let width = world
            .query::<&Sprite>()
            .iter(world)
            .next()
            .map_or(0.0, |sprite| sprite.width);
        (font_keys, spacings, width)
    }

    #[test]
    fn static_label_rasterizes_with_the_spacing_it_was_measured_with() {
        let mut world = world_with_default_font();
        let (_, spacings, width) = spawn_static_label(&mut world, "test_font");
        assert_eq!((spacings.as_slice(), width), (&[1.0][..], 21.0));

        let mut world = world_with_default_font();
        let (_, spacings, width) = spawn_static_label(&mut world, "default");
        assert_eq!((spacings.as_slice(), width), (&[0.0][..], 20.0));
    }

    #[test]
    fn static_label_with_an_empty_or_default_font_uses_the_default_font() {
        for font in ["", "default"] {
            let mut world = world_with_default_font();
            let (font_keys, _, width) = spawn_static_label(&mut world, font);
            assert_eq!(font_keys, ["default"], "{font:?}");
            assert_eq!(width, 20.0, "{font:?}");
        }
    }

    #[test]
    fn static_label_with_an_unknown_font_falls_back_and_warns_once() {
        let mut world = world_with_default_font();
        let (font_keys, _, width) = spawn_static_label(&mut world, "typo");
        assert_eq!(font_keys, ["default"], "measure and rasterize agree");
        assert_eq!(width, 20.0);
        assert!(
            !world
                .resource_mut::<FontMetricsWarnCache>()
                .warn_once("typo"),
            "the fallback already warned for 'typo'"
        );
    }

    #[test]
    fn static_label_with_a_pending_font_falls_back_without_warning() {
        let mut world = world_with_default_font();
        world
            .resource_mut::<PendingAssets>()
            .queue(AssetKind::Font, "later");
        let (font_keys, _, width) = spawn_static_label(&mut world, "later");
        assert_eq!(font_keys, ["default"]);
        assert_eq!(width, 20.0);
        assert!(
            world
                .resource_mut::<FontMetricsWarnCache>()
                .warn_once("later"),
            "a pending font is not a typo"
        );
    }

    #[derive(Resource, Default)]
    struct Selections(Vec<(Entity, String, usize)>);

    fn record_selection(trigger: On<MenuSelected>, mut s: ResMut<Selections>) {
        let e = trigger.event();
        s.0.push((e.entity, e.item_id.clone(), e.index));
    }

    /// Wires `world` for menu input and selection recording, spawns `menu`,
    /// and runs `menu_spawn_system` once; returns the menu entity.
    fn spawn_menu_in(world: &mut World, menu: Menu) -> Entity {
        world.insert_resource(Messages::<AudioCmd>::default());
        world.init_resource::<Selections>();
        world.add_observer(menu_controller_observer);
        world.add_observer(record_selection);
        let e = world.spawn(menu).id();
        world.run_system_once(menu_spawn_system).unwrap();
        e
    }

    /// Spawns `menu` in a fresh test world via `spawn_menu_in`.
    fn spawn_menu(menu: Menu) -> (World, Entity) {
        let mut world = new_test_world();
        let e = spawn_menu_in(&mut world, menu);
        (world, e)
    }

    fn press(world: &mut World, action: InputAction) {
        world.trigger(InputActionChanged {
            action,
            pressed: true,
        });
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

    /// A schedule running `menu_spawn_system`; unlike `run_system_once`, its
    /// system persists between runs, so change detection is real across runs.
    fn menu_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(menu_spawn_system);
        schedule
    }

    /// Inserts `MenuRebuild::new(items)` on `menu` and runs it once through a
    /// fresh persistent schedule, returned for further runs.
    fn rebuild(world: &mut World, menu: Entity, items: Vec<MenuItem>) -> Schedule {
        world.entity_mut(menu).insert(MenuRebuild::new(items));
        let mut schedule = menu_schedule();
        schedule.run(world);
        schedule
    }

    /// Ids of the menu's current items, in order.
    fn item_ids(world: &World, menu: Entity) -> Vec<String> {
        world
            .get::<Menu>(menu)
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect()
    }

    /// Entities `menu_spawn_system` created for `menu`: item entities plus
    /// the scroll indicators.
    fn spawned_entities(world: &World, menu: Entity) -> Vec<Entity> {
        let m = world.get::<Menu>(menu).unwrap();
        m.items
            .iter()
            .filter_map(|i| i.entity)
            .chain(m.top_indicator_entity)
            .chain(m.bottom_indicator_entity)
            .collect()
    }

    /// Number of entities in the menu's `menu_<entity>` group.
    fn menu_group_count(world: &World, menu: Entity) -> usize {
        let name = format!("menu_{menu}");
        world
            .iter_entities()
            .filter(|e| e.get::<Group>().is_some_and(|g| g.0 == name))
            .count()
    }

    /// Updates the render-asset message buffer and drains the queued commands.
    fn read_render_asset_cmds(world: &mut World) -> Vec<RenderAssetCmd> {
        world.resource_mut::<Messages<RenderAssetCmd>>().update();
        let mut reader_state =
            bevy_ecs::system::SystemState::<MessageReader<RenderAssetCmd>>::new(world);
        let mut reader = reader_state
            .get_mut(world)
            .expect("render asset reader should fetch");
        reader.read().cloned().collect()
    }

    fn text_color(world: &World, menu: Entity, i: usize) -> Color {
        world
            .get::<DynamicText>(item(world, menu, i))
            .unwrap()
            .color
    }

    #[test]
    fn spawn_positions_only_the_visible_window_and_the_indicators() {
        let (world, menu) = spawn_menu(five_items().with_visible_count(2));
        assert_eq!(
            visible_items(&world, menu),
            [
                Some(Vec2::new(100.0, 50.0)),
                Some(Vec2::new(100.0, 60.0)),
                None,
                None,
                None
            ]
        );
        let m = world.get::<Menu>(menu).unwrap();
        let (top, bottom) = (
            m.top_indicator_entity.unwrap(),
            m.bottom_indicator_entity.unwrap(),
        );
        assert_eq!(
            screen_pos(&world, top),
            None,
            "nothing above the first item"
        );
        assert_eq!(
            screen_pos(&world, bottom),
            Some(Vec2::new(100.0, 70.0)),
            "more items below"
        );
        for e in (0..5).map(|i| item(&world, menu, i)).chain([top, bottom]) {
            assert_eq!(
                world.get::<ZIndex>(e).unwrap().0,
                MENU_Z_INDEX,
                "hidden ones too"
            );
            assert_eq!(world.get::<Group>(e).unwrap().0, format!("menu_{menu}"));
        }
        assert!(
            world
                .get::<Signals>(menu)
                .unwrap()
                .has_flag("waiting_selection")
        );
        assert_eq!(
            world
                .get::<DynamicText>(item(&world, menu, 0))
                .unwrap()
                .color,
            Color::YELLOW
        );
        assert_eq!(
            world
                .get::<DynamicText>(item(&world, menu, 1))
                .unwrap()
                .color,
            Color::WHITE
        );
    }

    #[test]
    fn world_space_menu_uses_world_position() {
        let mut menu = five_items();
        menu.use_screen_space = false;
        let (world, menu) = spawn_menu(menu);
        let first = item(&world, menu, 0);
        assert_eq!(
            world.get::<WorldPosition>(first).unwrap().pos,
            Vec2::new(100.0, 50.0)
        );
        assert!(world.get::<ScreenPosition>(first).is_none());
    }

    #[test]
    fn navigation_wraps_around_without_visible_count() {
        let (mut world, menu) = spawn_menu(five_items());
        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            4,
            "up from first wraps to last"
        );
        press(&mut world, InputAction::SecondaryDirectionDown);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            0,
            "down from last wraps to first"
        );
    }

    #[derive(Resource, Default)]
    struct Changes(Vec<(Entity, String, usize, usize)>);

    fn record_change(trigger: On<MenuSelectionChanged>, mut c: ResMut<Changes>) {
        let e = trigger.event();
        c.0.push((e.entity, e.item_id.clone(), e.index, e.previous_index));
    }

    #[test]
    fn moving_the_selection_triggers_selection_changed() {
        let (mut world, menu) = spawn_menu(five_items());
        world.init_resource::<Changes>();
        world.add_observer(record_change);

        press(&mut world, InputAction::SecondaryDirectionDown);
        press(&mut world, InputAction::SecondaryDirectionUp);
        press(&mut world, InputAction::SecondaryDirectionUp); // wraps to the last item

        assert_eq!(
            world.resource::<Changes>().0,
            [
                (menu, "b".to_string(), 1, 0),
                (menu, "a".to_string(), 0, 1),
                (menu, "e".to_string(), 4, 0),
            ]
        );
    }

    #[test]
    fn selection_changed_is_not_triggered_without_a_move() {
        // A bounded (scrolling) menu does not wrap: up at the top changes nothing.
        let (mut world, _menu) = spawn_menu(five_items().with_visible_count(2));
        world.init_resource::<Changes>();
        world.add_observer(record_change);

        press(&mut world, InputAction::SecondaryDirectionUp);
        press(&mut world, InputAction::Action1); // a confirm is not a selection change

        assert!(world.resource::<Changes>().0.is_empty());
    }

    #[test]
    fn scrolling_navigation_is_bounded_and_moves_the_window() {
        let (mut world, menu) = spawn_menu(
            five_items()
                .with_visible_count(2)
                .with_selection_sound("blip"),
        );

        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            0,
            "no wrap at the top"
        );
        assert_eq!(played_sounds(&mut world), 0, "no change, no sound");

        press(&mut world, InputAction::SecondaryDirectionDown); // index 1, still in window
        press(&mut world, InputAction::SecondaryDirectionDown); // index 2, window scrolls to 1..=2
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!((m.selected_index, m.scroll_offset), (2, 1));
        let (top, bottom) = (
            m.top_indicator_entity.unwrap(),
            m.bottom_indicator_entity.unwrap(),
        );
        assert_eq!(
            visible_items(&world, menu),
            [
                None,
                Some(Vec2::new(100.0, 50.0)),
                Some(Vec2::new(100.0, 60.0)),
                None,
                None
            ]
        );
        assert_eq!(
            screen_pos(&world, top),
            Some(Vec2::new(100.0, 40.0)),
            "items above now"
        );
        assert_eq!(screen_pos(&world, bottom), Some(Vec2::new(100.0, 70.0)));
        assert_eq!(
            played_sounds(&mut world),
            2,
            "one sound per selection change"
        );

        for _ in 0..5 {
            press(&mut world, InputAction::SecondaryDirectionDown);
        }
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(
            (m.selected_index, m.scroll_offset),
            (4, 3),
            "stops at the last item"
        );
        let bottom = m.bottom_indicator_entity.unwrap();
        assert_eq!(
            screen_pos(&world, bottom),
            None,
            "nothing below the last item"
        );
    }

    #[test]
    fn selection_change_recolors_items_and_moves_the_cursor() {
        let mut world = new_test_world();
        let cursor = world.spawn_empty().id();
        let menu = spawn_menu_in(&mut world, five_items().with_cursor(cursor));
        assert_eq!(screen_pos(&world, cursor), Some(Vec2::new(100.0, 50.0)));

        press(&mut world, InputAction::SecondaryDirectionDown);
        let color = |w: &World, i| w.get::<DynamicText>(item(w, menu, i)).unwrap().color;
        assert_eq!(
            (color(&world, 0), color(&world, 1)),
            (Color::WHITE, Color::YELLOW)
        );
        assert_eq!(screen_pos(&world, cursor), Some(Vec2::new(100.0, 60.0)));
    }

    #[test]
    fn confirming_deactivates_the_menu_and_triggers_selection_once() {
        let (mut world, menu) = spawn_menu(five_items());
        press(&mut world, InputAction::SecondaryDirectionDown);
        world.trigger(InputActionChanged {
            action: InputAction::Action1,
            pressed: false,
        });
        world.flush();
        assert!(
            world.resource::<Selections>().0.is_empty(),
            "releases are ignored"
        );

        press(&mut world, InputAction::Action1);
        assert_eq!(
            world.resource::<Selections>().0,
            [(menu, "b".to_string(), 1)]
        );
        let signals = world.get::<Signals>(menu).unwrap();
        assert!(!signals.has_flag("waiting_selection"));
        assert_eq!(signals.get_string("selected_item"), Some("b"));
        assert!(!world.get::<Menu>(menu).unwrap().active);

        press(&mut world, InputAction::Action2);
        press(&mut world, InputAction::SecondaryDirectionDown);
        assert_eq!(
            world.resource::<Selections>().0.len(),
            1,
            "an inactive menu ignores input"
        );
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 1);
    }

    #[test]
    fn confirming_triggers_menu_selected_on_the_menu_entity_with_the_index() {
        // `spawn_menu` records globally; this adds a per-menu observer, so
        // each confirm is recorded once by each.
        let (mut world, menu) = spawn_menu(five_items());
        world.entity_mut(menu).observe(record_selection);
        world.flush();

        press(&mut world, InputAction::SecondaryDirectionDown);
        press(&mut world, InputAction::SecondaryDirectionDown);
        press(&mut world, InputAction::Action1);

        let c = (menu, "c".to_string(), 2);
        assert_eq!(world.resource::<Selections>().0, [c.clone(), c]);
    }

    #[test]
    fn menu_despawn_removes_all_menu_entities_and_label_textures() {
        let mut world = new_test_world();
        let cursor = world.spawn_empty().id();
        let bystander = world.spawn_empty().id();
        let menu = world
            .spawn(
                Menu::new(
                    &[("play", "play"), ("quit", "quit")],
                    Vec2::ZERO,
                    "test_font",
                    12.0,
                    10.0,
                    true,
                )
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
            .chain([
                m.top_indicator_entity.unwrap(),
                m.bottom_indicator_entity.unwrap(),
                cursor,
                menu,
            ])
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
        let plain = world.spawn_empty().id();
        world.run_system_once_with(menu_despawn, plain).unwrap();
        assert!(world.get_entity(plain).is_ok());
        assert_eq!(
            world
                .resource_mut::<Messages<RenderAssetCmd>>()
                .drain()
                .count(),
            0
        );
    }

    fn record_on_signals(trigger: On<MenuSelected>, mut ws: ResMut<WorldSignals>) {
        let e = trigger.event();
        ws.set_string("observed", format!("{}:{}", e.item_id, e.index));
    }

    /// World wired for `menu_selection_observer`.
    fn selection_world() -> World {
        let mut world = World::new();
        world.init_resource::<WorldSignals>();
        world.init_resource::<NextGameState>();
        world.init_resource::<NextScene>();
        world.add_observer(menu_selection_observer);
        world
    }

    fn select(world: &mut World, menu: Entity, item: &str) {
        let index = world
            .get::<Menu>(menu)
            .unwrap()
            .items
            .iter()
            .position(|i| i.id == item)
            .expect("select: item is in the menu");
        world.trigger(MenuSelected {
            entity: menu,
            item_id: item.to_string(),
            index,
        });
        world.flush();
    }

    fn actions() -> MenuActions {
        MenuActions::new()
            .with("play", MenuAction::RequestScene("level01".into()))
            .with("options", MenuAction::Noop)
            .with("quit", MenuAction::RequestQuit)
    }

    fn three_items() -> Menu {
        Menu::new(
            &[("play", "P"), ("options", "O"), ("quit", "Q")],
            Vec2::ZERO,
            "f",
            12.0,
            10.0,
            true,
        )
    }

    #[test]
    fn a_menu_observer_runs_alongside_menu_actions() {
        let mut world = selection_world();
        let menu = world.spawn((three_items(), actions())).id();
        world.entity_mut(menu).observe(record_on_signals);
        select(&mut world, menu, "play");
        let ws = world.resource::<WorldSignals>();
        assert_eq!(ws.get_string("observed"), Some("play:0"));
        assert_eq!(world.resource::<NextScene>().get(), Some("level01"));
    }

    #[test]
    fn selection_dispatches_each_menu_action() {
        let mut world = selection_world();
        let menu = world.spawn((three_items(), actions())).id();

        select(&mut world, menu, "play");
        assert_eq!(world.resource::<NextScene>().get(), Some("level01"));

        assert_eq!(world.resource::<NextGameState>().get(), None);
        select(&mut world, menu, "quit");
        assert!(world.resource::<NextGameState>().quit_requested());
    }

    #[test]
    fn selection_without_an_action_or_actions_component_is_a_noop() {
        let mut world = selection_world();
        let with_actions = world.spawn((three_items(), MenuActions::new())).id();
        let without = world.spawn(three_items()).id();
        select(&mut world, with_actions, "play");
        select(&mut world, without, "play");
        assert!(!world.resource::<NextScene>().is_pending());
        assert_eq!(world.resource::<NextGameState>().get(), None);
    }

    #[test]
    fn static_label_queues_rasterize_text_and_sizes_sprite() {
        let mut world = new_test_world();
        world.spawn(
            Menu::new(&[("ok", "ok")], Vec2::ZERO, "test_font", 20.0, 4.0, false)
                .with_dynamic_text(false),
        );

        world
            .run_system_once(menu_spawn_system)
            .expect("system should run");
        let cmds = read_render_asset_cmds(&mut world);
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
        assert!(read_render_asset_cmds(&mut world).is_empty());

        let mut sprite_query = world.query::<&Sprite>();
        assert!(sprite_query.iter(&world).next().is_none());
    }

    /// Entities that carry no component at all: a stray `spawn_empty` leaves one.
    fn count_empty_entities(world: &World) -> usize {
        world
            .iter_entities()
            .filter(|e| e.archetype().id() == bevy_ecs::archetype::ArchetypeId::EMPTY)
            .count()
    }

    #[test]
    fn a_static_item_without_font_metrics_leaves_no_stray_entity_across_rebuilds() {
        let static_menu = Menu::from_items(
            vec![MenuItem::new("a", "A"), MenuItem::new("b", "B")],
            Vec2::ZERO,
            "missing_font",
            20.0,
            4.0,
            false,
        )
        .with_dynamic_text(false);
        let (mut world, menu) = spawn_menu(static_menu);
        assert_eq!(count_empty_entities(&world), 0, "the spawn leaves none");

        for _ in 0..2 {
            rebuild(
                &mut world,
                menu,
                vec![MenuItem::new("c", "C"), MenuItem::new("d", "D")],
            );
            assert_eq!(
                count_empty_entities(&world),
                0,
                "a rebuild leaves none either"
            );
        }
    }

    #[test]
    fn spawn_keeps_a_preset_scroll_offset_that_already_shows_the_selection() {
        let items: Vec<MenuItem> = (0..6)
            .map(|i| MenuItem::new(format!("i{i}"), format!("I{i}")))
            .collect();
        let mut menu =
            Menu::from_items(items, Vec2::new(100.0, 50.0), "test_font", 12.0, 10.0, true)
                .with_visible_count(3);
        menu.selected_index = 3;
        menu.scroll_offset = 3;

        let (world, menu_entity) = spawn_menu(menu);

        let m = world.get::<Menu>(menu_entity).unwrap();
        assert_eq!(
            (m.selected_index, m.scroll_offset),
            (3, 3),
            "the window already shows the selection and stays"
        );
        assert_eq!(
            visible_items(&world, menu_entity),
            [
                None,
                None,
                None,
                Some(Vec2::new(100.0, 50.0)),
                Some(Vec2::new(100.0, 60.0)),
                Some(Vec2::new(100.0, 70.0)),
            ]
        );
    }

    /// Returns `five_items` with the given ids disabled; the selection starts
    /// at the first enabled item, like [`Menu::from_items`].
    fn five_items_with_disabled(disabled: &[&str]) -> Menu {
        let mut menu = five_items();
        for item in &mut menu.items {
            item.enabled = !disabled.contains(&item.id.as_str());
        }
        menu.selected_index = first_enabled(&menu.items).unwrap_or(0);
        menu
    }

    #[test]
    fn spawn_selects_the_first_enabled_item() {
        let (world, menu) = spawn_menu(five_items_with_disabled(&["a"]));
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 1);
    }

    #[test]
    fn wrap_navigation_skips_disabled_items() {
        let (mut world, menu) =
            spawn_menu(five_items_with_disabled(&["b"]).with_selection_sound("blip"));
        press(&mut world, InputAction::SecondaryDirectionDown);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            2,
            "down from a skips disabled b"
        );
        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            0,
            "up from c skips disabled b"
        );
        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            4,
            "up from the first wraps to the last"
        );
        assert_eq!(played_sounds(&mut world), 3, "one sound per change");
    }

    #[test]
    fn bounded_navigation_skips_disabled_items() {
        let (mut world, menu) = spawn_menu(five_items_with_disabled(&["b"]).with_visible_count(3));
        press(&mut world, InputAction::SecondaryDirectionDown);
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(
            (m.selected_index, m.scroll_offset),
            (2, 0),
            "skips disabled b and stays in the window"
        );
        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            0,
            "up from c skips disabled b"
        );
        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            0,
            "bounded at the top"
        );
    }

    #[test]
    fn a_skip_that_moves_more_than_one_row_scrolls_the_window() {
        let (mut world, menu) = spawn_menu(five_items_with_disabled(&["b"]).with_visible_count(2));
        press(&mut world, InputAction::SecondaryDirectionDown);
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!((m.selected_index, m.scroll_offset), (2, 1));
        assert_eq!(
            visible_items(&world, menu),
            [
                None,
                Some(Vec2::new(100.0, 50.0)),
                Some(Vec2::new(100.0, 60.0)),
                None,
                None
            ]
        );
    }

    #[test]
    fn confirming_a_disabled_selected_item_fires_no_selection_and_keeps_the_menu_active() {
        let mut menu = five_items_with_disabled(&["b"]);
        menu.selected_index = 1; // the disabled item
        let (mut world, menu_entity) = spawn_menu(menu);
        press(&mut world, InputAction::Action1);
        assert!(world.resource::<Selections>().0.is_empty());
        assert!(world.get::<Menu>(menu_entity).unwrap().active);
        assert!(
            world
                .get::<Signals>(menu_entity)
                .unwrap()
                .has_flag("waiting_selection")
        );
    }

    #[test]
    fn confirming_an_all_disabled_menu_fires_no_selection_and_keeps_the_menu_active() {
        let (mut world, menu) = spawn_menu(five_items_with_disabled(&["a", "b", "c", "d", "e"]));
        press(&mut world, InputAction::Action1);
        assert!(world.resource::<Selections>().0.is_empty());
        assert!(world.get::<Menu>(menu).unwrap().active);
    }

    #[test]
    fn spawn_scrolls_so_the_first_enabled_item_is_on_screen() {
        let (world, menu) =
            spawn_menu(five_items_with_disabled(&["a", "b", "c"]).with_visible_count(2));
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(m.selected_index, 3);
        assert_eq!(m.scroll_offset, ensure_visible(3, 0, 2));
        assert_eq!(m.scroll_offset, 2);
        assert_eq!(
            visible_items(&world, menu),
            [
                None,
                None,
                Some(Vec2::new(100.0, 50.0)),
                Some(Vec2::new(100.0, 60.0)),
                None
            ]
        );
    }

    #[test]
    fn an_all_disabled_menu_keeps_index_zero_and_navigation_is_a_noop() {
        let (mut world, menu) = spawn_menu(
            five_items_with_disabled(&["a", "b", "c", "d", "e"]).with_selection_sound("blip"),
        );
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 0);
        press(&mut world, InputAction::SecondaryDirectionDown);
        press(&mut world, InputAction::SecondaryDirectionUp);
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 0);
        assert_eq!(played_sounds(&mut world), 0, "no change, no sound");
    }

    #[test]
    fn spawn_colors_a_disabled_dynamic_item_with_the_default_disabled_color() {
        let (world, menu) = spawn_menu(five_items_with_disabled(&["b"]));
        let expected = world.get::<Menu>(menu).unwrap().disabled_color();
        assert_eq!(text_color(&world, menu, 1), expected);
        assert_eq!(
            text_color(&world, menu, 1),
            Color::new(255, 255, 255, 127),
            "the default is WHITE at half alpha"
        );
    }

    #[test]
    fn spawn_colors_a_disabled_dynamic_item_with_an_explicit_disabled_color() {
        let menu = five_items_with_disabled(&["b"])
            .with_colors(Color::new(10, 20, 30, 255), Color::YELLOW)
            .with_disabled_color(Color::new(1, 2, 3, 200));
        let (world, menu_entity) = spawn_menu(menu);
        let expected = world.get::<Menu>(menu_entity).unwrap().disabled_color();
        assert_eq!(text_color(&world, menu_entity, 1), expected);
        assert_eq!(text_color(&world, menu_entity, 1), Color::new(1, 2, 3, 200));
    }

    #[test]
    fn spawn_keeps_normal_and_selected_colors_on_enabled_items() {
        let menu = five_items_with_disabled(&["c"])
            .with_colors(Color::new(10, 20, 30, 255), Color::new(200, 200, 0, 255));
        let (world, menu_entity) = spawn_menu(menu);
        assert_eq!(world.get::<Menu>(menu_entity).unwrap().selected_index, 0);
        assert_eq!(
            text_color(&world, menu_entity, 0),
            Color::new(200, 200, 0, 255),
            "the selected enabled item keeps selected_color"
        );
        assert_eq!(
            text_color(&world, menu_entity, 1),
            Color::new(10, 20, 30, 255),
            "an enabled non-selected item keeps normal_color"
        );
    }

    #[test]
    fn spawn_draws_a_selected_but_disabled_item_with_the_disabled_color() {
        let (world, menu) = spawn_menu(five_items_with_disabled(&["a", "b", "c", "d", "e"]));
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 0);
        let expected = world.get::<Menu>(menu).unwrap().disabled_color();
        assert_eq!(
            text_color(&world, menu, 0),
            expected,
            "disabled wins over selected"
        );
    }

    /// Drains the queued `RasterizeText` commands as `(text, color)` pairs.
    fn rasterize_colors(world: &mut World) -> Vec<(String, Color)> {
        read_render_asset_cmds(world)
            .into_iter()
            .filter_map(|cmd| match cmd {
                RenderAssetCmd::RasterizeText { text, color, .. } => Some((text, color)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn spawn_rasterizes_a_disabled_static_item_with_the_disabled_color() {
        let menu = Menu::from_items(
            vec![MenuItem::new("a", "A"), MenuItem::new("b", "B").disabled()],
            Vec2::new(100.0, 50.0),
            "test_font",
            12.0,
            10.0,
            true,
        )
        .with_dynamic_text(false)
        .with_colors(Color::new(10, 20, 30, 255), Color::YELLOW);
        let (mut world, menu_entity) = spawn_menu(menu);
        let disabled = world.get::<Menu>(menu_entity).unwrap().disabled_color();
        let colors = rasterize_colors(&mut world);
        let color_of = |text: &str| {
            colors
                .iter()
                .find(|(t, _)| t.as_str() == text)
                .map(|(_, c)| *c)
        };
        assert_eq!(
            color_of("A"),
            Some(Color::new(10, 20, 30, 255)),
            "an enabled static item keeps normal_color"
        );
        assert_eq!(
            color_of("B"),
            Some(disabled),
            "a disabled static item is baked with the disabled color"
        );
    }

    #[test]
    fn rebuild_replaces_items_despawns_old_entities_and_clears_the_marker_once() {
        let (mut world, menu) = spawn_menu(five_items().with_visible_count(2));
        let old_entities = spawned_entities(&world, menu);
        let mut schedule = rebuild(
            &mut world,
            menu,
            vec![
                MenuItem::new("x", "X"),
                MenuItem::new("y", "Y"),
                MenuItem::new("z", "Z"),
            ],
        );

        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(item_ids(&world, menu), ["x", "y", "z"]);
        assert_eq!(
            m.items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(),
            ["X", "Y", "Z"]
        );
        assert!(
            world.get::<MenuRebuild>(menu).is_none(),
            "the marker is removed once applied"
        );
        for e in &old_entities {
            assert!(world.get_entity(*e).is_err(), "old entity {e:?} despawned");
        }
        let new_entities: Vec<Entity> = m.items.iter().filter_map(|i| i.entity).collect();
        assert_eq!(new_entities.len(), 3, "one entity per new item");
        for e in &new_entities {
            assert!(
                world.get_entity(*e).is_ok(),
                "new item entity {e:?} spawned"
            );
        }
        let top = m
            .top_indicator_entity
            .expect("visible_count keeps indicators");
        let bottom = m
            .bottom_indicator_entity
            .expect("visible_count keeps indicators");
        assert!(world.get_entity(top).is_ok() && world.get_entity(bottom).is_ok());

        // A second run changes nothing: the marker is gone and nothing is Added.
        let before: Vec<(String, Option<Entity>)> =
            m.items.iter().map(|i| (i.id.clone(), i.entity)).collect();
        schedule.run(&mut world);
        let after = world.get::<Menu>(menu).unwrap();
        assert_eq!(
            after
                .items
                .iter()
                .map(|i| (i.id.clone(), i.entity))
                .collect::<Vec<_>>(),
            before
        );
        assert!(world.get::<MenuRebuild>(menu).is_none());
    }

    #[test]
    fn rebuild_keeps_the_cursor_entity_and_moves_it_to_the_selected_row() {
        let mut world = new_test_world();
        let cursor = world.spawn_empty().id();
        let menu = spawn_menu_in(&mut world, five_items().with_cursor(cursor));

        rebuild(
            &mut world,
            menu,
            vec![
                MenuItem::new("n0", "N0").disabled(),
                MenuItem::new("n1", "N1"),
                MenuItem::new("n2", "N2"),
            ],
        );

        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(item_ids(&world, menu), ["n0", "n1", "n2"]);
        assert_eq!(m.selected_index, 1, "the first enabled new item");
        assert_eq!(m.cursor_entity, Some(cursor), "the cursor entity is kept");
        assert!(world.get_entity(cursor).is_ok(), "the cursor still exists");
        assert_eq!(
            screen_pos(&world, cursor),
            Some(Vec2::new(100.0, 60.0)),
            "the cursor sits on the selected row"
        );
    }

    #[test]
    fn rebuild_reactivates_a_confirmed_menu_and_restores_waiting_selection() {
        let (mut world, menu) = spawn_menu(five_items());
        press(&mut world, InputAction::Action1);
        assert!(!world.get::<Menu>(menu).unwrap().active);
        assert!(
            !world
                .get::<Signals>(menu)
                .unwrap()
                .has_flag("waiting_selection")
        );

        rebuild(&mut world, menu, vec![MenuItem::new("again", "Again")]);

        assert_eq!(item_ids(&world, menu), ["again"]);
        assert!(
            world.get::<Menu>(menu).unwrap().active,
            "a rebuild re-activates the menu"
        );
        assert!(
            world
                .get::<Signals>(menu)
                .unwrap()
                .has_flag("waiting_selection"),
            "a rebuild restores the waiting_selection flag"
        );
        assert!(world.get::<MenuRebuild>(menu).is_none());
    }

    #[test]
    fn rebuild_selects_the_first_enabled_new_item() {
        let (mut world, menu) = spawn_menu(five_items());
        rebuild(
            &mut world,
            menu,
            vec![
                MenuItem::new("n0", "N0").disabled(),
                MenuItem::new("n1", "N1"),
            ],
        );
        assert_eq!(item_ids(&world, menu), ["n0", "n1"]);
        assert_eq!(
            world.get::<Menu>(menu).unwrap().selected_index,
            1,
            "a leading disabled new item is skipped"
        );
    }

    #[test]
    fn rebuild_with_all_new_items_disabled_selects_index_zero() {
        let (mut world, menu) = spawn_menu(five_items());
        rebuild(
            &mut world,
            menu,
            vec![
                MenuItem::new("n0", "N0").disabled(),
                MenuItem::new("n1", "N1").disabled(),
            ],
        );
        assert_eq!(item_ids(&world, menu), ["n0", "n1"]);
        assert_eq!(world.get::<Menu>(menu).unwrap().selected_index, 0);
    }

    #[test]
    fn rebuild_recomputes_scroll_offset_so_the_first_enabled_item_is_visible() {
        let (mut world, menu) = spawn_menu(five_items().with_visible_count(2));
        let new_items: Vec<MenuItem> = (0..5)
            .map(|i| {
                let item = MenuItem::new(format!("n{i}"), format!("N{i}"));
                if i < 3 { item.disabled() } else { item }
            })
            .collect();
        rebuild(&mut world, menu, new_items);

        assert_eq!(item_ids(&world, menu), ["n0", "n1", "n2", "n3", "n4"]);
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(m.selected_index, 3, "the first enabled new item");
        assert_eq!(m.scroll_offset, 2, "the window scrolled to reveal it");
        assert_eq!(
            visible_items(&world, menu),
            [
                None,
                None,
                Some(Vec2::new(100.0, 50.0)),
                Some(Vec2::new(100.0, 60.0)),
                None
            ]
        );
    }

    #[test]
    fn rebuild_without_visible_count_keeps_scroll_offset_at_zero() {
        let (mut world, menu) = spawn_menu(five_items());
        rebuild(
            &mut world,
            menu,
            vec![MenuItem::new("n0", "N0"), MenuItem::new("n1", "N1")],
        );
        assert_eq!(item_ids(&world, menu), ["n0", "n1"]);
        assert_eq!(world.get::<Menu>(menu).unwrap().scroll_offset, 0);
    }

    #[test]
    fn same_tick_menu_and_rebuild_spawn_only_the_new_items() {
        let mut world = new_test_world();
        let menu = world
            .spawn((
                five_items(),
                MenuRebuild::new(vec![
                    MenuItem::new("n1", "N1"),
                    MenuItem::new("n2", "N2"),
                    MenuItem::new("n3", "N3"),
                ]),
            ))
            .id();
        let mut schedule = menu_schedule();
        schedule.run(&mut world);

        assert_eq!(item_ids(&world, menu), ["n1", "n2", "n3"]);
        assert!(
            world
                .get::<Signals>(menu)
                .unwrap()
                .has_flag("waiting_selection")
        );
        assert!(world.get::<MenuRebuild>(menu).is_none());
        assert_eq!(
            menu_group_count(&world, menu),
            3,
            "no entities leaked for the original items"
        );
    }

    #[test]
    fn rebuild_of_a_static_menu_removes_old_textures_and_rasterizes_the_new_items() {
        let static_menu = Menu::from_items(
            vec![MenuItem::new("old1", "O1"), MenuItem::new("old2", "O2")],
            Vec2::new(100.0, 50.0),
            "test_font",
            12.0,
            10.0,
            true,
        )
        .with_dynamic_text(false);
        let (mut world, menu) = spawn_menu(static_menu);
        world.resource_mut::<Messages<RenderAssetCmd>>().clear();

        // `MenuItem::new` defaults to dynamic; the replaced items inherit the
        // static `dynamic_text` of the current first item.
        rebuild(
            &mut world,
            menu,
            vec![MenuItem::new("new1", "N1"), MenuItem::new("new2", "N2")],
        );

        assert_eq!(item_ids(&world, menu), ["new1", "new2"]);
        assert!(
            world
                .get::<Menu>(menu)
                .unwrap()
                .items
                .iter()
                .all(|i| !i.dynamic_text),
            "the replaced items stay static"
        );
        let cmds = read_render_asset_cmds(&mut world);
        let removed: Vec<&str> = cmds
            .iter()
            .filter_map(|c| match c {
                RenderAssetCmd::RemoveTexture { key } => Some(key.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(removed, ["menu_old1", "menu_old2"]);
        let rasterized: Vec<&str> = cmds
            .iter()
            .filter_map(|c| match c {
                RenderAssetCmd::RasterizeText { key, .. } => Some(key.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(rasterized, ["menu_new1", "menu_new2"]);
    }

    #[test]
    fn rebuild_with_zero_items_empties_the_menu_without_panicking() {
        let (mut world, menu) = spawn_menu(five_items());
        let old_entities = spawned_entities(&world, menu);
        rebuild(&mut world, menu, vec![]);

        let m = world.get::<Menu>(menu).unwrap();
        assert!(m.items.is_empty());
        assert_eq!(m.selected_index, 0);
        assert!(world.get::<MenuRebuild>(menu).is_none());
        for e in &old_entities {
            assert!(
                world.get_entity(*e).is_err(),
                "old item entity {e:?} despawned"
            );
        }
    }

    /// The indent a text `cursor` produces on a `five_items` menu: the
    /// measured width of `"{cursor} "` in `"test_font"` at size 12.
    fn cursor_indent(world: &World, cursor: &str) -> f32 {
        world.resource::<FontMetricsStore>().0["test_font"]
            .measure_text(&format!("{cursor} "), 12.0)
            .x
    }

    /// The menu's cursor entity; panics when no text cursor was spawned.
    fn text_cursor(world: &World, menu: Entity) -> Entity {
        world
            .get::<Menu>(menu)
            .unwrap()
            .cursor_entity
            .expect("the text cursor is spawned and stored in cursor_entity")
    }

    /// Spawns `menu` (a `five_items` variant with a text cursor) and returns
    /// the world, the menu entity, the spawned cursor entity, and the measured
    /// indent, asserted positive.
    fn spawn_cursor_menu(menu: Menu) -> (World, Entity, Entity, f32) {
        let cursor = menu
            .cursor_text
            .clone()
            .expect("fixture menus set a cursor text");
        let (world, entity) = spawn_menu(menu);
        let indent = cursor_indent(&world, &cursor);
        assert!(
            indent > 0.0,
            "the fixture font measures \"{cursor} \" wider than zero"
        );
        let cursor_entity = text_cursor(&world, entity);
        (world, entity, cursor_entity, indent)
    }

    /// Expected `visible_items` for a menu with `count` items, scroll window
    /// `[offset, offset + visible)`, origin (100, 50), spacing 10, and items
    /// at x = 100.0 + `indent`: windowed items positioned, the rest not.
    fn expected_rows(
        count: usize,
        offset: usize,
        visible: usize,
        indent: f32,
    ) -> Vec<Option<Vec2>> {
        (0..count)
            .map(|i| {
                (i >= offset && i < offset + visible)
                    .then(|| Vec2::new(100.0 + indent, 50.0 + (i - offset) as f32 * 10.0))
            })
            .collect()
    }

    #[test]
    fn text_cursor_indents_the_items_and_sits_at_the_origin_on_row_zero() {
        let (world, menu, cursor, indent) = spawn_cursor_menu(five_items().with_cursor_text("a"));
        assert_eq!(
            world.get::<Menu>(menu).unwrap().item_indent,
            indent,
            "item_indent is the measured width of the cursor text plus one space"
        );
        assert_eq!(
            screen_pos(&world, cursor),
            Some(Vec2::new(100.0, 50.0)),
            "the cursor sits at origin.x on the selected row"
        );
        assert_eq!(
            visible_items(&world, menu),
            expected_rows(5, 0, 5, indent),
            "every item sits at origin.x + item_indent"
        );
    }

    #[test]
    fn the_text_cursor_is_a_selected_color_dynamic_text_in_the_menu_group_and_wins_over_with_cursor()
     {
        let mut world = new_test_world();
        let other = world.spawn_empty().id();
        let menu = spawn_menu_in(
            &mut world,
            five_items().with_cursor(other).with_cursor_text(">"),
        );
        let cursor = world.get::<Menu>(menu).unwrap().cursor_entity;
        assert!(
            cursor.is_some_and(|c| c != other),
            "the spawned text cursor overrides the with_cursor entity"
        );
        let cursor = cursor.unwrap();
        let text = world.get::<DynamicText>(cursor);
        assert!(text.is_some(), "the cursor entity is a DynamicText");
        let text = text.unwrap();
        assert_eq!(&*text.text, ">");
        assert_eq!(&*text.font, "test_font");
        assert_eq!(text.font_size, 12.0);
        assert_eq!(text.color, Color::YELLOW, "the cursor uses selected_color");
        assert_eq!(
            world.get::<Group>(cursor).unwrap().0,
            format!("menu_{menu}")
        );
        assert_eq!(world.get::<ZIndex>(cursor).unwrap().0, MENU_Z_INDEX);
        assert!(
            world.get_entity(other).is_ok(),
            "the with_cursor entity is untouched"
        );
    }

    #[test]
    fn navigation_moves_the_text_cursor_down_a_row_at_the_origin_x() {
        let (mut world, menu, cursor, indent) =
            spawn_cursor_menu(five_items().with_cursor_text("a"));
        assert_eq!(screen_pos(&world, cursor), Some(Vec2::new(100.0, 50.0)));

        press(&mut world, InputAction::SecondaryDirectionDown);

        assert_eq!(
            screen_pos(&world, cursor),
            Some(Vec2::new(100.0, 60.0)),
            "the cursor follows the selection to row 1 at origin.x"
        );
        assert_eq!(
            visible_items(&world, menu),
            expected_rows(5, 0, 5, indent),
            "the items keep their indented x"
        );
    }

    #[test]
    fn a_visible_window_indents_the_indicators_and_the_scrolled_items() {
        let (mut world, menu, cursor, indent) =
            spawn_cursor_menu(five_items().with_visible_count(2).with_cursor_text("a"));
        let m = world.get::<Menu>(menu).unwrap();
        let bottom = m
            .bottom_indicator_entity
            .expect("visible_count spawns the indicators");
        assert_eq!(
            screen_pos(&world, bottom),
            Some(Vec2::new(100.0 + indent, 70.0)),
            "the bottom indicator sits at origin.x + item_indent below the window"
        );
        assert_eq!(visible_items(&world, menu), expected_rows(5, 0, 2, indent));

        press(&mut world, InputAction::SecondaryDirectionDown);
        press(&mut world, InputAction::SecondaryDirectionDown);
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(
            (m.selected_index, m.scroll_offset),
            (2, 1),
            "the window scrolled"
        );
        let top = m.top_indicator_entity.unwrap();
        assert_eq!(
            screen_pos(&world, top),
            Some(Vec2::new(100.0 + indent, 40.0)),
            "the top indicator is indented too"
        );
        assert_eq!(
            screen_pos(&world, bottom),
            Some(Vec2::new(100.0 + indent, 70.0))
        );
        assert_eq!(
            visible_items(&world, menu),
            expected_rows(5, 1, 2, indent),
            "the scrolled items keep origin.x + item_indent"
        );
        assert_eq!(
            screen_pos(&world, cursor),
            Some(Vec2::new(100.0, 60.0)),
            "the cursor stays at origin.x on the selected row"
        );
    }

    #[test]
    fn a_rebuild_keeps_the_text_cursor_entity_on_the_new_selected_row_and_remeasures_the_indent() {
        let (mut world, menu, cursor, indent) =
            spawn_cursor_menu(five_items().with_cursor_text("a"));
        assert_eq!(
            world.get::<Menu>(menu).unwrap().item_indent,
            indent,
            "the initial indent is measured"
        );

        rebuild(
            &mut world,
            menu,
            vec![
                MenuItem::new("n0", "N0").disabled(),
                MenuItem::new("n1", "N1"),
                MenuItem::new("n2", "N2"),
            ],
        );

        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(item_ids(&world, menu), ["n0", "n1", "n2"]);
        assert_eq!(
            m.cursor_entity,
            Some(cursor),
            "the cursor entity is not respawned"
        );
        assert!(world.get_entity(cursor).is_ok(), "the cursor still exists");
        assert_eq!(
            screen_pos(&world, cursor),
            Some(Vec2::new(100.0, 60.0)),
            "the cursor sits on the new selected row at origin.x"
        );
        assert_eq!(m.item_indent, indent, "the rebuild re-measures item_indent");
        assert_eq!(
            visible_items(&world, menu),
            expected_rows(3, 0, 3, indent),
            "the rebuilt items are indented"
        );

        press(&mut world, InputAction::SecondaryDirectionDown);
        assert_eq!(
            screen_pos(&world, cursor),
            Some(Vec2::new(100.0, 70.0)),
            "the surviving cursor still follows the selection"
        );
    }

    #[test]
    fn a_text_cursor_with_missing_font_metrics_spawns_without_indent_and_warns_once() {
        let menu = Menu::new(
            &[("a", "A"), ("b", "B")],
            Vec2::new(100.0, 50.0),
            "ghost_font",
            12.0,
            10.0,
            true,
        )
        .with_cursor_text("a");
        let (mut world, menu) = spawn_menu(menu);
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(m.item_indent, 0.0, "no cached metrics: no indent");
        let cursor = text_cursor(&world, menu);
        assert!(world.get_entity(cursor).is_ok());
        assert!(
            !world
                .resource_mut::<FontMetricsWarnCache>()
                .warn_once("ghost_font"),
            "the spawn consumed the missing-font warning for 'ghost_font'"
        );
    }

    #[test]
    fn an_all_disabled_text_cursor_menu_spawns_the_cursor_without_a_position() {
        let (world, menu) =
            spawn_menu(five_items_with_disabled(&["a", "b", "c", "d", "e"]).with_cursor_text("a"));
        let cursor = text_cursor(&world, menu);
        assert!(world.get_entity(cursor).is_ok());
        assert!(
            world.get::<ScreenPosition>(cursor).is_none(),
            "no enabled item: the cursor has no screen position"
        );
        assert!(world.get::<WorldPosition>(cursor).is_none());
    }

    #[test]
    fn menu_despawn_despawns_the_text_cursor_entity_with_the_menu() {
        let (mut world, menu) = spawn_menu(five_items().with_cursor_text("a"));
        let cursor = text_cursor(&world, menu);

        world.run_system_once_with(menu_despawn, menu).unwrap();

        assert!(
            world.get_entity(cursor).is_err(),
            "the text cursor is despawned"
        );
        assert!(world.get_entity(menu).is_err(), "the menu is despawned");
    }

    #[test]
    fn sprite_cursor_and_cursor_less_menus_keep_zero_indent_and_items_at_the_origin() {
        let mut world = new_test_world();
        let cursor = world.spawn_empty().id();
        let menu = spawn_menu_in(&mut world, five_items().with_cursor(cursor));
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(m.item_indent, 0.0, "a sprite cursor indents nothing");
        assert_eq!(m.cursor_entity, Some(cursor));
        assert_eq!(visible_items(&world, menu), expected_rows(5, 0, 5, 0.0));

        let (world, menu) = spawn_menu(five_items());
        let m = world.get::<Menu>(menu).unwrap();
        assert_eq!(m.item_indent, 0.0, "no cursor indents nothing");
        assert_eq!(m.cursor_entity, None);
        assert_eq!(visible_items(&world, menu), expected_rows(5, 0, 5, 0.0));
    }
}
