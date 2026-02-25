//! Game state systems.
//!
//! - [`check_pending_state`] monitors [`NextGameState`] and triggers a
//!   [`GameStateChangedEvent`](crate::events::gamestate::GameStateChangedEvent)
//!   when a transition is requested.
//! - [`state_is_playing`] helper for run conditions that returns true when the
//!   current state is [`GameStates::Playing`].
//! - [`quit_game`] sets the `quit_game` world signal flag to exit the main loop.
//! - [`clean_all_entities`] despawns all entities that are not marked
//!   [`Persistent`](crate::components::persistent::Persistent).

use crate::components::persistent::Persistent;
use crate::events::gamestate::GameStateChangedEvent;
use crate::resources::gamestate::{GameState, GameStates, NextGameState, NextGameStates};
use crate::resources::worldsignals::WorldSignals;
use bevy_ecs::prelude::*;
use log::info;

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

/// Set the `quit_game` world signal flag, causing the main loop to exit.
pub fn quit_game(mut world_signals: ResMut<WorldSignals>) {
    info!("Quitting game...");
    world_signals.set_flag("quit_game");
}

/// Despawn all entities that are not marked [`Persistent`].
pub fn clean_all_entities(mut commands: Commands, query: Query<Entity, Without<Persistent>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
