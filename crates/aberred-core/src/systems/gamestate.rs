//! Game state systems.
//!
//! - [`check_pending_state`] monitors [`NextGameState`] and triggers a
//!   [`GameStateChangedEvent`]
//!   when a transition is requested.
//! - [`state_is_playing`] helper for run conditions that returns true when the
//!   current state is [`GameStates::Playing`].
//! - [`take_quit_request`] / [`quit_flag_poll`] turn the `quit_game` world
//!   signal flag into a `Quitting` request.
//! - [`quit_game`] asks the render thread to exit the main loop.
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

/// Turns a set `quit_game` world signal flag into a [`GameStates::Quitting`]
/// request, consuming the flag. Returns whether it did. Shared by
/// [`quit_flag_poll`] and the Lua plugin's update, so a quit request means
/// the same thing with and without Lua.
pub fn take_quit_request(world_signals: &mut WorldSignals, next_state: &mut NextGameState) -> bool {
    let requested = world_signals.take_flag(sk::QUIT_GAME);
    if requested {
        next_state.set(GameStates::Quitting);
    }
    requested
}

/// Polls the `quit_game` flag via [`take_quit_request`] in games without
/// Lua (the Lua plugin's update polls it itself).
pub fn quit_flag_poll(
    mut world_signals: ResMut<WorldSignals>,
    mut next_state: ResMut<NextGameState>,
) {
    take_quit_request(&mut world_signals, &mut next_state);
}

/// Ask the render thread to exit; runs on entering [`GameStates::Quitting`].
///
/// Runs on the logic thread (no raylib handle exists there);
/// `RenderMsg::Quit` makes the render loop break, which then sends
/// `LogicMsg::Shutdown` back — same teardown path as a window close.
pub fn quit_game(render_tx: Res<RenderTx>) {
    info!("Quitting game...");
    let _ = render_tx.0.send(RenderMsg::Quit);
}

/// Despawn all entities that are not marked [`Persistent`](crate::components::persistent::Persistent).
pub fn clean_all_entities(mut commands: Commands, query: Query<Entity, CleanableEntity>) {
    for entity in query.iter() {
        commands.entity(entity).try_despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    /// Number of `GameStateChangedEvent`s observed.
    #[derive(Resource, Default)]
    struct ChangedCount(u32);

    /// World with `GameState`, the given `NextGameState`, and an observer
    /// counting `GameStateChangedEvent` into `ChangedCount`.
    fn world_with_next_state(next: NextGameState) -> World {
        let mut world = World::new();
        world.init_resource::<GameState>();
        world.insert_resource(next);
        world.init_resource::<ChangedCount>();
        world.add_observer(
            |_trigger: On<GameStateChangedEvent>, mut count: ResMut<ChangedCount>| {
                count.0 += 1;
            },
        );
        world.flush();
        world
    }

    /// Helper: run `check_pending_state` once.
    fn tick_check_pending_state(world: &mut World) {
        world
            .run_system_once(check_pending_state)
            .expect("check_pending_state should run");
    }

    #[test]
    fn check_pending_state_triggers_event_when_pending() {
        let mut next = NextGameState::new();
        next.set(GameStates::Playing);
        let mut world = world_with_next_state(next);

        tick_check_pending_state(&mut world);

        assert_eq!(world.resource::<ChangedCount>().0, 1);
        // Clearing the pending value is the observer's job, not this system's.
        let ns = world.resource::<NextGameState>();
        assert_eq!(*ns.get(), NextGameStates::Pending(GameStates::Playing));
    }

    #[test]
    fn take_quit_request_consumes_the_flag_and_requests_quitting() {
        let mut signals = WorldSignals::default();
        let mut next = NextGameState::new();
        assert!(
            !take_quit_request(&mut signals, &mut next),
            "no flag, no request"
        );
        assert_eq!(*next.get(), NextGameStates::Unchanged);

        signals.set_flag(sk::QUIT_GAME);
        assert!(take_quit_request(&mut signals, &mut next));
        assert_eq!(*next.get(), NextGameStates::Pending(GameStates::Quitting));
        assert!(!signals.has_flag(sk::QUIT_GAME), "flag consumed");
    }

    #[test]
    fn check_pending_state_does_nothing_when_unchanged() {
        // NextGameState defaults to Unchanged.
        let mut world = world_with_next_state(NextGameState::default());

        tick_check_pending_state(&mut world);

        assert_eq!(world.resource::<ChangedCount>().0, 0);
        let ns = world.resource::<NextGameState>();
        assert_eq!(*ns.get(), NextGameStates::Unchanged);
    }
}
