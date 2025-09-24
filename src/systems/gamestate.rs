use crate::events::gamestate::GameStateChangedEvent;
use bevy_ecs::prelude::*;

pub fn check_pending_state(
    mut commands: Commands,
    //game_state: ResMut<crate::resources::gamestate::GameState>,
    next_state: ResMut<crate::resources::gamestate::NextGameState>,
) {
    // Check if there is a pending state change
    if let crate::resources::gamestate::NextGameStates::Pending(_new_state) = next_state.get() {
        // If there is, trigger the GameStateChangedEvent
        commands.trigger(GameStateChangedEvent {});
    }
}
