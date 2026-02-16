//! Lua-based phase state machine component.
//!
//! The [`LuaPhase`] component provides a phase state machine similar to
//! [`Phase`](super::phase::Phase), but with callbacks defined in Lua scripts
//! rather than Rust function pointers.
//!
//! # How It Works
//!
//! 1. Entity is spawned with a `LuaPhase` containing phase definitions
//! 2. The `lua_phase_system` runs each frame:
//!    - Looks up the current phase's callback function names
//!    - Calls the named Lua function (e.g., `scene_playing_update(time)`)
//!    - Lua can call `engine.phase_transition(entity_id, "next_phase")` to request transitions
//! 3. Lua has access to world signals, group counts, and can queue audio/spawn commands
//!
//! # Lua API
//!
//! ```lua
//! -- Define phases with named callback functions
//! engine.spawn()
//!     :with_group("scene_phases")
//!     :with_phase({
//!         initial = "init",
//!         phases = {
//!             init = { on_update = "scene_init_update" },
//!             get_started = {
//!                 on_enter = "scene_get_started_enter",
//!                 on_update = "scene_get_started_update"
//!             },
//!             playing = { on_update = "scene_playing_update" },
//!         }
//!     })
//!     :build()
//!
//! -- Callback functions receive entity_id and time_in_phase (for update)
//! function scene_init_update(entity_id, time_in_phase)
//!     engine.phase_transition(entity_id, "get_started")
//! end
//!
//! function scene_get_started_enter(entity_id, previous_phase)
//!     engine.play_music("player_ready", false)
//! end
//! ```

use bevy_ecs::prelude::Component;
use rustc_hash::FxHashMap;

/// Callback function names for a single phase.
#[derive(Clone, Debug, Default)]
pub struct PhaseCallbacks {
    /// Function to call when entering this phase (receives entity_id, previous_phase)
    pub on_enter: Option<String>,
    /// Function to call each frame (receives entity_id, time_in_phase)
    pub on_update: Option<String>,
    /// Function to call when exiting this phase (receives entity_id, next_phase)
    pub on_exit: Option<String>,
}

/// Lua-based phase state machine component.
///
/// Unlike the Rust [`Phase`](super::phase::Phase) component which stores
/// function pointers, this component stores callback function NAMES that
/// are looked up and called in the Lua runtime.
#[derive(Component, Clone, Debug)]
pub struct LuaPhase {
    /// The current phase label (e.g., "init", "playing").
    pub current: String,
    /// The phase before the last transition, if any.
    pub previous: Option<String>,
    /// Set to request a transition to a new phase. Cleared after processing.
    pub next: Option<String>,
    /// Seconds elapsed since entering the current phase.
    pub time_in_phase: f32,
    /// Whether to call on_enter on the first frame.
    pub needs_enter_callback: bool,
    /// Map of phase name -> callback function names.
    pub phases: FxHashMap<String, PhaseCallbacks>,
}

impl LuaPhase {
    /// Create a new LuaPhase with the given initial phase and phase definitions.
    pub fn new(
        initial_phase: impl Into<String>,
        phases: FxHashMap<String, PhaseCallbacks>,
    ) -> Self {
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
    pub fn current_callbacks(&self) -> Option<&PhaseCallbacks> {
        self.phases.get(&self.current)
    }

    /// Get the callbacks for a specific phase.
    pub fn get_callbacks(&self, phase: &str) -> Option<&PhaseCallbacks> {
        self.phases.get(phase)
    }

    // Request a transition to another phase.
    //
    // The transition occurs on the next frame when the lua_phase_system runs.
    /*
    pub fn transition_to(&mut self, next_phase: impl Into<String>) {
        self.next = Some(next_phase.into());
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_phases() -> FxHashMap<String, PhaseCallbacks> {
        let mut phases = FxHashMap::default();
        phases.insert(
            "idle".to_string(),
            PhaseCallbacks {
                on_enter: Some("idle_enter".to_string()),
                on_update: Some("idle_update".to_string()),
                on_exit: None,
            },
        );
        phases.insert(
            "moving".to_string(),
            PhaseCallbacks {
                on_enter: None,
                on_update: Some("moving_update".to_string()),
                on_exit: Some("moving_exit".to_string()),
            },
        );
        phases
    }

    #[test]
    fn test_new_sets_initial_phase() {
        let phase = LuaPhase::new("idle", make_phases());
        assert_eq!(phase.current, "idle");
    }

    #[test]
    fn test_new_defaults() {
        let phase = LuaPhase::new("idle", make_phases());
        assert!(phase.previous.is_none());
        assert!(phase.next.is_none());
        assert_eq!(phase.time_in_phase, 0.0);
        assert!(phase.needs_enter_callback);
    }

    #[test]
    fn test_new_accepts_string() {
        let phase = LuaPhase::new(String::from("moving"), make_phases());
        assert_eq!(phase.current, "moving");
    }

    #[test]
    fn test_current_callbacks_found() {
        let phase = LuaPhase::new("idle", make_phases());
        let cbs = phase.current_callbacks().unwrap();
        assert_eq!(cbs.on_enter.as_deref(), Some("idle_enter"));
        assert_eq!(cbs.on_update.as_deref(), Some("idle_update"));
        assert!(cbs.on_exit.is_none());
    }

    #[test]
    fn test_current_callbacks_not_found() {
        let phase = LuaPhase::new("nonexistent", make_phases());
        assert!(phase.current_callbacks().is_none());
    }

    #[test]
    fn test_get_callbacks_found() {
        let phase = LuaPhase::new("idle", make_phases());
        let cbs = phase.get_callbacks("moving").unwrap();
        assert!(cbs.on_enter.is_none());
        assert_eq!(cbs.on_update.as_deref(), Some("moving_update"));
        assert_eq!(cbs.on_exit.as_deref(), Some("moving_exit"));
    }

    #[test]
    fn test_get_callbacks_not_found() {
        let phase = LuaPhase::new("idle", make_phases());
        assert!(phase.get_callbacks("unknown").is_none());
    }

    #[test]
    fn test_phase_callbacks_default_all_none() {
        let cbs = PhaseCallbacks::default();
        assert!(cbs.on_enter.is_none());
        assert!(cbs.on_update.is_none());
        assert!(cbs.on_exit.is_none());
    }

    #[test]
    fn test_new_with_empty_phases() {
        let phase = LuaPhase::new("start", FxHashMap::default());
        assert_eq!(phase.current, "start");
        assert!(phase.current_callbacks().is_none());
    }
}
