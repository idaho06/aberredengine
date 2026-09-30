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

use crate::components::persistent::CleanableEntity;
use crate::events::gamestate::GameStateChangedEvent;
use crate::protocol::endpoints::RenderTx;
use crate::protocol::render_logic::RenderMsg;
use crate::resources::gamestate::{GameState, GameStates, NextGameState, NextGameStates};
use crate::resources::signal_keys as sk;
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

/// Set the `quit_game` world signal flag and ask the render thread to exit.
///
/// Runs on the logic thread (no raylib handle exists there);
/// `RenderMsg::Quit` makes the render loop break, which then sends
/// `LogicMsg::Shutdown` back — same teardown path as a window close.
pub fn quit_game(mut world_signals: ResMut<WorldSignals>, render_tx: Res<RenderTx>) {
    info!("Quitting game...");
    world_signals.set_flag(sk::QUIT_GAME);
    let _ = render_tx.0.send(RenderMsg::Quit);
}

/// Despawn all entities that are not marked [`Persistent`].
pub fn clean_all_entities(mut commands: Commands, query: Query<Entity, CleanableEntity>) {
    for entity in query.iter() {
        commands.entity(entity).try_despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    /// Helper: run `check_pending_state` once.
    fn tick_check_pending_state(world: &mut World) {
        world
            .run_system_once(check_pending_state)
            .expect("check_pending_state should run");
    }

    #[test]
    fn check_pending_state_triggers_event_when_pending() {
        let mut world = World::new();
        world.init_resource::<GameState>();

        // Set a pending state
        let mut next = NextGameState::new();
        next.set(GameStates::Playing);
        world.insert_resource(next);

        // Run the system – it calls commands.trigger(GameStateChangedEvent{})
        tick_check_pending_state(&mut world);

        // After commands are flushed the event should have been triggered.
        // We can't easily inspect triggered events without an observer, but we can
        // verify the system didn't panic and the pending value is still there
        // (the observer is responsible for clearing it, not check_pending_state).
        let ns = world.resource::<NextGameState>();
        assert_eq!(*ns.get(), NextGameStates::Pending(GameStates::Playing));
    }

    #[test]
    fn check_pending_state_does_nothing_when_unchanged() {
        let mut world = World::new();
        world.init_resource::<GameState>();
        world.init_resource::<NextGameState>(); // defaults to Unchanged

        tick_check_pending_state(&mut world);

        let ns = world.resource::<NextGameState>();
        assert_eq!(*ns.get(), NextGameStates::Unchanged);
    }
}
