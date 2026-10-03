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
//!   [`Persistent`](crate::components::persistent::Persistent), keeping their observers.

use crate::components::persistent::SceneCleanup;
use crate::events::gamestate::GameStateChangedEvent;
use crate::protocol::endpoints::RenderTx;
use crate::protocol::render_logic::RenderMsg;
use crate::resources::gamestate::{GameState, GameStates, NextGameState, NextGameStates};
use crate::resources::pending_assets::PendingAssets;
use crate::resources::signal_keys as sk;
use crate::resources::worldsignals::WorldSignals;
use bevy_ecs::prelude::*;
use log::info;

/// If a state transition is pending, trigger a `GameStateChangedEvent`.
///
/// `Setup` → `Playing` is left to [`finish_setup`], which waits for the
/// queued asset loads. A request for any other state (e.g. `Quitting` from
/// the setup hook) applies here at once.
pub fn check_pending_state(
    mut commands: Commands,
    next_state: Res<NextGameState>,
    state: Res<GameState>,
) {
    if matches!(next_state.get(), NextGameStates::Pending(_)) && !ends_setup(&state, &next_state) {
        commands.trigger(GameStateChangedEvent {});
    }
}

/// Applies a requested `Setup` → `Playing` transition once [`PendingAssets`]
/// is empty, whether the engine or the setup hook made the request. Runs
/// last in the sim tick, after both asset forwarders, so every load queued
/// so far (including this tick's) is counted.
pub fn finish_setup(
    mut commands: Commands,
    next_state: Res<NextGameState>,
    state: Res<GameState>,
    pending: Res<PendingAssets>,
) {
    if ends_setup(&state, &next_state) && pending.is_empty() {
        commands.trigger(GameStateChangedEvent {});
    }
}

/// Whether the pending request is the `Setup` → `Playing` transition.
fn ends_setup(state: &GameState, next_state: &NextGameState) -> bool {
    *state.get() == GameStates::Setup
        && *next_state.get() == NextGameStates::Pending(GameStates::Playing)
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

/// Despawn all entities that are not marked [`Persistent`](crate::components::persistent::Persistent),
/// keeping the observers of persistent entities (see [`SceneCleanup`]).
pub fn clean_all_entities(mut commands: Commands, scene_cleanup: SceneCleanup) {
    scene_cleanup.despawn_all(&mut commands);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::asset_kind::AssetKind;
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
        world.init_resource::<PendingAssets>();
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

    /// A world in `Setup` with `next` requested and, if `loading`, one
    /// texture load pending.
    fn setup_world(next: GameStates, loading: bool) -> World {
        let mut requested = NextGameState::new();
        requested.set(next);
        let mut world = world_with_next_state(requested);
        world.resource_mut::<GameState>().set(GameStates::Setup);
        if loading {
            world
                .resource_mut::<PendingAssets>()
                .queue(AssetKind::Texture, "player");
        }
        world
    }

    #[test]
    fn check_pending_state_leaves_the_end_of_setup_to_finish_setup() {
        let mut world = setup_world(GameStates::Playing, false);
        tick_check_pending_state(&mut world);
        assert_eq!(world.resource::<ChangedCount>().0, 0);
    }

    #[test]
    fn finish_setup_waits_for_pending_loads() {
        let mut world = setup_world(GameStates::Playing, true);

        world.run_system_once(finish_setup).unwrap();
        assert_eq!(world.resource::<ChangedCount>().0, 0, "still loading");
        assert_eq!(
            *world.resource::<NextGameState>().get(),
            NextGameStates::Pending(GameStates::Playing),
            "the request is kept, not dropped"
        );

        world
            .resource_mut::<PendingAssets>()
            .settle(AssetKind::Texture, "player");
        world.run_system_once(finish_setup).unwrap();
        assert_eq!(world.resource::<ChangedCount>().0, 1);
    }

    #[test]
    fn finish_setup_ignores_other_requests() {
        let mut world = setup_world(GameStates::Quitting, false);
        world.run_system_once(finish_setup).unwrap();
        assert_eq!(world.resource::<ChangedCount>().0, 0);
    }

    #[test]
    fn leaving_setup_for_another_state_does_not_wait() {
        let mut world = setup_world(GameStates::Quitting, true);
        tick_check_pending_state(&mut world);
        assert_eq!(world.resource::<ChangedCount>().0, 1);
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
