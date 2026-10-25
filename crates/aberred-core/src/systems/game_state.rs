//! Game state systems.
//!
//! - [`check_pending_state`] monitors [`NextGameState`] and triggers a
//!   [`GameStateChanged`]
//!   when a transition is requested.
//! - [`state_is_playing`] helper for run conditions that returns true when the
//!   current state is [`GameState::Playing`]; [`state_runs_scenes`] also
//!   holds during `Setup`, for scene systems.
//! - [`quit_game`] asks the render thread to exit the main loop.
//! - [`clean_all_entities`] despawns all entities that are not marked
//!   [`Persistent`](crate::components::persistent::Persistent), keeping their observers.

use crate::components::persistent::SceneCleanup;
use crate::events::game_state::GameStateChanged;
use crate::protocol::endpoints::RenderTx;
use crate::protocol::render_logic::RenderMsg;
use crate::resources::collision_contacts::CollisionContacts;
use crate::resources::game_state::{CurrentGameState, GameState, NextGameState};
use crate::resources::loading_hold::LoadingHold;
use crate::resources::pending_assets::PendingAssets;
use bevy_ecs::prelude::*;
use log::info;

/// If a state transition is pending, trigger a `GameStateChanged`.
///
/// `Setup` → `Playing` is left to [`finish_setup`], which waits for the
/// queued asset loads. A request for any other state (e.g. `Quitting` from
/// the setup hook) applies here at once.
pub fn check_pending_state(
    mut commands: Commands,
    next_state: Res<NextGameState>,
    state: Res<CurrentGameState>,
) {
    if next_state.get().is_some() && !ends_setup(&state, &next_state) {
        commands.trigger(GameStateChanged {});
    }
}

/// Applies a requested `Setup` → `Playing` transition once [`PendingAssets`]
/// is empty and any [`LoadingHold`] has elapsed, whether the engine or the
/// setup hook made the request. Runs last in the sim tick, after both asset
/// forwarders, so every load queued so far (including this tick's) is counted.
pub fn finish_setup(
    mut commands: Commands,
    next_state: Res<NextGameState>,
    state: Res<CurrentGameState>,
    pending: Res<PendingAssets>,
    hold: Option<Res<LoadingHold>>,
) {
    let held = hold.is_some_and(|hold| !hold.is_done());
    if ends_setup(&state, &next_state) && pending.is_empty() && !held {
        commands.trigger(GameStateChanged {});
    }
}

/// Whether the pending request is the `Setup` → `Playing` transition.
fn ends_setup(state: &CurrentGameState, next_state: &NextGameState) -> bool {
    *state.get() == GameState::Setup && next_state.get() == Some(&GameState::Playing)
}

/// Returns true when the current game state is `Setup`.
pub fn state_is_setup(state: Res<CurrentGameState>) -> bool {
    matches!(state.get(), GameState::Setup)
}

/// Returns true when the current game state is `Playing`.
pub fn state_is_playing(state: Res<CurrentGameState>) -> bool {
    matches!(state.get(), GameState::Playing)
}

/// Returns true in the states a scene can be active in: `Setup` (the loading
/// scene) and `Playing`. Scene systems run under this plus `in_scene`.
pub fn state_runs_scenes(state: Res<CurrentGameState>) -> bool {
    matches!(state.get(), GameState::Setup | GameState::Playing)
}

/// Ask the render thread to exit; runs on entering [`GameState::Quitting`].
///
/// Runs on the logic thread (no raylib handle exists there);
/// `RenderMsg::Quit` makes the render loop break, which then sends
/// `LogicMsg::Shutdown` back — same teardown path as a window close.
pub fn quit_game(render_tx: Res<RenderTx>) {
    info!("Quitting game...");
    let _ = render_tx.0.send(RenderMsg::Quit);
}

/// Despawn all entities that are not marked [`Persistent`](crate::components::persistent::Persistent),
/// keeping the observers of persistent entities (see [`SceneCleanup`]), and
/// forget every [`CollisionContacts`] contact without ending it.
pub fn clean_all_entities(
    mut commands: Commands,
    scene_cleanup: SceneCleanup,
    mut contacts: ResMut<CollisionContacts>,
) {
    scene_cleanup.despawn_all(&mut commands);
    contacts.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::asset_kind::AssetKind;
    use bevy_ecs::system::RunSystemOnce;

    /// Scene systems run while a scene can be active: during `Setup` (the
    /// loading scene) and `Playing`, never before `Setup` or while quitting.
    #[test]
    fn state_runs_scenes_holds_in_setup_and_playing_only() {
        let mut world = World::new();
        world.init_resource::<CurrentGameState>();
        let mut runs_in = |state| {
            world.resource_mut::<CurrentGameState>().set(state);
            world.run_system_once(state_runs_scenes).unwrap()
        };
        assert!(!runs_in(GameState::None));
        assert!(runs_in(GameState::Setup));
        assert!(runs_in(GameState::Playing));
        assert!(!runs_in(GameState::Quitting));
    }

    /// The engine loading screen's systems run in `Setup` only.
    #[test]
    fn state_is_setup_holds_in_setup_only() {
        let mut world = World::new();
        world.init_resource::<CurrentGameState>();
        let mut runs_in = |state| {
            world.resource_mut::<CurrentGameState>().set(state);
            world.run_system_once(state_is_setup).unwrap()
        };
        assert!(!runs_in(GameState::None));
        assert!(runs_in(GameState::Setup));
        assert!(!runs_in(GameState::Playing));
        assert!(!runs_in(GameState::Quitting));
    }

    /// Cleanup despawns the touching pairs, so their contacts are forgotten
    /// without ending.
    #[test]
    fn clean_all_entities_forgets_collision_contacts() {
        let mut world = World::new();
        let [rule, a, b] = [(); 3].map(|_| world.spawn_empty().id());
        let mut contacts = CollisionContacts::default();
        contacts.begin(rule, a, b);
        contacts.end_tick();
        world.insert_resource(contacts);

        world.run_system_once(clean_all_entities).unwrap();

        assert!(world.resource::<CollisionContacts>().is_empty());
    }

    /// Number of `GameStateChanged`s observed.
    #[derive(Resource, Default)]
    struct ChangedCount(u32);

    /// World with `CurrentGameState`, the given `NextGameState`, and an observer
    /// counting `GameStateChanged` into `ChangedCount`.
    fn world_with_next_state(next: NextGameState) -> World {
        let mut world = World::new();
        world.init_resource::<CurrentGameState>();
        world.insert_resource(next);
        world.init_resource::<PendingAssets>();
        world.init_resource::<ChangedCount>();
        world.add_observer(
            |_trigger: On<GameStateChanged>, mut count: ResMut<ChangedCount>| {
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
        next.set(GameState::Playing);
        let mut world = world_with_next_state(next);

        tick_check_pending_state(&mut world);

        assert_eq!(world.resource::<ChangedCount>().0, 1);
        // Clearing the pending value is the observer's job, not this system's.
        let ns = world.resource::<NextGameState>();
        assert_eq!(ns.get(), Some(&GameState::Playing));
    }

    /// A world in `Setup` with `next` requested and, if `loading`, one
    /// texture load pending.
    fn setup_world(next: GameState, loading: bool) -> World {
        let mut requested = NextGameState::new();
        requested.set(next);
        let mut world = world_with_next_state(requested);
        world
            .resource_mut::<CurrentGameState>()
            .set(GameState::Setup);
        if loading {
            world
                .resource_mut::<PendingAssets>()
                .queue(AssetKind::Texture, "player");
        }
        world
    }

    #[test]
    fn check_pending_state_leaves_the_end_of_setup_to_finish_setup() {
        let mut world = setup_world(GameState::Playing, false);
        tick_check_pending_state(&mut world);
        assert_eq!(world.resource::<ChangedCount>().0, 0);
    }

    #[test]
    fn finish_setup_waits_for_pending_loads() {
        let mut world = setup_world(GameState::Playing, true);

        world.run_system_once(finish_setup).unwrap();
        assert_eq!(world.resource::<ChangedCount>().0, 0, "still loading");
        assert_eq!(
            world.resource::<NextGameState>().get(),
            Some(&GameState::Playing),
            "the request is kept, not dropped"
        );

        world
            .resource_mut::<PendingAssets>()
            .settle(AssetKind::Texture, "player");
        world.run_system_once(finish_setup).unwrap();
        assert_eq!(world.resource::<ChangedCount>().0, 1);
    }

    #[test]
    fn finish_setup_waits_for_the_loading_hold() {
        let mut world = setup_world(GameState::Playing, false);
        world.insert_resource(LoadingHold::for_seconds(1.0, 2.0));

        for _ in 0..2 {
            world.run_system_once(finish_setup).unwrap();
            assert_eq!(world.resource::<ChangedCount>().0, 0, "still held");
            world.resource_mut::<LoadingHold>().tick();
        }
        world.run_system_once(finish_setup).unwrap();
        assert_eq!(world.resource::<ChangedCount>().0, 1);
    }

    #[test]
    fn finish_setup_ignores_other_requests() {
        let mut world = setup_world(GameState::Quitting, false);
        world.run_system_once(finish_setup).unwrap();
        assert_eq!(world.resource::<ChangedCount>().0, 0);
    }

    #[test]
    fn leaving_setup_for_another_state_does_not_wait() {
        let mut world = setup_world(GameState::Quitting, true);
        tick_check_pending_state(&mut world);
        assert_eq!(world.resource::<ChangedCount>().0, 1);
    }

    #[test]
    fn check_pending_state_does_nothing_without_a_request() {
        // NextGameState defaults to no request (`None`).
        let mut world = world_with_next_state(NextGameState::default());

        tick_check_pending_state(&mut world);

        assert_eq!(world.resource::<ChangedCount>().0, 0);
        let ns = world.resource::<NextGameState>();
        assert_eq!(ns.get(), None);
    }
}
