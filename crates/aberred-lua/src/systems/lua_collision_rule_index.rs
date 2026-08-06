//! Lua-aware [`CollisionRuleIndex`](aberred_core::resources::collision_rule_index::CollisionRuleIndex)
//! rebuild.
//!
//! Shadows `aberred_core::systems::collision_rule_index::rebuild_collision_rule_index`
//! with a variant that also indexes `LuaCollisionRule`. Re-exported by
//! the facade's `systems::collision_rule_index` under `#[cfg(feature = "lua")]`.
//! The Rust-bucket half is not duplicated here -- it calls core's
//! `rebuild_rust_bucket` (the same shared-tail pattern
//! `aberred_core::systems::menu::dispatch_menu_action` already uses).

use crate::components::luacollision::LuaCollisionRule;
use aberred_core::components::collision::CollisionRule;
use aberred_core::resources::collision_rule_index::CollisionRuleIndex;
use aberred_core::systems::collision_rule_index::rebuild_rust_bucket;
use bevy_ecs::prelude::*;

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
