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
//! Two context builders are provided:
//! - [`build_entity_context`] - Creates fresh tables (legacy, higher allocation overhead)
//! - [`build_entity_context_pooled`] - Uses pre-allocated tables from [`EntityCtxPool`](super::runtime::EntityCtxTables)
//!
//! The pooled version is preferred for hot paths (phase updates, timer callbacks) to
//! reduce Lua GC pressure. Only signal inner maps (flags, integers, scalars, strings)
//! are created fresh since they have variable keys per entity.
//!
//! **Important**: Pooled context tables are reused. Lua scripts must not store
//! references to `ctx` or its subtables for later use.

use super::runtime::EntityCtxTables;
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

/// Snapshot of LuaTimer data for context building.
#[derive(Debug)]
pub struct LuaTimerSnapshot<'a> {
    pub duration: f32,
    pub elapsed: f32,
    pub callback: &'a str,
}

/// Build a Lua table representing entity context for phase/timer callbacks.
///
/// This function creates a context table with all available entity information.
/// Fields are only set if the corresponding data is provided (Some). Missing
/// components result in nil fields in Lua.
///
/// ** REPLACED BY POOLED VERSION **
/// See: [`build_entity_context_pooled`] for a more efficient implementation that
/// uses pre-allocated tables to reduce allocations and GC pressure.
///
/// # Arguments
///
/// * `lua` - Reference to the Lua state
/// * `entity_id` - Entity ID as u64 (always present)
/// * `group` - Entity group name (optional)
/// * `map_pos` - World position from MapPosition (optional)
/// * `screen_pos` - Screen position from ScreenPosition (optional)
/// * `rigid_body` - Physics snapshot from RigidBody (optional)
/// * `rotation` - Rotation in degrees from Rotation component (optional)
/// * `scale` - Scale factors from Scale component (optional)
/// * `rect` - Collision rectangle as (x, y, w, h) from BoxCollider (optional)
/// * `sprite` - Sprite snapshot (optional)
/// * `animation` - Animation snapshot (optional)
/// * `signals` - Entity signals reference (optional)
/// * `lua_phase` - Phase snapshot (optional)
/// * `lua_timer` - Timer snapshot (optional)
/// * `previous_phase` - Previous phase name for on_enter callbacks (optional)
///
/// # Returns
///
/// A Lua table containing the entity context.
/* #[allow(clippy::too_many_arguments)]
pub fn build_entity_context<'a>(
    lua: &Lua,
    entity_id: u64,
    group: Option<&'a str>,
    map_pos: Option<(f32, f32)>,
    screen_pos: Option<(f32, f32)>,
    rigid_body: Option<&'a RigidBodySnapshot>,
    rotation: Option<f32>,
    scale: Option<(f32, f32)>,
    rect: Option<(f32, f32, f32, f32)>,
    sprite: Option<&'a SpriteSnapshot>,
    animation: Option<&'a AnimationSnapshot>,
    signals: Option<&'a Signals>,
    lua_phase: Option<&'a LuaPhaseSnapshot>,
    lua_timer: Option<&'a LuaTimerSnapshot>,
    previous_phase: Option<&'a str>,
) -> LuaResult<LuaTable> {
    let ctx = lua.create_table()?;

    // Core identity (id is always present)
    ctx.set("id", entity_id)?;

    // Group (optional)
    if let Some(g) = group {
        ctx.set("group", g)?;
    }

    // Position - MapPosition
    if let Some((x, y)) = map_pos {
        let pos_table = lua.create_table()?;
        pos_table.set("x", x)?;
        pos_table.set("y", y)?;
        ctx.set("pos", pos_table)?;
    }

    // Position - ScreenPosition
    if let Some((x, y)) = screen_pos {
        let pos_table = lua.create_table()?;
        pos_table.set("x", x)?;
        pos_table.set("y", y)?;
        ctx.set("screen_pos", pos_table)?;
    }

    // Physics from RigidBody
    if let Some(rb) = rigid_body {
        let vel_table = lua.create_table()?;
        vel_table.set("x", rb.velocity.0)?;
        vel_table.set("y", rb.velocity.1)?;
        ctx.set("vel", vel_table)?;
        ctx.set("speed_sq", rb.speed_sq)?;
        ctx.set("frozen", rb.frozen)?;
    }

    // Transform - Rotation
    if let Some(degrees) = rotation {
        ctx.set("rotation", degrees)?;
    }

    // Transform - Scale
    if let Some((sx, sy)) = scale {
        let scale_table = lua.create_table()?;
        scale_table.set("x", sx)?;
        scale_table.set("y", sy)?;
        ctx.set("scale", scale_table)?;
    }

    // Collision rect from BoxCollider
    if let Some((x, y, w, h)) = rect {
        let rect_table = lua.create_table()?;
        rect_table.set("x", x)?;
        rect_table.set("y", y)?;
        rect_table.set("w", w)?;
        rect_table.set("h", h)?;
        ctx.set("rect", rect_table)?;
    }

    // Sprite
    if let Some(spr) = sprite {
        let sprite_table = lua.create_table()?;
        sprite_table.set("tex_key", spr.tex_key.as_str())?;
        sprite_table.set("flip_h", spr.flip_h)?;
        sprite_table.set("flip_v", spr.flip_v)?;
        ctx.set("sprite", sprite_table)?;
    }

    // Animation
    if let Some(anim) = animation {
        let anim_table = lua.create_table()?;
        anim_table.set("key", anim.key.as_str())?;
        anim_table.set("frame_index", anim.frame_index)?;
        anim_table.set("elapsed", anim.elapsed)?;
        ctx.set("animation", anim_table)?;
    }

    // Signals (flags as 1-indexed array, others as key-value maps)
    if let Some(signals) = signals {
        let sig_table = lua.create_table()?;

        // Flags as 1-indexed array
        let flags_table = lua.create_table()?;
        for (i, flag) in signals.get_flags().iter().enumerate() {
            flags_table.set(i + 1, flag.as_str())?;
        }
        sig_table.set("flags", flags_table)?;

        // Integers as key-value map
        let integers_table = lua.create_table()?;
        for (key, value) in signals.get_integers() {
            integers_table.set(key.as_str(), *value)?;
        }
        sig_table.set("integers", integers_table)?;

        // Scalars as key-value map
        let scalars_table = lua.create_table()?;
        for (key, value) in signals.get_scalars() {
            scalars_table.set(key.as_str(), *value)?;
        }
        sig_table.set("scalars", scalars_table)?;

        // Strings as key-value map
        let strings_table = lua.create_table()?;
        for (key, value) in signals.strings.iter() {
            strings_table.set(key.as_str(), value.as_str())?;
        }
        sig_table.set("strings", strings_table)?;

        ctx.set("signals", sig_table)?;
    }

    // Phase info from LuaPhase
    if let Some(phase) = lua_phase {
        ctx.set("phase", phase.current.as_str())?;
        ctx.set("time_in_phase", phase.time_in_phase)?;
    }

    // Previous phase (only set during on_enter)
    if let Some(prev) = previous_phase {
        ctx.set("previous_phase", prev)?;
    }

    // Timer info from LuaTimer
    if let Some(timer) = lua_timer {
        let timer_table = lua.create_table()?;
        timer_table.set("duration", timer.duration)?;
        timer_table.set("elapsed", timer.elapsed)?;
        timer_table.set("callback", timer.callback.as_str())?;
        ctx.set("timer", timer_table)?;
    }

    Ok(ctx)
}
 */
/// Populate entity signal tables (creates fresh tables for variable-length data).
fn populate_entity_signals(
    lua: &Lua,
    signals_table: &LuaTable,
    signals: &Signals,
) -> LuaResult<()> {
    // Create fresh flags array (variable length)
    let flags_table = lua.create_table()?;
    for (i, flag) in signals.get_flags().iter().enumerate() {
        flags_table.set(i + 1, flag.as_str())?;
    }
    signals_table.set("flags", flags_table)?;

    // Create fresh integers map (variable keys)
    let integers_table = lua.create_table()?;
    for (key, value) in signals.get_integers() {
        integers_table.set(key.as_str(), *value)?;
    }
    signals_table.set("integers", integers_table)?;

    // Create fresh scalars map (variable keys)
    let scalars_table = lua.create_table()?;
    for (key, value) in signals.get_scalars() {
        scalars_table.set(key.as_str(), *value)?;
    }
    signals_table.set("scalars", scalars_table)?;

    // Create fresh strings map (variable keys)
    let strings_table = lua.create_table()?;
    for (key, value) in &signals.strings {
        strings_table.set(key.as_str(), value.as_str())?;
    }
    signals_table.set("strings", strings_table)?;

    Ok(())
}

/// Build entity context using pooled tables.
/// Uses pre-allocated tables for fixed-structure data to reduce allocations.
/// Only signal data maps (flags, integers, scalars, strings) are created fresh.
///
/// IMPORTANT: The returned table is pooled and reused. Do not store references
/// to the context table or its subtables for later use.
#[allow(clippy::too_many_arguments)]
pub fn build_entity_context_pooled<'a>(
    lua: &Lua,
    tables: &EntityCtxTables,
    entity_id: u64,
    group: Option<&'a str>,
    map_pos: Option<(f32, f32)>,
    screen_pos: Option<(f32, f32)>,
    rigid_body: Option<&'a RigidBodySnapshot>,
    rotation: Option<f32>,
    scale: Option<(f32, f32)>,
    rect: Option<(f32, f32, f32, f32)>,
    sprite: Option<&'a SpriteSnapshot<'a>>,
    animation: Option<&'a AnimationSnapshot<'a>>,
    signals: Option<&'a Signals>,
    lua_phase: Option<&'a LuaPhaseSnapshot<'a>>,
    lua_timer: Option<&'a LuaTimerSnapshot<'a>>,
    previous_phase: Option<&'a str>,
) -> LuaResult<LuaTable> {
    // Core identity (id is always present)
    tables.ctx.set("id", entity_id)?;

    // Group (optional scalar)
    if let Some(g) = group {
        tables.ctx.set("group", g)?;
    } else {
        tables.ctx.set("group", LuaValue::Nil)?;
    }

    // Position - MapPosition
    if let Some((x, y)) = map_pos {
        tables.pos.set("x", x)?;
        tables.pos.set("y", y)?;
        tables.ctx.set("pos", tables.pos.clone())?;
    } else {
        tables.ctx.set("pos", LuaValue::Nil)?;
    }

    // Position - ScreenPosition
    if let Some((x, y)) = screen_pos {
        tables.screen_pos.set("x", x)?;
        tables.screen_pos.set("y", y)?;
        tables.ctx.set("screen_pos", tables.screen_pos.clone())?;
    } else {
        tables.ctx.set("screen_pos", LuaValue::Nil)?;
    }

    // Physics from RigidBody
    if let Some(rb) = rigid_body {
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

    // Transform - Rotation
    if let Some(degrees) = rotation {
        tables.ctx.set("rotation", degrees)?;
    } else {
        tables.ctx.set("rotation", LuaValue::Nil)?;
    }

    // Transform - Scale
    if let Some((sx, sy)) = scale {
        tables.scale.set("x", sx)?;
        tables.scale.set("y", sy)?;
        tables.ctx.set("scale", tables.scale.clone())?;
    } else {
        tables.ctx.set("scale", LuaValue::Nil)?;
    }

    // Collision rect from BoxCollider
    if let Some((x, y, w, h)) = rect {
        tables.rect.set("x", x)?;
        tables.rect.set("y", y)?;
        tables.rect.set("w", w)?;
        tables.rect.set("h", h)?;
        tables.ctx.set("rect", tables.rect.clone())?;
    } else {
        tables.ctx.set("rect", LuaValue::Nil)?;
    }

    // Sprite
    if let Some(spr) = sprite {
        tables.sprite.set("tex_key", spr.tex_key)?;
        tables.sprite.set("flip_h", spr.flip_h)?;
        tables.sprite.set("flip_v", spr.flip_v)?;
        tables.ctx.set("sprite", tables.sprite.clone())?;
    } else {
        tables.ctx.set("sprite", LuaValue::Nil)?;
    }

    // Animation
    if let Some(anim) = animation {
        tables.animation.set("key", anim.key)?;
        tables.animation.set("frame_index", anim.frame_index)?;
        tables.animation.set("elapsed", anim.elapsed)?;
        tables.ctx.set("animation", tables.animation.clone())?;
    } else {
        tables.ctx.set("animation", LuaValue::Nil)?;
    }

    // Signals (creates fresh inner tables for variable-length data)
    if let Some(signals) = signals {
        populate_entity_signals(lua, &tables.signals, signals)?;
        tables.ctx.set("signals", tables.signals.clone())?;
    } else {
        tables.ctx.set("signals", LuaValue::Nil)?;
    }

    // Phase info from LuaPhase
    if let Some(phase) = lua_phase {
        tables.ctx.set("phase", phase.current)?;
        tables.ctx.set("time_in_phase", phase.time_in_phase)?;
    } else {
        tables.ctx.set("phase", LuaValue::Nil)?;
        tables.ctx.set("time_in_phase", LuaValue::Nil)?;
    }

    // Previous phase (only set during on_enter)
    if let Some(prev) = previous_phase {
        tables.ctx.set("previous_phase", prev)?;
    } else {
        tables.ctx.set("previous_phase", LuaValue::Nil)?;
    }

    // Timer info from LuaTimer
    if let Some(timer) = lua_timer {
        tables.timer.set("duration", timer.duration)?;
        tables.timer.set("elapsed", timer.elapsed)?;
        tables.timer.set("callback", timer.callback)?;
        tables.ctx.set("timer", tables.timer.clone())?;
    } else {
        tables.ctx.set("timer", LuaValue::Nil)?;
    }

    Ok(tables.ctx.clone())
}
