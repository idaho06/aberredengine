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
//! reduce GC pressure. See [`CollisionCtxTables`](crate::resources::lua_runtime::CollisionCtxTables)
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
use crate::events::collision::CollisionEvent;
use crate::protocol::audio::AudioCmd;
use crate::resources::animationstore::AnimationStore;
use crate::resources::lua_runtime::{
    CtxOccupancy, LuaRuntime, PhaseCmd, SignalsCtxTables, clear_array_table,
    populate_entity_signals, set_opt,
};
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::WorldSignals;
use crate::systems::collision::{
    compute_sides, resolve_collider_rect, resolve_groups, resolve_world_pos,
};
use crate::systems::lua_commands::{
    DrainScope, EffectCmdBufs, EntityCmdQueries, drain_and_process_effect_commands,
    process_phase_command,
};
use log::{error, warn};

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

/// Observes `CollisionEvent`, invokes the matching Lua collision callback, and
/// queues any phase/animation/timer effects it requests.
pub fn lua_collision_observer(
    trigger: On<CollisionEvent>,
    mut params: LuaCollisionObserverParams,
    mut phase_buf: Local<Vec<PhaseCmd>>,
    mut effect_bufs: Local<EffectCmdBufs>,
) {
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
            let (group_a, group_b) = if ent_a == a { (ga, gb) } else { (gb, ga) };

            // Refresh the cached world-signal snapshot only when something has
            // changed since the last refresh. lua_plugin::update primes the
            // cache every frame; within a collision-heavy frame the common case
            // (no signal writes between collisions) skips the snapshot entirely,
            // avoiding a full per-collision re-clone of the dirtied domains.
            if params.world_signals.is_dirty() {
                params
                    .lua_runtime
                    .update_signal_cache(params.world_signals.snapshot());
            }

            let callback_result = call_lua_collision_callback(
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
                Some(group_a),
                Some(group_b),
            );

            params
                .lua_runtime
                .drain_collision_phase_commands_into(&mut phase_buf);
            for cmd in phase_buf.drain(..) {
                process_phase_command(&mut params.luaphase_query, cmd);
            }

            drain_and_process_effect_commands(
                &params.lua_runtime,
                DrainScope::Collision,
                &mut effect_bufs,
                &mut params.commands,
                &mut params.world_signals,
                &mut params.entity_cmds,
                &mut params.audio_cmds,
                &params.systems_store,
                &params.animation_store,
            );

            if let Err(e) = callback_result {
                error!(target: "lua", "Collision callback '{}' error: {}", callback_name, e);
            }

            return;
        }
    }
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

/// Bit assignments for one side's `CtxOccupancy` mask (see `CollisionCtxTables::occupancy_a/b`).
const COLL_BIT_GROUP: u32 = 1 << 0;
const COLL_BIT_POS: u32 = 1 << 1;
const COLL_BIT_VEL: u32 = 1 << 2;
const COLL_BIT_RECT: u32 = 1 << 3;
const COLL_BIT_SIGNALS: u32 = 1 << 4;

/// Populate one side of a pooled collision context table for a single entity.
#[allow(clippy::too_many_arguments)]
fn populate_collision_entity(
    entity_table: &mlua::Table,
    pos_table: &mlua::Table,
    vel_table: &mlua::Table,
    rect_table: &mlua::Table,
    signals_table: &mlua::Table,
    signals_inner: &SignalsCtxTables,
    occupancy: &CtxOccupancy,
    id: u64,
    group: Option<&str>,
    speed_sq: f32,
    pos: Option<(f32, f32)>,
    vel: Option<(f32, f32)>,
    rect: Option<(f32, f32, f32, f32)>,
    signals: Option<&Signals>,
) -> mlua::Result<()> {
    entity_table.set("id", id)?;
    entity_table.set("speed_sq", speed_sq)?;

    let old = occupancy.get();
    let mut new: u32 = 0;

    match group {
        Some(g) => {
            entity_table.set("group", g)?;
            new |= COLL_BIT_GROUP;
        }
        None if old & COLL_BIT_GROUP != 0 => entity_table.set("group", mlua::Value::Nil)?,
        None => {}
    }

    set_opt!(entity_table, "pos", pos, (x, y), COLL_BIT_POS, old, new, {
        pos_table.set("x", x)?;
        pos_table.set("y", y)?;
        entity_table.set("pos", pos_table.clone())?;
    });

    set_opt!(entity_table, "vel", vel, (vx, vy), COLL_BIT_VEL, old, new, {
        vel_table.set("x", vx)?;
        vel_table.set("y", vy)?;
        entity_table.set("vel", vel_table.clone())?;
    });

    set_opt!(
        entity_table,
        "rect",
        rect,
        (x, y, w, h),
        COLL_BIT_RECT,
        old,
        new,
        {
            rect_table.set("x", x)?;
            rect_table.set("y", y)?;
            rect_table.set("w", w)?;
            rect_table.set("h", h)?;
            entity_table.set("rect", rect_table.clone())?;
        }
    );

    set_opt!(
        entity_table,
        "signals",
        signals,
        s,
        COLL_BIT_SIGNALS,
        old,
        new,
        {
            populate_entity_signals(signals_table, signals_inner, s)?;
            entity_table.set("signals", signals_table.clone())?;
        }
    );

    occupancy.set(new);

    Ok(())
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
    let tables = lua_runtime.get_collision_ctx_pool();

    populate_collision_entity(
        &tables.entity_a,
        &tables.pos_a,
        &tables.vel_a,
        &tables.rect_a,
        &tables.signals_a,
        &tables.signals_a_inner,
        &tables.occupancy_a,
        entity_a_id,
        group_a,
        speed_sq_a,
        pos_a,
        vel_a,
        rect_a,
        signals_a,
    )?;

    populate_collision_entity(
        &tables.entity_b,
        &tables.pos_b,
        &tables.vel_b,
        &tables.rect_b,
        &tables.signals_b,
        &tables.signals_b_inner,
        &tables.occupancy_b,
        entity_b_id,
        group_b,
        speed_sq_b,
        pos_b,
        vel_b,
        rect_b,
        signals_b,
    )?;

    clear_array_table(&tables.sides_a)?;
    for (i, side) in sides_a.iter().enumerate() {
        tables.sides_a.set(i + 1, box_side_to_str(side))?;
    }

    clear_array_table(&tables.sides_b)?;
    for (i, side) in sides_b.iter().enumerate() {
        tables.sides_b.set(i + 1, box_side_to_str(side))?;
    }

    match lua_runtime.get_function_cached(callback_name)? {
        Some(func) => {
            func.call::<()>(tables.ctx)?;
        }
        None => {
            warn!(target: "lua", "Collision callback '{}' not found", callback_name);
        }
    }

    Ok(())
}

#[cfg(test)]
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

    #[test]
    fn populate_collision_entity_group_none_is_nil() {
        let lua = mlua::Lua::new();
        let entity_table = lua.create_table().unwrap();
        let pos_table = lua.create_table().unwrap();
        let vel_table = lua.create_table().unwrap();
        let rect_table = lua.create_table().unwrap();
        let signals_table = lua.create_table().unwrap();
        let signals_inner = SignalsCtxTables {
            flags: lua.create_table().unwrap(),
            integers: lua.create_table().unwrap(),
            scalars: lua.create_table().unwrap(),
            strings: lua.create_table().unwrap(),
            scratch_keys: std::cell::RefCell::new(Vec::new()),
        };

        let occupancy = CtxOccupancy::default();
        populate_collision_entity(
            &entity_table,
            &pos_table,
            &vel_table,
            &rect_table,
            &signals_table,
            &signals_inner,
            &occupancy,
            1,
            None,
            0.0,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let group: mlua::Value = entity_table.get("group").unwrap();
        assert!(matches!(group, mlua::Value::Nil));
    }

    #[test]
    fn populate_collision_entity_group_some_is_string() {
        let lua = mlua::Lua::new();
        let entity_table = lua.create_table().unwrap();
        let pos_table = lua.create_table().unwrap();
        let vel_table = lua.create_table().unwrap();
        let rect_table = lua.create_table().unwrap();
        let signals_table = lua.create_table().unwrap();
        let signals_inner = SignalsCtxTables {
            flags: lua.create_table().unwrap(),
            integers: lua.create_table().unwrap(),
            scalars: lua.create_table().unwrap(),
            strings: lua.create_table().unwrap(),
            scratch_keys: std::cell::RefCell::new(Vec::new()),
        };

        let occupancy = CtxOccupancy::default();
        populate_collision_entity(
            &entity_table,
            &pos_table,
            &vel_table,
            &rect_table,
            &signals_table,
            &signals_inner,
            &occupancy,
            1,
            Some("enemy"),
            0.0,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let group: String = entity_table.get("group").unwrap();
        assert_eq!(group, "enemy");
    }

    struct CollisionEntityTables {
        entity: mlua::Table,
        pos: mlua::Table,
        vel: mlua::Table,
        rect: mlua::Table,
        signals: mlua::Table,
        signals_inner: SignalsCtxTables,
    }

    fn make_collision_entity_tables(lua: &mlua::Lua) -> CollisionEntityTables {
        CollisionEntityTables {
            entity: lua.create_table().unwrap(),
            pos: lua.create_table().unwrap(),
            vel: lua.create_table().unwrap(),
            rect: lua.create_table().unwrap(),
            signals: lua.create_table().unwrap(),
            signals_inner: SignalsCtxTables {
                flags: lua.create_table().unwrap(),
                integers: lua.create_table().unwrap(),
                scalars: lua.create_table().unwrap(),
                strings: lua.create_table().unwrap(),
                scratch_keys: std::cell::RefCell::new(Vec::new()),
            },
        }
    }

    fn assert_sparse_collision_ctx_shape(entity_table: &mlua::Table) {
        let pos: mlua::Table = entity_table.get("pos").unwrap();
        assert_eq!(pos.get::<f32>("x").unwrap(), 1.0);
        assert_eq!(pos.get::<f32>("y").unwrap(), 2.0);
        for key in ["group", "vel", "rect", "signals"] {
            let value: mlua::Value = entity_table.get(key).unwrap();
            assert!(matches!(value, mlua::Value::Nil), "expected {key} to be nil");
        }
    }

    fn assert_full_collision_ctx_shape(entity_table: &mlua::Table) {
        assert_eq!(entity_table.get::<String>("group").unwrap(), "enemy");
        assert!(entity_table.get::<mlua::Table>("pos").is_ok());
        assert!(entity_table.get::<mlua::Table>("vel").is_ok());
        assert!(entity_table.get::<mlua::Table>("rect").is_ok());
        assert!(entity_table.get::<mlua::Table>("signals").is_ok());
    }

    #[test]
    fn populate_collision_entity_sparse_then_sparse_leaves_absent_fields_nil() {
        let lua = mlua::Lua::new();
        let t = make_collision_entity_tables(&lua);
        let occupancy = CtxOccupancy::default();

        for _ in 0..2 {
            populate_collision_entity(
                &t.entity,
                &t.pos,
                &t.vel,
                &t.rect,
                &t.signals,
                &t.signals_inner,
                &occupancy,
                1,
                None,
                0.0,
                Some((1.0, 2.0)),
                None,
                None,
                None,
            )
            .unwrap();
            assert_sparse_collision_ctx_shape(&t.entity);
        }
    }

    #[test]
    fn populate_collision_entity_sparse_then_full_reveals_fields() {
        let lua = mlua::Lua::new();
        let t = make_collision_entity_tables(&lua);
        let occupancy = CtxOccupancy::default();
        let signals = Signals::default();

        populate_collision_entity(
            &t.entity,
            &t.pos,
            &t.vel,
            &t.rect,
            &t.signals,
            &t.signals_inner,
            &occupancy,
            1,
            None,
            0.0,
            Some((1.0, 2.0)),
            None,
            None,
            None,
        )
        .unwrap();

        populate_collision_entity(
            &t.entity,
            &t.pos,
            &t.vel,
            &t.rect,
            &t.signals,
            &t.signals_inner,
            &occupancy,
            1,
            Some("enemy"),
            0.0,
            Some((1.0, 2.0)),
            Some((3.0, 4.0)),
            Some((0.0, 0.0, 10.0, 10.0)),
            Some(&signals),
        )
        .unwrap();
        assert_full_collision_ctx_shape(&t.entity);
    }

    #[test]
    fn populate_collision_entity_full_then_sparse_scrubs_fields() {
        let lua = mlua::Lua::new();
        let t = make_collision_entity_tables(&lua);
        let occupancy = CtxOccupancy::default();
        let signals = Signals::default();

        populate_collision_entity(
            &t.entity,
            &t.pos,
            &t.vel,
            &t.rect,
            &t.signals,
            &t.signals_inner,
            &occupancy,
            1,
            Some("enemy"),
            0.0,
            Some((1.0, 2.0)),
            Some((3.0, 4.0)),
            Some((0.0, 0.0, 10.0, 10.0)),
            Some(&signals),
        )
        .unwrap();

        populate_collision_entity(
            &t.entity,
            &t.pos,
            &t.vel,
            &t.rect,
            &t.signals,
            &t.signals_inner,
            &occupancy,
            1,
            None,
            0.0,
            Some((1.0, 2.0)),
            None,
            None,
            None,
        )
        .unwrap();
        assert_sparse_collision_ctx_shape(&t.entity);
    }
}
