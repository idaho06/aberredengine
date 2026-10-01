//! Lua-based collision rule component.
//!
//! [`LuaCollisionRule`] is the Lua-flavoured alias of the shared generic
//! [`CollisionRule`] component, using a Lua
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
/// ```
/// # use aberred_core::components::collision::CollisionRule;
/// # use aberred_lua::components::luacollision::{LuaCollisionCallback, LuaCollisionRule};
/// let rule: LuaCollisionRule =
///     CollisionRule::new("ball", "brick", LuaCollisionCallback { name: "on_ball_brick".into() });
/// ```
pub type LuaCollisionRule = CollisionRule<LuaCollisionCallback>;

#[cfg(test)]
mod tests {
    //! `CollisionRule` and `LuaCollisionRule` share one generic
    //! `match_and_order`, so they must agree for the same group inputs.

    use super::*;
    use aberred_core::components::collision::BoxSides;
    use aberred_core::systems::GameCtx;
    use bevy_ecs::entity::Entity;

    fn dummy_callback(_a: Entity, _b: Entity, _sa: &BoxSides, _sb: &BoxSides, _ctx: &mut GameCtx) {}

    /// Build matching CollisionRule and LuaCollisionRule pairs with the same groups.
    fn make_matching_rules(ga: &str, gb: &str) -> (CollisionRule, LuaCollisionRule) {
        let rust_rule = CollisionRule::rust(ga, gb, dummy_callback);
        let lua_rule = CollisionRule::new(ga, gb, LuaCollisionCallback { name: "cb".into() });
        (rust_rule, lua_rule)
    }

    #[test]
    fn collision_rule_and_lua_rule_match_direct_groups_consistently() {
        let (rust_rule, lua_rule) = make_matching_rules("ball", "brick");
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        assert_eq!(
            rust_rule.match_and_order(ent_a, ent_b, "ball", "brick"),
            lua_rule.match_and_order(ent_a, ent_b, "ball", "brick"),
        );
        assert_eq!(
            lua_rule.match_and_order(ent_a, ent_b, "ball", "brick"),
            Some((ent_a, ent_b))
        );
    }

    #[test]
    fn collision_rule_and_lua_rule_reorder_entities_consistently_when_groups_swapped() {
        let (rust_rule, lua_rule) = make_matching_rules("ball", "brick");
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        // Groups arrive swapped relative to the rule — both types must reorder identically.
        assert_eq!(
            rust_rule.match_and_order(ent_a, ent_b, "brick", "ball"),
            lua_rule.match_and_order(ent_a, ent_b, "brick", "ball"),
        );
        assert_eq!(
            lua_rule.match_and_order(ent_a, ent_b, "brick", "ball"),
            Some((ent_b, ent_a))
        );
    }

    #[test]
    fn collision_rule_and_lua_rule_both_return_none_for_non_matching_groups() {
        let (rust_rule, lua_rule) = make_matching_rules("ball", "brick");
        let ent_a = Entity::from_bits(1);
        let ent_b = Entity::from_bits(2);
        assert_eq!(
            rust_rule.match_and_order(ent_a, ent_b, "player", "enemy"),
            lua_rule.match_and_order(ent_a, ent_b, "player", "enemy"),
        );
        assert_eq!(
            lua_rule.match_and_order(ent_a, ent_b, "player", "enemy"),
            None
        );
    }
}
