//! Lua-callback systems, observers, and command dispatch.
//!
//! See the crate root doc comment for the sim/logic-world boundary this
//! crate sits at. `lua_menu`/`lua_gui_interactable_click`/
//! `lua_collision_rule_index`/`lua_mapspawn` are the Lua-priority bodies the
//! facade's same-named shadow modules re-export under
//! `#[cfg(feature = "lua")]`.

pub mod lua_animation_finished;
pub mod lua_collision;
pub mod lua_collision_rule_index;
pub mod lua_commands;
pub mod lua_gui_interactable_click;
pub mod lua_mapspawn;
pub mod lua_menu;
pub mod lua_setup_entity;
pub mod lua_tween_finished;
pub mod luaphase;
pub mod luatimer;
