//! Lua callbacks of a core collision rule.
//!
//! A Lua collision rule is a core `CollisionRule` (indexed, matched and
//! contact-tracked by core like any Rust rule) plus a [`LuaOnCollision`] on
//! the same entity naming the Lua functions to call. The observers in
//! [`crate::systems::lua_collision`] call them from core's events:
//! `CollisionStarted` -> `enter`, `Collided` -> `stay`, `CollisionEnded` ->
//! `exit`. Rules without a [`LuaOnCollision`] are Rust rules and are ignored.
//!
//! An entity has one `CollisionRule`, and one rule fires per overlapping pair
//! per tick (the lowest-`Entity` match), whether it is a Lua or a Rust rule.
//!
//! # Usage from Lua
//!
//! ```lua
//! engine.spawn()
//!     :with_lua_collision_rule("ball", "brick", "on_ball_brick")
//!     :build()
//!
//! -- Enter/exit only: the every-tick callback may be nil.
//! engine.spawn()
//!     :with_lua_collision_rule("player", "coin", nil)
//!     :with_lua_collision_enter("on_coin_enter")
//!     :with_lua_collision_exit("on_coin_exit")
//!     :build()
//! ```

use std::sync::Arc;

use aberred_core::components::collision::CollisionRule;
use bevy_ecs::prelude::Component;

/// Names the Lua functions to call for the entity's core `CollisionRule`.
///
/// ```
/// # use aberred_lua::components::lua_on_collision::LuaOnCollision;
/// let rule = LuaOnCollision::rule(
///     "player",
///     "coin",
///     LuaOnCollision {
///         enter: Some("on_coin_enter".into()),
///         ..Default::default()
///     },
/// );
/// ```
#[derive(Component, Clone, Debug, Default)]
pub struct LuaOnCollision {
    /// Called every tick the groups touch, after `enter` on the first one.
    pub stay: Option<Arc<str>>,
    /// Called on the first tick the groups touch.
    pub enter: Option<Arc<str>>,
    /// Called on the first tick the groups are apart; the ctx carries only
    /// each side's `id` and `group`.
    pub exit: Option<Arc<str>>,
}

impl LuaOnCollision {
    /// A Lua collision rule: the core rule for `group_a`/`group_b` plus the
    /// Lua `callbacks`.
    pub fn rule(
        group_a: impl Into<String>,
        group_b: impl Into<String>,
        callbacks: LuaOnCollision,
    ) -> (CollisionRule, LuaOnCollision) {
        (CollisionRule::new(group_a, group_b), callbacks)
    }
}
