//! Applies buffered [`SignalIntent`](crate::resources::signal_intents::SignalIntent)s to
//! [`WorldSignals`], [`NextScene`] and [`NextGameState`], logic-side.
//!
//! `GuiCallback` runs inside `render_system` and has no live `WorldSignals` to write to, so it
//! queues [`SignalIntent`](crate::resources::signal_intents::SignalIntent)s instead. This system
//! drains that buffer at the top of the sim schedule (`SimSet::ApplyIntents`), before
//! `check_pending_state` and everything that follows from it, so a write queued during the
//! previous frame's `render_system` is visible to this tick's scene logic, and a scene request
//! is picked up by this tick's scene switch poll.

use bevy_ecs::prelude::*;

use crate::resources::gamestate::NextGameState;
use crate::resources::next_scene::NextScene;
use crate::resources::signal_intents::SignalIntents;
use crate::resources::worldsignals::WorldSignals;

/// Drains [`SignalIntents`] into [`WorldSignals`], [`NextScene`] and [`NextGameState`]. See the
/// module doc comment for cadence.
pub fn apply_signal_intents(
    mut intents: ResMut<SignalIntents>,
    mut signals: ResMut<WorldSignals>,
    mut next_scene: ResMut<NextScene>,
    mut next_state: ResMut<NextGameState>,
) {
    intents.apply_to(&mut signals, &mut next_scene, &mut next_state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    #[test]
    fn apply_signal_intents_drains_into_every_target() {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world.insert_resource(NextScene::default());
        world.insert_resource(NextGameState::default());
        let mut intents = SignalIntents::default();
        intents.set_flag("gui:action:save");
        intents.request_scene("b");
        intents.request_quit();
        world.insert_resource(intents);

        world.run_system_once(apply_signal_intents).unwrap();

        let signals = world.resource::<WorldSignals>();
        assert!(signals.has_flag("gui:action:save"));
        assert_eq!(world.resource::<NextScene>().get(), Some("b"));
        assert!(world.resource::<NextGameState>().quit_requested());
        assert!(world.resource::<SignalIntents>().0.is_empty());
    }
}
