//! Lua-based collision rule component.
//!
//! [`LuaCollisionRule`] is the Lua-flavoured alias of the shared generic
//! [`CollisionRule`](super::collision::CollisionRule) component, using a Lua
//! callback function name instead of a Rust function pointer.
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

use aberred_core::components::collision::CollisionRule;

/// Lua callback function name for a collision rule.
///
/// Stores the name of the Lua function to call when a collision is detected.
/// Used as the callback payload type in [`LuaCollisionRule`].
#[derive(Clone, Debug)]
pub struct LuaCollisionCallback {
    /// Name of the Lua function to call on collision.
    pub name: Arc<str>,
}

/// Collision rule that invokes a Lua callback function.
///
/// Type alias over the generic [`CollisionRule`] using [`LuaCollisionCallback`]
/// as the callback payload. When a collision is detected between entities with
/// groups matching `group_a` and `group_b`, the Lua function named
/// `callback.name` is invoked with a context table containing collision data.
///
/// # Construction
///
/// Use [`CollisionRule::new`] with a [`LuaCollisionCallback`] payload:
///
/// ```ignore
/// CollisionRule::new("ball", "brick", LuaCollisionCallback { name: "on_ball_brick".into() })
/// ```
pub type LuaCollisionRule = CollisionRule<LuaCollisionCallback>;
