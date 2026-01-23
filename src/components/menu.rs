//! Interactive menu components.
//!
//! This module provides components for building in-game menus:
//! - [`Menu`] – holds a list of menu items and selection state
//! - [`MenuItem`] – describes a single menu entry (label, position, etc.)
//! - [`MenuActions`] – maps menu item IDs to actions like scene switching
//! - [`MenuAction`] – the action to perform when a menu item is selected
//!
//! See [`crate::systems::menu`] for the menu spawn, input, and selection systems.

use bevy_ecs::prelude::{Component, Entity};
use raylib::prelude::{Color, Vector2};
use rustc_hash::FxHashMap;

/// A single item within a [`Menu`].
///
/// Stores the item's identifier, display label, and optional entity
/// reference for rendering.
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub dynamic_text: bool,
    // pub enabled: bool,
    pub entity: Option<Entity>, // If not dynamic_text, the entity holding the text sprite
}

/// Interactive menu component.
///
/// Holds the menu's display state, items, selection index, and visual
/// configuration. Use with [`MenuActions`] to define what happens when
/// items are selected.
#[derive(Component, Clone, Debug)]
pub struct Menu {
    /// Whether the menu is currently active and responding to input.
    pub active: bool,
    /// List of menu items.
    pub items: Vec<MenuItem>,
    /// Currently selected item index.
    pub selected_index: usize,
    /// Font key for rendering menu text.
    pub font: String,
    /// Font size in pixels.
    pub font_size: f32,
    /// Vertical spacing between menu items.
    pub item_spacing: f32,
    /// Color for unselected items.
    pub normal_color: Color,
    /// Color for the selected item.
    pub selected_color: Color,
    /// Optional cursor/pointer entity.
    pub cursor_entity: Option<Entity>,
    /// Optional sound to play on selection change.
    pub selection_change_sound: Option<String>,
    /// Origin position of the menu.
    pub origin: Vector2,
    /// Whether to use screen-space positioning (true) or world-space (false).
    pub use_screen_space: bool,
    /// Optional Lua callback invoked when any item is selected.
    pub on_select_callback: Option<String>,
    /// Maximum number of visible items (None = show all).
    pub visible_count: Option<usize>,
    /// Index of first visible item when scrolling.
    pub scroll_offset: usize,
    /// Entity for "..." indicator above visible items.
    pub top_indicator_entity: Option<Entity>,
    /// Entity for "..." indicator below visible items.
    pub bottom_indicator_entity: Option<Entity>,
}

impl Menu {
    pub fn new(
        labels: &[(&str, &str)], // (id, label)
        origin: Vector2,
        font: impl Into<String>,
        font_size: f32,
        item_spacing: f32,
        use_screen_space: bool,
    ) -> Self {
        let options = labels
            .iter()
            .map(|(id, label)| MenuItem {
                id: id.to_string(),
                label: label.to_string(),
                dynamic_text: true,
                // enabled: true,
                entity: None,
            })
            .collect();
        Self {
            active: true,
            items: options,
            selected_index: 0,
            font: font.into(),
            font_size,
            item_spacing,
            normal_color: Color::WHITE,
            selected_color: Color::YELLOW,
            cursor_entity: None,
            selection_change_sound: None,
            origin,
            use_screen_space,
            on_select_callback: None,
            visible_count: None,
            scroll_offset: 0,
            top_indicator_entity: None,
            bottom_indicator_entity: None,
        }
    }
    pub fn with_cursor(mut self, cursor_entity: Entity) -> Self {
        self.cursor_entity = Some(cursor_entity);
        self
    }
    pub fn with_colors(mut self, normal: Color, selected: Color) -> Self {
        self.normal_color = normal;
        self.selected_color = selected;
        self
    }
    pub fn with_dynamic_text(mut self, dynamic: bool) -> Self {
        for item in &mut self.items {
            item.dynamic_text = dynamic;
        }
        self
    }
    pub fn with_selection_sound(mut self, sound_key: impl Into<String>) -> Self {
        self.selection_change_sound = Some(sound_key.into());
        self
    }
    pub fn with_on_select_callback(mut self, callback: impl Into<String>) -> Self {
        self.on_select_callback = Some(callback.into());
        self
    }
    pub fn with_visible_count(mut self, count: usize) -> Self {
        self.visible_count = Some(count);
        self
    }
}

/// Action to perform when a menu item is selected.
#[derive(Clone, Debug)]
pub enum MenuAction {
    /// Switch to a different scene by name.
    SetScene(String),
    /// Quit the game.
    QuitGame,
    /// Show a sub-menu by name.
    ShowSubMenu(String),
    /// Do nothing (placeholder or disabled item).
    Noop,
}

/// Maps menu item IDs to their corresponding actions.
///
/// Attach this component alongside [`Menu`] to define what happens when
/// each item is selected.
#[derive(Component, Default, Clone, Debug)]
pub struct MenuActions {
    /// Map from item ID to action.
    pub map: FxHashMap<String, MenuAction>,
}

impl MenuActions {
    pub fn new() -> Self {
        Self {
            map: FxHashMap::default(),
        }
    }
    pub fn with(mut self, item_id: impl Into<String>, action: MenuAction) -> Self {
        self.map.insert(item_id.into(), action);
        self
    }
    pub fn get(&self, item_id: &str) -> MenuAction {
        self.map.get(item_id).cloned().unwrap_or(MenuAction::Noop)
    }
}
