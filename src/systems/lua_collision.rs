//! Lua collision observer and callback dispatch.
//!
//! This module provides the Lua-specific collision handling:
//!
//! - [`lua_collision_observer`] – receives [`CollisionEvent`](crate::events::collision::CollisionEvent)s
//!   and dispatches to [`LuaCollisionRule`](crate::components::luacollision::LuaCollisionRule) callbacks
//!
//! # Collision Flow
//!
//! 1. [`collision_detector`](crate::systems::collision_detector::collision_detector) detects overlaps
//!    and emits `CollisionEvent`s
//! 2. `lua_collision_observer` looks up matching Lua collision rules by
//!    [`Group`](crate::components::group::Group) names
//! 3. For each match, calls [`call_lua_collision_callback`] with pooled context tables
//!
//! # Lua Collision Callbacks
//!
//! Lua collision rules are defined via `engine.spawn():with_lua_collision_rule()`.
//! The callback receives a context table with entity data for both colliders:
//!
//! ```lua
//! function on_player_enemy(ctx)
//!     -- ctx.a and ctx.b contain entity data
//!     -- ctx.sides.a and ctx.sides.b contain collision sides
//! end
//! ```
//!
//! **Performance**: Context tables are pooled and reused between collisions to
//! reduce GC pressure. See [`CollisionCtxPool`](crate::resources::lua_runtime::CollisionCtxTables)
//! in runtime.rs for implementation details.
//!
//! # Related
//!
//! - [`crate::systems::collision_detector`] – pure Rust collision detection
//! - [`crate::components::luacollision::LuaCollisionRule`] – defines Lua collision handlers
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision::CollisionEvent`] – emitted on each collision

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::boxcollider::BoxCollider;
use crate::components::group::Group;
use crate::components::luacollision::LuaCollisionRule;
use crate::components::luaphase::LuaPhase;
use crate::components::signals::Signals;
use crate::events::audio::AudioCmd;
use crate::events::collision::CollisionEvent;
use crate::resources::animationstore::AnimationStore;
use crate::resources::lua_runtime::{LuaRuntime, populate_entity_signals};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::systems::collision::{
    compute_sides, resolve_collider_rect, resolve_groups, resolve_world_pos,
};
use crate::systems::lua_commands::{
    DrainScope, EntityCmdQueries, drain_and_process_effect_commands, process_phase_command,
};
use log::error;

/// System parameters for the Lua collision observer.
#[derive(SystemParam)]
pub struct LuaCollisionObserverParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub groups: Query<'w, 's, &'static Group>,
    pub lua_rules: Query<'w, 's, &'static LuaCollisionRule>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub luaphase_query: Query<'w, 's, (Entity, &'static mut LuaPhase)>,
    pub entity_cmds: EntityCmdQueries<'w, 's>,
    pub world_signals: ResMut<'w, WorldSignals>,
    pub audio_cmds: MessageWriter<'w, AudioCmd>,
    pub lua_runtime: NonSend<'w, LuaRuntime>,
    pub systems_store: Res<'w, SystemsStore>,
    pub animation_store: Res<'w, AnimationStore>,
}

pub fn lua_collision_observer(trigger: On<CollisionEvent>, mut params: LuaCollisionObserverParams) {
    if params.lua_rules.is_empty() {
        return;
    }

    let a = trigger.event().a;
    let b = trigger.event().b;

    let (ga, gb) = match resolve_groups(&params.groups, a, b) {
        Some(names) => names,
        None => return,
    };

    for lua_rule in params.lua_rules.iter() {
        if let Some((ent_a, ent_b)) = lua_rule.match_and_order(a, b, ga, gb) {
            let callback_name = lua_rule.callback.name.as_str();
            let pos_a = resolve_world_pos(
                &params.entity_cmds.positions.as_readonly(),
                &params.entity_cmds.global_transforms,
                ent_a,
            )
            .map(|v| (v.x, v.y));
            let pos_b = resolve_world_pos(
                &params.entity_cmds.positions.as_readonly(),
                &params.entity_cmds.global_transforms,
                ent_b,
            )
            .map(|v| (v.x, v.y));

            let (vel_a, speed_sq_a) = params
                .entity_cmds
                .rigid_bodies
                .get(ent_a)
                .ok()
                .map(|rb| {
                    (
                        Some((rb.velocity.x, rb.velocity.y)),
                        rb.velocity.length_sqr(),
                    )
                })
                .unwrap_or((None, 0.0));
            let (vel_b, speed_sq_b) = params
                .entity_cmds
                .rigid_bodies
                .get(ent_b)
                .ok()
                .map(|rb| {
                    (
                        Some((rb.velocity.x, rb.velocity.y)),
                        rb.velocity.length_sqr(),
                    )
                })
                .unwrap_or((None, 0.0));

            let rect_a = resolve_collider_rect(
                &params.entity_cmds.positions.as_readonly(),
                &params.entity_cmds.global_transforms,
                &params.box_colliders,
                ent_a,
            );
            let rect_b = resolve_collider_rect(
                &params.entity_cmds.positions.as_readonly(),
                &params.entity_cmds.global_transforms,
                &params.box_colliders,
                ent_b,
            );
            let (sides_a, sides_b) = compute_sides(rect_a, rect_b);

            let signals_a = params.entity_cmds.signals.get(ent_a).ok();
            let signals_b = params.entity_cmds.signals.get(ent_b).ok();
            let group_a = params.groups.get(ent_a).ok().map(|g| g.name().to_string());
            let group_b = params.groups.get(ent_b).ok().map(|g| g.name().to_string());

            // Update signal cache so Lua can read current world signals
            params
                .lua_runtime
                .update_signal_cache(params.world_signals.snapshot());

            if let Err(e) = call_lua_collision_callback(
                &params.lua_runtime,
                callback_name,
                ent_a.to_bits(),
                ent_b.to_bits(),
                pos_a,
                pos_b,
                vel_a,
                vel_b,
                speed_sq_a,
                speed_sq_b,
                rect_a.map(|r| (r.x, r.y, r.width, r.height)),
                rect_b.map(|r| (r.x, r.y, r.width, r.height)),
                &sides_a,
                &sides_b,
                signals_a,
                signals_b,
                group_a.as_deref(),
                group_b.as_deref(),
            ) {
                error!(target: "lua", "Collision callback '{}' error: {}", callback_name, e);
                return;
            }

            for cmd in params.lua_runtime.drain_collision_phase_commands() {
                process_phase_command(&mut params.luaphase_query, cmd);
            }

            drain_and_process_effect_commands(
                &params.lua_runtime,
                DrainScope::Collision,
                &mut params.commands,
                &mut params.world_signals,
                &mut params.entity_cmds,
                &mut params.audio_cmds,
                &params.systems_store,
                &params.animation_store,
            );

            return;
        }
    }
}

/// Clear all numeric indices from a Lua table (for reusing array tables).
fn clear_lua_array(table: &mlua::Table) -> mlua::Result<()> {
    let len = table.raw_len();
    for i in 1..=len {
        table.raw_set(i, mlua::Value::Nil)?;
    }
    Ok(())
}

/// Convert BoxSide to string representation.
fn box_side_to_str(side: &crate::components::collision::BoxSide) -> &'static str {
    match side {
        crate::components::collision::BoxSide::Left => "left",
        crate::components::collision::BoxSide::Right => "right",
        crate::components::collision::BoxSide::Top => "top",
        crate::components::collision::BoxSide::Bottom => "bottom",
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::components::collision::BoxSide;

    #[test]
    fn test_box_side_to_str_left() {
        assert_eq!(box_side_to_str(&BoxSide::Left), "left");
    }

    #[test]
    fn test_box_side_to_str_right() {
        assert_eq!(box_side_to_str(&BoxSide::Right), "right");
    }

    #[test]
    fn test_box_side_to_str_top() {
        assert_eq!(box_side_to_str(&BoxSide::Top), "top");
    }

    #[test]
    fn test_box_side_to_str_bottom() {
        assert_eq!(box_side_to_str(&BoxSide::Bottom), "bottom");
    }
}

/// Call a Lua collision callback with context data.
/// Uses pooled tables for fixed-structure data to reduce allocations.
#[allow(clippy::too_many_arguments)]
fn call_lua_collision_callback(
    lua_runtime: &LuaRuntime,
    callback_name: &str,
    entity_a_id: u64,
    entity_b_id: u64,
    pos_a: Option<(f32, f32)>,
    pos_b: Option<(f32, f32)>,
    vel_a: Option<(f32, f32)>,
    vel_b: Option<(f32, f32)>,
    speed_sq_a: f32,
    speed_sq_b: f32,
    rect_a: Option<(f32, f32, f32, f32)>,
    rect_b: Option<(f32, f32, f32, f32)>,
    sides_a: &[crate::components::collision::BoxSide],
    sides_b: &[crate::components::collision::BoxSide],
    signals_a: Option<&Signals>,
    signals_b: Option<&Signals>,
    group_a: Option<&str>,
    group_b: Option<&str>,
) -> mlua::Result<()> {
    let lua = lua_runtime.lua();

    let tables = lua_runtime.get_collision_ctx_pool()?;

    // === Populate Entity A ===
    tables.entity_a.set("id", entity_a_id)?;
    tables.entity_a.set("group", group_a.unwrap_or(""))?;
    tables.entity_a.set("speed_sq", speed_sq_a)?;

    if let Some((x, y)) = pos_a {
        tables.pos_a.set("x", x)?;
        tables.pos_a.set("y", y)?;
        tables.entity_a.set("pos", tables.pos_a.clone())?;
    } else {
        tables.entity_a.set("pos", mlua::Value::Nil)?;
    }

    if let Some((vx, vy)) = vel_a {
        tables.vel_a.set("x", vx)?;
        tables.vel_a.set("y", vy)?;
        tables.entity_a.set("vel", tables.vel_a.clone())?;
    } else {
        tables.entity_a.set("vel", mlua::Value::Nil)?;
    }

    if let Some((x, y, w, h)) = rect_a {
        tables.rect_a.set("x", x)?;
        tables.rect_a.set("y", y)?;
        tables.rect_a.set("w", w)?;
        tables.rect_a.set("h", h)?;
        tables.entity_a.set("rect", tables.rect_a.clone())?;
    } else {
        tables.entity_a.set("rect", mlua::Value::Nil)?;
    }

    // Signals A (creates fresh tables for variable-length data)
    if let Some(signals) = signals_a {
        populate_entity_signals(lua, &tables.signals_a, signals)?;
        tables.entity_a.set("signals", tables.signals_a.clone())?;
    } else {
        tables.entity_a.set("signals", mlua::Value::Nil)?;
    }

    // === Populate Entity B ===
    tables.entity_b.set("id", entity_b_id)?;
    tables.entity_b.set("group", group_b.unwrap_or(""))?;
    tables.entity_b.set("speed_sq", speed_sq_b)?;

    if let Some((x, y)) = pos_b {
        tables.pos_b.set("x", x)?;
        tables.pos_b.set("y", y)?;
        tables.entity_b.set("pos", tables.pos_b.clone())?;
    } else {
        tables.entity_b.set("pos", mlua::Value::Nil)?;
    }

    if let Some((vx, vy)) = vel_b {
        tables.vel_b.set("x", vx)?;
        tables.vel_b.set("y", vy)?;
        tables.entity_b.set("vel", tables.vel_b.clone())?;
    } else {
        tables.entity_b.set("vel", mlua::Value::Nil)?;
    }

    if let Some((x, y, w, h)) = rect_b {
        tables.rect_b.set("x", x)?;
        tables.rect_b.set("y", y)?;
        tables.rect_b.set("w", w)?;
        tables.rect_b.set("h", h)?;
        tables.entity_b.set("rect", tables.rect_b.clone())?;
    } else {
        tables.entity_b.set("rect", mlua::Value::Nil)?;
    }

    // Signals B (creates fresh tables for variable-length data)
    if let Some(signals) = signals_b {
        populate_entity_signals(lua, &tables.signals_b, signals)?;
        tables.entity_b.set("signals", tables.signals_b.clone())?;
    } else {
        tables.entity_b.set("signals", mlua::Value::Nil)?;
    }

    // === Populate Sides (clear and repopulate pooled arrays) ===
    clear_lua_array(&tables.sides_a)?;
    for (i, side) in sides_a.iter().enumerate() {
        tables.sides_a.set(i + 1, box_side_to_str(side))?;
    }

    clear_lua_array(&tables.sides_b)?;
    for (i, side) in sides_b.iter().enumerate() {
        tables.sides_b.set(i + 1, box_side_to_str(side))?;
    }

    let func: mlua::Function = lua.globals().get(callback_name)?;
    func.call::<()>(tables.ctx)?;

    Ok(())
}
