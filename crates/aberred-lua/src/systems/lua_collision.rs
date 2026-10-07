//! Lua collision observers and callback dispatch.
//!
//! A Lua collision rule is a core `CollisionRule` plus a [`LuaOnCollision`].
//! Core matches it, tracks its contacts and triggers its events; these
//! observers call the Lua functions the [`LuaOnCollision`] names:
//!
//! - [`lua_collision_enter_observer`] – on `CollisionStarted`, calls `enter`
//! - [`lua_collision_stay_observer`] – on `Collided`, calls `stay`
//! - [`lua_collision_exit_observer`] – on `CollisionEnded`, calls `exit`
//!
//! Each callback gets a fresh pooled ctx and its `engine.collision_*`
//! commands are applied right after it. Core triggers `CollisionStarted`
//! just before the pair's first `Collided`, so `enter` runs before that
//! tick's `stay`; `CollisionEnded` comes from `collision_ended_system`, after
//! detection.
//!
//! # Lua Collision Callbacks
//!
//! Lua collision rules are defined via `engine.spawn():with_lua_collision_rule()`,
//! with optional `:with_lua_collision_enter()`/`:with_lua_collision_exit()`.
//! The enter and every-tick callbacks receive a context table with entity
//! data for both colliders:
//!
//! ```lua
//! function on_player_enemy(ctx)
//!     -- ctx.a and ctx.b contain entity data
//!     -- ctx.sides.a and ctx.sides.b contain collision sides
//! end
//! ```
//!
//! The exit callback's context carries only `ctx.a.id`/`ctx.a.group` and
//! `ctx.b.id`/`ctx.b.group`; either entity may already be despawned.
//!
//! **Performance**: Context tables are pooled and reused between collisions to
//! reduce GC pressure. See `CollisionCtxTables`
//! in runtime.rs for implementation details.
//!
//! # Related
//!
//! - [`aberred_core::systems::collision_detector`] – pure Rust collision detection
//! - [`crate::components::lua_on_collision::LuaOnCollision`] – names a rule's Lua callbacks
//! - [`aberred_core::systems::collision_rule::collision_rule_observer`] – matches rules, triggers the events
//! - [`aberred_core::components::boxcollider::BoxCollider`] – axis-aligned collider

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::lua_on_collision::LuaOnCollision;
use crate::components::luaphase::LuaPhase;
use crate::resources::lua_runtime::{
    CtxOccupancy, LuaRuntime, OccMask, PhaseCmd, SignalsCtxTables, clear_table,
    populate_entity_signals, set_opt,
};
use crate::systems::lua_commands::{
    DrainScope, EffectCmdBufs, EntityCmdQueries, drain_and_process_effect_commands,
    process_phase_command,
};
use aberred_core::components::boxcollider::BoxCollider;
use aberred_core::components::collision::{BoxSide, CollisionRule};
use aberred_core::components::signals::Signals;
use aberred_core::events::collision::{Collided, CollisionEnded, CollisionStarted};
use aberred_core::math::Rect;
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_core::systems::collision::{compute_sides, resolve_collider_rect, resolve_world_pos};
use log::error;

/// What a Lua collision callback reads (its ctx) and what its
/// `engine.collision_*` commands write.
#[derive(SystemParam)]
struct LuaCollisionEffects<'w, 's> {
    commands: Commands<'w, 's>,
    luaphase_query: Query<'w, 's, (Entity, &'static mut LuaPhase)>,
    entity_cmds: EntityCmdQueries<'w, 's>,
    world_signals: ResMut<'w, WorldSignals>,
    audio_cmds: MessageWriter<'w, AudioCmd>,
    lua_runtime: NonSend<'w, LuaRuntime>,
    systems_store: Res<'w, SystemsStore>,
    animation_store: Res<'w, AnimationStore>,
}

/// One side of a pooled collision context. `Default` leaves everything but
/// `id` and `group` nil (the exit callback's ctx).
#[derive(Clone, Copy, Default)]
struct CollisionSide<'a> {
    id: u64,
    group: Option<&'a str>,
    pos: Option<(f32, f32)>,
    vel: Option<(f32, f32)>,
    speed_sq: f32,
    rect: Option<(f32, f32, f32, f32)>,
    sides: &'a [BoxSide],
    signals: Option<&'a Signals>,
}

impl LuaCollisionEffects<'_, '_> {
    /// The full ctx side of a live entity, touching the other side on `sides`.
    fn live_side<'a>(
        &'a self,
        entity: Entity,
        group: &'a str,
        rect: Option<Rect>,
        sides: &'a [BoxSide],
    ) -> CollisionSide<'a> {
        let pos = resolve_world_pos(
            &self.entity_cmds.positions.as_readonly(),
            &self.entity_cmds.global_transforms,
            entity,
        );
        let velocity = self
            .entity_cmds
            .rigid_bodies
            .get(entity)
            .ok()
            .map(|rb| rb.velocity);
        CollisionSide {
            id: entity.to_bits(),
            group: Some(group),
            pos: pos.map(|v| (v.x, v.y)),
            vel: velocity.map(|v| (v.x, v.y)),
            speed_sq: velocity.map_or(0.0, |v| v.length_squared()),
            rect: rect.map(|r| (r.x, r.y, r.width, r.height)),
            sides,
            signals: self.entity_cmds.signals.get(entity).ok(),
        }
    }

    /// Applies the `engine.collision_*` commands a callback just queued.
    fn drain_collision_commands(
        &mut self,
        phase_buf: &mut Vec<PhaseCmd>,
        effect_bufs: &mut EffectCmdBufs,
    ) {
        self.lua_runtime
            .drain_collision_phase_commands_into(phase_buf);
        for cmd in phase_buf.drain(..) {
            process_phase_command(&mut self.luaphase_query, cmd);
        }

        drain_and_process_effect_commands(
            &self.lua_runtime,
            DrainScope::Collision,
            effect_bufs,
            &mut self.commands,
            &mut self.world_signals,
            &mut self.entity_cmds,
            &mut self.audio_cmds,
            &self.systems_store,
            &self.animation_store,
        );
    }
}

/// What the Lua collision observers read and write: the Lua rules (core rule
/// entities that also carry a [`LuaOnCollision`]), their colliders, the
/// callback effects and this observer's command buffers.
#[derive(SystemParam)]
pub struct LuaRuleDispatch<'w, 's> {
    rules: Query<'w, 's, (&'static CollisionRule, &'static LuaOnCollision)>,
    box_colliders: Query<'w, 's, &'static BoxCollider>,
    effects: LuaCollisionEffects<'w, 's>,
    phase_buf: Local<'s, Vec<PhaseCmd>>,
    effect_bufs: Local<'s, EffectCmdBufs>,
}

impl LuaRuleDispatch<'_, '_> {
    /// Calls `pick`'s callback of the Lua rule `rule` (if it is one and has
    /// that callback) with the full ctx of the touching pair `a`/`b`, then
    /// applies the `engine.collision_*` commands it queued.
    ///
    /// Rects and sides are resolved here rather than taken from the event, so
    /// a stay callback sees what the same tick's enter callback moved.
    fn call_touching(
        &mut self,
        (rule, a, b): (Entity, Entity, Entity),
        pick: fn(&LuaOnCollision) -> Option<&str>,
    ) {
        let Ok((rule, callbacks)) = self.rules.get(rule) else {
            return;
        };
        let Some(name) = pick(callbacks) else {
            return;
        };

        let effects = &mut self.effects;
        effects.lua_runtime.sync_signals(&mut effects.world_signals);
        {
            let rect_of = |entity| {
                resolve_collider_rect(
                    &effects.entity_cmds.positions.as_readonly(),
                    &effects.entity_cmds.global_transforms,
                    &self.box_colliders,
                    entity,
                )
            };
            let (rect_a, rect_b) = (rect_of(a), rect_of(b));
            let (sides_a, sides_b) = compute_sides(rect_a, rect_b);
            call_lua_collision_callback(
                &effects.lua_runtime,
                name,
                effects.live_side(a, &rule.group_a, rect_a, &sides_a),
                effects.live_side(b, &rule.group_b, rect_b, &sides_b),
            );
        }
        effects.drain_collision_commands(&mut self.phase_buf, &mut self.effect_bufs);
    }

    /// Calls the `exit` callback of the Lua rule `rule` (if it is one and has
    /// one) for the pair `a`/`b`, with only each side's `id` and `group`, then
    /// applies the `engine.collision_*` commands it queued.
    fn call_exit(&mut self, CollisionEnded { rule, a, b }: CollisionEnded) {
        let Ok((rule, callbacks)) = self.rules.get(rule) else {
            return;
        };
        let Some(on_exit) = callbacks.exit.as_deref() else {
            return;
        };

        let effects = &mut self.effects;
        effects.lua_runtime.sync_signals(&mut effects.world_signals);
        call_lua_collision_callback(
            &effects.lua_runtime,
            on_exit,
            CollisionSide {
                id: a.to_bits(),
                group: Some(&rule.group_a),
                ..Default::default()
            },
            CollisionSide {
                id: b.to_bits(),
                group: Some(&rule.group_b),
                ..Default::default()
            },
        );
        effects.drain_collision_commands(&mut self.phase_buf, &mut self.effect_bufs);
    }
}

/// Observes core `CollisionStarted` and calls the Lua rule's `enter` callback.
///
/// Core triggers `CollisionStarted` just before the pair's first `Collided`,
/// so `enter` runs before that tick's `stay`.
pub fn lua_collision_enter_observer(trigger: On<CollisionStarted>, mut dispatch: LuaRuleDispatch) {
    let ev = trigger.event();
    dispatch.call_touching((ev.rule, ev.a, ev.b), |callbacks| {
        callbacks.enter.as_deref()
    });
}

/// Observes core `Collided` and calls the Lua rule's `stay` callback, every
/// tick the pair touches.
pub fn lua_collision_stay_observer(trigger: On<Collided>, mut dispatch: LuaRuleDispatch) {
    let ev = trigger.event();
    dispatch.call_touching((ev.rule, ev.a, ev.b), |callbacks| callbacks.stay.as_deref());
}

/// Observes core `CollisionEnded` and calls the Lua rule's `exit` callback.
///
/// Either entity may already be despawned, so the ctx carries only `id` and
/// `group` per side (from the rule's groups); `pos`, `vel`, `rect` and
/// `signals` are nil and the sides are empty. A rule entity that is gone, or
/// is not a Lua rule, is skipped.
pub fn lua_collision_exit_observer(trigger: On<CollisionEnded>, mut dispatch: LuaRuleDispatch) {
    dispatch.call_exit(*trigger.event());
}

/// Convert BoxSide to string representation.
fn box_side_to_str(side: &aberred_core::components::collision::BoxSide) -> &'static str {
    match side {
        aberred_core::components::collision::BoxSide::Left => "left",
        aberred_core::components::collision::BoxSide::Right => "right",
        aberred_core::components::collision::BoxSide::Top => "top",
        aberred_core::components::collision::BoxSide::Bottom => "bottom",
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
    side: &CollisionSide,
) -> mlua::Result<()> {
    let CollisionSide {
        id,
        group,
        pos,
        vel,
        speed_sq,
        rect,
        signals,
        ..
    } = *side;
    entity_table.raw_set("id", id)?;
    entity_table.raw_set("speed_sq", speed_sq)?;

    let mut mask = OccMask::new(occupancy.get());

    set_opt!(entity_table, "group", group, COLL_BIT_GROUP, mask);

    set_opt!(entity_table, "pos", pos, (x, y), COLL_BIT_POS, mask, {
        pos_table.raw_set("x", x)?;
        pos_table.raw_set("y", y)?;
        entity_table.raw_set("pos", pos_table.clone())?;
    });

    set_opt!(entity_table, "vel", vel, (vx, vy), COLL_BIT_VEL, mask, {
        vel_table.raw_set("x", vx)?;
        vel_table.raw_set("y", vy)?;
        entity_table.raw_set("vel", vel_table.clone())?;
    });

    set_opt!(
        entity_table,
        "rect",
        rect,
        (x, y, w, h),
        COLL_BIT_RECT,
        mask,
        {
            rect_table.raw_set("x", x)?;
            rect_table.raw_set("y", y)?;
            rect_table.raw_set("w", w)?;
            rect_table.raw_set("h", h)?;
            entity_table.raw_set("rect", rect_table.clone())?;
        }
    );

    set_opt!(
        entity_table,
        "signals",
        signals,
        s,
        COLL_BIT_SIGNALS,
        mask,
        {
            populate_entity_signals(signals_inner, s)?;
            entity_table.raw_set("signals", signals_table.clone())?;
        }
    );

    occupancy.set(mask.new);

    Ok(())
}

/// Fills the pooled collision context from `a` and `b` and calls the Lua
/// function `name` with it, logging any error.
fn call_lua_collision_callback(
    lua_runtime: &LuaRuntime,
    name: &str,
    a: CollisionSide,
    b: CollisionSide,
) {
    let tables = lua_runtime.get_collision_ctx_pool();
    let filled = (|| -> mlua::Result<()> {
        populate_collision_entity(
            &tables.entity_a,
            &tables.pos_a,
            &tables.vel_a,
            &tables.rect_a,
            &tables.signals_a,
            &tables.signals_a_inner,
            &tables.occupancy_a,
            &a,
        )?;
        populate_collision_entity(
            &tables.entity_b,
            &tables.pos_b,
            &tables.vel_b,
            &tables.rect_b,
            &tables.signals_b,
            &tables.signals_b_inner,
            &tables.occupancy_b,
            &b,
        )?;
        for (table, sides) in [(&tables.sides_a, a.sides), (&tables.sides_b, b.sides)] {
            clear_table(table)?;
            for (i, side) in sides.iter().enumerate() {
                table.raw_set(i + 1, box_side_to_str(side))?;
            }
        }
        Ok(())
    })();
    if let Err(e) = filled {
        error!(target: "lua", "Collision context for '{}' failed: {}", name, e);
        return;
    }

    lua_runtime.call_named(name, "Collision", |f| f.call::<()>(&tables.ctx));
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::components::collision::BoxSide;

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
        let t = make_collision_entity_tables(&lua);
        let occupancy = CtxOccupancy::default();

        populate_collision_entity(
            &t.entity,
            &t.pos,
            &t.vel,
            &t.rect,
            &t.signals,
            &t.signals_inner,
            &occupancy,
            &CollisionSide {
                id: 1,
                ..Default::default()
            },
        )
        .unwrap();

        let group: mlua::Value = t.entity.get("group").unwrap();
        assert!(matches!(group, mlua::Value::Nil));
    }

    #[test]
    fn populate_collision_entity_group_some_is_string() {
        let lua = mlua::Lua::new();
        let t = make_collision_entity_tables(&lua);
        let occupancy = CtxOccupancy::default();

        populate_collision_entity(
            &t.entity,
            &t.pos,
            &t.vel,
            &t.rect,
            &t.signals,
            &t.signals_inner,
            &occupancy,
            &CollisionSide {
                id: 1,
                group: Some("enemy"),
                ..Default::default()
            },
        )
        .unwrap();

        let group: String = t.entity.get("group").unwrap();
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
            },
        }
    }

    fn assert_sparse_collision_ctx_shape(entity_table: &mlua::Table) {
        let pos: mlua::Table = entity_table.get("pos").unwrap();
        assert_eq!(pos.get::<f32>("x").unwrap(), 1.0);
        assert_eq!(pos.get::<f32>("y").unwrap(), 2.0);
        for key in ["group", "vel", "rect", "signals"] {
            let value: mlua::Value = entity_table.get(key).unwrap();
            assert!(
                matches!(value, mlua::Value::Nil),
                "expected {key} to be nil"
            );
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
                &CollisionSide {
                    id: 1,
                    pos: Some((1.0, 2.0)),
                    ..Default::default()
                },
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
            &CollisionSide {
                id: 1,
                pos: Some((1.0, 2.0)),
                ..Default::default()
            },
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
            &CollisionSide {
                id: 1,
                group: Some("enemy"),
                pos: Some((1.0, 2.0)),
                vel: Some((3.0, 4.0)),
                rect: Some((0.0, 0.0, 10.0, 10.0)),
                signals: Some(&signals),
                ..Default::default()
            },
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
            &CollisionSide {
                id: 1,
                group: Some("enemy"),
                pos: Some((1.0, 2.0)),
                vel: Some((3.0, 4.0)),
                rect: Some((0.0, 0.0, 10.0, 10.0)),
                signals: Some(&signals),
                ..Default::default()
            },
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
            &CollisionSide {
                id: 1,
                pos: Some((1.0, 2.0)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_sparse_collision_ctx_shape(&t.entity);
    }
}
