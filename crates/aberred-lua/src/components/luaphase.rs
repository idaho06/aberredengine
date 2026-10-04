//! Lua-based phase state machine component.
//!
//! [`LuaPhase`] is a per-entity state machine whose phases name Lua callback
//! functions.
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

/// Lua-based phase state machine component, processed by
/// [`lua_phase_system`](crate::systems::luaphase::lua_phase_system).
#[derive(Clone, Debug, Component)]
pub struct LuaPhase {
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
    /// Map of phase name → callback function names.
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
}
