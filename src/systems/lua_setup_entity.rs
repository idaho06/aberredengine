//! One-shot Lua entity setup system.
//!
//! [`lua_setup_entity_system`] reacts to every entity that gains a
//! [`LuaSetup`] component and calls the named Lua function once, passing the
//! standard entity context table. It is ordered before `animation_controller`
//! so that setup callbacks can set animation state in the same frame.
//!
//! For timing and contract details see [`crate::components::luasetup`].

use bevy_ecs::prelude::*;

use crate::components::luasetup::LuaSetup;
use crate::systems::lua_commands::{
    CallShape, LuaDispatch, call_entity_callback, drain_dispatch_commands, refresh_signal_cache,
};

pub fn lua_setup_entity_system(
    query: Query<(Entity, &LuaSetup), Added<LuaSetup>>,
    mut p: LuaDispatch,
) {
    if query.is_empty() {
        return;
    }

    refresh_signal_cache(&mut p);

    for (entity, lua_setup) in &query {
        call_entity_callback(
            &mut p,
            entity,
            &lua_setup.callback,
            "LuaSetup",
            CallShape::CtxOnly,
        );
    }

    drain_dispatch_commands(&mut p);
}
