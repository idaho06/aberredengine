//! Entity context builder for Lua callbacks.
//!
//! This module provides a unified way to build Lua context tables containing
//! entity state information. Used by LuaPhase and LuaTimer systems to pass
//! rich entity context to Lua callbacks.
//!
//! # Context Structure
//!
//! The context table contains:
//! - Core identity: `id`, `group`
//! - Position: `pos` (MapPosition) or `screen_pos` (ScreenPosition)
//! - Physics: `vel`, `speed_sq`, `frozen` (from RigidBody)
//! - Transform: `rotation`, `scale`
//! - Collision: `rect` (BoxCollider AABB)
//! - Sprite: `sprite` with `tex_key`, `flip_h`, `flip_v`
//! - Animation: `animation` with `key`, `frame_index`, `elapsed`
//! - Signals: `signals` with `flags`, `integers`, `scalars`, `strings`
//! - Phase: `phase`, `time_in_phase`, `previous_phase`
//! - Timer: `timer` with `duration`, `elapsed`, `callback`
//!
//! # Table Pooling
//!
//! [`build_entity_context_pooled`] uses pre-allocated tables from
//! [`EntityCtxTables`](super::runtime::EntityCtxTables) to reduce Lua GC pressure on hot
//! paths (phase updates, timer callbacks). Only signal inner maps are created fresh since
//! they have variable keys per entity.
//!
//! **Important**: Pooled context tables are reused. Lua scripts must not store
//! references to `ctx` or its subtables for later use.

use super::runtime::{EntityCtxTables, SignalsCtxTables};
use crate::components::signals::Signals;
use mlua::{Lua, Result as LuaResult, Table as LuaTable, Value as LuaValue};

/// Snapshot of RigidBody data for context building.
#[derive(Debug, Clone)]
pub struct RigidBodySnapshot {
    pub velocity: (f32, f32),
    pub speed_sq: f32,
    pub frozen: bool,
}

/// Snapshot of Sprite data for context building.
#[derive(Debug)]
pub struct SpriteSnapshot<'a> {
    pub tex_key: &'a str,
    pub flip_h: bool,
    pub flip_v: bool,
}

/// Snapshot of Animation data for context building.
#[derive(Debug)]
pub struct AnimationSnapshot<'a> {
    pub key: &'a str,
    pub frame_index: usize,
    pub elapsed: f32,
}

/// Snapshot of LuaPhase data for context building.
#[derive(Debug)]
pub struct LuaPhaseSnapshot<'a> {
    pub current: &'a str,
    pub time_in_phase: f32,
}

impl<'a> From<&'a crate::components::luaphase::LuaPhase> for LuaPhaseSnapshot<'a> {
    fn from(phase: &'a crate::components::luaphase::LuaPhase) -> Self {
        Self {
            current: phase.current.as_str(),
            time_in_phase: phase.time_in_phase,
        }
    }
}

/// Snapshot of LuaTimer data for context building.
#[derive(Debug)]
pub struct LuaTimerSnapshot<'a> {
    pub duration: f32,
    pub elapsed: f32,
    pub callback: &'a str,
}

/// Full entity snapshot used to build Lua callback context tables.
#[derive(Debug)]
pub struct EntitySnapshot<'a> {
    pub entity_id: u64,
    pub group: Option<&'a str>,
    pub map_pos: Option<(f32, f32)>,
    pub screen_pos: Option<(f32, f32)>,
    pub rigid_body: Option<RigidBodySnapshot>,
    pub rotation: Option<f32>,
    pub scale: Option<(f32, f32)>,
    pub rect: Option<(f32, f32, f32, f32)>,
    pub sprite: Option<SpriteSnapshot<'a>>,
    pub animation: Option<AnimationSnapshot<'a>>,
    pub signals: Option<&'a Signals>,
    pub lua_phase: Option<LuaPhaseSnapshot<'a>>,
    pub lua_timer: Option<LuaTimerSnapshot<'a>>,
    pub previous_phase: Option<&'a str>,
    pub world_pos: Option<(f32, f32)>,
    pub world_rotation: Option<f32>,
    pub world_scale: Option<(f32, f32)>,
    pub parent_id: Option<u64>,
}

/// Expand an `Option` into a Lua context field, setting `LuaValue::Nil` in the absent case.
///
/// Two forms:
/// - `set_opt!(ctx, "key", opt)` — scalar: sets `opt`'s inner value or Nil directly on ctx.
/// - `set_opt!(ctx, "key", opt, pat, { body })` — block: runs `body` (which is responsible for
///   setting ctx["key"] via a subtable) or sets Nil. The key is only used for the Nil branch.
macro_rules! set_opt {
    ($ctx:expr, $key:literal, $val:expr) => {
        if let Some(v) = $val {
            $ctx.set($key, v)?;
        } else {
            $ctx.set($key, LuaValue::Nil)?;
        }
    };
    ($ctx:expr, $key:literal, $val:expr, $v:pat, $body:block) => {
        if let Some($v) = $val {
            $body
        } else {
            $ctx.set($key, LuaValue::Nil)?;
        }
    };
}

/// Clears all numeric indices `1..=len` from an array-style Lua table.
fn clear_array_table(table: &LuaTable) -> LuaResult<()> {
    let len = table.raw_len();
    for i in 1..=len {
        table.raw_set(i, LuaValue::Nil)?;
    }
    Ok(())
}

/// Clears all entries from a hash-style (string/number keyed) Lua table.
///
/// Deliberately does NOT use `mlua::Table::clear()`: in mlua 0.11.6's
/// non-Luau implementation, `clear()` pushes the table onto the Lua stack via
/// `push_ref` but never pops it, leaking one stack slot per call — with this
/// called 3x per entity per callback, the main Lua stack overflows
/// (`StackError`) within seconds. Collecting keys via `pairs` and nil-ing
/// them through the safe `set` path does not leak.
fn clear_map_table(table: &LuaTable) -> LuaResult<()> {
    let keys: Vec<LuaValue> = table
        .pairs::<LuaValue, LuaValue>()
        .map(|pair| pair.map(|(k, _)| k))
        .collect::<LuaResult<_>>()?;
    for key in keys {
        table.set(key, LuaValue::Nil)?;
    }
    Ok(())
}

/// Populate entity signal tables, reusing the pooled inner tables in place.
pub(crate) fn populate_entity_signals(
    signals_table: &LuaTable,
    inner: &SignalsCtxTables,
    signals: &Signals,
) -> LuaResult<()> {
    // Flags array (variable length)
    clear_array_table(&inner.flags)?;
    for (i, flag) in signals.get_flags().iter().enumerate() {
        inner.flags.set(i + 1, flag.as_str())?;
    }
    signals_table.set("flags", inner.flags.clone())?;

    // Integers map (variable keys)
    clear_map_table(&inner.integers)?;
    for (key, value) in signals.get_integers() {
        inner.integers.set(key.as_str(), *value)?;
    }
    signals_table.set("integers", inner.integers.clone())?;

    // Scalars map (variable keys)
    clear_map_table(&inner.scalars)?;
    for (key, value) in signals.get_scalars() {
        inner.scalars.set(key.as_str(), *value)?;
    }
    signals_table.set("scalars", inner.scalars.clone())?;

    // Strings map (variable keys)
    clear_map_table(&inner.strings)?;
    for (key, value) in signals.get_strings() {
        inner.strings.set(key.as_str(), value.as_str())?;
    }
    signals_table.set("strings", inner.strings.clone())?;

    Ok(())
}

pub fn build_entity_context_pooled<'a>(
    _lua: &Lua,
    tables: &EntityCtxTables,
    snapshot: &EntitySnapshot<'a>,
) -> LuaResult<LuaTable> {
    // Core identity (id is always present)
    tables.ctx.set("id", snapshot.entity_id)?;

    // Scalar optionals
    set_opt!(tables.ctx, "group", snapshot.group);
    set_opt!(tables.ctx, "rotation", snapshot.rotation);
    set_opt!(tables.ctx, "previous_phase", snapshot.previous_phase);
    set_opt!(tables.ctx, "world_rotation", snapshot.world_rotation);
    set_opt!(tables.ctx, "parent_id", snapshot.parent_id);

    // XY position subtables
    set_opt!(tables.ctx, "pos", snapshot.map_pos, (x, y), {
        tables.pos.set("x", x)?;
        tables.pos.set("y", y)?;
        tables.ctx.set("pos", tables.pos.clone())?;
    });
    set_opt!(tables.ctx, "screen_pos", snapshot.screen_pos, (x, y), {
        tables.screen_pos.set("x", x)?;
        tables.screen_pos.set("y", y)?;
        tables.ctx.set("screen_pos", tables.screen_pos.clone())?;
    });
    set_opt!(tables.ctx, "scale", snapshot.scale, (sx, sy), {
        tables.scale.set("x", sx)?;
        tables.scale.set("y", sy)?;
        tables.ctx.set("scale", tables.scale.clone())?;
    });
    set_opt!(tables.ctx, "world_pos", snapshot.world_pos, (x, y), {
        tables.world_pos.set("x", x)?;
        tables.world_pos.set("y", y)?;
        tables.ctx.set("world_pos", tables.world_pos.clone())?;
    });
    set_opt!(tables.ctx, "world_scale", snapshot.world_scale, (sx, sy), {
        tables.world_scale.set("x", sx)?;
        tables.world_scale.set("y", sy)?;
        tables.ctx.set("world_scale", tables.world_scale.clone())?;
    });

    // Physics from RigidBody (sets three ctx keys — not a single-key pattern)
    if let Some(rb) = snapshot.rigid_body.as_ref() {
        tables.vel.set("x", rb.velocity.0)?;
        tables.vel.set("y", rb.velocity.1)?;
        tables.ctx.set("vel", tables.vel.clone())?;
        tables.ctx.set("speed_sq", rb.speed_sq)?;
        tables.ctx.set("frozen", rb.frozen)?;
    } else {
        tables.ctx.set("vel", LuaValue::Nil)?;
        tables.ctx.set("speed_sq", LuaValue::Nil)?;
        tables.ctx.set("frozen", LuaValue::Nil)?;
    }

    // Collision rect from BoxCollider
    set_opt!(tables.ctx, "rect", snapshot.rect, (x, y, w, h), {
        tables.rect.set("x", x)?;
        tables.rect.set("y", y)?;
        tables.rect.set("w", w)?;
        tables.rect.set("h", h)?;
        tables.ctx.set("rect", tables.rect.clone())?;
    });

    // Sprite
    set_opt!(tables.ctx, "sprite", snapshot.sprite.as_ref(), spr, {
        tables.sprite.set("tex_key", spr.tex_key)?;
        tables.sprite.set("flip_h", spr.flip_h)?;
        tables.sprite.set("flip_v", spr.flip_v)?;
        tables.ctx.set("sprite", tables.sprite.clone())?;
    });

    // Animation
    set_opt!(
        tables.ctx,
        "animation",
        snapshot.animation.as_ref(),
        anim,
        {
            tables.animation.set("key", anim.key)?;
            tables.animation.set("frame_index", anim.frame_index)?;
            tables.animation.set("elapsed", anim.elapsed)?;
            tables.ctx.set("animation", tables.animation.clone())?;
        }
    );

    // Signals (creates fresh inner tables for variable-length data)
    set_opt!(tables.ctx, "signals", snapshot.signals, signals, {
        populate_entity_signals(&tables.signals, &tables.signals_inner, signals)?;
        tables.ctx.set("signals", tables.signals.clone())?;
    });

    // Phase info from LuaPhase (sets two ctx keys — not a single-key pattern)
    if let Some(phase) = snapshot.lua_phase.as_ref() {
        tables.ctx.set("phase", phase.current)?;
        tables.ctx.set("time_in_phase", phase.time_in_phase)?;
    } else {
        tables.ctx.set("phase", LuaValue::Nil)?;
        tables.ctx.set("time_in_phase", LuaValue::Nil)?;
    }

    // Timer info from LuaTimer
    set_opt!(tables.ctx, "timer", snapshot.lua_timer.as_ref(), timer, {
        tables.timer.set("duration", timer.duration)?;
        tables.timer.set("elapsed", timer.elapsed)?;
        tables.timer.set("callback", timer.callback)?;
        tables.ctx.set("timer", tables.timer.clone())?;
    });

    Ok(tables.ctx.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn populate_entity_signals_replaces_variable_length_tables() {
        let lua = Lua::new();
        let signals_table = lua.create_table().unwrap();
        let inner = SignalsCtxTables {
            flags: lua.create_table().unwrap(),
            integers: lua.create_table().unwrap(),
            scalars: lua.create_table().unwrap(),
            strings: lua.create_table().unwrap(),
        };

        let mut first = Signals::default();
        first.set_flag("active");
        first.set_integer("score", 7);
        first.set_scalar("speed", 2.5);
        first.set_string("state", "running");
        populate_entity_signals(&signals_table, &inner, &first).unwrap();

        let mut second = Signals::default();
        second.set_flag("paused");
        second.set_scalar("momentum", 1.25);
        populate_entity_signals(&signals_table, &inner, &second).unwrap();

        let flags: LuaTable = signals_table.get("flags").unwrap();
        let integers: LuaTable = signals_table.get("integers").unwrap();
        let scalars: LuaTable = signals_table.get("scalars").unwrap();
        let strings: LuaTable = signals_table.get("strings").unwrap();

        assert_eq!(flags.get::<String>(1).unwrap(), "paused");
        assert!(flags.get::<Option<String>>(2).unwrap().is_none());
        assert!(integers.get::<Option<i32>>("score").unwrap().is_none());
        assert!(scalars.get::<Option<f32>>("speed").unwrap().is_none());
        assert_eq!(scalars.get::<f32>("momentum").unwrap(), 1.25);
        assert!(strings.get::<Option<String>>("state").unwrap().is_none());
    }

    #[test]
    fn populate_entity_signals_does_not_leak_lua_stack_slots() {
        // Regression test: mlua 0.11.6's `Table::clear()` (non-Luau) pushes
        // the table ref via `push_ref` and never pops it, leaking one main
        // stack slot per call. populate_entity_signals must not rely on
        // `Table::clear()` directly, or repeated calls overflow the Lua
        // stack (`StackError`) within a few thousand iterations.
        let lua = Lua::new();
        let signals_table = lua.create_table().unwrap();
        let inner = SignalsCtxTables {
            flags: lua.create_table().unwrap(),
            integers: lua.create_table().unwrap(),
            scalars: lua.create_table().unwrap(),
            strings: lua.create_table().unwrap(),
        };

        let mut signals = Signals::default();
        signals.set_flag("active");
        signals.set_integer("score", 7);
        signals.set_scalar("speed", 2.5);
        signals.set_string("state", "running");

        for _ in 0..20_000 {
            populate_entity_signals(&signals_table, &inner, &signals).unwrap();
        }
    }
}
