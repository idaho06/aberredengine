//! Lua-based collision rule component.
//!
//! This module provides [`LuaCollisionRule`], a data-only collision rule that
//! stores the name of a Lua callback function instead of a Rust function pointer.
//! This allows collision behavior to be defined in Lua scripts.
//!
//! # Example
//!
//! ```lua
//! -- In level01.lua
//! engine.spawn()
//!     :with_group("collision_rules")
//!     :with_lua_collision_rule("ball", "brick", "on_ball_brick")
//!     :build()
//!
//! -- Later in the same or another Lua file
//! function on_ball_brick(ball_id, brick_id, ctx)
//!     -- Handle collision...
//! end
//! ```
//!
//! # Related
//!
//! - [`crate::components::collision::CollisionRule`] – Rust-based collision rules
//! - [`crate::systems::collision`] – collision detection and observer systems

use bevy_ecs::prelude::*;

/// Collision rule that invokes a Lua callback function.
///
/// When a collision is detected between entities with groups matching
/// `group_a` and `group_b`, the Lua function named `callback` is invoked
/// with the entity IDs and a context table containing collision data.
#[derive(Component, Debug, Clone)]
pub struct LuaCollisionRule {
    /// First group name to match
    pub group_a: String,
    /// Second group name to match
    pub group_b: String,
    /// Name of the Lua function to call on collision
    pub callback: String,
}

impl LuaCollisionRule {
    /// Create a new Lua collision rule.
    pub fn new(
        group_a: impl Into<String>,
        group_b: impl Into<String>,
        callback: impl Into<String>,
    ) -> Self {
        Self {
            group_a: group_a.into(),
            group_b: group_b.into(),
            callback: callback.into(),
        }
    }

    /// Check if this rule matches the given groups and return entities in order.
    ///
    /// Returns `Some((entity_a, entity_b, callback))` if the rule matches,
    /// with entities ordered to match `group_a` and `group_b` respectively.
    pub fn match_and_order(
        &self,
        ent_a: Entity,
        ent_b: Entity,
        group_a: &str,
        group_b: &str,
    ) -> Option<(Entity, Entity, &str)> {
        if self.group_a == group_a && self.group_b == group_b {
            Some((ent_a, ent_b, &self.callback))
        } else if self.group_a == group_b && self.group_b == group_a {
            Some((ent_b, ent_a, &self.callback))
        } else {
            None
        }
    }
}
