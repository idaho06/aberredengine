//! Applies buffered [`SignalIntent`](crate::resources::signal_intents::SignalIntent)s to
//! [`WorldSignals`], logic-side.
//!
//! Part of the Phase 5d callback seam
//! (`docs/render-simulation-separation-brainstorm.md`): `GuiCallback` runs inside
//! `render_system` and can no longer write directly to a live `WorldSignals`, so it queues
//! [`SignalIntent`](crate::resources::signal_intents::SignalIntent)s instead. This system
//! drains that buffer at the top of the FIXED schedule (Phase 6d; was the top of `VARIABLE`
//! pre-6d), before `check_pending_state`/`fixed_update` and everything that follows from them,
//! so a write queued during last frame's `render_system` is visible to this substep's scene
//! logic.

use bevy_ecs::prelude::*;

use crate::resources::signal_intents::SignalIntents;
use crate::resources::worldsignals::WorldSignals;

/// Drains [`SignalIntents`] into [`WorldSignals`]. See the module doc comment for cadence.
pub fn apply_signal_intents(mut intents: ResMut<SignalIntents>, mut signals: ResMut<WorldSignals>) {
    intents.apply_to(&mut signals);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    #[test]
    fn apply_signal_intents_drains_into_world_signals() {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        let mut intents = SignalIntents::default();
        intents.set_flag("gui:action:save");
        world.insert_resource(intents);

        world.run_system_once(apply_signal_intents).unwrap();

        let signals = world.resource::<WorldSignals>();
        assert!(signals.has_flag("gui:action:save"));
        assert!(world.resource::<SignalIntents>().0.is_empty());
    }
}
