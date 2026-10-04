//! Lua-based collision rule component.
//!
//! [`LuaCollisionRule`] names the Lua function to call when two entity groups
//! collide.
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
//! function on_ball_brick(ctx)
//!     -- Handle collision...
//! end
//! ```
//!
//! # Related
//!
//! - [`aberred_core::components::collision::CollisionRule`] – Rust-based collision rules
//! - [`aberred_core::systems::collision_detector`] – collision detection system
//! - [`crate::systems::lua_collision`] – Lua collision observer

use std::sync::Arc;

use aberred_core::resources::collision_rule_index::{RuleGroups, RuleIndex};
use bevy_ecs::prelude::*;

/// Collision rule that invokes a Lua callback function.
///
/// When a collision is detected between entities with groups matching
/// `group_a` and `group_b`, the Lua function named `callback` is invoked with
/// a context table containing collision data.
///
/// ```
/// # use aberred_lua::components::luacollision::LuaCollisionRule;
/// let rule = LuaCollisionRule::new("ball", "brick", "on_ball_brick");
/// ```
#[derive(Component, Clone, Debug)]
pub struct LuaCollisionRule {
    /// First group name to match.
    pub group_a: String,
    /// Second group name to match.
    pub group_b: String,
    /// Name of the Lua function to call on collision.
    pub callback: Arc<str>,
}

impl LuaCollisionRule {
    /// Create a rule calling the Lua function `callback` when `group_a` and
    /// `group_b` collide.
    pub fn new(
        group_a: impl Into<String>,
        group_b: impl Into<String>,
        callback: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            group_a: group_a.into(),
            group_b: group_b.into(),
            callback: callback.into(),
        }
    }
}

impl RuleGroups for LuaCollisionRule {
    fn groups(&self) -> (&str, &str) {
        (&self.group_a, &self.group_b)
    }
}

/// Index of [`LuaCollisionRule`] entities, rebuilt by core's
/// `rebuild_rule_index::<LuaCollisionRule>` and read by `lua_collision_observer`.
pub type LuaCollisionRuleIndex = RuleIndex<LuaCollisionRule>;
