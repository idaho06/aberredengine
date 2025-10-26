//! Game state systems.
//!
//! - [`check_pending_state`] monitors [`NextGameState`] and triggers a
//!   [`GameStateChangedEvent`](crate::events::gamestate::GameStateChangedEvent)
//!   when a transition is requested.
//! - [`state_is_playing`] helper for run conditions that returns true when the
//!   current state is [`GameStates::Playing`].

use crate::events::gamestate::GameStateChangedEvent;
use crate::resources::gamestate::{GameState, GameStates, NextGameState, NextGameStates};
use bevy_ecs::prelude::*;

/// If a state transition is pending, trigger a `GameStateChangedEvent`.
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

/// Returns true when the current game state is `Playing`.
pub fn state_is_playing(state: Res<GameState>) -> bool {
    matches!(state.get(), GameStates::Playing)
}
