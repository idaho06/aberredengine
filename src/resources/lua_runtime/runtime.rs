//! Lua runtime core implementation.
//!
//! This module contains the `LuaRuntime` struct which manages the Lua interpreter
//! and provides the `engine` table API to Lua scripts.

use super::commands::*;
use super::entity_builder::LuaEntityBuilder;
use super::input_snapshot::InputSnapshot;
use super::spawn_data::*;
use crate::resources::worldsignals::SignalSnapshot;
use mlua::prelude::*;
use rustc_hash::FxHashSet;
use std::cell::RefCell;
use std::sync::Arc;

use log::{error, info, warn};

/// Shared state accessible from Lua function closures.
/// This is stored in Lua's app_data and allows Lua functions to queue commands.
pub(super) struct LuaAppData {
    asset_commands: RefCell<Vec<AssetCmd>>,
    pub(super) spawn_commands: RefCell<Vec<SpawnCmd>>,
    audio_commands: RefCell<Vec<AudioLuaCmd>>,
    signal_commands: RefCell<Vec<SignalCmd>>,
    phase_commands: RefCell<Vec<PhaseCmd>>,
    entity_commands: RefCell<Vec<EntityCmd>>,
    group_commands: RefCell<Vec<GroupCmd>>,
    tilemap_commands: RefCell<Vec<TilemapCmd>>,
    camera_commands: RefCell<Vec<CameraCmd>>,
    animation_commands: RefCell<Vec<AnimationCmd>>,
    render_commands: RefCell<Vec<RenderCmd>>,
    /// Clone commands for regular context (scene setup, phase callbacks)
    pub(super) clone_commands: RefCell<Vec<CloneCmd>>,
    // Collision-scoped command queues (processed immediately after each collision callback)
    collision_entity_commands: RefCell<Vec<EntityCmd>>,
    collision_signal_commands: RefCell<Vec<SignalCmd>>,
    collision_audio_commands: RefCell<Vec<AudioLuaCmd>>,
    pub(super) collision_spawn_commands: RefCell<Vec<SpawnCmd>>,
    collision_phase_commands: RefCell<Vec<PhaseCmd>>,
    collision_camera_commands: RefCell<Vec<CameraCmd>>,
    /// Clone commands for collision context (processed after collision callbacks)
    pub(super) collision_clone_commands: RefCell<Vec<CloneCmd>>,
    /// Cached world signal snapshot (read-only for Lua).
    /// Updated before calling Lua callbacks via `update_signal_cache()`.
    /// Using Arc allows cheap sharing without cloning all maps on every callback.
    signal_snapshot: RefCell<Arc<SignalSnapshot>>,
    /// Cached tracked group names (read-only snapshot for Lua)
    tracked_groups: RefCell<FxHashSet<String>>,
}

/// Registry keys for pooled collision context tables.
/// Created once during LuaRuntime initialization, reused for every collision.
struct CollisionCtxPool {
    // Main structure
    ctx: LuaRegistryKey,
    entity_a: LuaRegistryKey,
    entity_b: LuaRegistryKey,

    // Entity A subtables
    pos_a: LuaRegistryKey,
    vel_a: LuaRegistryKey,
    rect_a: LuaRegistryKey,
    signals_a: LuaRegistryKey,

    // Entity B subtables
    pos_b: LuaRegistryKey,
    vel_b: LuaRegistryKey,
    rect_b: LuaRegistryKey,
    signals_b: LuaRegistryKey,

    // Sides
    // sides: LuaRegistryKey,
    sides_a: LuaRegistryKey,
    sides_b: LuaRegistryKey,
}

/// Borrowed references to pooled collision context tables.
/// Used by collision system to populate and pass context to Lua callbacks.
pub struct CollisionCtxTables {
    pub ctx: LuaTable,
    pub entity_a: LuaTable,
    pub entity_b: LuaTable,
    pub pos_a: LuaTable,
    pub pos_b: LuaTable,
    pub vel_a: LuaTable,
    pub vel_b: LuaTable,
    pub rect_a: LuaTable,
    pub rect_b: LuaTable,
    pub signals_a: LuaTable,
    pub signals_b: LuaTable,
    // pub sides: LuaTable,
    pub sides_a: LuaTable,
    pub sides_b: LuaTable,
}

/// Registry keys for pooled entity context tables.
/// Created once during LuaRuntime initialization, reused for phase/timer callbacks.
struct EntityCtxPool {
    ctx: LuaRegistryKey,
    pos: LuaRegistryKey,
    screen_pos: LuaRegistryKey,
    vel: LuaRegistryKey,
    scale: LuaRegistryKey,
    rect: LuaRegistryKey,
    sprite: LuaRegistryKey,
    animation: LuaRegistryKey,
    timer: LuaRegistryKey,
    signals: LuaRegistryKey,
}

/// Borrowed references to pooled entity context tables.
/// Used by LuaPhase and LuaTimer systems to populate and pass context to Lua callbacks.
pub struct EntityCtxTables {
    pub ctx: LuaTable,
    pub pos: LuaTable,
    pub screen_pos: LuaTable,
    pub vel: LuaTable,
    pub scale: LuaTable,
    pub rect: LuaTable,
    pub sprite: LuaTable,
    pub animation: LuaTable,
    pub timer: LuaTable,
    pub signals: LuaTable,
}

/// Resource holding the Lua interpreter state.
///
/// This is a `NonSend` resource because the Lua state is not thread-safe.
/// It should be initialized once at startup and reused throughout the game.
pub struct LuaRuntime {
    lua: Lua,
    /// Pooled collision context tables for reuse across collisions.
    collision_ctx_pool: Option<CollisionCtxPool>,
    /// Pooled entity context tables for reuse across phase/timer callbacks.
    entity_ctx_pool: Option<EntityCtxPool>,
}

/// Pushes function metadata to `engine.__meta.functions[name]`.
fn push_fn_meta(
    lua: &Lua,
    meta_fns: &LuaTable,
    name: &str,
    desc: &str,
    category: &str,
    params: &[(&str, &str)],
    returns: Option<&str>,
) -> LuaResult<()> {
    let tbl = lua.create_table()?;
    tbl.set("description", desc)?;
    tbl.set("category", category)?;
    let params_tbl = lua.create_table()?;
    for (i, (pname, ptype)) in params.iter().enumerate() {
        let p = lua.create_table()?;
        p.set("name", *pname)?;
        p.set("type", *ptype)?;
        params_tbl.set(i + 1, p)?;
    }
    tbl.set("params", params_tbl)?;
    if let Some(ret) = returns {
        let r = lua.create_table()?;
        r.set("type", ret)?;
        tbl.set("returns", r)?;
    }
    meta_fns.set(name, tbl)?;
    Ok(())
}

/// Registers a Lua function that pushes a command to a queue in `LuaAppData`.
macro_rules! register_cmd {
    // Variant with metadata
    ($engine:expr, $lua:expr, $meta_fns:expr, $name:expr, $queue:ident,
     |$args:pat_param| $arg_ty:ty, $cmd:expr,
     desc = $desc:expr, cat = $cat:expr,
     params = [ $( ($pname:expr, $pty:expr) ),* $(,)? ]
     $(, returns = $ret:expr )?
    ) => {
        $engine.set(
            $name,
            $lua.create_function(|lua, $args: $arg_ty| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .$queue
                    .borrow_mut()
                    .push($cmd);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &$lua, &$meta_fns, $name, $desc, $cat,
            &[ $(($pname, $pty)),* ],
            register_cmd!(@opt_ret $($ret)?)
        )?;
    };
    // Helper to produce Option<&str> from optional returns
    (@opt_ret $ret:expr) => { Some($ret) };
    (@opt_ret) => { None };
}

/// Registers a batch of entity commands with a name prefix to a specific queue.
macro_rules! register_entity_cmds {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident, [
        $( ($name:literal,
            |$args:pat_param| $arg_ty:ty, $cmd:expr,
            desc = $desc:expr,
            params = [ $( ($pname:expr, $pty:expr) ),* $(,)? ]
        ) ),* $(,)?
    ]) => {
        $(
            register_cmd!($engine, $lua, $meta_fns, concat!($prefix, $name), $queue,
                |$args| $arg_ty, $cmd,
                desc = $desc, cat = "entity",
                params = [ $( ($pname, $pty) ),* ]);
        )*
    };
}

/// Defines and registers all entity commands for a given prefix and queue.
/// Called with `""` prefix for regular commands, `"collision_"` for collision commands.
macro_rules! define_entity_cmds {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident) => {
        register_entity_cmds!($engine, $lua, $meta_fns, $prefix, $queue, [
            ("entity_despawn", |entity_id| u64, EntityCmd::Despawn { entity_id },
                desc = "Despawn an entity",
                params = [("entity_id", "integer")]),
            ("entity_menu_despawn", |entity_id| u64, EntityCmd::MenuDespawn { entity_id },
                desc = "Despawn a menu entity and its children",
                params = [("entity_id", "integer")]),
            ("release_stuckto", |entity_id| u64, EntityCmd::ReleaseStuckTo { entity_id },
                desc = "Release entity from its StuckTo target, restoring stored velocity",
                params = [("entity_id", "integer")]),
            ("entity_signal_set_flag",
                |(entity_id, flag)| (u64, String), EntityCmd::SignalSetFlag { entity_id, flag },
                desc = "Set a flag on an entity's signals",
                params = [("entity_id", "integer"), ("flag", "string")]),
            ("entity_signal_clear_flag",
                |(entity_id, flag)| (u64, String), EntityCmd::SignalClearFlag { entity_id, flag },
                desc = "Clear a flag on an entity's signals",
                params = [("entity_id", "integer"), ("flag", "string")]),
            ("entity_set_velocity",
                |(entity_id, vx, vy)| (u64, f32, f32), EntityCmd::SetVelocity { entity_id, vx, vy },
                desc = "Set entity velocity",
                params = [("entity_id", "integer"), ("vx", "number"), ("vy", "number")]),
            ("entity_insert_stuckto",
                |(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)|
                (u64, u64, bool, bool, f32, f32, f32, f32),
                EntityCmd::InsertStuckTo {
                    entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy,
                },
                desc = "Attach entity to a target entity",
                params = [("entity_id", "integer"), ("target_id", "integer"),
                          ("follow_x", "boolean"), ("follow_y", "boolean"),
                          ("offset_x", "number"), ("offset_y", "number"),
                          ("stored_vx", "number"), ("stored_vy", "number")]),
            ("entity_restart_animation", |entity_id| u64, EntityCmd::RestartAnimation { entity_id },
                desc = "Restart entity animation from frame 0",
                params = [("entity_id", "integer")]),
            ("entity_set_animation",
                |(entity_id, animation_key)| (u64, String), EntityCmd::SetAnimation { entity_id, animation_key },
                desc = "Set entity animation by key",
                params = [("entity_id", "integer"), ("animation_key", "string")]),
            ("entity_insert_lua_timer",
                |(entity_id, duration, callback)| (u64, f32, String),
                EntityCmd::InsertLuaTimer { entity_id, duration, callback },
                desc = "Insert a Lua timer on an entity",
                params = [("entity_id", "integer"), ("duration", "number"), ("callback", "string")]),
            ("entity_remove_lua_timer", |entity_id| u64, EntityCmd::RemoveLuaTimer { entity_id },
                desc = "Remove the Lua timer from an entity",
                params = [("entity_id", "integer")]),
            ("entity_insert_ttl",
                |(entity_id, seconds)| (u64, f32), EntityCmd::InsertTtl { entity_id, seconds },
                desc = "Insert a time-to-live component on an entity",
                params = [("entity_id", "integer"), ("seconds", "number")]),
            ("entity_insert_tween_position",
                |(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenPosition {
                    entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards,
                },
                desc = "Insert a position tween on an entity",
                params = [("entity_id", "integer"), ("from_x", "number"), ("from_y", "number"),
                          ("to_x", "number"), ("to_y", "number"), ("duration", "number"),
                          ("easing", "string"), ("loop_mode", "string"), ("backwards", "boolean")]),
            ("entity_insert_tween_rotation",
                |(entity_id, from, to, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenRotation {
                    entity_id, from, to, duration, easing, loop_mode, backwards,
                },
                desc = "Insert a rotation tween on an entity",
                params = [("entity_id", "integer"), ("from", "number"), ("to", "number"),
                          ("duration", "number"), ("easing", "string"), ("loop_mode", "string"),
                          ("backwards", "boolean")]),
            ("entity_insert_tween_scale",
                |(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenScale {
                    entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards,
                },
                desc = "Insert a scale tween on an entity",
                params = [("entity_id", "integer"), ("from_x", "number"), ("from_y", "number"),
                          ("to_x", "number"), ("to_y", "number"), ("duration", "number"),
                          ("easing", "string"), ("loop_mode", "string"), ("backwards", "boolean")]),
            ("entity_remove_tween_position", |entity_id| u64, EntityCmd::RemoveTweenPosition { entity_id },
                desc = "Remove position tween from an entity",
                params = [("entity_id", "integer")]),
            ("entity_remove_tween_rotation", |entity_id| u64, EntityCmd::RemoveTweenRotation { entity_id },
                desc = "Remove rotation tween from an entity",
                params = [("entity_id", "integer")]),
            ("entity_remove_tween_scale", |entity_id| u64, EntityCmd::RemoveTweenScale { entity_id },
                desc = "Remove scale tween from an entity",
                params = [("entity_id", "integer")]),
            ("entity_set_rotation",
                |(entity_id, degrees)| (u64, f32), EntityCmd::SetRotation { entity_id, degrees },
                desc = "Set entity rotation in degrees",
                params = [("entity_id", "integer"), ("degrees", "number")]),
            ("entity_set_scale",
                |(entity_id, sx, sy)| (u64, f32, f32), EntityCmd::SetScale { entity_id, sx, sy },
                desc = "Set entity scale",
                params = [("entity_id", "integer"), ("sx", "number"), ("sy", "number")]),
            ("entity_signal_set_scalar",
                |(entity_id, key, value)| (u64, String, f32),
                EntityCmd::SignalSetScalar { entity_id, key, value },
                desc = "Set a scalar signal on an entity",
                params = [("entity_id", "integer"), ("key", "string"), ("value", "number")]),
            ("entity_signal_set_string",
                |(entity_id, key, value)| (u64, String, String),
                EntityCmd::SignalSetString { entity_id, key, value },
                desc = "Set a string signal on an entity",
                params = [("entity_id", "integer"), ("key", "string"), ("value", "string")]),
            ("entity_add_force",
                |(entity_id, name, x, y, enabled)| (u64, String, f32, f32, bool),
                EntityCmd::AddForce { entity_id, name, x, y, enabled },
                desc = "Add a named acceleration force to an entity",
                params = [("entity_id", "integer"), ("name", "string"),
                          ("x", "number"), ("y", "number"), ("enabled", "boolean")]),
            ("entity_remove_force",
                |(entity_id, name)| (u64, String), EntityCmd::RemoveForce { entity_id, name },
                desc = "Remove a named force from an entity",
                params = [("entity_id", "integer"), ("name", "string")]),
            ("entity_set_force_enabled",
                |(entity_id, name, enabled)| (u64, String, bool),
                EntityCmd::SetForceEnabled { entity_id, name, enabled },
                desc = "Enable or disable a named force on an entity",
                params = [("entity_id", "integer"), ("name", "string"), ("enabled", "boolean")]),
            ("entity_set_force_value",
                |(entity_id, name, x, y)| (u64, String, f32, f32),
                EntityCmd::SetForceValue { entity_id, name, x, y },
                desc = "Set the acceleration value of a named force",
                params = [("entity_id", "integer"), ("name", "string"), ("x", "number"), ("y", "number")]),
            ("entity_set_friction",
                |(entity_id, friction)| (u64, f32), EntityCmd::SetFriction { entity_id, friction },
                desc = "Set entity friction",
                params = [("entity_id", "integer"), ("friction", "number")]),
            ("entity_set_max_speed",
                |(entity_id, max_speed)| (u64, Option<f32>), EntityCmd::SetMaxSpeed { entity_id, max_speed },
                desc = "Set entity max speed (nil to remove)",
                params = [("entity_id", "integer"), ("max_speed", "number?")]),
            ("entity_freeze", |entity_id| u64, EntityCmd::FreezeEntity { entity_id },
                desc = "Freeze entity (zero velocity, ignore forces)",
                params = [("entity_id", "integer")]),
            ("entity_unfreeze", |entity_id| u64, EntityCmd::UnfreezeEntity { entity_id },
                desc = "Unfreeze entity",
                params = [("entity_id", "integer")]),
            ("entity_set_speed",
                |(entity_id, speed)| (u64, f32), EntityCmd::SetSpeed { entity_id, speed },
                desc = "Set entity speed (scales velocity to this magnitude)",
                params = [("entity_id", "integer"), ("speed", "number")]),
            ("entity_set_position",
                |(entity_id, x, y)| (u64, f32, f32), EntityCmd::SetPosition { entity_id, x, y },
                desc = "Set entity world position",
                params = [("entity_id", "integer"), ("x", "number"), ("y", "number")]),
            ("entity_signal_set_integer",
                |(entity_id, key, value)| (u64, String, i32),
                EntityCmd::SignalSetInteger { entity_id, key, value },
                desc = "Set an integer signal on an entity",
                params = [("entity_id", "integer"), ("key", "string"), ("value", "integer")]),
            ("entity_set_shader",
                |(entity_id, key)| (u64, String), EntityCmd::SetShader { entity_id, key },
                desc = "Set per-entity shader by key",
                params = [("entity_id", "integer"), ("key", "string")]),
            ("entity_remove_shader", |entity_id| u64, EntityCmd::RemoveShader { entity_id },
                desc = "Remove per-entity shader",
                params = [("entity_id", "integer")]),
            ("entity_set_tint",
                |(entity_id, r, g, b, a)| (u64, u8, u8, u8, u8),
                EntityCmd::SetTint { entity_id, r, g, b, a },
                desc = "Set entity tint color (RGBA 0-255)",
                params = [("entity_id", "integer"), ("r", "integer"), ("g", "integer"),
                          ("b", "integer"), ("a", "integer")]),
            ("entity_remove_tint", |entity_id| u64, EntityCmd::RemoveTint { entity_id },
                desc = "Remove entity tint",
                params = [("entity_id", "integer")]),
            ("entity_shader_set_float",
                |(entity_id, name, value)| (u64, String, f32),
                EntityCmd::ShaderSetFloat { entity_id, name, value },
                desc = "Set a float uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string"), ("value", "number")]),
            ("entity_shader_set_int",
                |(entity_id, name, value)| (u64, String, i32),
                EntityCmd::ShaderSetInt { entity_id, name, value },
                desc = "Set an int uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string"), ("value", "integer")]),
            ("entity_shader_set_vec2",
                |(entity_id, name, x, y)| (u64, String, f32, f32),
                EntityCmd::ShaderSetVec2 { entity_id, name, x, y },
                desc = "Set a vec2 uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string"), ("x", "number"), ("y", "number")]),
            ("entity_shader_set_vec4",
                |(entity_id, name, x, y, z, w)| (u64, String, f32, f32, f32, f32),
                EntityCmd::ShaderSetVec4 { entity_id, name, x, y, z, w },
                desc = "Set a vec4 uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string"),
                          ("x", "number"), ("y", "number"), ("z", "number"), ("w", "number")]),
            ("entity_shader_clear_uniform",
                |(entity_id, name)| (u64, String), EntityCmd::ShaderClearUniform { entity_id, name },
                desc = "Clear a uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string")]),
            ("entity_shader_clear_uniforms", |entity_id| u64, EntityCmd::ShaderClearUniforms { entity_id },
                desc = "Clear all uniforms on entity shader",
                params = [("entity_id", "integer")]),
        ]);
    };
}

impl LuaRuntime {
    /// Creates a new Lua runtime and registers the base engine API.
    ///
    /// # Errors
    ///
    /// Returns an error if Lua initialization or API registration fails.
    pub fn new() -> LuaResult<Self> {
        let lua = Lua::new();

        // Set up the package path so `require` can find scripts in assets/scripts/
        lua.load(r#"package.path = "./assets/scripts/?.lua;./assets/scripts/?/init.lua;" .. package.path"#)
            .exec()?;

        // Set up shared app data for Lua closures to access
        lua.set_app_data(LuaAppData {
            asset_commands: RefCell::new(Vec::new()),
            spawn_commands: RefCell::new(Vec::new()),
            audio_commands: RefCell::new(Vec::new()),
            signal_commands: RefCell::new(Vec::new()),
            phase_commands: RefCell::new(Vec::new()),
            entity_commands: RefCell::new(Vec::new()),
            group_commands: RefCell::new(Vec::new()),
            tilemap_commands: RefCell::new(Vec::new()),
            camera_commands: RefCell::new(Vec::new()),
            animation_commands: RefCell::new(Vec::new()),
            render_commands: RefCell::new(Vec::new()),
            clone_commands: RefCell::new(Vec::new()),
            collision_entity_commands: RefCell::new(Vec::new()),
            collision_signal_commands: RefCell::new(Vec::new()),
            collision_audio_commands: RefCell::new(Vec::new()),
            collision_spawn_commands: RefCell::new(Vec::new()),
            collision_phase_commands: RefCell::new(Vec::new()),
            collision_camera_commands: RefCell::new(Vec::new()),
            collision_clone_commands: RefCell::new(Vec::new()),
            signal_snapshot: RefCell::new(Arc::new(SignalSnapshot::default())),
            tracked_groups: RefCell::new(FxHashSet::default()),
        });

        // Create collision context pool for table reuse
        let collision_ctx_pool = Some(Self::create_collision_ctx_pool(&lua)?);

        // Create entity context pool for table reuse (LuaPhase/LuaTimer)
        let entity_ctx_pool = Some(Self::create_entity_ctx_pool(&lua)?);

        let runtime = Self {
            lua,
            collision_ctx_pool,
            entity_ctx_pool,
        };
        runtime.register_base_api()?;
        runtime.register_asset_api()?;
        runtime.register_spawn_api()?;
        runtime.register_audio_api()?;
        runtime.register_signal_api()?;
        runtime.register_phase_api()?;
        runtime.register_entity_api()?;
        runtime.register_group_api()?;
        runtime.register_tilemap_api()?;
        runtime.register_camera_api()?;
        runtime.register_collision_api()?;
        runtime.register_animation_api()?;
        runtime.register_render_api()?;
        runtime.register_builder_meta()?;
        runtime.register_types_meta()?;
        runtime.register_enums_meta()?;
        runtime.register_callbacks_meta()?;

        Ok(runtime)
    }

    /// Creates the pooled collision context tables.
    /// These tables are stored in the Lua registry and reused for every collision.
    fn create_collision_ctx_pool(lua: &Lua) -> LuaResult<CollisionCtxPool> {
        // Create all tables
        let ctx = lua.create_table()?;
        let entity_a = lua.create_table()?;
        let entity_b = lua.create_table()?;
        let pos_a = lua.create_table()?;
        let pos_b = lua.create_table()?;
        let vel_a = lua.create_table()?;
        let vel_b = lua.create_table()?;
        let rect_a = lua.create_table()?;
        let rect_b = lua.create_table()?;
        let signals_a = lua.create_table()?;
        let signals_b = lua.create_table()?;
        let sides = lua.create_table()?;
        let sides_a = lua.create_table()?;
        let sides_b = lua.create_table()?;

        // Wire up entity A structure
        entity_a.set("pos", pos_a.clone())?;
        entity_a.set("vel", vel_a.clone())?;
        entity_a.set("rect", rect_a.clone())?;
        entity_a.set("signals", signals_a.clone())?;

        // Wire up entity B structure
        entity_b.set("pos", pos_b.clone())?;
        entity_b.set("vel", vel_b.clone())?;
        entity_b.set("rect", rect_b.clone())?;
        entity_b.set("signals", signals_b.clone())?;

        // Wire up sides
        sides.set("a", sides_a.clone())?;
        sides.set("b", sides_b.clone())?;

        // Wire up main context
        ctx.set("a", entity_a.clone())?;
        ctx.set("b", entity_b.clone())?;
        ctx.set("sides", sides.clone())?;

        // Store in registry to prevent GC
        Ok(CollisionCtxPool {
            ctx: lua.create_registry_value(ctx)?,
            entity_a: lua.create_registry_value(entity_a)?,
            entity_b: lua.create_registry_value(entity_b)?,
            pos_a: lua.create_registry_value(pos_a)?,
            pos_b: lua.create_registry_value(pos_b)?,
            vel_a: lua.create_registry_value(vel_a)?,
            vel_b: lua.create_registry_value(vel_b)?,
            rect_a: lua.create_registry_value(rect_a)?,
            rect_b: lua.create_registry_value(rect_b)?,
            signals_a: lua.create_registry_value(signals_a)?,
            signals_b: lua.create_registry_value(signals_b)?,
            // sides: lua.create_registry_value(sides)?,
            sides_a: lua.create_registry_value(sides_a)?,
            sides_b: lua.create_registry_value(sides_b)?,
        })
    }

    /// Returns the pooled collision context tables for reuse.
    /// The caller must populate fields before passing to Lua callbacks.
    ///
    /// # Errors
    ///
    /// Returns an error if the pool is not initialized or registry retrieval fails.
    pub fn get_collision_ctx_pool(&self) -> LuaResult<CollisionCtxTables> {
        let pool = self
            .collision_ctx_pool
            .as_ref()
            .ok_or_else(|| LuaError::runtime("Collision context pool not initialized"))?;

        Ok(CollisionCtxTables {
            ctx: self.lua.registry_value(&pool.ctx)?,
            entity_a: self.lua.registry_value(&pool.entity_a)?,
            entity_b: self.lua.registry_value(&pool.entity_b)?,
            pos_a: self.lua.registry_value(&pool.pos_a)?,
            pos_b: self.lua.registry_value(&pool.pos_b)?,
            vel_a: self.lua.registry_value(&pool.vel_a)?,
            vel_b: self.lua.registry_value(&pool.vel_b)?,
            rect_a: self.lua.registry_value(&pool.rect_a)?,
            rect_b: self.lua.registry_value(&pool.rect_b)?,
            signals_a: self.lua.registry_value(&pool.signals_a)?,
            signals_b: self.lua.registry_value(&pool.signals_b)?,
            //sides: self.lua.registry_value(&pool.sides)?,
            sides_a: self.lua.registry_value(&pool.sides_a)?,
            sides_b: self.lua.registry_value(&pool.sides_b)?,
        })
    }

    /// Creates the pooled entity context tables for LuaPhase/LuaTimer callbacks.
    /// These tables are stored in the Lua registry and reused for every callback.
    fn create_entity_ctx_pool(lua: &Lua) -> LuaResult<EntityCtxPool> {
        // Create all tables (not wired together since fields are optional)
        let ctx = lua.create_table()?;
        let pos = lua.create_table()?;
        let screen_pos = lua.create_table()?;
        let vel = lua.create_table()?;
        let scale = lua.create_table()?;
        let rect = lua.create_table()?;
        let sprite = lua.create_table()?;
        let animation = lua.create_table()?;
        let timer = lua.create_table()?;
        let signals = lua.create_table()?;

        // Store in registry to prevent GC
        Ok(EntityCtxPool {
            ctx: lua.create_registry_value(ctx)?,
            pos: lua.create_registry_value(pos)?,
            screen_pos: lua.create_registry_value(screen_pos)?,
            vel: lua.create_registry_value(vel)?,
            scale: lua.create_registry_value(scale)?,
            rect: lua.create_registry_value(rect)?,
            sprite: lua.create_registry_value(sprite)?,
            animation: lua.create_registry_value(animation)?,
            timer: lua.create_registry_value(timer)?,
            signals: lua.create_registry_value(signals)?,
        })
    }

    /// Returns the pooled entity context tables for reuse.
    /// The caller must populate fields before passing to Lua callbacks.
    ///
    /// # Errors
    ///
    /// Returns an error if the pool is not initialized or registry retrieval fails.
    pub fn get_entity_ctx_pool(&self) -> LuaResult<EntityCtxTables> {
        let pool = self
            .entity_ctx_pool
            .as_ref()
            .ok_or_else(|| LuaError::runtime("Entity context pool not initialized"))?;

        Ok(EntityCtxTables {
            ctx: self.lua.registry_value(&pool.ctx)?,
            pos: self.lua.registry_value(&pool.pos)?,
            screen_pos: self.lua.registry_value(&pool.screen_pos)?,
            vel: self.lua.registry_value(&pool.vel)?,
            scale: self.lua.registry_value(&pool.scale)?,
            rect: self.lua.registry_value(&pool.rect)?,
            sprite: self.lua.registry_value(&pool.sprite)?,
            animation: self.lua.registry_value(&pool.animation)?,
            timer: self.lua.registry_value(&pool.timer)?,
            signals: self.lua.registry_value(&pool.signals)?,
        })
    }

    /// Registers the base `engine` table with logging functions.
    fn register_base_api(&self) -> LuaResult<()> {
        let engine = self.lua.create_table()?;

        // Create __meta table with functions and classes subtables
        let meta = self.lua.create_table()?;
        let meta_fns = self.lua.create_table()?;
        let meta_classes = self.lua.create_table()?;
        let meta_types = self.lua.create_table()?;
        let meta_enums = self.lua.create_table()?;
        let meta_callbacks = self.lua.create_table()?;
        meta.set("functions", &meta_fns)?;
        meta.set("classes", &meta_classes)?;
        meta.set("types", &meta_types)?;
        meta.set("enums", &meta_enums)?;
        meta.set("callbacks", &meta_callbacks)?;
        engine.set("__meta", meta)?;

        // engine.log(message) - General purpose logging
        engine.set(
            "log",
            self.lua.create_function(|_, msg: String| {
                info!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "log",
            "General purpose logging",
            "base",
            &[("message", "string")],
            None,
        )?;

        // engine.log_info(message) - Info level logging
        engine.set(
            "log_info",
            self.lua.create_function(|_, msg: String| {
                info!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "log_info",
            "Info level logging",
            "base",
            &[("message", "string")],
            None,
        )?;

        // engine.log_warn(message) - Warning level logging
        engine.set(
            "log_warn",
            self.lua.create_function(|_, msg: String| {
                warn!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "log_warn",
            "Warning level logging",
            "base",
            &[("message", "string")],
            None,
        )?;

        // engine.log_error(message) - Error level logging
        engine.set(
            "log_error",
            self.lua.create_function(|_, msg: String| {
                error!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "log_error",
            "Error level logging",
            "base",
            &[("message", "string")],
            None,
        )?;

        self.lua.globals().set("engine", engine)?;

        Ok(())
    }

    fn register_asset_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_texture",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::LoadTexture { id, path },
            desc = "Load a texture from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_font",
            asset_commands,
            |(id, path, size)| (String, String, i32),
            AssetCmd::LoadFont { id, path, size },
            desc = "Load a font from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string"), ("size", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_music",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::LoadMusic { id, path },
            desc = "Load music from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_sound",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::LoadSound { id, path },
            desc = "Load a sound effect from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_tilemap",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::LoadTilemap { id, path },
            desc = "Load a tilemap from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        Ok(())
    }

    /// Registers entity spawning functions in the `engine` table.
    fn register_spawn_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        // engine.spawn() - Create a new entity builder
        engine.set(
            "spawn",
            self.lua
                .create_function(|_, ()| Ok(LuaEntityBuilder::new()))?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "spawn",
            "Create a new entity builder",
            "spawn",
            &[],
            Some("EntityBuilder"),
        )?;

        // engine.clone(source_key) - Clone an entity from WorldSignals
        // Returns a LuaEntityBuilder that clones the source entity and applies overrides
        engine.set(
            "clone",
            self.lua.create_function(|_, source_key: String| {
                Ok(LuaEntityBuilder::new_clone(source_key))
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "clone",
            "Clone a registered entity with optional overrides",
            "spawn",
            &[("source_key", "string")],
            Some("EntityBuilder"),
        )?;

        Ok(())
    }

    fn register_audio_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "play_music",
            audio_commands,
            |(id, looped)| (String, bool),
            AudioLuaCmd::PlayMusic { id, looped },
            desc = "Play music track",
            cat = "audio",
            params = [("id", "string"), ("looped", "boolean")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "play_sound",
            audio_commands,
            |id| String,
            AudioLuaCmd::PlaySound { id },
            desc = "Play a sound effect",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "play_sound_pitched",
            audio_commands,
            |(id, pitch)| (String, f32),
            AudioLuaCmd::PlaySoundPitched { id, pitch },
            desc = "Play a sound effect with pitch override (1.0 = normal)",
            cat = "audio",
            params = [("id", "string"), ("pitch", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "stop_all_music",
            audio_commands,
            |()| (),
            AudioLuaCmd::StopAllMusic,
            desc = "Stop all playing music",
            cat = "audio",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "stop_all_sounds",
            audio_commands,
            |()| (),
            AudioLuaCmd::StopAllSounds,
            desc = "Stop all playing sounds",
            cat = "audio",
            params = []
        );
        Ok(())
    }

    /// Registers signal read/write functions in the `engine` table.
    fn register_signal_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        // ===== READ functions (from cached snapshot) =====

        // engine.get_scalar(key) -> number or nil
        engine.set(
            "get_scalar",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().scalars.get(&key).copied());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_scalar",
            "Get a world signal scalar value",
            "signal",
            &[("key", "string")],
            Some("number?"),
        )?;

        // engine.get_integer(key) -> integer or nil
        engine.set(
            "get_integer",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().integers.get(&key).copied());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_integer",
            "Get a world signal integer value",
            "signal",
            &[("key", "string")],
            Some("integer?"),
        )?;

        // engine.get_string(key) -> string or nil
        engine.set(
            "get_string",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().strings.get(&key).cloned());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_string",
            "Get a world signal string value",
            "signal",
            &[("key", "string")],
            Some("string?"),
        )?;

        // engine.has_flag(key) -> boolean
        engine.set(
            "has_flag",
            self.lua.create_function(|lua, key: String| {
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.signal_snapshot.borrow().flags.contains(&key))
                    .unwrap_or(false);
                Ok(has)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "has_flag",
            "Check if a world signal flag is set",
            "signal",
            &[("key", "string")],
            Some("boolean"),
        )?;

        // engine.get_group_count(group) -> integer or nil
        engine.set(
            "get_group_count",
            self.lua.create_function(|lua, group: String| {
                let count = lua.app_data_ref::<LuaAppData>().and_then(|data| {
                    data.signal_snapshot
                        .borrow()
                        .group_counts
                        .get(&group)
                        .copied()
                });
                Ok(count)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_group_count",
            "Get the count of entities in a tracked group",
            "signal",
            &[("group", "string")],
            Some("integer?"),
        )?;

        // engine.get_entity(key) -> integer (entity ID) or nil
        // Returns the entity ID as a u64 that can be used with with_stuckto()
        engine.set(
            "get_entity",
            self.lua.create_function(|lua, key: String| {
                let entity_id = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().entities.get(&key).copied());
                Ok(entity_id)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_entity",
            "Get a registered entity ID by key",
            "signal",
            &[("key", "string")],
            Some("integer?"),
        )?;

        // ===== WRITE functions (queue commands) =====

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_scalar",
            signal_commands,
            |(key, value)| (String, f32),
            SignalCmd::SetScalar { key, value },
            desc = "Set a world signal scalar value",
            cat = "signal",
            params = [("key", "string"), ("value", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_integer",
            signal_commands,
            |(key, value)| (String, i32),
            SignalCmd::SetInteger { key, value },
            desc = "Set a world signal integer value",
            cat = "signal",
            params = [("key", "string"), ("value", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_string",
            signal_commands,
            |(key, value)| (String, String),
            SignalCmd::SetString { key, value },
            desc = "Set a world signal string value",
            cat = "signal",
            params = [("key", "string"), ("value", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_flag",
            signal_commands,
            |key| String,
            SignalCmd::SetFlag { key },
            desc = "Set a world signal flag",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_flag",
            signal_commands,
            |key| String,
            SignalCmd::ClearFlag { key },
            desc = "Clear a world signal flag",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_scalar",
            signal_commands,
            |key| String,
            SignalCmd::ClearScalar { key },
            desc = "Clear a world signal scalar",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_integer",
            signal_commands,
            |key| String,
            SignalCmd::ClearInteger { key },
            desc = "Clear a world signal integer",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_string",
            signal_commands,
            |key| String,
            SignalCmd::ClearString { key },
            desc = "Clear a world signal string",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_entity",
            signal_commands,
            |(key, entity_id)| (String, u64),
            SignalCmd::SetEntity { key, entity_id },
            desc = "Register an entity ID in world signals",
            cat = "signal",
            params = [("key", "string"), ("entity_id", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "remove_entity",
            signal_commands,
            |key| String,
            SignalCmd::RemoveEntity { key },
            desc = "Remove a registered entity from world signals",
            cat = "signal",
            params = [("key", "string")]
        );

        Ok(())
    }

    fn register_phase_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "phase_transition",
            phase_commands,
            |(entity_id, phase)| (u64, String),
            PhaseCmd::TransitionTo { entity_id, phase },
            desc = "Transition an entity to a new phase",
            cat = "phase",
            params = [("entity_id", "integer"), ("phase", "string")]
        );
        Ok(())
    }

    fn register_entity_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        define_entity_cmds!(engine, self.lua, meta_fns, "", entity_commands);
        Ok(())
    }

    fn register_group_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "track_group",
            group_commands,
            |name| String,
            GroupCmd::TrackGroup { name },
            desc = "Start tracking a named entity group",
            cat = "group",
            params = [("name", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "untrack_group",
            group_commands,
            |name| String,
            GroupCmd::UntrackGroup { name },
            desc = "Stop tracking a named entity group",
            cat = "group",
            params = [("name", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_tracked_groups",
            group_commands,
            |()| (),
            GroupCmd::ClearTrackedGroups,
            desc = "Stop tracking all entity groups",
            cat = "group",
            params = []
        );

        // Read function (returns value, not push-to-queue)
        engine.set(
            "has_tracked_group",
            self.lua.create_function(|lua, name: String| {
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.tracked_groups.borrow().contains(&name))
                    .unwrap_or(false);
                Ok(has)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "has_tracked_group",
            "Check if a group is being tracked",
            "group",
            &[("name", "string")],
            Some("boolean"),
        )?;

        Ok(())
    }

    fn register_tilemap_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "spawn_tiles",
            tilemap_commands,
            |id| String,
            TilemapCmd::SpawnTiles { id },
            desc = "Spawn tilemap entities from a loaded tilemap",
            cat = "tilemap",
            params = [("id", "string")]
        );
        Ok(())
    }

    fn register_camera_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_camera",
            camera_commands,
            |(target_x, target_y, offset_x, offset_y, rotation, zoom)| (
                f32, f32, f32, f32, f32, f32
            ),
            CameraCmd::SetCamera2D {
                target_x,
                target_y,
                offset_x,
                offset_y,
                rotation,
                zoom
            },
            desc = "Set the 2D camera target, offset, rotation and zoom",
            cat = "camera",
            params = [
                ("target_x", "number"),
                ("target_y", "number"),
                ("offset_x", "number"),
                ("offset_y", "number"),
                ("rotation", "number"),
                ("zoom", "number")
            ]
        );
        Ok(())
    }

    fn register_collision_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        // All entity commands, collision-prefixed
        define_entity_cmds!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_entity_commands
        );

        // Non-entity collision commands
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_play_sound",
            collision_audio_commands,
            |id| String,
            AudioLuaCmd::PlaySound { id },
            desc = "Play a sound effect (collision context)",
            cat = "collision",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_play_sound_pitched",
            collision_audio_commands,
            |(id, pitch)| (String, f32),
            AudioLuaCmd::PlaySoundPitched { id, pitch },
            desc = "Play a sound effect with pitch override (collision context)",
            cat = "collision",
            params = [("id", "string"), ("pitch", "number")]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_scalar",
            collision_signal_commands,
            |(key, value)| (String, f32),
            SignalCmd::SetScalar { key, value },
            desc = "Set a world signal scalar (collision context)",
            cat = "collision",
            params = [("key", "string"), ("value", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_integer",
            collision_signal_commands,
            |(key, value)| (String, i32),
            SignalCmd::SetInteger { key, value },
            desc = "Set a world signal integer (collision context)",
            cat = "collision",
            params = [("key", "string"), ("value", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_string",
            collision_signal_commands,
            |(key, value)| (String, String),
            SignalCmd::SetString { key, value },
            desc = "Set a world signal string (collision context)",
            cat = "collision",
            params = [("key", "string"), ("value", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_flag",
            collision_signal_commands,
            |flag| String,
            SignalCmd::SetFlag { key: flag },
            desc = "Set a world signal flag (collision context)",
            cat = "collision",
            params = [("flag", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_clear_flag",
            collision_signal_commands,
            |flag| String,
            SignalCmd::ClearFlag { key: flag },
            desc = "Clear a world signal flag (collision context)",
            cat = "collision",
            params = [("flag", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_clear_scalar",
            collision_signal_commands,
            |key| String,
            SignalCmd::ClearScalar { key },
            desc = "Clear a world signal scalar (collision context)",
            cat = "collision",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_clear_integer",
            collision_signal_commands,
            |key| String,
            SignalCmd::ClearInteger { key },
            desc = "Clear a world signal integer (collision context)",
            cat = "collision",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_clear_string",
            collision_signal_commands,
            |key| String,
            SignalCmd::ClearString { key },
            desc = "Clear a world signal string (collision context)",
            cat = "collision",
            params = [("key", "string")]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_phase_transition",
            collision_phase_commands,
            |(entity_id, phase)| (u64, String),
            PhaseCmd::TransitionTo { entity_id, phase },
            desc = "Transition an entity to a new phase (collision context)",
            cat = "collision",
            params = [("entity_id", "integer"), ("phase", "string")]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_camera",
            collision_camera_commands,
            |(target_x, target_y, offset_x, offset_y, rotation, zoom)| (
                f32, f32, f32, f32, f32, f32
            ),
            CameraCmd::SetCamera2D {
                target_x,
                target_y,
                offset_x,
                offset_y,
                rotation,
                zoom
            },
            desc = "Set the 2D camera (collision context)",
            cat = "collision",
            params = [
                ("target_x", "number"),
                ("target_y", "number"),
                ("offset_x", "number"),
                ("offset_y", "number"),
                ("rotation", "number"),
                ("zoom", "number")
            ]
        );

        // Spawn/clone (return LuaEntityBuilder, not push-to-queue)
        engine.set(
            "collision_spawn",
            self.lua
                .create_function(|_, ()| Ok(LuaEntityBuilder::new_collision()))?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "collision_spawn",
            "Create a new entity builder (collision context)",
            "collision",
            &[],
            Some("CollisionEntityBuilder"),
        )?;

        engine.set(
            "collision_clone",
            self.lua.create_function(|_, source_key: String| {
                Ok(LuaEntityBuilder::new_collision_clone(source_key))
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "collision_clone",
            "Clone a registered entity (collision context)",
            "collision",
            &[("source_key", "string")],
            Some("CollisionEntityBuilder"),
        )?;

        Ok(())
    }

    fn register_animation_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "register_animation",
            animation_commands,
            |(id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped)| (
                String, String, f32, f32, f32, usize, f32, bool
            ),
            AnimationCmd::RegisterAnimation {
                id,
                tex_key,
                pos_x,
                pos_y,
                displacement,
                frame_count,
                fps,
                looped,
            },
            desc = "Register an animation definition",
            cat = "animation",
            params = [
                ("id", "string"),
                ("tex_key", "string"),
                ("pos_x", "number"),
                ("pos_y", "number"),
                ("displacement", "number"),
                ("frame_count", "integer"),
                ("fps", "number"),
                ("looped", "boolean")
            ]
        );
        Ok(())
    }

    fn register_render_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        // load_shader has validation before push — keep manual
        engine.set(
            "load_shader",
            self.lua.create_function(
                |lua, (id, vs_path, fs_path): (String, Option<String>, Option<String>)| {
                    if vs_path.is_none() && fs_path.is_none() {
                        return Err(LuaError::runtime(
                            "load_shader: at least one of vs_path or fs_path must be provided",
                        ));
                    }
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadShader {
                            id,
                            vs_path,
                            fs_path,
                        });
                    Ok(())
                },
            )?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "load_shader",
            "Load a shader (at least one of vs_path/fs_path required)",
            "render",
            &[
                ("id", "string"),
                ("vs_path", "string?"),
                ("fs_path", "string?"),
            ],
            None,
        )?;

        // post_process_shader has complex table parsing — keep manual
        engine.set(
            "post_process_shader",
            self.lua.create_function(|lua, value: LuaValue| {
                let ids: Option<Vec<String>> = match value {
                    LuaValue::Nil => None,
                    LuaValue::Table(t) => {
                        let mut vec = Vec::new();
                        for pair in t.pairs::<i64, String>() {
                            let (_, id) = pair?;
                            vec.push(id);
                        }
                        if vec.is_empty() {
                            return Err(LuaError::runtime(
                                "post_process_shader: table must contain at least one shader ID",
                            ));
                        }
                        Some(vec)
                    }
                    _ => {
                        return Err(LuaError::runtime(
                            "post_process_shader: expected nil or table of shader IDs",
                        ));
                    }
                };
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .render_commands
                    .borrow_mut()
                    .push(RenderCmd::SetPostProcessShader { ids });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "post_process_shader",
            "Set active post-processing shader chain (nil to clear)",
            "render",
            &[("shader_ids", "string[]?")],
            None,
        )?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_float",
            render_commands,
            |(name, value)| (String, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Float(value)
            },
            desc = "Set a float uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("value", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_int",
            render_commands,
            |(name, value)| (String, i32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Int(value)
            },
            desc = "Set an int uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("value", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_vec2",
            render_commands,
            |(name, x, y)| (String, f32, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Vec2 { x, y }
            },
            desc = "Set a vec2 uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("x", "number"), ("y", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_vec4",
            render_commands,
            |(name, x, y, z, w)| (String, f32, f32, f32, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Vec4 { x, y, z, w }
            },
            desc = "Set a vec4 uniform on post-process shader",
            cat = "render",
            params = [
                ("name", "string"),
                ("x", "number"),
                ("y", "number"),
                ("z", "number"),
                ("w", "number")
            ]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_clear_uniform",
            render_commands,
            |name| String,
            RenderCmd::ClearPostProcessUniform { name },
            desc = "Clear a uniform on post-process shader",
            cat = "render",
            params = [("name", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_clear_uniforms",
            render_commands,
            |()| (),
            RenderCmd::ClearPostProcessUniforms,
            desc = "Clear all uniforms on post-process shader",
            cat = "render",
            params = []
        );

        Ok(())
    }

    /// Drains all queued asset commands.
    ///
    /// Call this from a Rust system after Lua has queued commands via
    /// `engine.load_texture()`, etc. The system can then process them
    /// with access to the necessary resources (RaylibHandle, etc.).
    pub fn drain_asset_commands(&self) -> Vec<AssetCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.asset_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued spawn commands.
    ///
    /// Call this from a Rust system after Lua has queued entity spawns via
    /// `engine.spawn():...:build()`. The system can then process them
    /// with access to ECS Commands.
    pub fn drain_spawn_commands(&self) -> Vec<SpawnCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.spawn_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued audio commands.
    pub fn drain_audio_commands(&self) -> Vec<AudioLuaCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.audio_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued signal commands.
    pub fn drain_signal_commands(&self) -> Vec<SignalCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.signal_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued phase commands.
    pub fn drain_phase_commands(&self) -> Vec<PhaseCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.phase_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued entity commands.
    pub fn drain_entity_commands(&self) -> Vec<EntityCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.entity_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued group commands.
    pub fn drain_group_commands(&self) -> Vec<GroupCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.group_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued tilemap commands.
    pub fn drain_tilemap_commands(&self) -> Vec<TilemapCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.tilemap_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued camera commands.
    pub fn drain_camera_commands(&self) -> Vec<CameraCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.camera_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued animation commands.
    pub fn drain_animation_commands(&self) -> Vec<AnimationCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.animation_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued render commands.
    pub fn drain_render_commands(&self) -> Vec<RenderCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.render_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued collision entity commands.
    /// Call this after processing Lua collision callbacks to apply entity changes.
    pub fn drain_collision_entity_commands(&self) -> Vec<EntityCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_entity_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision signal commands.
    /// Call this after processing Lua collision callbacks to apply signal changes.
    pub fn drain_collision_signal_commands(&self) -> Vec<SignalCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_signal_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision audio commands.
    /// Call this after processing Lua collision callbacks to play sounds.
    pub fn drain_collision_audio_commands(&self) -> Vec<AudioLuaCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_audio_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision spawn commands.
    /// Call this after processing Lua collision callbacks to spawn entities.
    pub fn drain_collision_spawn_commands(&self) -> Vec<SpawnCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_spawn_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued clone commands.
    /// Call this from a Rust system after Lua has queued entity clones via
    /// `engine.clone(source_key):...:build()`. The system can then process them
    /// with access to ECS Commands.
    pub fn drain_clone_commands(&self) -> Vec<CloneCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.clone_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued collision clone commands.
    /// Call this after processing Lua collision callbacks to clone entities.
    pub fn drain_collision_clone_commands(&self) -> Vec<CloneCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_clone_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Clears all command queues without processing them.
    ///
    /// This should be called at the start of scene switches to discard any
    /// stale commands from the previous scene that might reference despawned entities.
    /// Only clears the main command queues, not collision-specific queues.
    pub fn clear_all_commands(&self) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            data.entity_commands.borrow_mut().clear();
            data.spawn_commands.borrow_mut().clear();
            data.clone_commands.borrow_mut().clear();
            data.signal_commands.borrow_mut().clear();
            data.phase_commands.borrow_mut().clear();
            data.audio_commands.borrow_mut().clear();
            data.group_commands.borrow_mut().clear();
            data.camera_commands.borrow_mut().clear();
            data.tilemap_commands.borrow_mut().clear();
            data.render_commands.borrow_mut().clear();
            // Note: Asset and animation commands are only used during setup,
            // so we don't clear them here.
        }
    }

    /// Drains all queued collision phase commands.
    /// Call this after processing Lua collision callbacks to apply phase transitions.
    pub fn drain_collision_phase_commands(&self) -> Vec<PhaseCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_phase_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision camera commands.
    /// Call this after processing Lua collision callbacks to update the camera.
    pub fn drain_collision_camera_commands(&self) -> Vec<CameraCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_camera_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Updates the cached world signal snapshot that Lua can read.
    ///
    /// Call this before invoking Lua callbacks so they have fresh data.
    /// Takes an `Arc<SignalSnapshot>` which is cheaply cloned (just increments refcount).
    pub fn update_signal_cache(&self, snapshot: Arc<SignalSnapshot>) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            *data.signal_snapshot.borrow_mut() = snapshot;
        }
    }

    /// Updates the cached tracked groups that Lua can read.
    /// Call this before invoking Lua callbacks so they have fresh data.
    pub fn update_tracked_groups_cache(&self, groups: &FxHashSet<String>) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            *data.tracked_groups.borrow_mut() = groups.clone();
        }
    }

    /// Creates a Lua table containing the current input state.
    ///
    /// This is called before each Lua callback to create the input table argument.
    /// The table structure is:
    /// ```lua
    /// input = {
    ///     digital = {
    ///         up = { pressed = bool, just_pressed = bool, just_released = bool },
    ///         down = { ... },
    ///         left = { ... },
    ///         right = { ... },
    ///         action_1 = { ... },
    ///         action_2 = { ... },
    ///         back = { ... },
    ///         special = { ... },
    ///     },
    ///     analog = {
    ///         -- Reserved for future gamepad support
    ///     },
    /// }
    /// ```
    pub fn create_input_table(&self, snapshot: &InputSnapshot) -> LuaResult<LuaTable> {
        let lua = &self.lua;

        // Helper to create a button state table
        let create_button_table =
            |state: &super::input_snapshot::DigitalButtonState| -> LuaResult<LuaTable> {
                let table = lua.create_table()?;
                table.set("pressed", state.pressed)?;
                table.set("just_pressed", state.just_pressed)?;
                table.set("just_released", state.just_released)?;
                Ok(table)
            };

        // Create digital inputs table
        let digital = lua.create_table()?;
        digital.set("up", create_button_table(&snapshot.digital.up)?)?;
        digital.set("down", create_button_table(&snapshot.digital.down)?)?;
        digital.set("left", create_button_table(&snapshot.digital.left)?)?;
        digital.set("right", create_button_table(&snapshot.digital.right)?)?;
        digital.set("action_1", create_button_table(&snapshot.digital.action_1)?)?;
        digital.set("action_2", create_button_table(&snapshot.digital.action_2)?)?;
        digital.set("back", create_button_table(&snapshot.digital.back)?)?;
        digital.set("special", create_button_table(&snapshot.digital.special)?)?;

        // Create analog inputs table (empty for now, reserved for future gamepad support)
        let analog = lua.create_table()?;

        // Create root input table
        let input = lua.create_table()?;
        input.set("digital", digital)?;
        input.set("analog", analog)?;

        Ok(input)
    }

    /// Loads and executes a Lua script from a file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Lua script file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the script has syntax/runtime errors.
    pub fn run_script(&self, path: &str) -> LuaResult<()> {
        let script = std::fs::read_to_string(path)
            .map_err(|e| LuaError::ExternalError(std::sync::Arc::new(e)))?;
        self.lua.load(&script).set_name(path).exec()
    }

    /// Calls a global Lua function by name with the given arguments.
    ///
    /// # Type Parameters
    ///
    /// * `A` - Argument types (must implement `IntoLuaMulti`)
    /// * `R` - Return type (must implement `FromLuaMulti`)
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the global function to call
    /// * `args` - Arguments to pass to the function
    ///
    /// # Errors
    ///
    /// Returns an error if the function doesn't exist or execution fails.
    pub fn call_function<A, R>(&self, name: &str, args: A) -> LuaResult<R>
    where
        A: IntoLuaMulti,
        R: FromLuaMulti,
    {
        let func: LuaFunction = self.lua.globals().get(name)?;
        func.call(args)
    }

    /// Checks if a global function exists.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the function to check
    pub fn has_function(&self, name: &str) -> bool {
        self.lua.globals().get::<LuaFunction>(name).is_ok()
    }

    /// Returns a reference to the underlying Lua state.
    ///
    /// Use this for advanced operations like registering custom userdata types.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Registers builder class metadata in `engine.__meta.classes`.
    fn register_builder_meta(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_classes: LuaTable = meta.get("classes")?;

        // Builder method definitions: (name, description, params as &[(&str, &str)])
        let builder_methods: &[(&str, &str, &[(&str, &str)])] = &[
            ("with_group", "Set entity group", &[("name", "string")]),
            (
                "with_position",
                "Set world position",
                &[("x", "number"), ("y", "number")],
            ),
            (
                "with_sprite",
                "Set sprite",
                &[
                    ("tex_key", "string"),
                    ("width", "number"),
                    ("height", "number"),
                    ("origin_x", "number"),
                    ("origin_y", "number"),
                ],
            ),
            (
                "with_sprite_offset",
                "Set sprite offset",
                &[("offset_x", "number"), ("offset_y", "number")],
            ),
            (
                "with_sprite_flip",
                "Set sprite flipping",
                &[("flip_h", "boolean"), ("flip_v", "boolean")],
            ),
            ("with_zindex", "Set render order", &[("z", "number")]),
            (
                "with_velocity",
                "Set velocity (creates RigidBody if needed)",
                &[("vx", "number"), ("vy", "number")],
            ),
            (
                "with_friction",
                "Set friction (creates RigidBody if needed)",
                &[("friction", "number")],
            ),
            (
                "with_max_speed",
                "Set max speed clamp (creates RigidBody if needed)",
                &[("speed", "number")],
            ),
            (
                "with_accel",
                "Add a named acceleration force",
                &[
                    ("name", "string"),
                    ("x", "number"),
                    ("y", "number"),
                    ("enabled", "boolean"),
                ],
            ),
            (
                "with_frozen",
                "Mark entity as frozen (physics skipped)",
                &[],
            ),
            (
                "with_collider",
                "Set box collider",
                &[
                    ("width", "number"),
                    ("height", "number"),
                    ("origin_x", "number"),
                    ("origin_y", "number"),
                ],
            ),
            (
                "with_collider_offset",
                "Set collider offset",
                &[("offset_x", "number"), ("offset_y", "number")],
            ),
            (
                "with_mouse_controlled",
                "Enable mouse position tracking",
                &[("follow_x", "boolean"), ("follow_y", "boolean")],
            ),
            (
                "with_rotation",
                "Set rotation in degrees",
                &[("degrees", "number")],
            ),
            (
                "with_scale",
                "Set scale",
                &[("sx", "number"), ("sy", "number")],
            ),
            ("with_persistent", "Survive scene transitions", &[]),
            (
                "with_signal_scalar",
                "Add a scalar signal",
                &[("key", "string"), ("value", "number")],
            ),
            (
                "with_signal_integer",
                "Add an integer signal",
                &[("key", "string"), ("value", "integer")],
            ),
            (
                "with_signal_flag",
                "Add a flag signal",
                &[("key", "string")],
            ),
            (
                "with_signal_string",
                "Add a string signal",
                &[("key", "string"), ("value", "string")],
            ),
            (
                "with_screen_position",
                "Set screen position (UI elements)",
                &[("x", "number"), ("y", "number")],
            ),
            (
                "with_text",
                "Set DynamicText component",
                &[
                    ("content", "string"),
                    ("font", "string"),
                    ("font_size", "number"),
                    ("r", "integer"),
                    ("g", "integer"),
                    ("b", "integer"),
                    ("a", "integer"),
                ],
            ),
            (
                "with_menu",
                "Add interactive menu",
                &[
                    ("items", "table"),
                    ("origin_x", "number"),
                    ("origin_y", "number"),
                    ("font", "string"),
                    ("font_size", "number"),
                    ("item_spacing", "number"),
                    ("use_screen_space", "boolean"),
                ],
            ),
            (
                "with_menu_colors",
                "Set menu normal/selected colors (RGBA)",
                &[
                    ("nr", "integer"),
                    ("ng", "integer"),
                    ("nb", "integer"),
                    ("na", "integer"),
                    ("sr", "integer"),
                    ("sg", "integer"),
                    ("sb", "integer"),
                    ("sa", "integer"),
                ],
            ),
            (
                "with_menu_dynamic_text",
                "Enable dynamic text updates for menu items",
                &[("dynamic", "boolean")],
            ),
            (
                "with_menu_cursor",
                "Set cursor entity for menu",
                &[("key", "string")],
            ),
            (
                "with_menu_selection_sound",
                "Set sound for menu selection changes",
                &[("sound_key", "string")],
            ),
            (
                "with_menu_action_set_scene",
                "Set scene-switch action for menu item",
                &[("item_id", "string"), ("scene", "string")],
            ),
            (
                "with_menu_action_show_submenu",
                "Set submenu action for menu item",
                &[("item_id", "string"), ("submenu", "string")],
            ),
            (
                "with_menu_action_quit",
                "Set quit action for menu item",
                &[("item_id", "string")],
            ),
            (
                "with_menu_callback",
                "Set Lua callback for menu selection",
                &[("callback", "string")],
            ),
            (
                "with_menu_visible_count",
                "Set max visible menu items (enables scrolling)",
                &[("count", "integer")],
            ),
            ("with_signals", "Add empty Signals component", &[]),
            (
                "with_phase",
                "Add phase state machine\n\nExample:\n```lua\nengine.spawn()\n    :with_phase({\n        initial = \"idle\",\n        phases = {\n            idle = {\n                on_enter = \"on_idle_enter\",\n                on_update = \"on_idle_update\",\n                on_exit = \"on_idle_exit\"\n            },\n            moving = { on_enter = \"on_moving_enter\" }\n        }\n    })\n    :build()\n```",
                &[("table", "table")],
            ),
            (
                "with_stuckto",
                "Attach entity to a target entity",
                &[
                    ("target_entity_id", "integer"),
                    ("follow_x", "boolean"),
                    ("follow_y", "boolean"),
                ],
            ),
            (
                "with_stuckto_offset",
                "Set offset for StuckTo",
                &[("offset_x", "number"), ("offset_y", "number")],
            ),
            (
                "with_stuckto_stored_velocity",
                "Set velocity to restore when unstuck",
                &[("vx", "number"), ("vy", "number")],
            ),
            (
                "with_lua_timer",
                "Add a Lua timer callback",
                &[("duration", "number"), ("callback", "string")],
            ),
            (
                "with_ttl",
                "Set time-to-live (auto-despawn)",
                &[("seconds", "number")],
            ),
            (
                "with_signal_binding",
                "Bind text to a WorldSignal value",
                &[("key", "string")],
            ),
            (
                "with_signal_binding_format",
                "Set format string for signal binding (use {} as placeholder)",
                &[("format", "string")],
            ),
            (
                "with_grid_layout",
                "Spawn entities from a JSON grid layout",
                &[
                    ("path", "string"),
                    ("group", "string"),
                    ("zindex", "number"),
                ],
            ),
            (
                "with_tween_position",
                "Add position tween animation",
                &[
                    ("from_x", "number"),
                    ("from_y", "number"),
                    ("to_x", "number"),
                    ("to_y", "number"),
                    ("duration", "number"),
                ],
            ),
            (
                "with_tween_position_easing",
                "Set easing for position tween",
                &[("easing", "string")],
            ),
            (
                "with_tween_position_loop",
                "Set loop mode for position tween",
                &[("loop_mode", "string")],
            ),
            (
                "with_tween_position_backwards",
                "Start position tween in reverse",
                &[],
            ),
            (
                "with_tween_rotation",
                "Add rotation tween animation",
                &[("from", "number"), ("to", "number"), ("duration", "number")],
            ),
            (
                "with_tween_rotation_easing",
                "Set easing for rotation tween",
                &[("easing", "string")],
            ),
            (
                "with_tween_rotation_loop",
                "Set loop mode for rotation tween",
                &[("loop_mode", "string")],
            ),
            (
                "with_tween_rotation_backwards",
                "Start rotation tween in reverse",
                &[],
            ),
            (
                "with_tween_scale",
                "Add scale tween animation",
                &[
                    ("from_x", "number"),
                    ("from_y", "number"),
                    ("to_x", "number"),
                    ("to_y", "number"),
                    ("duration", "number"),
                ],
            ),
            (
                "with_tween_scale_easing",
                "Set easing for scale tween",
                &[("easing", "string")],
            ),
            (
                "with_tween_scale_loop",
                "Set loop mode for scale tween",
                &[("loop_mode", "string")],
            ),
            (
                "with_tween_scale_backwards",
                "Start scale tween in reverse",
                &[],
            ),
            (
                "with_lua_collision_rule",
                "Add collision callback between two groups",
                &[
                    ("group_a", "string"),
                    ("group_b", "string"),
                    ("callback", "string"),
                ],
            ),
            (
                "with_animation",
                "Set animation by key",
                &[("animation_key", "string")],
            ),
            (
                "with_animation_controller",
                "Add animation controller with fallback",
                &[("fallback_key", "string")],
            ),
            (
                "with_animation_rule",
                "Add animation rule to controller",
                &[("condition_table", "table"), ("set_key", "string")],
            ),
            (
                "with_particle_emitter",
                "Add particle emitter",
                &[("table", "table")],
            ),
            (
                "with_tint",
                "Set color tint (RGBA 0-255)",
                &[
                    ("r", "integer"),
                    ("g", "integer"),
                    ("b", "integer"),
                    ("a", "integer"),
                ],
            ),
            (
                "with_shader",
                "Set per-entity shader with optional uniforms",
                &[("shader_key", "string"), ("uniforms", "table?")],
            ),
            (
                "register_as",
                "Register entity in WorldSignals for later retrieval",
                &[("key", "string")],
            ),
            ("build", "Queue entity for spawning or cloning", &[]),
        ];

        // Schema references for complex table params: (method_name, param_name, schema_type)
        let schema_refs: &[(&str, &str, &str)] = &[
            ("with_phase", "table", "PhaseDefinition"),
            ("with_particle_emitter", "table", "ParticleEmitterConfig"),
            (
                "with_animation_rule",
                "condition_table",
                "AnimationRuleCondition",
            ),
            ("with_menu", "items", "MenuItem[]"),
        ];

        // Generate metadata for both EntityBuilder and CollisionEntityBuilder
        for class_name in &["EntityBuilder", "CollisionEntityBuilder"] {
            let class_tbl = self.lua.create_table()?;
            class_tbl.set(
                "description",
                format!(
                    "Fluent builder for entity construction ({})",
                    if *class_name == "EntityBuilder" {
                        "regular context"
                    } else {
                        "collision context"
                    }
                ),
            )?;

            let methods_tbl = self.lua.create_table()?;
            for (name, desc, params) in builder_methods {
                let method_tbl = self.lua.create_table()?;
                method_tbl.set("description", *desc)?;
                let params_tbl = self.lua.create_table()?;
                for (i, (pname, ptype)) in params.iter().enumerate() {
                    let p = self.lua.create_table()?;
                    p.set("name", *pname)?;
                    p.set("type", *ptype)?;
                    // Add schema reference if this param has one
                    for (method, param, schema) in schema_refs {
                        if *method == *name && *param == *pname {
                            p.set("schema", *schema)?;
                        }
                    }
                    params_tbl.set(i + 1, p)?;
                }
                method_tbl.set("params", params_tbl)?;
                // All with_* methods return Self, build() returns nil
                if *name != "build" {
                    let ret = self.lua.create_table()?;
                    ret.set("type", *class_name)?;
                    method_tbl.set("returns", ret)?;
                }
                methods_tbl.set(*name, method_tbl)?;
            }
            class_tbl.set("methods", methods_tbl)?;
            meta_classes.set(*class_name, class_tbl)?;
        }

        Ok(())
    }

    /// Registers type shape definitions in `engine.__meta.types`.
    fn register_types_meta(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_types: LuaTable = meta.get("types")?;

        // Type definitions: (name, description, fields as &[(name, type, optional, description?)])
        let type_defs: &[(&str, &str, &[(&str, &str, bool, Option<&str>)])] = &[
            (
                "Vector2",
                "2D vector / point",
                &[("x", "number", false, None), ("y", "number", false, None)],
            ),
            (
                "Rect",
                "Axis-aligned rectangle",
                &[
                    ("x", "number", false, None),
                    ("y", "number", false, None),
                    ("w", "number", false, None),
                    ("h", "number", false, None),
                ],
            ),
            (
                "SpriteInfo",
                "Sprite state snapshot",
                &[
                    ("tex_key", "string", false, None),
                    ("flip_h", "boolean", false, None),
                    ("flip_v", "boolean", false, None),
                ],
            ),
            (
                "AnimationInfo",
                "Animation state snapshot",
                &[
                    ("key", "string", false, None),
                    ("frame_index", "integer", false, None),
                    ("elapsed", "number", false, None),
                ],
            ),
            (
                "TimerInfo",
                "Lua timer state snapshot",
                &[
                    ("duration", "number", false, None),
                    ("elapsed", "number", false, None),
                    ("callback", "string", false, None),
                ],
            ),
            (
                "SignalSet",
                "Entity signal snapshot",
                &[
                    ("flags", "string[]", false, None),
                    ("integers", "{[string]: integer}", false, None),
                    ("scalars", "{[string]: number}", false, None),
                    ("strings", "{[string]: string}", false, None),
                ],
            ),
            (
                "EntityContext",
                "Entity state passed to phase/timer callbacks",
                &[
                    ("id", "integer", false, Some("Entity ID")),
                    ("group", "string", true, None),
                    ("pos", "Vector2", true, None),
                    ("screen_pos", "Vector2", true, None),
                    ("vel", "Vector2", true, None),
                    ("speed_sq", "number", true, None),
                    ("frozen", "boolean", true, None),
                    ("rotation", "number", true, None),
                    ("scale", "Vector2", true, None),
                    ("rect", "Rect", true, None),
                    ("sprite", "SpriteInfo", true, None),
                    ("animation", "AnimationInfo", true, None),
                    ("signals", "SignalSet", true, None),
                    ("phase", "string", true, None),
                    ("time_in_phase", "number", true, None),
                    ("previous_phase", "string", true, Some("Only in on_enter")),
                    ("timer", "TimerInfo", true, None),
                ],
            ),
            (
                "CollisionEntity",
                "Entity data in a collision context",
                &[
                    ("id", "integer", false, Some("Entity ID")),
                    ("group", "string", false, None),
                    ("pos", "Vector2", false, None),
                    ("vel", "Vector2", false, None),
                    ("speed_sq", "number", false, None),
                    ("rect", "Rect", false, None),
                    ("signals", "SignalSet", false, None),
                ],
            ),
            (
                "CollisionSides",
                "Collision contact sides",
                &[
                    ("a", "string[]", false, Some("Sides of entity A in contact")),
                    ("b", "string[]", false, Some("Sides of entity B in contact")),
                ],
            ),
            (
                "CollisionContext",
                "Context passed to collision callbacks",
                &[
                    ("a", "CollisionEntity", false, None),
                    ("b", "CollisionEntity", false, None),
                    ("sides", "CollisionSides", false, None),
                ],
            ),
            (
                "DigitalButtonState",
                "State of a single digital button",
                &[
                    ("pressed", "boolean", false, None),
                    ("just_pressed", "boolean", false, None),
                    ("just_released", "boolean", false, None),
                ],
            ),
            (
                "DigitalInputs",
                "All digital button states",
                &[
                    ("up", "DigitalButtonState", false, None),
                    ("down", "DigitalButtonState", false, None),
                    ("left", "DigitalButtonState", false, None),
                    ("right", "DigitalButtonState", false, None),
                    ("action_1", "DigitalButtonState", false, None),
                    ("action_2", "DigitalButtonState", false, None),
                    ("back", "DigitalButtonState", false, None),
                    ("special", "DigitalButtonState", false, None),
                ],
            ),
            (
                "InputSnapshot",
                "Input state passed to callbacks",
                &[
                    ("digital", "DigitalInputs", false, None),
                    (
                        "analog",
                        "table",
                        false,
                        Some("Reserved for future gamepad support"),
                    ),
                ],
            ),
            (
                "PhaseCallbacks",
                "Callbacks for a single phase",
                &[
                    (
                        "on_enter",
                        "string",
                        true,
                        Some("Function name called on phase enter"),
                    ),
                    (
                        "on_update",
                        "string",
                        true,
                        Some("Function name called each frame"),
                    ),
                    (
                        "on_exit",
                        "string",
                        true,
                        Some("Function name called on phase exit"),
                    ),
                ],
            ),
            (
                "PhaseDefinition",
                "Phase state machine definition",
                &[
                    ("initial", "string", false, Some("Initial phase name")),
                    (
                        "phases",
                        "{[string]: PhaseCallbacks}",
                        false,
                        Some("Map of phase name to callbacks"),
                    ),
                ],
            ),
            (
                "ParticleEmitterConfig",
                "Particle emitter configuration table",
                &[
                    (
                        "templates",
                        "string[]",
                        false,
                        Some("Entity template keys to emit"),
                    ),
                    (
                        "shape",
                        "string|table",
                        true,
                        Some("Emitter shape: 'point' or table {type='rect', width, height}"),
                    ),
                    (
                        "offset",
                        "table",
                        true,
                        Some("{x, y} offset from entity position"),
                    ),
                    ("particles_per_emission", "integer", true, None),
                    ("emissions_per_second", "number", true, None),
                    (
                        "emissions_remaining",
                        "integer",
                        true,
                        Some("nil = infinite"),
                    ),
                    ("arc", "table", true, Some("{min, max} angle in degrees")),
                    ("speed", "table", true, Some("{min, max} or single number")),
                    (
                        "ttl",
                        "number|table",
                        true,
                        Some("{min, max}, number, or 'none'"),
                    ),
                ],
            ),
            (
                "MenuItem",
                "Menu item definition",
                &[
                    ("id", "string", false, None),
                    ("label", "string", false, None),
                ],
            ),
            (
                "AnimationRuleCondition",
                "Animation rule condition (polymorphic)",
                &[
                    (
                        "type",
                        "string",
                        false,
                        Some(
                            "Condition type: has_flag, lacks_flag, scalar_cmp, scalar_range, integer_cmp, integer_range, all, any, not",
                        ),
                    ),
                    (
                        "key",
                        "string",
                        true,
                        Some("Signal key (for flag/scalar/integer conditions)"),
                    ),
                    (
                        "op",
                        "string",
                        true,
                        Some("Comparison operator (for cmp conditions)"),
                    ),
                    (
                        "value",
                        "number",
                        true,
                        Some("Comparison value (for cmp conditions)"),
                    ),
                    (
                        "min",
                        "number",
                        true,
                        Some("Range minimum (for range conditions)"),
                    ),
                    (
                        "max",
                        "number",
                        true,
                        Some("Range maximum (for range conditions)"),
                    ),
                    (
                        "conditions",
                        "AnimationRuleCondition[]",
                        true,
                        Some("Sub-conditions (for all/any/not)"),
                    ),
                ],
            ),
        ];

        for (name, description, fields_def) in type_defs {
            let tbl = self.lua.create_table()?;
            tbl.set("description", *description)?;
            let fields = self.lua.create_table()?;
            for (i, (fname, ftype, optional, fdesc)) in fields_def.iter().enumerate() {
                push_type_field(&self.lua, &fields, i, fname, ftype, *optional, *fdesc)?;
            }
            tbl.set("fields", fields)?;
            meta_types.set(*name, tbl)?;
        }

        Ok(())
    }

    /// Registers enum value sets in `engine.__meta.enums`.
    fn register_enums_meta(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_enums: LuaTable = meta.get("enums")?;

        let enum_defs: &[(&str, &str, &[&str])] = &[
            (
                "Easing",
                "Tween easing function",
                &[
                    "linear",
                    "quad_in",
                    "quad_out",
                    "quad_in_out",
                    "cubic_in",
                    "cubic_out",
                    "cubic_in_out",
                ],
            ),
            (
                "LoopMode",
                "Tween loop mode",
                &["once", "loop", "ping_pong"],
            ),
            (
                "BoxSide",
                "Collision side",
                &["left", "right", "top", "bottom"],
            ),
            (
                "ComparisonOp",
                "Comparison operator for animation rules",
                &["lt", "le", "gt", "ge", "eq", "ne"],
            ),
            (
                "ConditionType",
                "Animation rule condition type",
                &[
                    "has_flag",
                    "lacks_flag",
                    "scalar_cmp",
                    "scalar_range",
                    "integer_cmp",
                    "integer_range",
                    "all",
                    "any",
                    "not",
                ],
            ),
            (
                "EmitterShape",
                "Particle emitter shape type",
                &["point", "rect"],
            ),
            (
                "TtlSpec",
                "Time-to-live specification (number, {min,max} table, or 'none')",
                &["none"],
            ),
            (
                "Category",
                "Function category",
                &[
                    "base",
                    "asset",
                    "spawn",
                    "audio",
                    "signal",
                    "phase",
                    "entity",
                    "group",
                    "tilemap",
                    "camera",
                    "collision",
                    "animation",
                    "render",
                ],
            ),
        ];

        for (name, description, values) in enum_defs {
            let tbl = self.lua.create_table()?;
            tbl.set("description", *description)?;
            let vals = self.lua.create_table()?;
            for (i, val) in values.iter().enumerate() {
                vals.set(i + 1, *val)?;
            }
            tbl.set("values", vals)?;
            meta_enums.set(*name, tbl)?;
        }

        Ok(())
    }

    /// Registers well-known callback signatures in `engine.__meta.callbacks`.
    fn register_callbacks_meta(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_callbacks: LuaTable = meta.get("callbacks")?;

        // Callback definitions: (name, description, params, returns?, context?, note?)
        struct CbDef {
            name: &'static str,
            description: &'static str,
            params: &'static [(&'static str, &'static str)],
            returns: Option<&'static str>,
            context: Option<&'static str>,
            note: Option<&'static str>,
        }

        let callback_defs: &[CbDef] = &[
            CbDef {
                name: "on_setup",
                description: "Called once during game setup for asset loading",
                params: &[],
                returns: None,
                context: Some("setup"),
                note: None,
            },
            CbDef {
                name: "on_enter_play",
                description: "Called when entering Playing state",
                params: &[],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "on_switch_scene",
                description: "Called when switching scenes",
                params: &[("scene", "string")],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "on_update_<scene>",
                description: "Called each frame during a scene",
                params: &[("input", "InputSnapshot"), ("dt", "number")],
                returns: None,
                context: Some("play"),
                note: Some("Function name is dynamic: on_update_ + scene name"),
            },
            CbDef {
                name: "phase_on_enter",
                description: "Called when entering a phase",
                params: &[("ctx", "EntityContext"), ("input", "InputSnapshot")],
                returns: Some("string?"),
                context: Some("play"),
                note: Some("Return phase name to trigger transition"),
            },
            CbDef {
                name: "phase_on_update",
                description: "Called each frame during a phase",
                params: &[
                    ("ctx", "EntityContext"),
                    ("input", "InputSnapshot"),
                    ("dt", "number"),
                ],
                returns: Some("string?"),
                context: Some("play"),
                note: Some("Return phase name to trigger transition"),
            },
            CbDef {
                name: "phase_on_exit",
                description: "Called when exiting a phase",
                params: &[("ctx", "EntityContext")],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "timer_callback",
                description: "Called when a Lua timer fires",
                params: &[("ctx", "EntityContext"), ("input", "InputSnapshot")],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "collision_callback",
                description: "Called when two colliding groups overlap",
                params: &[("ctx", "CollisionContext")],
                returns: None,
                context: Some("play"),
                note: None,
            },
            CbDef {
                name: "menu_callback",
                description: "Called when a menu item is selected",
                params: &[
                    ("menu_id", "integer"),
                    ("item_id", "string"),
                    ("item_index", "integer"),
                ],
                returns: None,
                context: Some("play"),
                note: None,
            },
        ];

        for cb in callback_defs {
            let tbl = self.lua.create_table()?;
            tbl.set("description", cb.description)?;

            let params_tbl = self.lua.create_table()?;
            for (i, (pname, ptype)) in cb.params.iter().enumerate() {
                let p = self.lua.create_table()?;
                p.set("name", *pname)?;
                p.set("type", *ptype)?;
                params_tbl.set(i + 1, p)?;
            }
            tbl.set("params", params_tbl)?;

            if let Some(ret) = cb.returns {
                let r = self.lua.create_table()?;
                r.set("type", ret)?;
                tbl.set("returns", r)?;
            }
            if let Some(ctx) = cb.context {
                tbl.set("context", ctx)?;
            }
            if let Some(note) = cb.note {
                tbl.set("note", note)?;
            }

            meta_callbacks.set(cb.name, tbl)?;
        }

        Ok(())
    }
}

/// Helper to push a type field entry to a fields table.
fn push_type_field(
    lua: &Lua,
    fields: &LuaTable,
    index: usize,
    name: &str,
    typ: &str,
    optional: bool,
    description: Option<&str>,
) -> LuaResult<()> {
    let f = lua.create_table()?;
    f.set("name", name)?;
    f.set("type", typ)?;
    f.set("optional", optional)?;
    if let Some(desc) = description {
        f.set("description", desc)?;
    }
    fields.set(index + 1, f)?;
    Ok(())
}

impl Default for LuaRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create Lua runtime")
    }
}
