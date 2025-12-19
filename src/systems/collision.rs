//! Collision detection and handling systems.
//!
//! This module provides two main systems:
//!
//! - [`collision_detector`] – pairwise AABB overlap checks, emits [`CollisionEvent`](crate::events::collision::CollisionEvent)
//! - [`collision_observer`] – receives collision events and dispatches to [`CollisionRule`](crate::components::collision::CollisionRule) callbacks
//!
//! # Collision Flow
//!
//! 1. `collision_detector` iterates all entity pairs with [`BoxCollider`](crate::components::boxcollider::BoxCollider) + [`MapPosition`](crate::components::mapposition::MapPosition)
//! 2. For each overlap, triggers a `CollisionEvent`
//! 3. `collision_observer` looks up matching `CollisionRule` components by [`Group`](crate::components::group::Group) names
//! 4. Invokes the rule's callback with both entities and a [`CollisionContext`](crate::components::collision::CollisionContext)
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
//! - [`crate::components::collision::CollisionRule`] – defines collision handlers
//! - [`crate::components::boxcollider::BoxCollider`] – axis-aligned collider
//! - [`crate::events::collision::CollisionEvent`] – emitted on each collision

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::boxcollider::BoxCollider;
use crate::components::collision::{CollisionContext, CollisionRule, get_colliding_sides};
use crate::components::group::Group;
use crate::components::luacollision::LuaCollisionRule;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::events::audio::AudioCmd;
use crate::events::collision::CollisionEvent;
use crate::resources::lua_runtime::LuaRuntime;
use crate::resources::worldsignals::WorldSignals;
use crate::systems::lua_commands::{
    process_audio_command, process_collision_entity_commands, process_signal_command,
};
// use crate::resources::worldtime::WorldTime; // Collisions are independent of time

/// Broad-phase pairwise overlap test with event emission.
///
/// Uses ECS `iter_combinations_mut()` to efficiently iterate unique pairs,
/// checks overlap, and triggers an event for each collision. Observers can
/// react to despawn, apply damage, or play sounds.
pub fn collision_detector(
    mut query: Query<(Entity, &mut MapPosition, &BoxCollider)>,
    mut commands: Commands,
) {
    // first we create a Vector of pairs of entities
    let mut pairs: Vec<(Entity, Entity)> = Vec::new();

    let mut combos = query.iter_combinations_mut();
    while let Some(
        [
            (entity_a, position_a, collider_a),
            (entity_b, position_b, collider_b),
        ],
    ) = combos.fetch_next()
    {
        /* if collider_a.overlaps(position_a.pos, collider_b, position_b.pos) {
            pairs.push((entity_a, entity_b));
        } */
        let rect_a = collider_a.as_rectangle(position_a.pos);
        let rect_b = collider_b.as_rectangle(position_b.pos);
        if rect_a.check_collision_recs(&rect_b) {
            pairs.push((entity_a, entity_b));
        }
    }

    // Trigger a CollisionEvent for each pair. Observers will run immediately when commands flush.
    for (entity_a, entity_b) in pairs {
        // println!(
        //     "Triggering CollisionEvent between {:?} and {:?}",
        //     entity_a, entity_b
        // );
        commands.trigger(CollisionEvent {
            a: entity_a,
            b: entity_b,
        });
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
            let vel_a = params
                .rigid_bodies
                .get(ent_a)
                .ok()
                .map(|rb| (rb.velocity.x, rb.velocity.y));
            let vel_b = params
                .rigid_bodies
                .get(ent_b)
                .ok()
                .map(|rb| (rb.velocity.x, rb.velocity.y));

            // Get collider rects for side detection
            let rect_a = params
                .box_colliders
                .get(ent_a)
                .ok()
                .and_then(|c| pos_a.map(|(px, py)| c.as_rectangle(raylib::math::Vector2 { x: px, y: py })));
            let rect_b = params
                .box_colliders
                .get(ent_b)
                .ok()
                .and_then(|c| pos_b.map(|(px, py)| c.as_rectangle(raylib::math::Vector2 { x: px, y: py })));

            // Get colliding sides
            let (sides_a, sides_b) = match (rect_a, rect_b) {
                (Some(ra), Some(rb)) => get_colliding_sides(&ra, &rb).unwrap_or_else(|| (Vec::new(), Vec::new())),
                _ => (Vec::new(), Vec::new()),
            };

            // Get entity signals (integers and flags)
            let signals_a = params.signals.get(ent_a).ok();
            let signals_b = params.signals.get(ent_b).ok();

            // Get group names
            let group_a = params.groups.get(ent_a).ok().map(|g| g.name().to_string());
            let group_b = params.groups.get(ent_b).ok().map(|g| g.name().to_string());

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
            process_collision_entity_commands(
                &mut params.commands,
                params.lua_runtime.drain_collision_entity_commands(),
                &mut params.positions,
                &mut params.rigid_bodies,
                &mut params.signals,
            );

            // Process collision signal commands
            for cmd in params.lua_runtime.drain_collision_signal_commands() {
                process_signal_command(&mut params.world_signals, cmd);
            }

            // Process collision audio commands
            for cmd in params.lua_runtime.drain_collision_audio_commands() {
                process_audio_command(&mut params.audio_cmds, cmd);
            }

            return;
        }
    }
}

/// Call a Lua collision callback with context data.
fn call_lua_collision_callback(
    lua_runtime: &LuaRuntime,
    callback_name: &str,
    entity_a_id: u64,
    entity_b_id: u64,
    pos_a: Option<(f32, f32)>,
    pos_b: Option<(f32, f32)>,
    vel_a: Option<(f32, f32)>,
    vel_b: Option<(f32, f32)>,
    rect_a: Option<(f32, f32, f32, f32)>,
    rect_b: Option<(f32, f32, f32, f32)>,
    sides_a: &[crate::components::collision::BoxSide],
    sides_b: &[crate::components::collision::BoxSide],
    signals_a: Option<&Signals>,
    signals_b: Option<&Signals>,
    group_a: Option<&str>,
    group_b: Option<&str>,
) -> mlua::Result<()> {
    use mlua::IntoLua;

    let lua = lua_runtime.lua();

    // Create ctx table
    let ctx = lua.create_table()?;

    // Entity A
    let a_table = lua.create_table()?;
    a_table.set("id", entity_a_id)?;
    a_table.set("group", group_a.unwrap_or(""))?;
    if let Some((x, y)) = pos_a {
        let pos_table = lua.create_table()?;
        pos_table.set("x", x)?;
        pos_table.set("y", y)?;
        a_table.set("pos", pos_table)?;
    }
    if let Some((vx, vy)) = vel_a {
        let vel_table = lua.create_table()?;
        vel_table.set("x", vx)?;
        vel_table.set("y", vy)?;
        a_table.set("vel", vel_table)?;
    }
    if let Some((x, y, w, h)) = rect_a {
        let rect_table = lua.create_table()?;
        rect_table.set("x", x)?;
        rect_table.set("y", y)?;
        rect_table.set("w", w)?;
        rect_table.set("h", h)?;
        a_table.set("rect", rect_table)?;
    }
    if let Some(signals) = signals_a {
        let sig_table = lua.create_table()?;
        let flags_table = lua.create_table()?;
        for (i, flag) in signals.get_flags().iter().enumerate() {
            flags_table.set(i + 1, flag.as_str())?;
        }
        sig_table.set("flags", flags_table)?;
        let integers_table = lua.create_table()?;
        for (key, value) in signals.get_integers() {
            integers_table.set(key.as_str(), *value)?;
        }
        sig_table.set("integers", integers_table)?;
        a_table.set("signals", sig_table)?;
    }
    ctx.set("a", a_table)?;

    // Entity B
    let b_table = lua.create_table()?;
    b_table.set("id", entity_b_id)?;
    b_table.set("group", group_b.unwrap_or(""))?;
    if let Some((x, y)) = pos_b {
        let pos_table = lua.create_table()?;
        pos_table.set("x", x)?;
        pos_table.set("y", y)?;
        b_table.set("pos", pos_table)?;
    }
    if let Some((vx, vy)) = vel_b {
        let vel_table = lua.create_table()?;
        vel_table.set("x", vx)?;
        vel_table.set("y", vy)?;
        b_table.set("vel", vel_table)?;
    }
    if let Some((x, y, w, h)) = rect_b {
        let rect_table = lua.create_table()?;
        rect_table.set("x", x)?;
        rect_table.set("y", y)?;
        rect_table.set("w", w)?;
        rect_table.set("h", h)?;
        b_table.set("rect", rect_table)?;
    }
    if let Some(signals) = signals_b {
        let sig_table = lua.create_table()?;
        let flags_table = lua.create_table()?;
        for (i, flag) in signals.get_flags().iter().enumerate() {
            flags_table.set(i + 1, flag.as_str())?;
        }
        sig_table.set("flags", flags_table)?;
        let integers_table = lua.create_table()?;
        for (key, value) in signals.get_integers() {
            integers_table.set(key.as_str(), *value)?;
        }
        sig_table.set("integers", integers_table)?;
        b_table.set("signals", sig_table)?;
    }
    ctx.set("b", b_table)?;

    // Sides
    let sides_table = lua.create_table()?;
    let sides_a_table = lua.create_table()?;
    for (i, side) in sides_a.iter().enumerate() {
        let side_str = match side {
            crate::components::collision::BoxSide::Left => "left",
            crate::components::collision::BoxSide::Right => "right",
            crate::components::collision::BoxSide::Top => "top",
            crate::components::collision::BoxSide::Bottom => "bottom",
        };
        sides_a_table.set(i + 1, side_str)?;
    }
    sides_table.set("a", sides_a_table)?;
    let sides_b_table = lua.create_table()?;
    for (i, side) in sides_b.iter().enumerate() {
        let side_str = match side {
            crate::components::collision::BoxSide::Left => "left",
            crate::components::collision::BoxSide::Right => "right",
            crate::components::collision::BoxSide::Top => "top",
            crate::components::collision::BoxSide::Bottom => "bottom",
        };
        sides_b_table.set(i + 1, side_str)?;
    }
    sides_table.set("b", sides_b_table)?;
    ctx.set("sides", sides_table)?;

    // Call the Lua function
    let func: mlua::Function = lua.globals().get(callback_name)?;
    func.call::<()>(ctx)?;

    Ok(())
}
