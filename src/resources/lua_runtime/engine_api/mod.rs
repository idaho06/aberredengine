//! Engine table API registration for the Lua runtime.
//!
//! Registers all `engine.*` functions in the Lua global `engine` table.

#[macro_use]
mod macros;
mod animation;
mod assets;
mod audio;
mod base;
mod camera;
mod entity;
mod gameconfig;
mod input;
mod phase_group;
mod render;
mod signal;
mod spawn;

use super::commands::*;
use super::runtime::{LuaAppData, LuaRuntime};
use mlua::prelude::*;
use macros::push_fn_meta;
