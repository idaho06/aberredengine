//! State machine component for controlling entity behavior.
//!
//! The [`Phase`] component represents a finite state machine where each state
//! (phase) is identified by a string label such as `"idle"`, `"moving"`, or
//! `"attacking"`. Phases govern how an entity behaves by allowing systems and
//! callbacks to react to the current phase.
//!
//! # Architecture
//!
//! - **Phases are string labels** – flexible and easy to debug
//! - **Three callback types per phase:**
//!   - `on_enter` – called once when entering a phase
//!   - `on_update` – called every frame while in the phase
//!   - `on_exit` – called once when leaving a phase
//! - **Transition requests** – set `next` to trigger a transition on the next frame
//! - **Time tracking** – `time_in_phase` tracks how long the entity has been in the current phase
//!
//! # Example
//!
//! ```ignore
//! fn idle_update(entity: Entity, time: f32, prev: Option<String>, ctx: &mut PhaseContext) -> Option<String> {
//!     if time >= 3.0 {
//!         return Some("moving".into());
//!     }
//!     None
//! }
//!
//! commands.spawn((
//!     Phase::new("idle")
//!         .on_update("idle", idle_update),
//!     // other components...
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::systems::phase`] – systems that process phase transitions and callbacks
//! - [`crate::events::phase::PhaseChangeEvent`] – event emitted on phase transitions

use bevy_ecs::prelude::*;
use rustc_hash::FxHashMap;
use std::fmt;

use crate::components::boxcollider::BoxCollider;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::events::audio::AudioCmd;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

/// Context passed to phase callbacks, providing access to ECS queries and resources.
///
/// This struct bundles references to commonly needed queries and resources so that
/// phase callbacks can read and modify entity state without needing direct world access.
pub struct PhaseContext<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub groups: &'a Query<'w, 's, &'static Group>,
    pub positions: &'a mut Query<'w, 's, &'static mut MapPosition>,
    pub rigid_bodies: &'a mut Query<'w, 's, &'static mut RigidBody>,
    pub box_colliders: &'a Query<'w, 's, &'static BoxCollider>,
    pub signals: &'a mut Query<'w, 's, &'static mut Signals>,
    pub world_signals: &'a mut ResMut<'w, WorldSignals>,
    pub world_time: &'a Res<'w, WorldTime>,
    pub audio_cmds: &'a mut MessageWriter<'w, AudioCmd>,
}

/// Function pointer type for phase callbacks.
///
/// # Parameters
///
/// - `Entity` – the entity whose phase is being processed
/// - `time: f32` – seconds spent in the current phase (0.0 for `on_enter`)
/// - `previous: Option<String>` – the phase the entity was in before (if any)
/// - `ctx: &mut PhaseContext` – mutable access to ECS queries and resources
///
/// # Returns
///
/// - `Some(next_phase)` – request a transition to `next_phase`
/// - `None` – remain in the current phase
///
/// > **Note:** The return value is currently only used by `on_update` callbacks.
/// > TODO: Consider using the return value from `on_enter` callbacks to allow
/// > immediate re-transitions (e.g., skip a phase based on conditions).
pub type PhaseCallback = for<'a, 'w, 's> fn(
    Entity,
    time: f32,
    previous: Option<String>,
    ctx: &mut PhaseContext<'a, 'w, 's>,
) -> Option<String>;

/// State machine component for controlling entity behavior through phases.
///
/// Each entity with a `Phase` component has a current phase (a string label)
/// and optional callbacks that run when entering, updating, or exiting phases.
///
/// # Fields
///
/// - `current` – the active phase label
/// - `previous` – the phase before the last transition (if any)
/// - `next` – set this to request a transition; cleared after processing
/// - `time_in_phase` – seconds elapsed since entering the current phase
/// - `on_enter` – callbacks invoked once when entering a phase
/// - `on_update` – callbacks invoked every frame while in a phase
/// - `on_exit` – callbacks invoked once when leaving a phase
///
/// # Transition Flow
///
/// 1. Set `phase.next = Some("new_phase".into())` (or call `transition_to`)
/// 2. On the next frame, [`phase_change_detector`](crate::systems::phase::phase_change_detector):
///    - Runs the `on_exit` callback for the old phase
///    - Emits a [`PhaseChangeEvent`](crate::events::phase::PhaseChangeEvent)
///    - Runs the `on_enter` callback for the new phase
///    - Resets `time_in_phase` to 0.0
/// 3. [`phase_update_system`](crate::systems::phase::phase_update_system) runs
///    the `on_update` callback each frame
#[derive(Component, Clone)]
pub struct Phase {
    /// The current phase label (e.g., `"idle"`, `"playing"`).
    pub current: String,
    /// The phase before the last transition, if any.
    pub previous: Option<String>,
    /// Set to request a transition to a new phase. Cleared after processing.
    pub next: Option<String>,
    /// Seconds elapsed since entering the current phase.
    pub time_in_phase: f32,
    /// Callbacks invoked once when entering each phase.
    pub on_enter: FxHashMap<String, PhaseCallback>,
    /// Callbacks invoked every frame while in each phase.
    pub on_update: FxHashMap<String, PhaseCallback>,
    /// Callbacks invoked once when exiting each phase.
    pub on_exit: FxHashMap<String, PhaseCallback>,
}
impl Phase {
    /// Create a new `Phase` component with the given initial phase.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let phase = Phase::new("idle");
    /// ```
    pub fn new(initial_phase: impl Into<String>) -> Self {
        Self {
            current: initial_phase.into(),
            previous: None,
            next: None,
            time_in_phase: 0.0,
            on_enter: FxHashMap::default(),
            on_update: FxHashMap::default(),
            on_exit: FxHashMap::default(),
        }
    }

    /// Request a transition to another phase.
    ///
    /// The transition occurs on the next frame when
    /// [`phase_change_detector`](crate::systems::phase::phase_change_detector) runs.
    ///
    /// # Example
    ///
    /// ```ignore
    /// phase.transition_to("attacking");
    /// ```
    pub fn transition_to(&mut self, next_phase: impl Into<String>) {
        self.next = Some(next_phase.into());
    }

    /// Register an `on_enter` callback for a specific phase (builder pattern).
    ///
    /// The callback runs once when the entity enters the specified phase.
    pub fn on_enter(mut self, phase: impl Into<String>, callback: PhaseCallback) -> Self {
        self.on_enter.insert(phase.into(), callback);
        self
    }

    /// Register an `on_update` callback for a specific phase (builder pattern).
    ///
    /// The callback runs every frame while the entity is in the specified phase.
    /// Return `Some(next_phase)` to request a transition.
    pub fn on_update(mut self, phase: impl Into<String>, callback: PhaseCallback) -> Self {
        self.on_update.insert(phase.into(), callback);
        self
    }

    /// Register an `on_exit` callback for a specific phase (builder pattern).
    ///
    /// The callback runs once when the entity leaves the specified phase.
    pub fn on_exit(mut self, phase: impl Into<String>, callback: PhaseCallback) -> Self {
        self.on_exit.insert(phase.into(), callback);
        self
    }
}

impl fmt::Debug for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Phase")
            .field("current", &self.current)
            .field("previous", &self.previous)
            .field("next", &self.next)
            .field("time_in_phase", &self.time_in_phase)
            .field("on_enter", &self.on_enter.keys().collect::<Vec<_>>())
            .field("on_update", &self.on_update.keys().collect::<Vec<_>>())
            .field("on_exit", &self.on_exit.keys().collect::<Vec<_>>())
            .finish()
    }
}
