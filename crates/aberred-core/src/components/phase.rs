//! Rust-based phase state machine component.
//!
//! [`Phase`] is the generic phase-state storage of the Rust callback path. The
//! default `Phase<PhaseCallbackFns>` form is the Rust-facing component.
//!
//! # How It Works
//!
//! 1. Entity is spawned with a `Phase` containing phase definitions
//! 2. The `phase_system` runs each frame:
//!    - Looks up the current phase's callback function pointers
//!    - Calls the Rust callbacks with `(entity, &mut PhaseCtx, &InputState, ...)`
//!    - Callbacks can return `Some(phase_name)` to request a transition
//! 3. Transitions can also be requested externally via `phase.next = Some("...")`
//!
//! # Callback Signatures
//!
//! ```ignore
//! fn my_enter(entity: Entity, ctx: &mut GameCtx, input: &InputState) -> Option<String> {
//!     // Return Some("next_phase") to transition, or None to stay
//!     None
//! }
//!
//! fn my_update(entity: Entity, ctx: &mut GameCtx, input: &InputState, dt: f32) -> Option<String> {
//!     if dt > 3.0 { Some("timeout".into()) } else { None }
//! }
//!
//! fn my_exit(entity: Entity, ctx: &mut GameCtx) {
//!     // Cleanup when leaving this phase
//! }
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use rustc_hash::FxHashMap;
//!
//! let mut phases = FxHashMap::default();
//! phases.insert("idle".into(), PhaseCallbackFns {
//!     on_enter: Some(idle_enter),
//!     on_update: Some(idle_update),
//!     on_exit: None,
//! });
//! phases.insert("moving".into(), PhaseCallbackFns {
//!     on_enter: None,
//!     on_update: Some(moving_update),
//!     on_exit: Some(moving_exit),
//! });
//!
//! commands.entity(my_entity).insert(Phase::new("idle", phases));
//! ```
//!
//! # Related
//!
//! - [`crate::systems::phase::phase_system`] – system that processes phase transitions and callbacks
//! - [`crate::systems::GameCtx`] – bundled ECS access passed to phase callbacks
//! - `aberred_lua::components::luaphase::LuaPhase` – Lua equivalent

use bevy_ecs::prelude::{Component, Entity};
use rustc_hash::FxHashMap;

use crate::resources::input::InputState;
use crate::systems::GameCtx;

/// Callback for entering a phase. Returns `Some(phase_name)` to immediately
/// transition, or `None` to stay in the current phase.
pub type PhaseEnterFn = for<'w, 's> fn(Entity, &mut GameCtx<'w, 's>, &InputState) -> Option<String>;

/// Callback for each frame while in a phase. Receives delta time as the last
/// argument. Returns `Some(phase_name)` to transition, or `None` to stay.
pub type PhaseUpdateFn =
    for<'w, 's> fn(Entity, &mut GameCtx<'w, 's>, &InputState, f32) -> Option<String>;

/// Callback for exiting a phase. No return value — the transition is already
/// committed.
pub type PhaseExitFn = for<'w, 's> fn(Entity, &mut GameCtx<'w, 's>);

/// Rust function-pointer callbacks for a single phase.
#[derive(Clone, Copy, Default)]
pub struct PhaseCallbackFns {
    /// Function to call when entering this phase.
    pub on_enter: Option<PhaseEnterFn>,
    /// Function to call each frame while in this phase.
    pub on_update: Option<PhaseUpdateFn>,
    /// Function to call when exiting this phase.
    pub on_exit: Option<PhaseExitFn>,
}

impl std::fmt::Debug for PhaseCallbackFns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhaseCallbackFns")
            .field("on_enter", &self.on_enter.map(|_| "fn(...)"))
            .field("on_update", &self.on_update.map(|_| "fn(...)"))
            .field("on_exit", &self.on_exit.map(|_| "fn(...)"))
            .finish()
    }
}

/// Generic phase state machine component.
///
/// The default `Phase` type stores Rust function pointers via
/// [`PhaseCallbackFns`] and is processed by
/// [`phase_system`](crate::systems::phase::phase_system).
#[derive(Clone, Component)]
pub struct Phase<C = PhaseCallbackFns> {
    /// The current phase label (e.g., "idle", "playing").
    pub current: String,
    /// The phase before the last transition, if any.
    pub previous: Option<String>,
    /// Set to request a transition to a new phase. Cleared after processing.
    pub next: Option<String>,
    /// Seconds elapsed since entering the current phase.
    pub time_in_phase: f32,
    /// Whether to call on_enter on the first frame.
    pub needs_enter_callback: bool,
    /// Map of phase name → callback function pointers.
    pub phases: FxHashMap<String, C>,
}

impl<C> Phase<C> {
    /// Create a new Phase with the given initial phase and phase definitions.
    pub fn new(initial_phase: impl Into<String>, phases: FxHashMap<String, C>) -> Self {
        Self {
            current: initial_phase.into(),
            previous: None,
            next: None,
            time_in_phase: 0.0,
            needs_enter_callback: true,
            phases,
        }
    }

    /// Get the callbacks for the current phase.
    pub fn current_callbacks(&self) -> Option<&C> {
        self.phases.get(&self.current)
    }

    /// Get the callbacks for a specific phase.
    pub fn get_callbacks(&self, phase: &str) -> Option<&C> {
        self.phases.get(phase)
    }
}

impl<C> std::fmt::Debug for Phase<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Phase")
            .field("current", &self.current)
            .field("previous", &self.previous)
            .field("next", &self.next)
            .field("time_in_phase", &self.time_in_phase)
            .field("needs_enter_callback", &self.needs_enter_callback)
            .field("phases", &self.phases.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_arms_enter_callback_for_initial_phase() {
        let phase: Phase<u8> = Phase::new("idle", FxHashMap::default());
        assert_eq!(phase.current, "idle");
        assert!(phase.needs_enter_callback);
        assert!(phase.previous.is_none() && phase.next.is_none());
        assert_eq!(phase.time_in_phase, 0.0);
    }

    #[test]
    fn current_callbacks_follow_current_phase() {
        let mut phases = FxHashMap::default();
        phases.insert("idle".to_string(), 1u8);
        phases.insert("run".to_string(), 2u8);
        let mut phase = Phase::new("idle", phases);
        assert_eq!(phase.current_callbacks(), Some(&1));
        phase.current = "run".into();
        assert_eq!(phase.current_callbacks(), Some(&2));
        phase.current = "missing".into();
        assert_eq!(phase.current_callbacks(), None);
    }
}
