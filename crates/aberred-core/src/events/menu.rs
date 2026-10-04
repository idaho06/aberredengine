//! Menu selection events.
//!
//! When a user confirms a menu item, `menu_controller_observer` triggers a
//! [`MenuSelected`] targeted at the menu entity. Observe it per menu
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
