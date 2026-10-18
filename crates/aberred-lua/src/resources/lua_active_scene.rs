//! The scene a Lua game is in.

use aberred_core::resources::signal_keys as sk;
use bevy_ecs::prelude::*;

/// The scene the Lua `switch_scene` system last entered; `update` calls its
/// `on_update_<scene>`.
///
/// Scripts cannot write it: the `scene` world signal is a published copy, so
/// a script write to that string changes only the string.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct LuaActiveScene(pub String);

impl Default for LuaActiveScene {
    fn default() -> Self {
        Self(sk::DEFAULT_SCENE.to_owned())
    }
}
