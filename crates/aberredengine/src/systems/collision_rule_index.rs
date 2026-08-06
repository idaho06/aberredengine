//! Lua-aware [`CollisionRuleIndex`](aberred_core::resources::collision_rule_index::CollisionRuleIndex)
//! rebuild.
//!
//! Shadows `aberred_core::systems::collision_rule_index::rebuild_collision_rule_index`
//! with a variant that also indexes `LuaCollisionRule`. The Lua-aware body
//! lives in [`crate::systems::lua_collision_rule_index`] (a separate file so
//! it can move to `aberred-lua` without dragging this file's
//! `--no-default-features` re-export along with it). Under
//! `#[cfg(not(feature = "lua"))]` this resolves to core's Rust-only variant
//! instead.

#[cfg(feature = "lua")]
pub use crate::systems::lua_collision_rule_index::rebuild_collision_rule_index;
#[cfg(not(feature = "lua"))]
pub use aberred_core::systems::collision_rule_index::rebuild_collision_rule_index;
