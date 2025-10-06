use crate::events::gamestate::GameStateChangedEvent;
use crate::resources::gamestate::{GameState, GameStates, NextGameState, NextGameStates};
use bevy_ecs::prelude::*;

pub fn check_pending_state(
    mut commands: Commands,
    //game_state: ResMut<crate::resources::gamestate::GameState>,
    next_state: ResMut<NextGameState>,
) {
    // Check if there is a pending state change
    if let NextGameStates::Pending(_new_state) = next_state.get() {
        // If there is, trigger the GameStateChangedEvent
        commands.trigger(GameStateChangedEvent {});
    }
}

pub fn state_is_playing(state: Res<GameState>) -> bool {
    matches!(state.get(), GameStates::Playing)
}
