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
//! - [`crate::components::collision::CollisionRule`] – Rust-based collision rules
//! - [`crate::systems::collision_detector`] – collision detection system
//! - [`crate::systems::lua_collision`] – Lua collision observer

use crate::components::collision::CollisionRule;

/// Lua callback function name for a collision rule.
///
/// Stores the name of the Lua function to call when a collision is detected.
/// Used as the callback payload type in [`LuaCollisionRule`].
#[derive(Clone, Debug)]
pub struct LuaCollisionCallback {
    /// Name of the Lua function to call on collision.
    pub name: String,
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::Entity;

    fn make_rule(ga: &str, gb: &str, cb: &str) -> LuaCollisionRule {
        CollisionRule::new(ga, gb, LuaCollisionCallback { name: cb.into() })
    }

    #[test]
    fn test_lua_match_and_order_direct() {
        let rule = make_rule("ball", "brick", "on_collision");
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        assert_eq!(
            rule.match_and_order(ent_a, ent_b, "ball", "brick"),
            Some((ent_a, ent_b))
        );
    }

    #[test]
    fn test_lua_match_and_order_reversed() {
        let rule = make_rule("ball", "brick", "on_collision");
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        // Groups arrive swapped relative to the rule — entities must be reordered.
        assert_eq!(
            rule.match_and_order(ent_a, ent_b, "brick", "ball"),
            Some((ent_b, ent_a))
        );
    }

    #[test]
    fn test_lua_match_and_order_no_match() {
        let rule = make_rule("ball", "brick", "on_collision");
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        assert_eq!(rule.match_and_order(ent_a, ent_b, "player", "enemy"), None);
    }

    #[test]
    fn test_lua_match_and_order_partial_match() {
        let rule = make_rule("ball", "brick", "on_collision");
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        assert_eq!(rule.match_and_order(ent_a, ent_b, "ball", "enemy"), None);
    }

    #[test]
    fn test_lua_callback_name_accessible() {
        let rule = make_rule("ball", "brick", "my_callback");
        assert_eq!(rule.callback.name, "my_callback");
    }
}
