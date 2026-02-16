//! Game state transition event and observer.
//!
//! Systems can request a change to the high-level [`GameStates`] by updating
//! [`NextGameState`]. Emitting a [`GameStateChangedEvent`] then triggers the
//! observer in this module, which applies the transition to [`GameState`]
//! and invokes the appropriate enter/exit systems stored in
//! [`crate::resources::systemsstore::SystemsStore`].
//!
//! This decouples the intent to change state from the mechanics of running
//! setup/teardown systems and avoids borrowing conflicts.
use crate::resources::gamestate::NextGameStates::{Pending, Unchanged};
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use crate::resources::systemsstore::SystemsStore;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;
use log::{debug, info, warn};

/// Event used to indicate that a pending game state transition should be
/// applied.
///
/// Emitting this event causes [`observe_gamestate_change_event`] to read
/// [`NextGameState`]. If it contains [`Pending`], the observer updates the
/// authoritative [`GameState`], runs exit/enter hooks, and clears the pending
/// value; if it is [`Unchanged`], nothing happens.
#[derive(Event, Debug, Clone, Copy)]
pub struct GameStateChangedEvent {}

/// Observer that applies a pending game state transition.
///
/// Contract
/// - Reads the intention from [`NextGameState`].
/// - If pending, copies the new value into [`GameState`], then:
///   - calls state-specific exit hooks for the previous state
///   - calls state-specific enter hooks for the new state
///   - resets [`NextGameState`] to [`Unchanged`]
/// - If any required resource is missing, logs a diagnostic and returns.
///
/// The enter hooks are executed by looking up system IDs in
/// [`SystemsStore`] under well-known keys (e.g. `"setup"`, `"enter_play"`).
pub fn observe_gamestate_change_event(
    _trigger: On<GameStateChangedEvent>,
    mut commands: Commands, // for spawning/despawning entities and triggering events
    mut next_game_state: Option<ResMut<NextGameState>>,
    mut game_state: Option<ResMut<GameState>>,
    systems_store: Res<SystemsStore>,
) {
    // This observer is triggered when a GameStateChangedEvent is fired.
    // It checks the NextGameState resource and updates the GameState resource accordingly.
    debug!("GameStateChangedEvent triggered");

    if let (Some(next_game_state), Some(game_state)) =
        (next_game_state.as_deref_mut(), game_state.as_deref_mut())
    {
        // Clone the next state value first so we don't keep an immutable borrow while mutating.
        let next_state_value = next_game_state.get().clone();
        match next_state_value {
            Pending(new_state) => {
                let old_state = game_state.get().clone();
                info!(
                    "Transitioning from {:?} to {:?}",
                    game_state.get(),
                    new_state
                );
                game_state.set(new_state.clone());
                next_game_state.reset();
                debug!("Calling on_state_exit()");
                on_state_exit(&old_state, &mut commands, &systems_store);
                debug!("Calling on_state_enter()");
                let systems_store = systems_store.as_ref();
                on_state_enter(&new_state, &mut commands, systems_store);
            }
            Unchanged => {
                debug!("No state change pending.");
            }
        }
    } else {
        warn!(
            "One or more resources missing in observe_gamestate_change_event. next_state: {:?}, game_state: {:?}",
            next_game_state.is_some(),
            game_state.is_some()
        );
    }
}

/// Internal: run state-specific "enter" systems for the given state.
fn on_state_enter(state: &GameStates, commands: &mut Commands, systems_store: &SystemsStore) {
    match state {
        GameStates::None => debug!("Entered None state"),
        GameStates::Setup => {
            let setup_system_id = systems_store
                .get("setup")
                .expect("Setup system not found in SystemsStore");
            commands.run_system(*setup_system_id);
        }
        GameStates::Playing => {
            let enter_play_system_id = systems_store
                .get("enter_play")
                .expect("EnterPlay system not found in SystemsStore");
            commands.run_system(*enter_play_system_id);
        }
        // GameStates::Paused => eprintln!("Entered Paused state"),
        GameStates::Quitting => {
            let quit_game_system_id = systems_store
                .get("quit_game")
                .expect("QuitGame system not found in SystemsStore");
            commands.run_system(*quit_game_system_id);
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
/// Internal: run state-specific "exit" systems for the given state.
fn on_state_exit(state: &GameStates, _commands: &mut Commands, _systems_store: &SystemsStore) {
    match state {
        GameStates::None => debug!("Exited None state"),
        GameStates::Setup => debug!("Exited Setup state"),
        GameStates::Playing => debug!("Exited Playing state"),
        // GameStates::Paused => debug!("Exited Paused state"),
        GameStates::Quitting => debug!("Exited Quitting state"),
    }
}
