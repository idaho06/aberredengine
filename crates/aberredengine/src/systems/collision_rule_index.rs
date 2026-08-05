//! Lua-aware [`CollisionRuleIndex`](aberred_core::resources::collision_rule_index::CollisionRuleIndex)
//! rebuild.
//!
//! Shadows `aberred_core::systems::collision_rule_index::rebuild_collision_rule_index`
//! with a variant that also indexes `LuaCollisionRule` under
//! `#[cfg(feature = "lua")]`. Under `#[cfg(not(feature = "lua"))]` the
//! re-export below resolves to core's Rust-only variant instead. The
//! Rust-bucket half is not duplicated here -- it calls core's
//! `rebuild_rust_bucket` (the same shared-tail pattern
//! `aberred_core::systems::menu::dispatch_menu_action` already uses).

#[cfg(not(feature = "lua"))]
pub use aberred_core::systems::collision_rule_index::rebuild_collision_rule_index;

#[cfg(feature = "lua")]
use crate::components::luacollision::LuaCollisionRule;
#[cfg(feature = "lua")]
use aberred_core::components::collision::CollisionRule;
#[cfg(feature = "lua")]
use aberred_core::resources::collision_rule_index::CollisionRuleIndex;
#[cfg(feature = "lua")]
use aberred_core::systems::collision_rule_index::rebuild_rust_bucket;
#[cfg(feature = "lua")]
use bevy_ecs::prelude::*;

#[cfg(feature = "lua")]
pub fn rebuild_collision_rule_index(
    mut index: ResMut<CollisionRuleIndex>,
    changed_lua: Query<(), Changed<LuaCollisionRule>>,
    changed_rust: Query<(), Changed<CollisionRule>>,
    mut removed_lua: RemovedComponents<LuaCollisionRule>,
    removed_rust: RemovedComponents<CollisionRule>,
    all_lua: Query<(Entity, &LuaCollisionRule)>,
    all_rust: Query<(Entity, &CollisionRule)>,
) {
    // RemovedComponents readers must be drained every call regardless of the
    // dirty check outcome, or the reader cursor falls behind.
    let lua_dirty = !changed_lua.is_empty() || removed_lua.read().count() > 0;

    if lua_dirty {
        index
            .lua
            .rebuild(all_lua.iter().map(|(e, r)| (e, &r.group_a, &r.group_b)));
    }
    rebuild_rust_bucket(&mut index, changed_rust, removed_rust, all_rust);
}
