//! Interactive menu components.
//!
//! This module provides components for building in-game menus:
//! - [`Menu`] – holds a list of menu items and selection state
//! - [`MenuItem`] – describes a single menu entry (label, position, etc.)
//! - [`MenuActions`] – maps menu item IDs to actions like scene switching
//! - [`MenuAction`] – the action to perform when a menu item is selected
//! - [`MenuRebuild`] – marker requesting an in-place replacement of a menu's items
//!
//! See [`crate::systems::menu`] for the menu spawn, input, and selection systems.
//! A confirmed item triggers [`MenuSelected`](crate::events::menu::MenuSelected)
//! on the menu entity.

use crate::math::Vec2;
use bevy_ecs::prelude::{Component, Entity};
use rustc_hash::FxHashMap;

use crate::math::Color;

/// A single item within a [`Menu`].
///
/// Stores the item's identifier, display label, and optional entity
/// reference for rendering.
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub dynamic_text: bool,
    pub entity: Option<Entity>, // If not dynamic_text, the entity holding the text sprite
    /// Whether the item can be selected; disabled items are skipped by
    /// navigation and ignored by confirm.
    pub enabled: bool,
}

impl MenuItem {
    /// Creates an enabled dynamic-text item with no entity yet.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> MenuItem {
        MenuItem {
            id: id.into(),
            label: label.into(),
            dynamic_text: true,
            entity: None,
            enabled: true,
        }
    }

    /// Returns the item with `enabled` set to `false`.
    pub fn disabled(mut self) -> MenuItem {
        self.enabled = false;
        self
    }
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
    /// Horizontal offset of the items and the "..." indicators from `origin.x`:
    /// the measured width of the cursor text plus one space, written by
    /// [`menu_spawn_system`](crate::systems::menu::menu_spawn_system); 0.0
    /// without a text cursor or when the font has no cached metrics.
    pub item_indent: f32,
    /// Color for unselected items.
    pub normal_color: Color,
    /// Color for the selected item.
    pub selected_color: Color,
    /// Explicit color for disabled items; `None` makes [`Menu::disabled_color`]
    /// fall back to `normal_color` with halved alpha.
    pub disabled_color: Option<Color>,
    /// Optional cursor/pointer entity.
    pub cursor_entity: Option<Entity>,
    /// Optional text drawn as the menu's cursor at `origin.x` on the selected
    /// item's row. When `Some`, [`menu_spawn_system`](crate::systems::menu::menu_spawn_system)
    /// spawns one [`DynamicText`](crate::components::dynamic_text::DynamicText)
    /// entity with this text, the menu font, `font_size` and `selected_color`,
    /// and stores it in [`cursor_entity`](Self::cursor_entity), overriding a
    /// caller-supplied one; `None` leaves the cursor to [`Menu::with_cursor`].
    pub cursor_text: Option<String>,
    /// Optional sound to play on selection change.
    pub selection_change_sound: Option<String>,
    /// Origin position of the menu.
    pub origin: Vec2,
    /// Whether to use screen-space positioning (true) or world-space (false).
    pub use_screen_space: bool,
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
        origin: Vec2,
        font: impl Into<String>,
        font_size: f32,
        item_spacing: f32,
        use_screen_space: bool,
    ) -> Self {
        let items = labels
            .iter()
            .map(|(id, label)| MenuItem::new(*id, *label))
            .collect();
        Self::from_items(
            items,
            origin,
            font,
            font_size,
            item_spacing,
            use_screen_space,
        )
    }
    pub fn with_cursor(mut self, cursor_entity: Entity) -> Self {
        self.cursor_entity = Some(cursor_entity);
        self
    }
    /// Sets the menu's text cursor. When [`cursor_text`](Self::cursor_text) is
    /// `Some`, the menu spawns its own [`DynamicText`](crate::components::dynamic_text::DynamicText)
    /// cursor entity, which wins over a cursor set with [`Menu::with_cursor`].
    pub fn with_cursor_text(mut self, text: impl Into<String>) -> Self {
        self.cursor_text = Some(text.into());
        self
    }
    pub fn with_colors(mut self, normal: Color, selected: Color) -> Self {
        self.normal_color = normal;
        self.selected_color = selected;
        self
    }
    /// Sets an explicit color for disabled items, overriding the
    /// half-alpha `normal_color` default returned by [`Menu::disabled_color`].
    pub fn with_disabled_color(mut self, color: Color) -> Self {
        self.disabled_color = Some(color);
        self
    }
    /// Returns the color disabled items are drawn with: the explicit
    /// `disabled_color` field when set, otherwise `normal_color` with its
    /// alpha halved (`a / 2`, integer division).
    pub fn disabled_color(&self) -> Color {
        match self.disabled_color {
            Some(color) => color,
            None => Color {
                a: self.normal_color.a / 2,
                ..self.normal_color
            },
        }
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
    pub fn with_visible_count(mut self, count: usize) -> Self {
        self.visible_count = Some(count);
        self
    }
    /// Builds a menu from existing items with the same defaults as [`Menu::new`].
    /// `selected_index` starts at the first enabled item ([`first_enabled`]), or 0
    /// when every item is disabled.
    pub fn from_items(
        items: Vec<MenuItem>,
        origin: Vec2,
        font: impl Into<String>,
        font_size: f32,
        item_spacing: f32,
        use_screen_space: bool,
    ) -> Menu {
        let selected_index = first_enabled(&items).unwrap_or(0);
        Menu {
            active: true,
            items,
            selected_index,
            font: font.into(),
            font_size,
            item_spacing,
            item_indent: 0.0,
            normal_color: Color::WHITE,
            selected_color: Color::YELLOW,
            disabled_color: None,
            cursor_entity: None,
            cursor_text: None,
            selection_change_sound: None,
            origin,
            use_screen_space,
            visible_count: None,
            scroll_offset: 0,
            top_indicator_entity: None,
            bottom_indicator_entity: None,
        }
    }
}

/// Marker requesting an in-place replacement of a [`Menu`]'s items.
///
/// Inserting this component on a menu entity replaces the menu's items in
/// place, re-activates the menu and selects the first enabled item; the
/// marker is removed once applied.
#[derive(Component, Clone, Debug)]
pub struct MenuRebuild {
    /// The replacement items.
    pub items: Vec<MenuItem>,
}

impl MenuRebuild {
    /// Creates a rebuild request carrying the replacement `items`.
    pub fn new(items: Vec<MenuItem>) -> Self {
        MenuRebuild { items }
    }
}

/// Direction of a selection step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuStep {
    /// Toward lower indices.
    Prev,
    /// Toward higher indices.
    Next,
}

/// Returns the index of the first enabled item, or `None` when the slice is
/// empty or every item is disabled.
pub fn first_enabled(items: &[MenuItem]) -> Option<usize> {
    items.iter().position(|item| item.enabled)
}

/// Returns the index of the next enabled item in the `step` direction from
/// `from` (exclusive). With `wrap`, the search cycles over all other items;
/// without it, the search stops at the ends of the slice. Returns `None` when
/// no other enabled item exists, meaning the selection stays put.
pub fn step_selection(
    items: &[MenuItem],
    from: usize,
    step: MenuStep,
    wrap: bool,
) -> Option<usize> {
    let len = items.len();
    if len == 0 {
        return None;
    }
    let from = from.min(len - 1);
    // Visits every other index once, nearest to `from` first; without `wrap`,
    // indices past an end are dropped.
    (1..len)
        .filter_map(|d| match step {
            MenuStep::Next => {
                let i = from + d;
                (i < len || wrap).then_some(i % len)
            }
            MenuStep::Prev => from.checked_sub(d).or_else(|| wrap.then(|| from + len - d)),
        })
        .find(|&i| items[i].enabled)
}

/// Returns the `scroll_offset` that puts `selected` inside
/// `[scroll_offset, scroll_offset + visible_count)`, changed as little as
/// possible; an already visible `selected` leaves the offset unchanged.
pub fn ensure_visible(selected: usize, scroll_offset: usize, visible_count: usize) -> usize {
    if selected < scroll_offset {
        selected
    } else if selected >= scroll_offset + visible_count {
        selected - visible_count + 1
    } else {
        scroll_offset
    }
}

/// Action to perform when a menu item is selected.
#[derive(Clone, Debug)]
pub enum MenuAction {
    /// Request a switch to a different scene by name, through
    /// [`NextScene`](crate::resources::next_scene::NextScene).
    RequestScene(String),
    /// Request quitting the game, through
    /// [`NextGameState`](crate::resources::game_state::NextGameState).
    RequestQuit,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_menu() -> Menu {
        Menu::new(
            &[("start", "Start Game"), ("quit", "Quit")],
            Vec2::ZERO,
            "arcade",
            16.0,
            20.0,
            true,
        )
    }

    #[test]
    fn new_builds_ordered_dynamic_items_active_at_first() {
        let menu = sample_menu();
        let items: Vec<_> = menu
            .items
            .iter()
            .map(|i| (i.id.as_str(), i.label.as_str(), i.dynamic_text, i.entity))
            .collect();
        assert_eq!(
            items,
            [
                ("start", "Start Game", true, None),
                ("quit", "Quit", true, None)
            ]
        );
        assert!(menu.active);
        assert_eq!(menu.selected_index, 0);
    }

    #[test]
    fn with_dynamic_text_applies_to_every_item() {
        let menu = sample_menu().with_dynamic_text(false);
        assert!(menu.items.iter().all(|i| !i.dynamic_text));
    }

    #[test]
    fn menu_actions_get_falls_back_to_noop() {
        let actions = MenuActions::new().with("quit", MenuAction::RequestQuit);
        assert!(matches!(actions.get("quit"), MenuAction::RequestQuit));
        assert!(matches!(actions.get("missing"), MenuAction::Noop));
    }

    #[test]
    fn new_marks_every_item_enabled() {
        let menu = sample_menu();
        assert!(menu.items.iter().all(|i| i.enabled));
    }

    #[test]
    fn from_items_leaves_disabled_color_unset() {
        assert_eq!(sample_menu().disabled_color, None);
    }

    #[test]
    fn disabled_color_defaults_to_normal_color_with_alpha_halved() {
        let menu = sample_menu().with_colors(Color::new(10, 20, 30, 255), Color::YELLOW);
        assert_eq!(menu.disabled_color(), Color::new(10, 20, 30, 127));
    }

    #[test]
    fn with_disabled_color_wins_over_the_default() {
        let menu = sample_menu()
            .with_colors(Color::new(10, 20, 30, 255), Color::YELLOW)
            .with_disabled_color(Color::new(1, 2, 3, 4));
        assert_eq!(menu.disabled_color(), Color::new(1, 2, 3, 4));
    }

    #[test]
    fn with_colors_changes_the_default_disabled_color() {
        let menu = sample_menu().with_colors(Color::new(200, 100, 50, 200), Color::YELLOW);
        assert_eq!(menu.disabled_color(), Color::new(200, 100, 50, 100));
    }

    fn item(id: &str, enabled: bool) -> MenuItem {
        let item = MenuItem::new(id, id);
        if enabled { item } else { item.disabled() }
    }

    #[test]
    fn from_items_selects_the_first_enabled_item() {
        let items = vec![item("a", false), item("b", true), item("c", true)];
        let menu = Menu::from_items(items, Vec2::ZERO, "font", 12.0, 4.0, false);
        assert_eq!(menu.selected_index, 1);
        assert!(menu.active);
        assert_eq!(menu.items.len(), 3);
    }

    #[test]
    fn from_items_with_every_item_disabled_selects_index_zero() {
        let items = vec![item("a", false), item("b", false)];
        let menu = Menu::from_items(items, Vec2::ZERO, "font", 12.0, 4.0, false);
        assert_eq!(menu.selected_index, 0);
    }

    #[test]
    fn first_enabled_skips_a_leading_disabled_item() {
        let items = vec![item("a", false), item("b", true), item("c", true)];
        assert_eq!(first_enabled(&items), Some(1));
    }

    #[test]
    fn first_enabled_is_none_for_empty_and_all_disabled() {
        assert_eq!(first_enabled(&[]), None);
        let items = vec![item("a", false), item("b", false)];
        assert_eq!(first_enabled(&items), None);
    }

    #[test]
    fn step_selection_skips_a_disabled_item_in_each_direction() {
        let items = vec![
            item("a", true),
            item("b", false),
            item("c", true),
            item("d", true),
        ];
        assert_eq!(step_selection(&items, 0, MenuStep::Next, true), Some(2));
        assert_eq!(step_selection(&items, 2, MenuStep::Prev, true), Some(0));
    }

    #[test]
    fn step_selection_wraps_past_the_ends() {
        let items = vec![item("a", true), item("b", false), item("c", true)];
        assert_eq!(step_selection(&items, 2, MenuStep::Next, true), Some(0));
        assert_eq!(step_selection(&items, 0, MenuStep::Prev, true), Some(2));
    }

    #[test]
    fn step_selection_bounded_stops_at_the_ends() {
        let items = vec![item("a", true), item("b", true), item("c", true)];
        assert_eq!(step_selection(&items, 0, MenuStep::Next, false), Some(1));
        assert_eq!(step_selection(&items, 2, MenuStep::Prev, false), Some(1));
        assert_eq!(step_selection(&items, 0, MenuStep::Prev, false), None);
        assert_eq!(step_selection(&items, 2, MenuStep::Next, false), None);
    }

    #[test]
    fn step_selection_is_none_when_no_other_item_is_enabled() {
        let all_disabled = vec![item("a", false), item("b", false)];
        assert_eq!(step_selection(&all_disabled, 0, MenuStep::Next, true), None);
        assert_eq!(step_selection(&all_disabled, 1, MenuStep::Prev, true), None);
        let only_current = vec![item("a", true), item("b", false), item("c", false)];
        assert_eq!(step_selection(&only_current, 0, MenuStep::Next, true), None);
        assert_eq!(step_selection(&only_current, 0, MenuStep::Prev, true), None);
    }

    #[test]
    fn ensure_visible_leaves_an_on_screen_selection_alone() {
        assert_eq!(ensure_visible(0, 0, 3), 0);
        assert_eq!(ensure_visible(2, 0, 3), 0);
        assert_eq!(ensure_visible(3, 2, 3), 2);
    }

    #[test]
    fn ensure_visible_scrolls_minimally_to_reveal_the_selection() {
        assert_eq!(
            ensure_visible(0, 2, 3),
            0,
            "above the window: offset = selected"
        );
        assert_eq!(
            ensure_visible(5, 0, 3),
            3,
            "below: selected - visible_count + 1"
        );
    }
}
