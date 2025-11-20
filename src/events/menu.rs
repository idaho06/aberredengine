use bevy_ecs::prelude::*;

#[derive(Event, Debug, Clone)]
pub struct MenuSelectionEvent {
    pub menu: Entity,
    pub item_id: String,
}
