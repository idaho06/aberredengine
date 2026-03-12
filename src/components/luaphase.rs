//! Lua-based phase state machine component.
//!
//! [`LuaPhase`] is the Lua-flavoured alias of the shared generic
//! [`Phase`](super::phase::Phase) component, using callback function names
//! instead of Rust function pointers.
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

#[cfg(test)]
use rustc_hash::FxHashMap;

use super::phase::Phase;

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
/// Unlike the default Rust [`Phase`](super::phase::Phase) component which
/// stores function pointers, this alias stores callback function names that
/// are looked up and called in the Lua runtime.
pub type LuaPhase = Phase<PhaseCallbacks>;

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
