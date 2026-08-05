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
//! references to `ctx` or its subtables for later use, and must not *write* to
//! `ctx` or its subtables either — `build_entity_context_pooled` tracks which
//! optional fields are currently non-nil via an occupancy bitmask
//! (`EntityCtxTables::occupancy`) to skip redundant Nil writes; a script write
//! desyncs that mask, and the stale value can leak into a later callback's ctx.

use super::runtime::{EntityCtxTables, SignalsCtxTables};
use aberred_core::components::signals::Signals;
use mlua::{Lua, Result as LuaResult, Table as LuaTable};

/// Accumulates one `build_entity_context_pooled`/`populate_collision_entity` call's
/// occupancy mask transition: `old` is read once from `CtxOccupancy` at the top of the
/// call, `new` is OR'd into as each `set_opt!` site finds its field present, and the
/// final `new` is written back to `CtxOccupancy` at the end.
pub(crate) struct OccMask {
    pub old: u32,
    pub new: u32,
}

impl OccMask {
    pub(crate) fn new(old: u32) -> Self {
        Self { old, new: 0 }
    }
}

/// Bit assignments for `CtxOccupancy`'s entity-side mask (see `EntityCtxTables::occupancy`).
/// Each bit tracks one independently-set optional ctx key or key-group; unset bits mean
/// the ctx table already reads nil for those keys, so a rebuild can skip re-nil-ing them.
const BIT_GROUP: u32 = 1 << 0;
const BIT_ROTATION: u32 = 1 << 1;
const BIT_PREVIOUS_PHASE: u32 = 1 << 2;
const BIT_WORLD_ROTATION: u32 = 1 << 3;
const BIT_PARENT_ID: u32 = 1 << 4;
const BIT_POS: u32 = 1 << 5;
const BIT_SCREEN_POS: u32 = 1 << 6;
const BIT_SCALE: u32 = 1 << 7;
const BIT_WORLD_POS: u32 = 1 << 8;
const BIT_WORLD_SCALE: u32 = 1 << 9;
/// vel + speed_sq + frozen, set/cleared together — one bit.
const BIT_PHYSICS: u32 = 1 << 10;
const BIT_RECT: u32 = 1 << 11;
const BIT_SPRITE: u32 = 1 << 12;
const BIT_ANIMATION: u32 = 1 << 13;
const BIT_SIGNALS: u32 = 1 << 14;
/// phase + time_in_phase, set/cleared together — one bit.
const BIT_PHASE: u32 = 1 << 15;
const BIT_TIMER: u32 = 1 << 16;

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

/// Expand an `Option` into a Lua context field, setting `LuaValue::Nil` only when the
/// field was previously non-nil (tracked via an occupancy bitmask) — see `CtxOccupancy`.
///
/// Three forms:
/// - `set_opt!(ctx, "key", opt, bit, mask)` — scalar: sets `opt`'s inner value or
///   scrubs to Nil directly on ctx.
/// - `set_opt!(ctx, "key", opt, pat, bit, mask, { body })` — block: runs `body`
///   (responsible for setting ctx["key"] via a subtable) or scrubs to Nil. The key is
///   only used for the Nil branch.
/// - `set_opt!(ctx, ["key1", "key2", ...], opt, pat, bit, mask, { body })` — block
///   variant for a group that spans multiple ctx keys (e.g. physics' `vel`/`speed_sq`/
///   `frozen`): scrubs every listed key to Nil in the absent branch.
///
/// `mask: &mut OccMask` carries `old` (the occupancy read once before any fields are
/// built this call) and `new` (accumulates the bit for every field found present);
/// `new` is written back to the pool's `CtxOccupancy` once at the end.
///
/// Writes via `raw_set`, not `set` — safe only because these pooled ctx tables never
/// have a metatable attached anywhere in the codebase. If that ever changes (e.g. a
/// `__newindex` guard added to enforce the "never write to ctx" contract), `raw_set`
/// would silently bypass it; switch back to `set` here (and at the other pooled-table
/// call sites converted alongside this macro) if that's ever needed.
macro_rules! set_opt {
    ($ctx:expr, $key:literal, $val:expr, $bit:expr, $mask:expr) => {
        set_opt!($ctx, $key, $val, v, $bit, $mask, {
            $ctx.raw_set($key, v)?;
        });
    };
    ($ctx:expr, $key:literal, $val:expr, $v:pat, $bit:expr, $mask:expr, $body:block) => {
        if let Some($v) = $val {
            $body
            $mask.new |= $bit;
        } else if $mask.old & $bit != 0 {
            $ctx.raw_set($key, mlua::Value::Nil)?;
        }
    };
    ($ctx:expr, [$($key:literal),+ $(,)?], $val:expr, $v:pat, $bit:expr, $mask:expr, $body:block) => {
        if let Some($v) = $val {
            $body
            $mask.new |= $bit;
        } else if $mask.old & $bit != 0 {
            $( $ctx.raw_set($key, mlua::Value::Nil)?; )+
        }
    };
}
pub(crate) use set_opt;

/// Clears every entry (array and hash part) from a pooled Lua table via
/// `mlua::Table::clear()` (requires mlua >= 0.12 — see
/// `populate_entity_signals_does_not_leak_lua_stack_slots` below).
pub(crate) fn clear_table(table: &LuaTable) -> LuaResult<()> {
    table.clear()
}

/// Populate entity signal tables, reusing the pooled inner tables in place.
pub(crate) fn populate_entity_signals(
    inner: &SignalsCtxTables,
    signals: &Signals,
) -> LuaResult<()> {
    // Flags array (variable length)
    clear_table(&inner.flags)?;
    for (i, flag) in signals.get_flags().iter().enumerate() {
        inner.flags.raw_set(i + 1, flag.as_str())?;
    }

    // Integers map (variable keys)
    clear_table(&inner.integers)?;
    for (key, value) in signals.get_integers() {
        inner.integers.raw_set(key.as_str(), *value)?;
    }

    // Scalars map (variable keys)
    clear_table(&inner.scalars)?;
    for (key, value) in signals.get_scalars() {
        inner.scalars.raw_set(key.as_str(), *value)?;
    }

    // Strings map (variable keys)
    clear_table(&inner.strings)?;
    for (key, value) in signals.get_strings() {
        inner.strings.raw_set(key.as_str(), value.as_str())?;
    }

    Ok(())
}

/// Builds an entity ctx snapshot table by reusing pooled tables/subtables —
/// callers must not retain references past the callback (see `EntityCtxTables`).
pub fn build_entity_context_pooled<'a>(
    _lua: &Lua,
    tables: &EntityCtxTables,
    snapshot: &EntitySnapshot<'a>,
) -> LuaResult<LuaTable> {
    // Core identity (id is always present)
    tables.ctx.raw_set("id", snapshot.entity_id)?;

    let mut mask = OccMask::new(tables.occupancy.get());

    // Scalar optionals
    set_opt!(tables.ctx, "group", snapshot.group, BIT_GROUP, mask);
    set_opt!(tables.ctx, "rotation", snapshot.rotation, BIT_ROTATION, mask);
    set_opt!(
        tables.ctx,
        "previous_phase",
        snapshot.previous_phase,
        BIT_PREVIOUS_PHASE,
        mask
    );
    set_opt!(
        tables.ctx,
        "world_rotation",
        snapshot.world_rotation,
        BIT_WORLD_ROTATION,
        mask
    );
    set_opt!(
        tables.ctx,
        "parent_id",
        snapshot.parent_id,
        BIT_PARENT_ID,
        mask
    );

    // XY position subtables
    set_opt!(tables.ctx, "pos", snapshot.map_pos, (x, y), BIT_POS, mask, {
        tables.pos.raw_set("x", x)?;
        tables.pos.raw_set("y", y)?;
        tables.ctx.raw_set("pos", tables.pos.clone())?;
    });
    set_opt!(
        tables.ctx,
        "screen_pos",
        snapshot.screen_pos,
        (x, y),
        BIT_SCREEN_POS,
        mask,
        {
            tables.screen_pos.raw_set("x", x)?;
            tables.screen_pos.raw_set("y", y)?;
            tables
                .ctx
                .raw_set("screen_pos", tables.screen_pos.clone())?;
        }
    );
    set_opt!(
        tables.ctx,
        "scale",
        snapshot.scale,
        (sx, sy),
        BIT_SCALE,
        mask,
        {
            tables.scale.raw_set("x", sx)?;
            tables.scale.raw_set("y", sy)?;
            tables.ctx.raw_set("scale", tables.scale.clone())?;
        }
    );
    set_opt!(
        tables.ctx,
        "world_pos",
        snapshot.world_pos,
        (x, y),
        BIT_WORLD_POS,
        mask,
        {
            tables.world_pos.raw_set("x", x)?;
            tables.world_pos.raw_set("y", y)?;
            tables.ctx.raw_set("world_pos", tables.world_pos.clone())?;
        }
    );
    set_opt!(
        tables.ctx,
        "world_scale",
        snapshot.world_scale,
        (sx, sy),
        BIT_WORLD_SCALE,
        mask,
        {
            tables.world_scale.raw_set("x", sx)?;
            tables.world_scale.raw_set("y", sy)?;
            tables
                .ctx
                .raw_set("world_scale", tables.world_scale.clone())?;
        }
    );

    // Physics from RigidBody (sets three ctx keys)
    set_opt!(
        tables.ctx,
        ["vel", "speed_sq", "frozen"],
        snapshot.rigid_body.as_ref(),
        rb,
        BIT_PHYSICS,
        mask,
        {
            tables.vel.raw_set("x", rb.velocity.0)?;
            tables.vel.raw_set("y", rb.velocity.1)?;
            tables.ctx.raw_set("vel", tables.vel.clone())?;
            tables.ctx.raw_set("speed_sq", rb.speed_sq)?;
            tables.ctx.raw_set("frozen", rb.frozen)?;
        }
    );

    // Collision rect from BoxCollider
    set_opt!(
        tables.ctx,
        "rect",
        snapshot.rect,
        (x, y, w, h),
        BIT_RECT,
        mask,
        {
            tables.rect.raw_set("x", x)?;
            tables.rect.raw_set("y", y)?;
            tables.rect.raw_set("w", w)?;
            tables.rect.raw_set("h", h)?;
            tables.ctx.raw_set("rect", tables.rect.clone())?;
        }
    );

    // Sprite
    set_opt!(
        tables.ctx,
        "sprite",
        snapshot.sprite.as_ref(),
        spr,
        BIT_SPRITE,
        mask,
        {
            tables.sprite.raw_set("tex_key", spr.tex_key)?;
            tables.sprite.raw_set("flip_h", spr.flip_h)?;
            tables.sprite.raw_set("flip_v", spr.flip_v)?;
            tables.ctx.raw_set("sprite", tables.sprite.clone())?;
        }
    );

    // Animation
    set_opt!(
        tables.ctx,
        "animation",
        snapshot.animation.as_ref(),
        anim,
        BIT_ANIMATION,
        mask,
        {
            tables.animation.raw_set("key", anim.key)?;
            tables.animation.raw_set("frame_index", anim.frame_index)?;
            tables.animation.raw_set("elapsed", anim.elapsed)?;
            tables.ctx.raw_set("animation", tables.animation.clone())?;
        }
    );

    // Signals (creates fresh inner tables for variable-length data)
    set_opt!(
        tables.ctx,
        "signals",
        snapshot.signals,
        signals,
        BIT_SIGNALS,
        mask,
        {
            populate_entity_signals(&tables.signals_inner, signals)?;
            tables.ctx.raw_set("signals", tables.signals.clone())?;
        }
    );

    // Phase info from LuaPhase (sets two ctx keys)
    set_opt!(
        tables.ctx,
        ["phase", "time_in_phase"],
        snapshot.lua_phase.as_ref(),
        phase,
        BIT_PHASE,
        mask,
        {
            tables.ctx.raw_set("phase", phase.current)?;
            tables.ctx.raw_set("time_in_phase", phase.time_in_phase)?;
        }
    );

    // Timer info from LuaTimer
    set_opt!(
        tables.ctx,
        "timer",
        snapshot.lua_timer.as_ref(),
        timer,
        BIT_TIMER,
        mask,
        {
            tables.timer.raw_set("duration", timer.duration)?;
            tables.timer.raw_set("elapsed", timer.elapsed)?;
            tables.timer.raw_set("callback", timer.callback)?;
            tables.ctx.raw_set("timer", tables.timer.clone())?;
        }
    );

    tables.occupancy.set(mask.new);

    Ok(tables.ctx.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Value as LuaValue;

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
        signals_table.set("flags", inner.flags.clone()).unwrap();
        signals_table.set("integers", inner.integers.clone()).unwrap();
        signals_table.set("scalars", inner.scalars.clone()).unwrap();
        signals_table.set("strings", inner.strings.clone()).unwrap();

        let mut first = Signals::default();
        first.set_flag("active");
        first.set_integer("score", 7);
        first.set_scalar("speed", 2.5);
        first.set_string("state", "running");
        populate_entity_signals(&inner, &first).unwrap();

        let mut second = Signals::default();
        second.set_flag("paused");
        second.set_scalar("momentum", 1.25);
        populate_entity_signals(&inner, &second).unwrap();

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
        // Forward-looking regression guard: mlua 0.11.6's `Table::clear()`
        // (non-Luau) pushed the table ref via `push_ref` and never popped
        // it, leaking one main stack slot per call. mlua >= 0.12 fixes this
        // with a `StackGuard`, and populate_entity_signals now relies on
        // `Table::clear()` directly — if a future mlua version ever
        // regresses, this test overflows the Lua stack (`StackError`)
        // within a few thousand iterations.
        let lua = Lua::new();
        let signals_table = lua.create_table().unwrap();
        let inner = SignalsCtxTables {
            flags: lua.create_table().unwrap(),
            integers: lua.create_table().unwrap(),
            scalars: lua.create_table().unwrap(),
            strings: lua.create_table().unwrap(),
        };
        signals_table.set("flags", inner.flags.clone()).unwrap();
        signals_table.set("integers", inner.integers.clone()).unwrap();
        signals_table.set("scalars", inner.scalars.clone()).unwrap();
        signals_table.set("strings", inner.strings.clone()).unwrap();

        let mut signals = Signals::default();
        signals.set_flag("active");
        signals.set_integer("score", 7);
        signals.set_scalar("speed", 2.5);
        signals.set_string("state", "running");

        for _ in 0..20_000 {
            populate_entity_signals(&inner, &signals).unwrap();
        }
    }

    fn sparse_snapshot(signals: &Signals) -> EntitySnapshot<'_> {
        EntitySnapshot {
            entity_id: 1,
            group: None,
            map_pos: Some((1.0, 2.0)),
            screen_pos: None,
            rigid_body: None,
            rotation: None,
            scale: None,
            rect: None,
            sprite: None,
            animation: None,
            signals: Some(signals),
            lua_phase: None,
            lua_timer: None,
            previous_phase: None,
            world_pos: None,
            world_rotation: None,
            world_scale: None,
            parent_id: None,
        }
    }

    fn full_snapshot(signals: &Signals) -> EntitySnapshot<'_> {
        EntitySnapshot {
            entity_id: 1,
            group: Some("enemies"),
            map_pos: Some((1.0, 2.0)),
            screen_pos: Some((3.0, 4.0)),
            rigid_body: Some(RigidBodySnapshot {
                velocity: (5.0, 6.0),
                speed_sq: 61.0,
                frozen: false,
            }),
            rotation: Some(45.0),
            scale: Some((1.5, 1.5)),
            rect: Some((0.0, 0.0, 10.0, 10.0)),
            sprite: Some(SpriteSnapshot {
                tex_key: "player",
                flip_h: false,
                flip_v: false,
            }),
            animation: Some(AnimationSnapshot {
                key: "walk",
                frame_index: 0,
                elapsed: 0.0,
            }),
            signals: Some(signals),
            lua_phase: Some(LuaPhaseSnapshot {
                current: "idle",
                time_in_phase: 1.0,
            }),
            lua_timer: Some(LuaTimerSnapshot {
                duration: 1.0,
                elapsed: 0.5,
                callback: "on_timer",
            }),
            previous_phase: Some("attack"),
            world_pos: Some((7.0, 8.0)),
            world_rotation: Some(90.0),
            world_scale: Some((2.0, 2.0)),
            parent_id: Some(42),
        }
    }

    /// The set of optional ctx keys `sparse_snapshot` leaves absent.
    const SPARSE_ABSENT_KEYS: &[&str] = &[
        "group",
        "rotation",
        "previous_phase",
        "world_rotation",
        "parent_id",
        "screen_pos",
        "scale",
        "world_pos",
        "world_scale",
        "vel",
        "speed_sq",
        "frozen",
        "rect",
        "sprite",
        "animation",
        "phase",
        "time_in_phase",
        "timer",
    ];

    fn assert_sparse_ctx_shape(ctx: &LuaTable) {
        assert_eq!(ctx.get::<u64>("id").unwrap(), 1);
        let pos: LuaTable = ctx.get("pos").unwrap();
        assert_eq!(pos.get::<f32>("x").unwrap(), 1.0);
        assert_eq!(pos.get::<f32>("y").unwrap(), 2.0);
        assert!(ctx.get::<LuaTable>("signals").is_ok());
        for key in SPARSE_ABSENT_KEYS {
            let value: LuaValue = ctx.get(*key).unwrap();
            assert!(matches!(value, LuaValue::Nil), "expected {key} to be nil");
        }
    }

    fn assert_full_ctx_shape(ctx: &LuaTable) {
        assert_eq!(ctx.get::<u64>("id").unwrap(), 1);
        assert_eq!(ctx.get::<String>("group").unwrap(), "enemies");
        assert_eq!(ctx.get::<f32>("rotation").unwrap(), 45.0);
        assert_eq!(ctx.get::<String>("previous_phase").unwrap(), "attack");
        assert_eq!(ctx.get::<f32>("world_rotation").unwrap(), 90.0);
        assert_eq!(ctx.get::<u64>("parent_id").unwrap(), 42);
        assert!(ctx.get::<LuaTable>("screen_pos").is_ok());
        assert!(ctx.get::<LuaTable>("scale").is_ok());
        assert!(ctx.get::<LuaTable>("world_pos").is_ok());
        assert!(ctx.get::<LuaTable>("world_scale").is_ok());
        assert!(ctx.get::<LuaTable>("vel").is_ok());
        assert_eq!(ctx.get::<f32>("speed_sq").unwrap(), 61.0);
        assert!(!ctx.get::<bool>("frozen").unwrap());
        assert!(ctx.get::<LuaTable>("rect").is_ok());
        assert!(ctx.get::<LuaTable>("sprite").is_ok());
        assert!(ctx.get::<LuaTable>("animation").is_ok());
        assert_eq!(ctx.get::<String>("phase").unwrap(), "idle");
        assert_eq!(ctx.get::<f32>("time_in_phase").unwrap(), 1.0);
        assert!(ctx.get::<LuaTable>("timer").is_ok());
    }

    #[test]
    fn build_entity_context_pooled_sparse_then_sparse_leaves_absent_fields_nil() {
        let runtime = super::super::runtime::LuaRuntime::new().unwrap();
        let tables = runtime.get_entity_ctx_pool();
        let signals = Signals::default();

        let ctx = build_entity_context_pooled(runtime.lua(), &tables, &sparse_snapshot(&signals))
            .unwrap();
        assert_sparse_ctx_shape(&ctx);

        // Build again with the same sparse snapshot — must still be correct
        // (exercises the skip-path: already-nil keys are not rewritten).
        let ctx = build_entity_context_pooled(runtime.lua(), &tables, &sparse_snapshot(&signals))
            .unwrap();
        assert_sparse_ctx_shape(&ctx);
    }

    #[test]
    fn build_entity_context_pooled_sparse_then_full_reveals_fields() {
        let runtime = super::super::runtime::LuaRuntime::new().unwrap();
        let tables = runtime.get_entity_ctx_pool();
        let signals = Signals::default();

        build_entity_context_pooled(runtime.lua(), &tables, &sparse_snapshot(&signals)).unwrap();

        let ctx =
            build_entity_context_pooled(runtime.lua(), &tables, &full_snapshot(&signals)).unwrap();
        assert_full_ctx_shape(&ctx);
    }

    #[test]
    fn build_entity_context_pooled_full_then_sparse_scrubs_fields() {
        let runtime = super::super::runtime::LuaRuntime::new().unwrap();
        let tables = runtime.get_entity_ctx_pool();
        let signals = Signals::default();

        build_entity_context_pooled(runtime.lua(), &tables, &full_snapshot(&signals)).unwrap();

        let ctx = build_entity_context_pooled(runtime.lua(), &tables, &sparse_snapshot(&signals))
            .unwrap();
        assert_sparse_ctx_shape(&ctx);
    }
}
