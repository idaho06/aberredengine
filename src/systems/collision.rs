//! Collision detection and handling systems.
//!
//! This module provides two main systems:
//!
//! - [`collision_detector`] – pairwise AABB overlap checks, emits [`CollisionEvent`](crate::events::collision::CollisionEvent)
//! - [`collision_observer`] – receives collision events and dispatches to [`CollisionRule`](crate::components::collision::CollisionRule) or [`LuaCollisionRule`](crate::components::luacollision::LuaCollisionRule) callbacks
//!
//! # Collision Flow
//!
//! 1. `collision_detector` iterates all entity pairs with [`BoxCollider`](crate::components::boxcollider::BoxCollider) + [`MapPosition`](crate::components::mapposition::MapPosition)
//! 2. For each overlap, triggers a `CollisionEvent`
//! 3. `collision_observer` looks up matching collision rules by [`Group`](crate::components::group::Group) names
//! 4. For Rust rules: invokes the callback with a [`CollisionContext`](crate::components::collision::CollisionContext)
//! 5. For Lua rules: calls [`call_lua_collision_callback`] with pooled context tables
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
//! # Defining Collision Rules
//!
//! Collision rules are defined in game code and spawned as entities:
//!
//! ```ignore
//! commands.spawn((
//!     CollisionRule::new("ball", "brick", ball_brick_callback as CollisionCallback),
//!     Group::new("collision_rules"),
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::components::collision::CollisionRule`] – defines Rust collision handlers
//! - [`crate::components::luacollision::LuaCollisionRule`] – defines Lua collision handlers
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision::CollisionEvent`] – emitted on each collision

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::boxcollider::BoxCollider;
use crate::components::collision::{CollisionContext, CollisionRule, get_colliding_sides};
use crate::components::group::Group;
use crate::components::luacollision::LuaCollisionRule;
use crate::components::luaphase::LuaPhase;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::components::stuckto::StuckTo;
use crate::components::animation::Animation;
use crate::events::audio::AudioCmd;
use crate::events::collision::CollisionEvent;
use crate::resources::lua_runtime::LuaRuntime;
use crate::resources::worldsignals::WorldSignals;
use crate::systems::lua_commands::{
    process_audio_command, process_camera_command, process_entity_commands, process_phase_command,
    process_signal_command, process_spawn_command,
};
// use crate::resources::worldtime::WorldTime; // Collisions are independent of time

/// Broad-phase pairwise overlap test with event emission.
///
/// Uses ECS `iter_combinations_mut()` to efficiently iterate unique pairs,
/// checks overlap, and triggers an event for each collision. Observers can
/// react to despawn, apply damage, or play sounds.
pub fn collision_detector(
    mut query: Query<(Entity, &MapPosition, &BoxCollider)>,
    mut commands: Commands,
) {
    let mut combos = query.iter_combinations_mut();
    while let Some(
        [
            (entity_a, position_a, collider_a),
            (entity_b, position_b, collider_b),
        ],
    ) = combos.fetch_next()
    {
        let rect_a = collider_a.as_rectangle(position_a.pos);
        let rect_b = collider_b.as_rectangle(position_b.pos);
        if rect_a.check_collision_recs(&rect_b) {
            commands.trigger(CollisionEvent {
                a: entity_a,
                b: entity_b,
            });
        }
    }
}

/// Global observer when a CollisionEvent is triggered.
///
#[derive(SystemParam)]
pub struct CollisionObserverParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub groups: Query<'w, 's, &'static Group>,
    pub rules: Query<'w, 's, &'static CollisionRule>,
    pub lua_rules: Query<'w, 's, &'static LuaCollisionRule>,
    pub positions: Query<'w, 's, &'static mut MapPosition>,
    pub rigid_bodies: Query<'w, 's, &'static mut RigidBody>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub signals: Query<'w, 's, &'static mut Signals>,
    pub stuckto_query: Query<'w, 's, &'static StuckTo>,
    pub animation_query: Query<'w, 's, &'static mut Animation>,
    pub luaphase_query: Query<'w, 's, (Entity, &'static mut LuaPhase)>,
    pub world_signals: ResMut<'w, WorldSignals>,
    pub audio_cmds: MessageWriter<'w, AudioCmd>,
    pub lua_runtime: NonSend<'w, LuaRuntime>,
}

pub fn collision_observer(trigger: On<CollisionEvent>, mut params: CollisionObserverParams) {
    let a = trigger.event().a;
    let b = trigger.event().b;

    //eprintln!("Collision detected: {:?} and {:?}", a, b);
    let ga = if let Ok(group) = params.groups.get(a) {
        group.name()
    } else {
        return;
    };
    let gb = if let Ok(group) = params.groups.get(b) {
        group.name()
    } else {
        return;
    };

    // First, check Rust-based collision rules
    for rule in params.rules.iter() {
        if let Some((ent_a, ent_b)) = rule.match_and_order(a, b, ga, gb) {
            //eprintln!(
            //    "Collision rule matched for groups '{}' and '{}'",
            //    ga, gb
            //);
            let callback = rule.callback;
            let mut ctx = CollisionContext {
                commands: &mut params.commands,
                groups: &params.groups,
                positions: &mut params.positions,
                rigid_bodies: &mut params.rigid_bodies,
                box_colliders: &params.box_colliders,
                signals: &mut params.signals,
                world_signals: &mut params.world_signals,
                audio_cmds: &mut params.audio_cmds,
            };
            callback(ent_a, ent_b, &mut ctx);
            return;
        }
    }

    // Then, check Lua-based collision rules
    for lua_rule in params.lua_rules.iter() {
        if let Some((ent_a, ent_b, callback_name)) = lua_rule.match_and_order(a, b, ga, gb) {
            // Gather entity data for Lua callback
            let pos_a = params.positions.get(ent_a).ok().map(|p| (p.pos.x, p.pos.y));
            let pos_b = params.positions.get(ent_b).ok().map(|p| (p.pos.x, p.pos.y));
            let (vel_a, speed_sq_a) = params
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

            // Get collider rects for side detection
            let rect_a = params.box_colliders.get(ent_a).ok().and_then(|c| {
                pos_a.map(|(px, py)| c.as_rectangle(raylib::math::Vector2 { x: px, y: py }))
            });
            let rect_b = params.box_colliders.get(ent_b).ok().and_then(|c| {
                pos_b.map(|(px, py)| c.as_rectangle(raylib::math::Vector2 { x: px, y: py }))
            });

            // Get colliding sides (uses SmallVec to avoid heap allocation)
            let (sides_a, sides_b) = match (rect_a, rect_b) {
                (Some(ra), Some(rb)) => {
                    get_colliding_sides(&ra, &rb).unwrap_or_default()
                }
                _ => Default::default(),
            };

            // Get entity signals (integers and flags)
            let signals_a = params.signals.get(ent_a).ok();
            let signals_b = params.signals.get(ent_b).ok();

            // Get group names
            let group_a = params.groups.get(ent_a).ok().map(|g| g.name().to_string());
            let group_b = params.groups.get(ent_b).ok().map(|g| g.name().to_string());

            // Update signal cache so Lua can read current world signals
            params
                .lua_runtime
                .update_signal_cache(params.world_signals.snapshot());

            // Build ctx table in Lua
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
                signals_a.as_deref(),
                signals_b.as_deref(),
                group_a.as_deref(),
                group_b.as_deref(),
            ) {
                eprintln!(
                    "[Lua Collision] Error calling callback '{}': {}",
                    callback_name, e
                );
                return;
            }

            // Process collision commands after Lua callback returns
            process_entity_commands(
                &mut params.commands,
                params.lua_runtime.drain_collision_entity_commands(),
                &params.stuckto_query,
                &mut params.signals,
                &mut params.animation_query,
                &mut params.rigid_bodies,
                &mut params.positions,
            );

            // Process collision signal commands
            for cmd in params.lua_runtime.drain_collision_signal_commands() {
                process_signal_command(&mut params.world_signals, cmd);
            }

            // Process collision audio commands
            for cmd in params.lua_runtime.drain_collision_audio_commands() {
                process_audio_command(&mut params.audio_cmds, cmd);
            }

            // Process collision spawn commands
            for cmd in params.lua_runtime.drain_collision_spawn_commands() {
                process_spawn_command(&mut params.commands, cmd, &mut params.world_signals);
            }

            // Process collision phase commands
            for cmd in params.lua_runtime.drain_collision_phase_commands() {
                process_phase_command(&mut params.luaphase_query, cmd);
            }

            // Process collision camera commands
            for cmd in params.lua_runtime.drain_collision_camera_commands() {
                process_camera_command(&mut params.commands, cmd);
            }

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

/// Populate an entity's signal tables (creates fresh tables for variable-length data).
fn populate_entity_signals(
    lua: &mlua::Lua,
    signals_table: &mlua::Table,
    signals: &Signals,
) -> mlua::Result<()> {
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

    // Get pooled tables
    let tables = lua_runtime.get_collision_ctx_pool()?;

    // === Populate Entity A ===
    tables.entity_a.set("id", entity_a_id)?;
    tables.entity_a.set("group", group_a.unwrap_or(""))?;
    tables.entity_a.set("speed_sq", speed_sq_a)?;

    // Position A
    if let Some((x, y)) = pos_a {
        tables.pos_a.set("x", x)?;
        tables.pos_a.set("y", y)?;
        tables.entity_a.set("pos", tables.pos_a.clone())?;
    } else {
        tables.entity_a.set("pos", mlua::Value::Nil)?;
    }

    // Velocity A
    if let Some((vx, vy)) = vel_a {
        tables.vel_a.set("x", vx)?;
        tables.vel_a.set("y", vy)?;
        tables.entity_a.set("vel", tables.vel_a.clone())?;
    } else {
        tables.entity_a.set("vel", mlua::Value::Nil)?;
    }

    // Rect A
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

    // Position B
    if let Some((x, y)) = pos_b {
        tables.pos_b.set("x", x)?;
        tables.pos_b.set("y", y)?;
        tables.entity_b.set("pos", tables.pos_b.clone())?;
    } else {
        tables.entity_b.set("pos", mlua::Value::Nil)?;
    }

    // Velocity B
    if let Some((vx, vy)) = vel_b {
        tables.vel_b.set("x", vx)?;
        tables.vel_b.set("y", vy)?;
        tables.entity_b.set("vel", tables.vel_b.clone())?;
    } else {
        tables.entity_b.set("vel", mlua::Value::Nil)?;
    }

    // Rect B
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

    // Call the Lua function with pooled context
    let func: mlua::Function = lua.globals().get(callback_name)?;
    func.call::<()>(tables.ctx)?;

    Ok(())
}
