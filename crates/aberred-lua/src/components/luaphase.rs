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

use aberred_core::components::phase::Phase;

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
