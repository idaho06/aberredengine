use bevy_ecs::prelude::*;
use mlua::prelude::{LuaResult, LuaTable};

use crate::resources::lua_runtime::{
    AnimationSnapshot, EntitySnapshot, LuaPhaseSnapshot, LuaRuntime, LuaTimerSnapshot,
    RigidBodySnapshot, SpriteSnapshot, build_entity_context_pooled,
};

use super::{ContextQueries, EntityCmdQueries};

/// Build entity context for Lua callbacks using pooled tables.
///
/// Gathers all component data for the given entity and builds a Lua context table.
/// The `lua_phase` and `previous_phase` arguments are caller-supplied since phase
/// context is obtained differently by the phase system (direct borrow) vs the
/// timer system (ECS query).
pub(crate) fn build_entity_context(
    lua_runtime: &LuaRuntime,
    entity: Entity,
    ctx_queries: &ContextQueries,
    cmd_queries: &EntityCmdQueries,
    lua_phase: Option<LuaPhaseSnapshot<'_>>,
    previous_phase: Option<&str>,
) -> LuaResult<LuaTable> {
    let lua = lua_runtime.lua();
    let tables = lua_runtime.get_entity_ctx_pool()?;

    // Query all optional components
    let group = ctx_queries.groups.get(entity).ok().map(|g| g.name());

    let map_pos = cmd_queries
        .positions
        .get(entity)
        .ok()
        .map(|p| (p.pos.x, p.pos.y));

    let screen_pos = cmd_queries
        .screen_positions
        .get(entity)
        .ok()
        .map(|p| (p.pos.x, p.pos.y));

    let rigid_body = cmd_queries
        .rigid_bodies
        .get(entity)
        .ok()
        .map(|rb| RigidBodySnapshot {
            velocity: (rb.velocity.x, rb.velocity.y),
            speed_sq: rb.velocity.length_sqr(),
            frozen: rb.frozen,
        });

    let rotation = ctx_queries.rotations.get(entity).ok().map(|r| r.degrees);

    let scale = ctx_queries
        .scales
        .get(entity)
        .ok()
        .map(|s| (s.scale.x, s.scale.y));

    // Compute collider rect using position
    let rect = ctx_queries.box_colliders.get(entity).ok().and_then(|bc| {
        cmd_queries.positions.get(entity).ok().map(|pos| {
            let rect = bc.as_rectangle(pos.pos);
            (rect.x, rect.y, rect.width, rect.height)
        })
    });

    let sprite = cmd_queries
        .sprites
        .get(entity)
        .ok()
        .map(|s| SpriteSnapshot {
            tex_key: s.tex_key.as_ref(),
            flip_h: s.flip_h,
            flip_v: s.flip_v,
        });

    let animation = cmd_queries
        .animation
        .get(entity)
        .ok()
        .map(|a| AnimationSnapshot {
            key: a.animation_key.as_str(),
            frame_index: a.frame_index,
            elapsed: a.elapsed_time,
        });

    let signals_ref = cmd_queries.signals.get(entity).ok();

    let lua_timer = ctx_queries
        .lua_timers
        .get(entity)
        .ok()
        .map(|t| LuaTimerSnapshot {
            duration: t.duration,
            elapsed: t.elapsed,
            callback: t.callback.name.as_str(),
        });

    // World transform from GlobalTransform2D (hierarchy)
    let gt = ctx_queries.global_transforms.get(entity).ok();
    let world_pos = gt.map(|g| (g.position.x, g.position.y));
    let world_rotation = gt.map(|g| g.rotation_degrees);
    let world_scale = gt.map(|g| (g.scale.x, g.scale.y));

    // Parent entity ID from ChildOf
    let parent_id = ctx_queries.child_of.get(entity).ok().map(|c| c.0.to_bits());

    let snapshot = EntitySnapshot {
        entity_id: entity.to_bits(),
        group,
        map_pos,
        screen_pos,
        rigid_body,
        rotation,
        scale,
        rect,
        sprite,
        animation,
        signals: signals_ref,
        lua_phase,
        lua_timer,
        previous_phase,
        world_pos,
        world_rotation,
        world_scale,
        parent_id,
    };

    build_entity_context_pooled(lua, &tables, &snapshot)
}
