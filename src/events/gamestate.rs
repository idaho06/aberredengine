use crate::resources::gamestate::NextGameStates::{Pending, Unchanged};
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use crate::resources::systemsstore::SystemsStore;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;

#[derive(Event, Debug, Clone, Copy)]
pub struct GameStateChangedEvent {}

pub fn observe_gamestate_change_event(
    _trigger: On<GameStateChangedEvent>,
    mut commands: Commands, // for spawning/despawning entities and triggering events
    mut next_game_state: Option<ResMut<NextGameState>>,
    mut game_state: Option<ResMut<GameState>>,
    systems_store: Res<SystemsStore>,
) {
    // This observer is triggered when a GameStateChangedEvent is fired.
    // It checks the NextGameState resource and updates the GameState resource accordingly.
    eprintln!("GameStateChangedEvent triggered");

    if let (Some(next_game_state), Some(game_state)) =
        (next_game_state.as_deref_mut(), game_state.as_deref_mut())
    {
        // Clone the next state value first so we don't keep an immutable borrow while mutating.
        let next_state_value = next_game_state.get().clone();
        match next_state_value {
            Pending(new_state) => {
                let old_state = game_state.get().clone();
                eprintln!(
                    "Transitioning from {:?} to {:?}",
                    game_state.get(),
                    new_state
                );
                game_state.set(new_state.clone());
                next_game_state.reset();
                eprintln!("Calling on_state_exit()");
                on_state_exit(&old_state, &mut commands, &systems_store);
                eprintln!("Calling on_state_enter()");
                let systems_store = systems_store.as_ref();
                on_state_enter(&new_state, &mut commands, &systems_store);
            }
            Unchanged => {
                eprintln!("No state change pending.");
            }
        }
    } else {
        eprintln!(
            "One or more resources missing in observe_gamestate_change_event.
             next_state: {:?}, game_state: {:?}",
            next_game_state.is_some(),
            game_state.is_some()
        );
    }
}

fn on_state_enter(state: &GameStates, commands: &mut Commands, systems_store: &SystemsStore) {
    match state {
        GameStates::None => eprintln!("Entered None state"),
        GameStates::Setup => {
            let setup_system_id = systems_store
                .get("setup")
                .expect("Setup system not found in SystemsStore");
            commands.run_system(setup_system_id.clone());
        }
        GameStates::Playing => {
            let enter_play_system_id = systems_store
                .get("enter_play")
                .expect("EnterPlay system not found in SystemsStore");
            commands.run_system(enter_play_system_id.clone());
        }
        GameStates::Paused => eprintln!("Entered Paused state"),
        GameStates::Quitting => eprintln!("Entered Quitting state"),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn on_state_exit(state: &GameStates, _commands: &mut Commands, _systems_store: &SystemsStore) {
    match state {
        GameStates::None => eprintln!("Exited None state"),
        GameStates::Setup => eprintln!("Exited Setup state"),
        GameStates::Playing => eprintln!("Exited Playing state"),
        GameStates::Paused => eprintln!("Exited Paused state"),
        GameStates::Quitting => eprintln!("Exited Quitting state"),
    }
}
