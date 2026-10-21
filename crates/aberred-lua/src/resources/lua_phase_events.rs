//! Core phase events waiting for their Lua callbacks.

use std::sync::Arc;

use bevy_ecs::prelude::*;

/// `PhaseEntered` / `PhaseExited` events of every phase entity, in trigger
/// order, queued by the Lua phase observers and drained by
/// `lua_phase_callbacks_system`, which calls the Lua enter/exit callbacks after
/// the tick's collision, in Playing only.
#[derive(Resource, Default)]
pub struct LuaPhaseEvents(pub(crate) Vec<LuaPhaseEvent>);

/// One queued phase event.
pub(crate) enum LuaPhaseEvent {
    /// A `PhaseEntered`: the entered phase and the one left.
    Entered {
        entity: Entity,
        name: Arc<str>,
        previous: Option<Arc<str>>,
    },
    /// A `PhaseExited`: the phase left.
    Exited { entity: Entity, name: Arc<str> },
}
