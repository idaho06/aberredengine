//! Lua-based phase state machine component.
//!
//! [`LuaPhase`] is a per-entity state machine whose phases name Lua callback
//! functions.
//!
//! # How It Works
//!
//! 1. An entity is spawned with a `LuaPhase` holding the phase definitions.
//! 2. `lua_phase_system` runs once per sim tick and calls the current phase's
//!    named Lua functions: `on_enter(ctx, input)` on the first tick and after each
//!    transition, `on_exit(ctx)` after a swap (looked up by the old phase's name),
//!    and `on_update(ctx, input, dt)` every tick.
//! 3. `on_enter`/`on_update` can return a phase name, or call
//!    `engine.phase_transition(entity_id, "next_phase")`; either applies on the
//!    next tick.
//!
//! # Lua API
//!
//! ```lua
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
//! function scene_init_update(ctx, input, dt)
//!     return "get_started"
//! end
//!
//! function scene_get_started_enter(ctx, input)
//!     engine.play_music("player_ready", false)
//! end
//! ```

use aberred_core::components::phase::Phase;
use bevy_ecs::prelude::Component;
use rustc_hash::FxHashMap;

/// Callback function names for a single phase.
#[derive(Clone, Debug, Default)]
pub struct PhaseCallbacks {
    /// Function to call when entering this phase: `(ctx, input)`.
    pub on_enter: Option<String>,
    /// Function to call each sim tick in this phase: `(ctx, input, dt)`.
    pub on_update: Option<String>,
    /// Function to call when exiting this phase: `(ctx)`.
    pub on_exit: Option<String>,
}

/// Lua-based phase state machine component, processed by
/// [`lua_phase_system`](crate::systems::luaphase::lua_phase_system).
///
/// Wraps a core [`Phase`], sharing its swap and first-run flag, and adds the
/// names of the Lua callbacks for each phase.
#[derive(Clone, Debug, Component)]
pub struct LuaPhase {
    /// Current/previous/next phase and time in phase.
    pub phase: Phase,
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
            phase: Phase::new(initial_phase),
            phases,
        }
    }

    /// Get the callbacks for the current phase.
    pub(crate) fn current_callbacks(&self) -> Option<&PhaseCallbacks> {
        self.phases.get(&self.phase.current)
    }

    /// Get the callbacks for a specific phase.
    pub(crate) fn get_callbacks(&self, phase: &str) -> Option<&PhaseCallbacks> {
        self.phases.get(phase)
    }
}
