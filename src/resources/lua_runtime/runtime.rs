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

use log::{info, warn, error};

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

/// Registers a Lua function that pushes a command to a queue in `LuaAppData`.
macro_rules! register_cmd {
    ($engine:expr, $lua:expr, $name:expr, $queue:ident,
     |$args:pat_param| $arg_ty:ty, $cmd:expr) => {
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
    };
}

/// Registers a batch of entity commands with a name prefix to a specific queue.
macro_rules! register_entity_cmds {
    ($engine:expr, $lua:expr, $prefix:literal, $queue:ident, [
        $( ($name:literal, |$args:pat_param| $arg_ty:ty, $cmd:expr) ),* $(,)?
    ]) => {
        $(
            register_cmd!($engine, $lua, concat!($prefix, $name), $queue,
                |$args| $arg_ty, $cmd);
        )*
    };
}

/// Defines and registers all entity commands for a given prefix and queue.
/// Called with `""` prefix for regular commands, `"collision_"` for collision commands.
macro_rules! define_entity_cmds {
    ($engine:expr, $lua:expr, $prefix:literal, $queue:ident) => {
        register_entity_cmds!($engine, $lua, $prefix, $queue, [
            ("entity_despawn", |entity_id| u64, EntityCmd::Despawn { entity_id }),
            ("entity_menu_despawn", |entity_id| u64, EntityCmd::MenuDespawn { entity_id }),
            ("release_stuckto", |entity_id| u64, EntityCmd::ReleaseStuckTo { entity_id }),
            ("entity_signal_set_flag",
                |(entity_id, flag)| (u64, String), EntityCmd::SignalSetFlag { entity_id, flag }),
            ("entity_signal_clear_flag",
                |(entity_id, flag)| (u64, String), EntityCmd::SignalClearFlag { entity_id, flag }),
            ("entity_set_velocity",
                |(entity_id, vx, vy)| (u64, f32, f32), EntityCmd::SetVelocity { entity_id, vx, vy }),
            ("entity_insert_stuckto",
                |(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)|
                (u64, u64, bool, bool, f32, f32, f32, f32),
                EntityCmd::InsertStuckTo {
                    entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy,
                }),
            ("entity_restart_animation", |entity_id| u64, EntityCmd::RestartAnimation { entity_id }),
            ("entity_set_animation",
                |(entity_id, animation_key)| (u64, String), EntityCmd::SetAnimation { entity_id, animation_key }),
            ("entity_insert_lua_timer",
                |(entity_id, duration, callback)| (u64, f32, String),
                EntityCmd::InsertLuaTimer { entity_id, duration, callback }),
            ("entity_remove_lua_timer", |entity_id| u64, EntityCmd::RemoveLuaTimer { entity_id }),
            ("entity_insert_ttl",
                |(entity_id, seconds)| (u64, f32), EntityCmd::InsertTtl { entity_id, seconds }),
            ("entity_insert_tween_position",
                |(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenPosition {
                    entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards,
                }),
            ("entity_insert_tween_rotation",
                |(entity_id, from, to, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenRotation {
                    entity_id, from, to, duration, easing, loop_mode, backwards,
                }),
            ("entity_insert_tween_scale",
                |(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenScale {
                    entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards,
                }),
            ("entity_remove_tween_position", |entity_id| u64, EntityCmd::RemoveTweenPosition { entity_id }),
            ("entity_remove_tween_rotation", |entity_id| u64, EntityCmd::RemoveTweenRotation { entity_id }),
            ("entity_remove_tween_scale", |entity_id| u64, EntityCmd::RemoveTweenScale { entity_id }),
            ("entity_set_rotation",
                |(entity_id, degrees)| (u64, f32), EntityCmd::SetRotation { entity_id, degrees }),
            ("entity_set_scale",
                |(entity_id, sx, sy)| (u64, f32, f32), EntityCmd::SetScale { entity_id, sx, sy }),
            ("entity_signal_set_scalar",
                |(entity_id, key, value)| (u64, String, f32),
                EntityCmd::SignalSetScalar { entity_id, key, value }),
            ("entity_signal_set_string",
                |(entity_id, key, value)| (u64, String, String),
                EntityCmd::SignalSetString { entity_id, key, value }),
            ("entity_add_force",
                |(entity_id, name, x, y, enabled)| (u64, String, f32, f32, bool),
                EntityCmd::AddForce { entity_id, name, x, y, enabled }),
            ("entity_remove_force",
                |(entity_id, name)| (u64, String), EntityCmd::RemoveForce { entity_id, name }),
            ("entity_set_force_enabled",
                |(entity_id, name, enabled)| (u64, String, bool),
                EntityCmd::SetForceEnabled { entity_id, name, enabled }),
            ("entity_set_force_value",
                |(entity_id, name, x, y)| (u64, String, f32, f32),
                EntityCmd::SetForceValue { entity_id, name, x, y }),
            ("entity_set_friction",
                |(entity_id, friction)| (u64, f32), EntityCmd::SetFriction { entity_id, friction }),
            ("entity_set_max_speed",
                |(entity_id, max_speed)| (u64, Option<f32>), EntityCmd::SetMaxSpeed { entity_id, max_speed }),
            ("entity_freeze", |entity_id| u64, EntityCmd::FreezeEntity { entity_id }),
            ("entity_unfreeze", |entity_id| u64, EntityCmd::UnfreezeEntity { entity_id }),
            ("entity_set_speed",
                |(entity_id, speed)| (u64, f32), EntityCmd::SetSpeed { entity_id, speed }),
            ("entity_set_position",
                |(entity_id, x, y)| (u64, f32, f32), EntityCmd::SetPosition { entity_id, x, y }),
            ("entity_signal_set_integer",
                |(entity_id, key, value)| (u64, String, i32),
                EntityCmd::SignalSetInteger { entity_id, key, value }),
            ("entity_set_shader",
                |(entity_id, key)| (u64, String), EntityCmd::SetShader { entity_id, key }),
            ("entity_remove_shader", |entity_id| u64, EntityCmd::RemoveShader { entity_id }),
            ("entity_set_tint",
                |(entity_id, r, g, b, a)| (u64, u8, u8, u8, u8),
                EntityCmd::SetTint { entity_id, r, g, b, a }),
            ("entity_remove_tint", |entity_id| u64, EntityCmd::RemoveTint { entity_id }),
            ("entity_shader_set_float",
                |(entity_id, name, value)| (u64, String, f32),
                EntityCmd::ShaderSetFloat { entity_id, name, value }),
            ("entity_shader_set_int",
                |(entity_id, name, value)| (u64, String, i32),
                EntityCmd::ShaderSetInt { entity_id, name, value }),
            ("entity_shader_set_vec2",
                |(entity_id, name, x, y)| (u64, String, f32, f32),
                EntityCmd::ShaderSetVec2 { entity_id, name, x, y }),
            ("entity_shader_set_vec4",
                |(entity_id, name, x, y, z, w)| (u64, String, f32, f32, f32, f32),
                EntityCmd::ShaderSetVec4 { entity_id, name, x, y, z, w }),
            ("entity_shader_clear_uniform",
                |(entity_id, name)| (u64, String), EntityCmd::ShaderClearUniform { entity_id, name }),
            ("entity_shader_clear_uniforms", |entity_id| u64, EntityCmd::ShaderClearUniforms { entity_id }),
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

        // engine.log(message) - General purpose logging
        engine.set(
            "log",
            self.lua.create_function(|_, msg: String| {
                info!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_info(message) - Info level logging
        engine.set(
            "log_info",
            self.lua.create_function(|_, msg: String| {
                info!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_warn(message) - Warning level logging
        engine.set(
            "log_warn",
            self.lua.create_function(|_, msg: String| {
                warn!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_error(message) - Error level logging
        engine.set(
            "log_error",
            self.lua.create_function(|_, msg: String| {
                error!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;

        self.lua.globals().set("engine", engine)?;

        Ok(())
    }

    fn register_asset_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        register_cmd!(engine, self.lua, "load_texture", asset_commands,
            |(id, path)| (String, String), AssetCmd::LoadTexture { id, path });
        register_cmd!(engine, self.lua, "load_font", asset_commands,
            |(id, path, size)| (String, String, i32), AssetCmd::LoadFont { id, path, size });
        register_cmd!(engine, self.lua, "load_music", asset_commands,
            |(id, path)| (String, String), AssetCmd::LoadMusic { id, path });
        register_cmd!(engine, self.lua, "load_sound", asset_commands,
            |(id, path)| (String, String), AssetCmd::LoadSound { id, path });
        register_cmd!(engine, self.lua, "load_tilemap", asset_commands,
            |(id, path)| (String, String), AssetCmd::LoadTilemap { id, path });
        Ok(())
    }

    /// Registers entity spawning functions in the `engine` table.
    fn register_spawn_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.spawn() - Create a new entity builder
        engine.set(
            "spawn",
            self.lua
                .create_function(|_, ()| Ok(LuaEntityBuilder::new()))?,
        )?;

        // engine.clone(source_key) - Clone an entity from WorldSignals
        // Returns a LuaEntityBuilder that clones the source entity and applies overrides
        engine.set(
            "clone",
            self.lua.create_function(|_, source_key: String| {
                Ok(LuaEntityBuilder::new_clone(source_key))
            })?,
        )?;

        Ok(())
    }

    fn register_audio_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        register_cmd!(engine, self.lua, "play_music", audio_commands,
            |(id, looped)| (String, bool), AudioLuaCmd::PlayMusic { id, looped });
        register_cmd!(engine, self.lua, "play_sound", audio_commands,
            |id| String, AudioLuaCmd::PlaySound { id });
        register_cmd!(engine, self.lua, "stop_all_music", audio_commands,
            |()| (), AudioLuaCmd::StopAllMusic);
        register_cmd!(engine, self.lua, "stop_all_sounds", audio_commands,
            |()| (), AudioLuaCmd::StopAllSounds);
        Ok(())
    }

    /// Registers signal read/write functions in the `engine` table.
    fn register_signal_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

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

        // ===== WRITE functions (queue commands) =====

        register_cmd!(engine, self.lua, "set_scalar", signal_commands,
            |(key, value)| (String, f32), SignalCmd::SetScalar { key, value });
        register_cmd!(engine, self.lua, "set_integer", signal_commands,
            |(key, value)| (String, i32), SignalCmd::SetInteger { key, value });
        register_cmd!(engine, self.lua, "set_string", signal_commands,
            |(key, value)| (String, String), SignalCmd::SetString { key, value });
        register_cmd!(engine, self.lua, "set_flag", signal_commands,
            |key| String, SignalCmd::SetFlag { key });
        register_cmd!(engine, self.lua, "clear_flag", signal_commands,
            |key| String, SignalCmd::ClearFlag { key });
        register_cmd!(engine, self.lua, "clear_scalar", signal_commands,
            |key| String, SignalCmd::ClearScalar { key });
        register_cmd!(engine, self.lua, "clear_integer", signal_commands,
            |key| String, SignalCmd::ClearInteger { key });
        register_cmd!(engine, self.lua, "clear_string", signal_commands,
            |key| String, SignalCmd::ClearString { key });
        register_cmd!(engine, self.lua, "set_entity", signal_commands,
            |(key, entity_id)| (String, u64), SignalCmd::SetEntity { key, entity_id });
        register_cmd!(engine, self.lua, "remove_entity", signal_commands,
            |key| String, SignalCmd::RemoveEntity { key });

        Ok(())
    }

    fn register_phase_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        register_cmd!(engine, self.lua, "phase_transition", phase_commands,
            |(entity_id, phase)| (u64, String), PhaseCmd::TransitionTo { entity_id, phase });
        Ok(())
    }

    fn register_entity_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        define_entity_cmds!(engine, self.lua, "", entity_commands);
        Ok(())
    }

    fn register_group_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        register_cmd!(engine, self.lua, "track_group", group_commands,
            |name| String, GroupCmd::TrackGroup { name });
        register_cmd!(engine, self.lua, "untrack_group", group_commands,
            |name| String, GroupCmd::UntrackGroup { name });
        register_cmd!(engine, self.lua, "clear_tracked_groups", group_commands,
            |()| (), GroupCmd::ClearTrackedGroups);

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

        Ok(())
    }

    fn register_tilemap_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        register_cmd!(engine, self.lua, "spawn_tiles", tilemap_commands,
            |id| String, TilemapCmd::SpawnTiles { id });
        Ok(())
    }

    fn register_camera_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        register_cmd!(engine, self.lua, "set_camera", camera_commands,
            |(target_x, target_y, offset_x, offset_y, rotation, zoom)| (f32, f32, f32, f32, f32, f32),
            CameraCmd::SetCamera2D { target_x, target_y, offset_x, offset_y, rotation, zoom });
        Ok(())
    }

    fn register_collision_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // All entity commands, collision-prefixed
        define_entity_cmds!(engine, self.lua, "collision_", collision_entity_commands);

        // Non-entity collision commands
        register_cmd!(engine, self.lua, "collision_play_sound", collision_audio_commands,
            |id| String, AudioLuaCmd::PlaySound { id });

        register_cmd!(engine, self.lua, "collision_set_scalar", collision_signal_commands,
            |(key, value)| (String, f32), SignalCmd::SetScalar { key, value });
        register_cmd!(engine, self.lua, "collision_set_integer", collision_signal_commands,
            |(key, value)| (String, i32), SignalCmd::SetInteger { key, value });
        register_cmd!(engine, self.lua, "collision_set_string", collision_signal_commands,
            |(key, value)| (String, String), SignalCmd::SetString { key, value });
        register_cmd!(engine, self.lua, "collision_set_flag", collision_signal_commands,
            |flag| String, SignalCmd::SetFlag { key: flag });
        register_cmd!(engine, self.lua, "collision_clear_flag", collision_signal_commands,
            |flag| String, SignalCmd::ClearFlag { key: flag });
        register_cmd!(engine, self.lua, "collision_clear_scalar", collision_signal_commands,
            |key| String, SignalCmd::ClearScalar { key });
        register_cmd!(engine, self.lua, "collision_clear_integer", collision_signal_commands,
            |key| String, SignalCmd::ClearInteger { key });
        register_cmd!(engine, self.lua, "collision_clear_string", collision_signal_commands,
            |key| String, SignalCmd::ClearString { key });

        register_cmd!(engine, self.lua, "collision_phase_transition", collision_phase_commands,
            |(entity_id, phase)| (u64, String), PhaseCmd::TransitionTo { entity_id, phase });

        register_cmd!(engine, self.lua, "collision_set_camera", collision_camera_commands,
            |(target_x, target_y, offset_x, offset_y, rotation, zoom)| (f32, f32, f32, f32, f32, f32),
            CameraCmd::SetCamera2D { target_x, target_y, offset_x, offset_y, rotation, zoom });

        // Spawn/clone (return LuaEntityBuilder, not push-to-queue)
        engine.set(
            "collision_spawn",
            self.lua
                .create_function(|_, ()| Ok(LuaEntityBuilder::new_collision()))?,
        )?;
        engine.set(
            "collision_clone",
            self.lua.create_function(|_, source_key: String| {
                Ok(LuaEntityBuilder::new_collision_clone(source_key))
            })?,
        )?;

        Ok(())
    }

    fn register_animation_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        register_cmd!(engine, self.lua, "register_animation", animation_commands,
            |(id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped)|
            (String, String, f32, f32, f32, usize, f32, bool),
            AnimationCmd::RegisterAnimation {
                id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped,
            });
        Ok(())
    }

    fn register_render_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // load_shader has validation before push — keep manual
        engine.set(
            "load_shader",
            self.lua
                .create_function(|lua, (id, vs_path, fs_path): (String, Option<String>, Option<String>)| {
                    if vs_path.is_none() && fs_path.is_none() {
                        return Err(LuaError::runtime(
                            "load_shader: at least one of vs_path or fs_path must be provided",
                        ));
                    }
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadShader { id, vs_path, fs_path });
                    Ok(())
                })?,
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
                        ))
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

        register_cmd!(engine, self.lua, "post_process_set_float", render_commands,
            |(name, value)| (String, f32),
            RenderCmd::SetPostProcessUniform { name, value: UniformValue::Float(value) });
        register_cmd!(engine, self.lua, "post_process_set_int", render_commands,
            |(name, value)| (String, i32),
            RenderCmd::SetPostProcessUniform { name, value: UniformValue::Int(value) });
        register_cmd!(engine, self.lua, "post_process_set_vec2", render_commands,
            |(name, x, y)| (String, f32, f32),
            RenderCmd::SetPostProcessUniform { name, value: UniformValue::Vec2 { x, y } });
        register_cmd!(engine, self.lua, "post_process_set_vec4", render_commands,
            |(name, x, y, z, w)| (String, f32, f32, f32, f32),
            RenderCmd::SetPostProcessUniform { name, value: UniformValue::Vec4 { x, y, z, w } });
        register_cmd!(engine, self.lua, "post_process_clear_uniform", render_commands,
            |name| String, RenderCmd::ClearPostProcessUniform { name });
        register_cmd!(engine, self.lua, "post_process_clear_uniforms", render_commands,
            |()| (), RenderCmd::ClearPostProcessUniforms);

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
}

impl Default for LuaRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create Lua runtime")
    }
}
