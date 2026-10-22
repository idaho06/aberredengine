//! Game state transition event and observer.
//!
//! Systems can request a change to the high-level [`GameState`] by updating
//! [`NextGameState`]. Emitting a [`GameStateChanged`] then triggers the
//! observer in this module, which applies the transition to [`CurrentGameState`]
//! and invokes the appropriate enter/exit systems stored in
//! [`crate::resources::systems_store::SystemsStore`].
//!
//! This decouples the intent to change state from the mechanics of running
//! setup/teardown systems and avoids borrowing conflicts.
use crate::resources::game_state::{CurrentGameState, GameState, NextGameState};
use crate::resources::systems_store as hook_keys;
use crate::resources::systems_store::SystemsStore;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;
use log::{debug, info, warn};

/// Event used to indicate that a pending game state transition should be
/// applied.
///
/// Emitting this event causes [`game_state_changed_observer`] to read
/// [`NextGameState`]. If a transition is pending, the observer updates the
/// authoritative [`CurrentGameState`], runs exit/enter hooks, and clears the
/// request; otherwise nothing happens.
#[derive(Event, Debug, Clone, Copy)]
pub struct GameStateChanged {}

/// Observer that applies a pending game state transition.
///
/// Contract
/// - Reads the intention from [`NextGameState`].
/// - If pending, copies the new value into [`CurrentGameState`], then:
///   - calls state-specific exit hooks for the previous state
///   - calls state-specific enter hooks for the new state
///   - clears the [`NextGameState`] request
/// - If any required resource is missing, logs a diagnostic and returns.
///
/// The enter hooks are executed by looking up system IDs in
/// [`SystemsStore`] under well-known keys (e.g. `"setup"`, `"enter_play"`).
pub fn game_state_changed_observer(
    _trigger: On<GameStateChanged>,
    mut commands: Commands, // for spawning/despawning entities and triggering events
    mut next_game_state: Option<ResMut<NextGameState>>,
    mut game_state: Option<ResMut<CurrentGameState>>,
    systems_store: Res<SystemsStore>,
) {
    debug!("GameStateChanged triggered");

    if let (Some(next_game_state), Some(game_state)) =
        (next_game_state.as_deref_mut(), game_state.as_deref_mut())
    {
        // Copy the requested state out so no immutable borrow is held while mutating.
        match next_game_state.get().copied() {
            Some(new_state) => {
                let old_state = *game_state.get();
                info!(
                    "Transitioning from {:?} to {:?}",
                    game_state.get(),
                    new_state
                );
                game_state.set(new_state);
                next_game_state.reset();
                debug!("Calling on_state_exit()");
                on_state_exit(&old_state, &mut commands, &systems_store);
                debug!("Calling on_state_enter()");
                let systems_store = systems_store.as_ref();
                on_state_enter(&new_state, &mut commands, systems_store);
            }
            None => {
                debug!("No state change pending.");
            }
        }
    } else {
        warn!(
            "One or more resources missing in game_state_changed_observer. next_state: {:?}, game_state: {:?}",
            next_game_state.is_some(),
            game_state.is_some()
        );
    }
}

/// Internal: run state-specific "enter" systems for the given state.
fn on_state_enter(state: &GameState, commands: &mut Commands, systems_store: &SystemsStore) {
    match state {
        GameState::None => debug!("Entered None state"),
        GameState::Setup => {
            // In order, each seeing what the previous one inserted: the
            // script setup, the setup hook, then the loading scene.
            for key in [
                hook_keys::SCRIPT_SETUP,
                hook_keys::SETUP,
                hook_keys::ENTER_SETUP,
            ] {
                if let Some(id) = systems_store.get(key) {
                    commands.run_system(*id);
                }
            }
            // Queued after the hooks, so a state a hook requested wins.
            commands.queue(request_playing_if_unchanged);
        }
        GameState::Playing => {
            // The script's entry first, so a scene it requests is the one
            // the engine's entry enters.
            for key in [hook_keys::SCRIPT_ENTER_PLAY, hook_keys::ENTER_PLAY] {
                if let Some(id) = systems_store.get(key) {
                    commands.run_system(*id);
                }
            }
        }
        // GameState::Paused => eprintln!("Entered Paused state"),
        GameState::Quitting => {
            commands.run_system(
                *systems_store
                    .get(hook_keys::QUIT_GAME)
                    .expect("the engine registers the 'quit_game' system in every configuration"),
            );
        }
    }
}

/// Internal: the automatic `Setup` → `Playing` transition. Leaves
/// [`NextGameState`] alone when the setup hook already requested a state.
fn request_playing_if_unchanged(world: &mut World) {
    let mut next = world.resource_mut::<NextGameState>();
    if next.get().is_none() {
        next.set(GameState::Playing);
    }
}

/// Internal: run state-specific "exit" systems for the given state.
fn on_state_exit(state: &GameState, _commands: &mut Commands, _systems_store: &SystemsStore) {
    match state {
        GameState::None => debug!("Exited None state"),
        GameState::Setup => debug!("Exited Setup state"),
        GameState::Playing => debug!("Exited Playing state"),
        // GameState::Paused => debug!("Exited Paused state"),
        GameState::Quitting => debug!("Exited Quitting state"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world with the state resources, the observer, and (if given) a
    /// `setup` hook registered in its [`SystemsStore`].
    fn world_with_setup(setup: Option<fn(ResMut<NextGameState>)>) -> World {
        let mut world = World::new();
        world.insert_resource(CurrentGameState::new());
        world.insert_resource(NextGameState::new());
        world.add_observer(game_state_changed_observer);
        let mut store = SystemsStore::new();
        if let Some(setup) = setup {
            store.insert(hook_keys::SETUP, world.register_system(setup));
        }
        world.insert_resource(store);
        world
    }

    fn enter(world: &mut World, state: GameState) {
        world.resource_mut::<NextGameState>().set(state);
        world.trigger(GameStateChanged {});
        world.flush();
    }

    fn next(world: &World) -> Option<GameState> {
        world.resource::<NextGameState>().get().copied()
    }

    #[test]
    fn setup_without_a_hook_requests_playing() {
        let mut world = world_with_setup(None);
        enter(&mut world, GameState::Setup);
        assert_eq!(
            world.resource::<CurrentGameState>().get(),
            &GameState::Setup
        );
        assert_eq!(next(&world), Some(GameState::Playing));
    }

    #[test]
    fn setup_hook_that_sets_nothing_requests_playing() {
        let mut world = world_with_setup(Some(|_| {}));
        enter(&mut world, GameState::Setup);
        assert_eq!(next(&world), Some(GameState::Playing));
    }

    #[test]
    fn setup_hook_that_requests_another_state_wins() {
        let mut world = world_with_setup(Some(|mut next| next.set(GameState::Quitting)));
        enter(&mut world, GameState::Setup);
        assert_eq!(next(&world), Some(GameState::Quitting));
    }

    #[test]
    fn setup_hook_that_requests_playing_itself_is_unaffected() {
        let mut world = world_with_setup(Some(|mut next| next.set(GameState::Playing)));
        enter(&mut world, GameState::Setup);
        assert_eq!(next(&world), Some(GameState::Playing));
    }

    #[test]
    fn script_setup_runs_first_and_its_commands_reach_setup() {
        #[derive(Resource, Default)]
        struct SetupLog(Vec<&'static str>);
        #[derive(Resource)]
        struct FromScript;

        let mut world = world_with_setup(None);
        world.init_resource::<SetupLog>();
        let script_setup =
            world.register_system(|mut commands: Commands, mut log: ResMut<SetupLog>| {
                log.0.push("script_setup");
                commands.insert_resource(FromScript);
            });
        let setup = world.register_system(
            |mut log: ResMut<SetupLog>, from_script: Option<Res<FromScript>>| {
                log.0.push(if from_script.is_some() {
                    "setup (saw script)"
                } else {
                    "setup"
                });
            },
        );
        let enter_setup = world.register_system(|mut log: ResMut<SetupLog>| {
            log.0.push("enter_setup");
        });
        let mut store = world.resource_mut::<SystemsStore>();
        store.insert(hook_keys::SCRIPT_SETUP, script_setup);
        store.insert(hook_keys::SETUP, setup);
        store.insert(hook_keys::ENTER_SETUP, enter_setup);

        enter(&mut world, GameState::Setup);

        assert_eq!(
            world.resource::<SetupLog>().0,
            ["script_setup", "setup (saw script)", "enter_setup"]
        );
    }

    #[test]
    fn script_enter_play_runs_first_and_its_commands_reach_enter_play() {
        #[derive(Resource, Default)]
        struct PlayLog(Vec<&'static str>);
        #[derive(Resource)]
        struct FromScript;

        let mut world = world_with_setup(None);
        world.init_resource::<PlayLog>();
        let script_enter_play =
            world.register_system(|mut commands: Commands, mut log: ResMut<PlayLog>| {
                log.0.push("script_enter_play");
                commands.insert_resource(FromScript);
            });
        let enter_play = world.register_system(
            |mut log: ResMut<PlayLog>, from_script: Option<Res<FromScript>>| {
                log.0.push(if from_script.is_some() {
                    "enter_play (saw script)"
                } else {
                    "enter_play"
                });
            },
        );
        let mut store = world.resource_mut::<SystemsStore>();
        store.insert(hook_keys::SCRIPT_ENTER_PLAY, script_enter_play);
        store.insert(hook_keys::ENTER_PLAY, enter_play);

        enter(&mut world, GameState::Playing);

        assert_eq!(
            world.resource::<PlayLog>().0,
            ["script_enter_play", "enter_play (saw script)"]
        );
    }

    #[test]
    fn entering_playing_without_an_enter_play_hook_is_a_no_op() {
        let mut world = world_with_setup(None);
        enter(&mut world, GameState::Playing);
        assert_eq!(
            world.resource::<CurrentGameState>().get(),
            &GameState::Playing
        );
        assert_eq!(next(&world), None);
    }
}
