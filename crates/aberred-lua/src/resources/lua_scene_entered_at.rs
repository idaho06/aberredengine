//! The tick a Lua game last entered a scene.

use bevy_ecs::prelude::*;

/// The [`WorldTime::frame_count`](aberred_core::resources::worldtime::WorldTime::frame_count)
/// of the sim tick that last entered a scene, stamped by
/// [`lua_scene_entered_observer`](crate::systems::lua_scene::lua_scene_entered_observer).
/// `None` before the first scene.
#[derive(Resource, Debug, Default)]
pub struct LuaSceneEnteredAt(pub Option<u64>);
