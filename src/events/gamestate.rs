use crate::game;
use crate::resources::audio::AudioBridge;
use crate::resources::gamestate::NextGameStates::{Pending, Unchanged};
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use bevy_ecs::observer::Trigger;
use bevy_ecs::prelude::*;

#[derive(Event, Debug, Clone, Copy)]
pub struct GameStateChangedEvent {}

pub fn observe_gamestate_change_event(
    _trigger: Trigger<GameStateChangedEvent>,
    mut commands: Commands, // for spawning/despawning entities and triggering events
    mut next_state: Option<ResMut<NextGameState>>,
    mut game_state: Option<ResMut<GameState>>,
    mut rl: Option<NonSendMut<raylib::RaylibHandle>>,
    thread: Option<NonSend<raylib::RaylibThread>>,
    mut audio_bridge: Option<ResMut<AudioBridge>>,
) {
    //

    // This observer is triggered when a GameStateChangedEvent is fired.
    // It checks the NextGameState resource and updates the GameState resource accordingly.
    eprintln!("GameStateChangedEvent triggered");

    if let (Some(next_state), Some(game_state), Some(rl), Some(thread), Some(audio_bridge)) = (
        next_state.as_deref_mut(),
        game_state.as_deref_mut(),
        rl.as_deref_mut(),
        thread.as_deref(),
        audio_bridge.as_deref_mut(),
    ) {
        // Clone the next state value first so we don't keep an immutable borrow while mutating.
        let next_state_value = next_state.get().clone();
        match next_state_value {
            Pending(new_state) => {
                let old_state = game_state.get().clone();
                eprintln!(
                    "Transitioning from {:?} to {:?}",
                    game_state.get(),
                    new_state
                );
                game_state.set(new_state.clone());
                next_state.reset();
                eprintln!("Calling on_state_exit()");
                on_state_exit(&old_state);
                eprintln!("Calling on_state_enter()");
                on_state_enter(
                    &new_state,
                    &mut commands,
                    rl,
                    thread,
                    audio_bridge,
                    next_state,
                );
            }
            Unchanged => {
                eprintln!("No state change pending.");
            }
        }
    } else {
        eprintln!(
            "One or more resources missing in observe_gamestate_change_event.
             next_state: {:?}, game_state: {:?}, rl: {:?}, thread: {:?}, audio_bridge: {:?}",
            next_state.is_some(),
            game_state.is_some(),
            rl.is_some(),
            thread.is_some(),
            audio_bridge.is_some()
        );
    }
}

fn on_state_enter(
    state: &GameStates,
    commands: &mut Commands,
    rl: &mut raylib::RaylibHandle,
    thread: &raylib::RaylibThread,
    audio_bridge: &mut AudioBridge,
    next_state: &mut NextGameState,
) {
    match state {
        GameStates::None => eprintln!("Entered None state"),
        GameStates::Setup => {
            //eprintln!("Entered Setup state");
            game::setup(commands, rl, thread, audio_bridge, next_state);
        }
        GameStates::Playing => eprintln!("Entered Playing state"),
        GameStates::Paused => eprintln!("Entered Paused state"),
        GameStates::Quitting => eprintln!("Entered Quitting state"),
    }
}

fn on_state_exit(state: &GameStates) {
    match state {
        GameStates::None => eprintln!("Exited None state"),
        GameStates::Setup => eprintln!("Exited Setup state"),
        GameStates::Playing => eprintln!("Exited Playing state"),
        GameStates::Paused => eprintln!("Exited Paused state"),
        GameStates::Quitting => eprintln!("Exited Quitting state"),
    }
}
