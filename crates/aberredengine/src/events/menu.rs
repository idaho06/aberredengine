//! Menu selection events.
//!
//! This module provides the [`MenuSelectionEvent`] which is triggered when
//! a user confirms a menu item selection.

use bevy_ecs::prelude::*;

/// Event emitted when a menu item is selected.
///
/// Systems can observe this event to perform the associated action
/// (scene switch, quit, etc.).
#[derive(Event, Debug, Clone)]
pub struct MenuSelectionEvent {
    /// The menu entity that contains the selected item.
    pub menu: Entity,
    /// The ID of the selected menu item.
    pub item_id: String,
}
