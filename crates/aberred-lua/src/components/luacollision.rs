//! Lua-based collision rule component.
//!
//! [`LuaCollisionRule`] names the Lua functions to call when two entity groups
//! collide: every tick they touch, when they start touching, and when they
//! stop.
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
//! -- Enter/exit only: the every-tick callback may be nil.
//! engine.spawn()
//!     :with_lua_collision_rule("player", "coin", nil)
//!     :with_lua_collision_enter("on_coin_enter")
//!     :with_lua_collision_exit("on_coin_exit")
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

use aberred_core::components::collision::RuleGroups;
use aberred_core::resources::collision_contacts::RuleContacts;
use aberred_core::resources::collision_rule_index::RuleIndex;
use bevy_ecs::prelude::*;

/// Collision rule that invokes Lua callback functions.
///
/// While entities with groups matching `group_a` and `group_b` overlap, the
/// Lua function named `callback` (if any) is invoked every tick with a
/// context table containing collision data. `on_enter` (if any) is invoked
/// on the first tick they touch, before `callback`; `on_exit` (if any) on
/// the first tick they are apart.
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
    /// Name of the Lua function to call every tick the groups touch.
    pub callback: Option<Arc<str>>,
    /// Name of the Lua function to call when the groups start touching.
    pub on_enter: Option<Arc<str>>,
    /// Name of the Lua function to call when the groups stop touching.
    pub on_exit: Option<Arc<str>>,
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
            callback: Some(callback.into()),
            on_enter: None,
            on_exit: None,
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

/// Contacts of [`LuaCollisionRule`] pairs, recorded by
/// `lua_collision_observer` and ended by `lua_collision_ended_system`.
pub type LuaCollisionContacts = RuleContacts<LuaCollisionRule>;
