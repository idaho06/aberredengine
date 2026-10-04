//! Phase transition events.
//!
//! [`phase_system`](crate::systems::phase::phase_system) triggers [`PhaseEntered`]
//! once for an entity's initial phase, and [`PhaseExited`] then [`PhaseEntered`] for
//! each transition. Both are [`EntityEvent`]s targeted at the
//! [`Phase`](crate::components::phase::Phase)'s entity, so they reach per-entity
//! observers (`commands.spawn(..).observe(handler)`) as well as global ones
//! (`add_observer(handler)`).
//!
//! Observers run after `phase_system`, so read the phase from the event's `name`
//! rather than from `Phase::current`; see `phase_system` for the exact order.

use std::sync::Arc;

use bevy_ecs::prelude::*;

/// Triggered when an entity enters a phase.
#[derive(EntityEvent, Clone, Debug)]
pub struct PhaseEntered {
    /// The entity whose phase changed.
    #[event_target]
    pub entity: Entity,
    /// The entered phase.
    pub name: Arc<str>,
    /// The phase left by this transition; `None` for the initial phase.
    pub previous: Option<Arc<str>>,
}

/// Triggered when an entity leaves a phase, before the matching [`PhaseEntered`].
#[derive(EntityEvent, Clone, Debug)]
pub struct PhaseExited {
    /// The entity whose phase changed.
    #[event_target]
    pub entity: Entity,
    /// The exited phase.
    pub name: Arc<str>,
    /// The phase being entered.
    pub next: Arc<str>,
}
