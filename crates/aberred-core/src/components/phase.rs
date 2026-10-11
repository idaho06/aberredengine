//! Phase state machine component.
//!
//! [`Phase`] is a per-entity state machine over string-labeled phases. It holds
//! data only: [`phase_system`](crate::systems::phase::phase_system) applies
//! requested transitions and triggers
//! [`PhaseEntered`](crate::events::phase::PhaseEntered) /
//! [`PhaseExited`](crate::events::phase::PhaseExited). Per-phase behavior is
//! ordinary systems that match on [`Phase::current`] and request transitions by
//! setting [`Phase::next`], plus observers of those events.
//!
//! Phase names are never validated: a `next` that no system matches on switches to
//! a phase that does nothing. See [`phase_system`](crate::systems::phase::phase_system)
//! for when transitions apply and the events fire.
//!
//! # Usage
//!
//! ```ignore
//! commands.spawn(Phase::new("idle"));
//!
//! fn idle_system(mut phases: Query<&mut Phase, With<Player>>, input: Res<InputState>) {
//!     for mut phase in &mut phases {
//!         if phase.current == "idle" && input.action(InputAction::Action1).just_pressed {
//!             phase.next = Some("jumping".into());
//!         }
//!     }
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::systems::phase::phase_system`] – applies transitions and triggers the events
//! - [`crate::events::phase`] – `PhaseEntered` / `PhaseExited`
//! - `aberred_lua::components::lua_on_phase` – Lua callbacks on a `Phase`
//!   (observers of the same events, plus a per-tick update runner)

use bevy_ecs::entity::{ComponentCloneCtx, SourceComponent};
use bevy_ecs::prelude::Component;

/// Per-entity phase state machine; see the [module docs](self).
///
/// Cloning an entity (`clone_and_spawn`, a particle emission, a Lua
/// `engine.clone`) restarts its `Phase` in the source's current phase, so the
/// clone fires its own initial enter with `time_in_phase` at zero. Moving the
/// component to another entity keeps its state, and so does `Phase::clone`.
#[derive(Clone, Debug, PartialEq, Component)]
#[component(clone_behavior = Custom(restart_on_clone))]
pub struct Phase {
    /// The current phase label (e.g., "idle", "playing").
    pub current: String,
    /// The phase before the last transition, if any.
    pub previous: Option<String>,
    /// Set to request a transition to a new phase. Applied, then cleared, on the
    /// next run of `phase_system`.
    pub next: Option<String>,
    /// Seconds elapsed since entering the current phase.
    pub time_in_phase: f32,
    /// Whether the initial phase's enter has fired.
    pub(crate) entered: bool,
}

impl Phase {
    /// Create a phase state machine starting in `initial_phase`.
    pub fn new(initial_phase: impl Into<String>) -> Self {
        Self {
            current: initial_phase.into(),
            previous: None,
            next: None,
            time_in_phase: 0.0,
            entered: false,
        }
    }

    /// Marks the initial phase as entered. Returns `true` only on the first call,
    /// when `phase_system` fires the initial enter.
    pub(crate) fn begin(&mut self) -> bool {
        !std::mem::replace(&mut self.entered, true)
    }

    /// Applies a requested transition: `previous = current`, `current = next`,
    /// `time_in_phase = 0`. Returns the old phase name, or `None` (and changes
    /// nothing) when no transition is requested.
    pub(crate) fn apply_next(&mut self) -> Option<String> {
        let next = self.next.take()?;
        let old = std::mem::replace(&mut self.current, next);
        self.previous = Some(old.clone());
        self.time_in_phase = 0.0;
        Some(old)
    }
}

/// Entity clone handler for [`Phase`]: a clone restarts in the source's
/// current phase, a move keeps the whole state.
fn restart_on_clone(source: &SourceComponent, ctx: &mut ComponentCloneCtx) {
    if let Some(phase) = source.read::<Phase>() {
        let target = if ctx.moving() {
            phase.clone()
        } else {
            Phase::new(phase.current.clone())
        };
        ctx.write_target_component(target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    #[test]
    fn begin_is_true_only_once() {
        let mut phase = Phase::new("idle");
        assert!(phase.begin());
        assert!(!phase.begin());
    }

    #[test]
    fn apply_next_without_next_is_a_no_op() {
        let mut phase = Phase::new("idle");
        phase.time_in_phase = 1.0;
        assert_eq!(phase.apply_next(), None);
        assert_eq!(phase.current, "idle");
        assert!(phase.previous.is_none());
        assert_eq!(phase.time_in_phase, 1.0);
    }

    #[test]
    fn apply_next_swaps_and_resets_time() {
        let mut phase = Phase::new("idle");
        phase.time_in_phase = 1.0;
        phase.next = Some("run".into());
        assert_eq!(phase.apply_next().as_deref(), Some("idle"));
        assert_eq!(phase.current, "run");
        assert_eq!(phase.previous.as_deref(), Some("idle"));
        assert!(phase.next.is_none());
        assert_eq!(phase.time_in_phase, 0.0);
    }

    /// A source already in "run", entered, aged, with a pending transition.
    fn aged_phase() -> Phase {
        let mut phase = Phase::new("idle");
        phase.begin();
        phase.next = Some("run".into());
        phase.apply_next();
        phase.time_in_phase = 1.0;
        phase.next = Some("jump".into());
        phase
    }

    #[test]
    fn entity_clone_restarts_the_phase() {
        let mut world = World::new();
        let source = world.spawn(aged_phase()).id();

        let clone = world.entity_mut(source).clone_and_spawn();

        assert_eq!(world.get::<Phase>(clone).unwrap(), &Phase::new("run"));
        assert_eq!(world.get::<Phase>(source).unwrap(), &aged_phase());
    }

    #[test]
    fn moved_phase_keeps_its_state() {
        let mut world = World::new();
        let source = world.spawn(aged_phase()).id();
        let target = world.spawn_empty().id();

        world.entity_mut(source).move_components::<Phase>(target);

        assert!(world.get::<Phase>(source).is_none());
        assert_eq!(world.get::<Phase>(target).unwrap(), &aged_phase());
    }
}
