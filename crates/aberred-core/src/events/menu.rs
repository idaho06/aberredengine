//! Menu selection events.
//!
//! When a user confirms a menu item, `menu_controller_observer` triggers a
//! [`MenuSelected`] targeted at the menu entity; when the highlight moves to another
//! item it triggers a [`MenuSelectionChanged`]. Observe either per menu
//! (`commands.spawn(..).observe(handler)`) or globally (`add_observer(handler)`).

use bevy_ecs::prelude::*;

/// Triggered on a menu entity when one of its items is confirmed.
#[derive(EntityEvent, Debug, Clone)]
pub struct MenuSelected {
    /// The menu entity that contains the selected item.
    #[event_target]
    pub entity: Entity,
    /// The ID of the selected menu item (`items[index].id` when triggered).
    pub item_id: String,
    /// The position of the selected item in [`Menu::items`](crate::components::menu::Menu::items).
    pub index: usize,
}

/// Triggered on a menu entity when its highlight moves to another item (not on a confirm,
/// and not when a bounded menu is already at its edge).
#[derive(EntityEvent, Debug, Clone)]
pub struct MenuSelectionChanged {
    /// The menu entity whose selection moved.
    #[event_target]
    pub entity: Entity,
    /// The ID of the newly highlighted item (`items[index].id`).
    pub item_id: String,
    /// The position of the newly highlighted item in [`Menu::items`](crate::components::menu::Menu::items).
    pub index: usize,
    /// The position of the item that was highlighted before the move.
    pub previous_index: usize,
}
