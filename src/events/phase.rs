//! Phase transition event.
//!
//! This module defines [`PhaseChangeEvent`], which is triggered whenever an
//! entity's [`Phase`](crate::components::phase::Phase) component transitions
//! from one phase to another.
//!
//! # Usage
//!
//! Observers can listen for this event to react to phase changes:
//!
//! ```ignore
//! fn on_phase_change(trigger: Trigger<PhaseChangeEvent>, query: Query<&Phase>) {
//!     let entity = trigger.event().entity;
//!     if let Ok(phase) = query.get(entity) {
//!         println!("Entity {:?} is now in phase: {}", entity, phase.current);
//!     }
//! }
//!
//! world.add_observer(on_phase_change);
//! ```

use bevy_ecs::prelude::*;

/// Event emitted when an entity's phase changes.
///
/// This event is triggered by [`phase_change_detector`](crate::systems::phase::phase_change_detector)
/// after the `on_exit` callback runs and before the `on_enter` callback runs.
///
/// # Fields
///
/// - `entity` – the entity whose phase has changed
#[derive(Event, Debug, Clone)]
pub struct PhaseChangeEvent {
    /// The entity that transitioned to a new phase.
    pub entity: Entity,
}
