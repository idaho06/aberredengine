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
                eprintln!("[Lua] {}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_info(message) - Info level logging
        engine.set(
            "log_info",
            self.lua.create_function(|_, msg: String| {
                eprintln!("[Lua INFO] {}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_warn(message) - Warning level logging
        engine.set(
            "log_warn",
            self.lua.create_function(|_, msg: String| {
                eprintln!("[Lua WARN] {}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_error(message) - Error level logging
        engine.set(
            "log_error",
            self.lua.create_function(|_, msg: String| {
                eprintln!("[Lua ERROR] {}", msg);
                Ok(())
            })?,
        )?;

        self.lua.globals().set("engine", engine)?;

        Ok(())
    }

    /// Registers asset loading functions in the `engine` table.
    fn register_asset_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.load_texture(id, path) - Queue texture loading
        engine.set(
            "load_texture",
            self.lua
                .create_function(|lua, (id, path): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadTexture { id, path });
                    Ok(())
                })?,
        )?;

        // engine.load_font(id, path, size) - Queue font loading
        engine.set(
            "load_font",
            self.lua
                .create_function(|lua, (id, path, size): (String, String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadFont { id, path, size });
                    Ok(())
                })?,
        )?;

        // engine.load_music(id, path) - Queue music loading
        engine.set(
            "load_music",
            self.lua
                .create_function(|lua, (id, path): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadMusic { id, path });
                    Ok(())
                })?,
        )?;

        // engine.load_sound(id, path) - Queue sound effect loading
        engine.set(
            "load_sound",
            self.lua
                .create_function(|lua, (id, path): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadSound { id, path });
                    Ok(())
                })?,
        )?;

        // engine.load_tilemap(id, path) - Queue tilemap loading
        engine.set(
            "load_tilemap",
            self.lua
                .create_function(|lua, (id, path): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadTilemap { id, path });
                    Ok(())
                })?,
        )?;

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
            self.lua
                .create_function(|_, source_key: String| Ok(LuaEntityBuilder::new_clone(source_key)))?,
        )?;

        Ok(())
    }

    /// Registers audio functions in the `engine` table.
    fn register_audio_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.play_music(id, looped) - Queue music playback
        engine.set(
            "play_music",
            self.lua
                .create_function(|lua, (id, looped): (String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .audio_commands
                        .borrow_mut()
                        .push(AudioLuaCmd::PlayMusic { id, looped });
                    Ok(())
                })?,
        )?;

        // engine.play_sound(id) - Queue sound effect playback
        engine.set(
            "play_sound",
            self.lua.create_function(|lua, id: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .audio_commands
                    .borrow_mut()
                    .push(AudioLuaCmd::PlaySound { id });
                Ok(())
            })?,
        )?;

        // engine.stop_all_music() - Stop all music
        engine.set(
            "stop_all_music",
            self.lua.create_function(|lua, ()| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .audio_commands
                    .borrow_mut()
                    .push(AudioLuaCmd::StopAllMusic);
                Ok(())
            })?,
        )?;

        // engine.stop_all_sounds() - Stop all sound effects
        engine.set(
            "stop_all_sounds",
            self.lua.create_function(|lua, ()| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .audio_commands
                    .borrow_mut()
                    .push(AudioLuaCmd::StopAllSounds);
                Ok(())
            })?,
        )?;

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

        // engine.set_scalar(key, value)
        engine.set(
            "set_scalar",
            self.lua
                .create_function(|lua, (key, value): (String, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetScalar { key, value });
                    Ok(())
                })?,
        )?;

        // engine.set_integer(key, value)
        engine.set(
            "set_integer",
            self.lua
                .create_function(|lua, (key, value): (String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetInteger { key, value });
                    Ok(())
                })?,
        )?;

        // engine.set_string(key, value)
        engine.set(
            "set_string",
            self.lua
                .create_function(|lua, (key, value): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetString { key, value });
                    Ok(())
                })?,
        )?;

        // engine.set_flag(key)
        engine.set(
            "set_flag",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .signal_commands
                    .borrow_mut()
                    .push(SignalCmd::SetFlag { key });
                Ok(())
            })?,
        )?;

        // engine.clear_flag(key)
        engine.set(
            "clear_flag",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearFlag { key });
                Ok(())
            })?,
        )?;

        // engine.clear_scalar(key)
        engine.set(
            "clear_scalar",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearScalar { key });
                Ok(())
            })?,
        )?;

        // engine.clear_integer(key)
        engine.set(
            "clear_integer",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearInteger { key });
                Ok(())
            })?,
        )?;

        // engine.clear_string(key)
        engine.set(
            "clear_string",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearString { key });
                Ok(())
            })?,
        )?;

        // engine.set_entity(key, entity_id)
        engine.set(
            "set_entity",
            self.lua
                .create_function(|lua, (key, entity_id): (String, u64)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetEntity { key, entity_id });
                    Ok(())
                })?,
        )?;

        // engine.remove_entity(key)
        engine.set(
            "remove_entity",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .signal_commands
                    .borrow_mut()
                    .push(SignalCmd::RemoveEntity { key });
                Ok(())
            })?,
        )?;

        Ok(())
    }

    /// Registers phase transition functions in the `engine` table.
    fn register_phase_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.phase_transition(entity_id, phase) - Request phase transition for specific entity
        engine.set(
            "phase_transition",
            self.lua
                .create_function(|lua, (entity_id, phase): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .phase_commands
                        .borrow_mut()
                        .push(PhaseCmd::TransitionTo { entity_id, phase });
                    Ok(())
                })?,
        )?;

        Ok(())
    }

    /// Registers entity manipulation functions in the `engine` table.
    fn register_entity_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.entity_despawn(entity_id) - Despawn an entity
        engine.set(
            "entity_despawn",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::Despawn { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_menu_despawn(entity_id) - Despawn a menu and its items/cursor/textures
        engine.set(
            "entity_menu_despawn",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::MenuDespawn { entity_id });
                Ok(())
            })?,
        )?;

        // engine.release_stuckto(entity_id) - Release entity from StuckTo, restore velocity
        engine.set(
            "release_stuckto",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::ReleaseStuckTo { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_signal_set_flag(entity_id, flag) - Set a flag on entity's Signals
        engine.set(
            "entity_signal_set_flag",
            self.lua
                .create_function(|lua, (entity_id, flag): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalSetFlag { entity_id, flag });
                    Ok(())
                })?,
        )?;

        // engine.entity_signal_clear_flag(entity_id, flag) - Clear a flag on entity's Signals
        engine.set(
            "entity_signal_clear_flag",
            self.lua
                .create_function(|lua, (entity_id, flag): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalClearFlag { entity_id, flag });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_velocity(entity_id, vx, vy) - Set entity velocity
        engine.set(
            "entity_set_velocity",
            self.lua
                .create_function(|lua, (entity_id, vx, vy): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetVelocity { entity_id, vx, vy });
                    Ok(())
                })?,
        )?;

        // engine.entity_insert_stuckto(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)
        // Insert a StuckTo component on an entity
        engine.set(
            "entity_insert_stuckto",
            self.lua.create_function(
                |lua,
                 (
                    entity_id,
                    target_id,
                    follow_x,
                    follow_y,
                    offset_x,
                    offset_y,
                    stored_vx,
                    stored_vy,
                ): (u64, u64, bool, bool, f32, f32, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertStuckTo {
                            entity_id,
                            target_id,
                            follow_x,
                            follow_y,
                            offset_x,
                            offset_y,
                            stored_vx,
                            stored_vy,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.entity_restart_animation(entity_id) - Restart entity's current animation from frame 0
        engine.set(
            "entity_restart_animation",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RestartAnimation { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_set_animation(entity_id, animation_key) - Set entity's animation to a specific key
        engine.set(
            "entity_set_animation",
            self.lua
                .create_function(|lua, (entity_id, animation_key): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetAnimation {
                            entity_id,
                            animation_key,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_insert_lua_timer(entity_id, duration, callback) - Insert a LuaTimer component
        engine.set(
            "entity_insert_lua_timer",
            self.lua.create_function(
                |lua, (entity_id, duration, callback): (u64, f32, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertLuaTimer {
                            entity_id,
                            duration,
                            callback,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.entity_remove_lua_timer(entity_id) - Remove a LuaTimer component
        engine.set(
            "entity_remove_lua_timer",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RemoveLuaTimer { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_insert_ttl(entity_id, seconds) - Insert a Ttl component
        engine.set(
            "entity_insert_ttl",
            self.lua.create_function(|lua, (entity_id, seconds): (u64, f32)| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::InsertTtl { entity_id, seconds });
                Ok(())
            })?,
        )?;

        // engine.entity_insert_tween_position(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)
        engine.set(
            "entity_insert_tween_position",
            self.lua.create_function(
                |lua,
                 (
                    entity_id,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    duration,
                    easing,
                    loop_mode,
                    backwards,
                ): (u64, f32, f32, f32, f32, f32, String, String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertTweenPosition {
                            entity_id,
                            from_x,
                            from_y,
                            to_x,
                            to_y,
                            duration,
                            easing,
                            loop_mode,
                            backwards,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.entity_insert_tween_rotation(entity_id, from, to, duration, easing, loop_mode, backwards)
        engine.set(
            "entity_insert_tween_rotation",
            self.lua.create_function(
                |lua,
                 (entity_id, from, to, duration, easing, loop_mode, backwards): (
                    u64,
                    f32,
                    f32,
                    f32,
                    String,
                    String,
                    bool,
                )| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertTweenRotation {
                            entity_id,
                            from,
                            to,
                            duration,
                            easing,
                            loop_mode,
                            backwards,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.entity_insert_tween_scale(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)
        engine.set(
            "entity_insert_tween_scale",
            self.lua.create_function(
                |lua,
                 (
                    entity_id,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    duration,
                    easing,
                    loop_mode,
                    backwards,
                ): (u64, f32, f32, f32, f32, f32, String, String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertTweenScale {
                            entity_id,
                            from_x,
                            from_y,
                            to_x,
                            to_y,
                            duration,
                            easing,
                            loop_mode,
                            backwards,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.entity_remove_tween_position(entity_id)
        engine.set(
            "entity_remove_tween_position",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RemoveTweenPosition { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_remove_tween_rotation(entity_id)
        engine.set(
            "entity_remove_tween_rotation",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RemoveTweenRotation { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_remove_tween_scale(entity_id)
        engine.set(
            "entity_remove_tween_scale",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RemoveTweenScale { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_set_rotation(entity_id, degrees) - Set entity rotation
        engine.set(
            "entity_set_rotation",
            self.lua
                .create_function(|lua, (entity_id, degrees): (u64, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetRotation { entity_id, degrees });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_scale(entity_id, sx, sy) - Set entity scale
        engine.set(
            "entity_set_scale",
            self.lua
                .create_function(|lua, (entity_id, sx, sy): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetScale { entity_id, sx, sy });
                    Ok(())
                })?,
        )?;

        // engine.entity_signal_set_scalar(entity_id, key, value) - Set scalar signal on entity
        engine.set(
            "entity_signal_set_scalar",
            self.lua
                .create_function(|lua, (entity_id, key, value): (u64, String, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalSetScalar {
                            entity_id,
                            key,
                            value,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_signal_set_string(entity_id, key, value) - Set string signal on entity
        engine.set(
            "entity_signal_set_string",
            self.lua
                .create_function(|lua, (entity_id, key, value): (u64, String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalSetString {
                            entity_id,
                            key,
                            value,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_add_force(entity_id, name, x, y, enabled) - Add/update a named force on RigidBody
        engine.set(
            "entity_add_force",
            self.lua.create_function(
                |lua, (entity_id, name, x, y, enabled): (u64, String, f32, f32, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::AddForce {
                            entity_id,
                            name,
                            x,
                            y,
                            enabled,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.entity_remove_force(entity_id, name) - Remove a named force from RigidBody
        engine.set(
            "entity_remove_force",
            self.lua
                .create_function(|lua, (entity_id, name): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::RemoveForce { entity_id, name });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_force_enabled(entity_id, name, enabled) - Enable/disable a force
        engine.set(
            "entity_set_force_enabled",
            self.lua
                .create_function(|lua, (entity_id, name, enabled): (u64, String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetForceEnabled {
                            entity_id,
                            name,
                            enabled,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_force_value(entity_id, name, x, y) - Update force value
        engine.set(
            "entity_set_force_value",
            self.lua
                .create_function(|lua, (entity_id, name, x, y): (u64, String, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetForceValue {
                            entity_id,
                            name,
                            x,
                            y,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_friction(entity_id, friction) - Set RigidBody friction
        engine.set(
            "entity_set_friction",
            self.lua
                .create_function(|lua, (entity_id, friction): (u64, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetFriction {
                            entity_id,
                            friction,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_max_speed(entity_id, max_speed) - Set RigidBody max_speed (nil to remove limit)
        engine.set(
            "entity_set_max_speed",
            self.lua
                .create_function(|lua, (entity_id, max_speed): (u64, Option<f32>)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetMaxSpeed {
                            entity_id,
                            max_speed,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_freeze(entity_id) - Freeze entity (skip physics calculations)
        engine.set(
            "entity_freeze",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::FreezeEntity { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_unfreeze(entity_id) - Unfreeze entity (resume physics calculations)
        engine.set(
            "entity_unfreeze",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::UnfreezeEntity { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_set_speed(entity_id, speed) - Set speed while maintaining velocity direction
        engine.set(
            "entity_set_speed",
            self.lua
                .create_function(|lua, (entity_id, speed): (u64, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetSpeed { entity_id, speed });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_position(entity_id, x, y) - Set entity position
        engine.set(
            "entity_set_position",
            self.lua
                .create_function(|lua, (entity_id, x, y): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetPosition { entity_id, x, y });
                    Ok(())
                })?,
        )?;

        // engine.entity_signal_set_integer(entity_id, key, value) - Set integer signal on entity
        engine.set(
            "entity_signal_set_integer",
            self.lua
                .create_function(|lua, (entity_id, key, value): (u64, String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalSetInteger {
                            entity_id,
                            key,
                            value,
                        });
                    Ok(())
                })?,
        )?;

        Ok(())
    }

    /// Registers tracked group functions in the `engine` table.
    fn register_group_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.track_group(name) - Register a group for entity counting
        engine.set(
            "track_group",
            self.lua.create_function(|lua, name: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .group_commands
                    .borrow_mut()
                    .push(GroupCmd::TrackGroup { name });
                Ok(())
            })?,
        )?;

        // engine.untrack_group(name) - Stop tracking a group
        engine.set(
            "untrack_group",
            self.lua.create_function(|lua, name: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .group_commands
                    .borrow_mut()
                    .push(GroupCmd::UntrackGroup { name });
                Ok(())
            })?,
        )?;

        // engine.clear_tracked_groups() - Clear all tracked groups
        engine.set(
            "clear_tracked_groups",
            self.lua.create_function(|lua, ()| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .group_commands
                    .borrow_mut()
                    .push(GroupCmd::ClearTrackedGroups);
                Ok(())
            })?,
        )?;

        // engine.has_tracked_group(name) -> boolean
        // Check if a group is being tracked (reads from cached data)
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

    /// Registers tilemap functions in the `engine` table.
    fn register_tilemap_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.spawn_tiles(id) - Queue tile spawning from a loaded tilemap
        engine.set(
            "spawn_tiles",
            self.lua.create_function(|lua, id: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .tilemap_commands
                    .borrow_mut()
                    .push(TilemapCmd::SpawnTiles { id });
                Ok(())
            })?,
        )?;

        Ok(())
    }

    /// Registers camera functions in the `engine` table.
    fn register_camera_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.set_camera(target_x, target_y, offset_x, offset_y, rotation, zoom)
        // Set the 2D camera parameters
        engine.set(
            "set_camera",
            self.lua.create_function(
                |lua,
                 (target_x, target_y, offset_x, offset_y, rotation, zoom): (
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                )| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .camera_commands
                        .borrow_mut()
                        .push(CameraCmd::SetCamera2D {
                            target_x,
                            target_y,
                            offset_x,
                            offset_y,
                            rotation,
                            zoom,
                        });
                    Ok(())
                },
            )?,
        )?;

        Ok(())
    }

    fn register_collision_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.collision_entity_set_position(entity_id, x, y)
        // Sets the position of an entity during collision handling
        engine.set(
            "collision_entity_set_position",
            self.lua
                .create_function(|lua, (entity_id, x, y): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetPosition { entity_id, x, y });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_set_velocity(entity_id, vx, vy)
        // Sets the velocity of an entity during collision handling
        engine.set(
            "collision_entity_set_velocity",
            self.lua
                .create_function(|lua, (entity_id, vx, vy): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetVelocity { entity_id, vx, vy });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_despawn(entity_id)
        // Despawns an entity during collision handling
        engine.set(
            "collision_entity_despawn",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::Despawn { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_signal_set_integer(entity_id, key, value)
        // Sets an integer signal on an entity during collision handling
        engine.set(
            "collision_entity_signal_set_integer",
            self.lua
                .create_function(|lua, (entity_id, key, value): (u64, String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalSetInteger {
                            entity_id,
                            key,
                            value,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_signal_set_flag(entity_id, flag)
        // Sets a flag signal on an entity during collision handling
        engine.set(
            "collision_entity_signal_set_flag",
            self.lua
                .create_function(|lua, (entity_id, flag): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalSetFlag { entity_id, flag });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_signal_clear_flag(entity_id, flag)
        // Clears a flag signal on an entity during collision handling
        engine.set(
            "collision_entity_signal_clear_flag",
            self.lua
                .create_function(|lua, (entity_id, flag): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalClearFlag { entity_id, flag });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_insert_stuckto(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)
        // Inserts a StuckTo component on an entity during collision handling
        engine.set(
            "collision_entity_insert_stuckto",
            self.lua.create_function(
                |lua,
                 (
                    entity_id,
                    target_id,
                    follow_x,
                    follow_y,
                    offset_x,
                    offset_y,
                    stored_vx,
                    stored_vy,
                ): (u64, u64, bool, bool, f32, f32, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertStuckTo {
                            entity_id,
                            target_id,
                            follow_x,
                            follow_y,
                            offset_x,
                            offset_y,
                            stored_vx,
                            stored_vy,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_release_stuckto(entity_id)
        // Release entity from StuckTo during collision handling
        engine.set(
            "collision_release_stuckto",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::ReleaseStuckTo { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_signal_set_scalar(entity_id, key, value)
        // Sets a scalar signal on an entity during collision handling
        engine.set(
            "collision_entity_signal_set_scalar",
            self.lua
                .create_function(|lua, (entity_id, key, value): (u64, String, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalSetScalar {
                            entity_id,
                            key,
                            value,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_signal_set_string(entity_id, key, value)
        // Sets a string signal on an entity during collision handling
        engine.set(
            "collision_entity_signal_set_string",
            self.lua
                .create_function(|lua, (entity_id, key, value): (u64, String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SignalSetString {
                            entity_id,
                            key,
                            value,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_insert_lua_timer(entity_id, duration, callback)
        // Inserts a LuaTimer component on an entity during collision handling
        engine.set(
            "collision_entity_insert_lua_timer",
            self.lua.create_function(
                |lua, (entity_id, duration, callback): (u64, f32, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertLuaTimer {
                            entity_id,
                            duration,
                            callback,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_entity_remove_lua_timer(entity_id)
        // Removes a LuaTimer component during collision handling
        engine.set(
            "collision_entity_remove_lua_timer",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RemoveLuaTimer { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_insert_ttl(entity_id, seconds)
        // Insert a Ttl component during collision handling
        engine.set(
            "collision_entity_insert_ttl",
            self.lua.create_function(|lua, (entity_id, seconds): (u64, f32)| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::InsertTtl { entity_id, seconds });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_restart_animation(entity_id)
        // Restarts entity animation during collision handling
        engine.set(
            "collision_entity_restart_animation",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RestartAnimation { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_set_animation(entity_id, animation_key)
        // Sets entity animation during collision handling
        engine.set(
            "collision_entity_set_animation",
            self.lua
                .create_function(|lua, (entity_id, animation_key): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetAnimation {
                            entity_id,
                            animation_key,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_insert_tween_position(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)
        // Inserts TweenPosition during collision handling
        engine.set(
            "collision_entity_insert_tween_position",
            self.lua.create_function(
                |lua,
                 (
                    entity_id,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    duration,
                    easing,
                    loop_mode,
                    backwards,
                ): (u64, f32, f32, f32, f32, f32, String, String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertTweenPosition {
                            entity_id,
                            from_x,
                            from_y,
                            to_x,
                            to_y,
                            duration,
                            easing,
                            loop_mode,
                            backwards,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_entity_insert_tween_rotation(entity_id, from, to, duration, easing, loop_mode, backwards)
        // Inserts TweenRotation during collision handling
        engine.set(
            "collision_entity_insert_tween_rotation",
            self.lua.create_function(
                |lua,
                 (entity_id, from, to, duration, easing, loop_mode, backwards): (
                    u64,
                    f32,
                    f32,
                    f32,
                    String,
                    String,
                    bool,
                )| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertTweenRotation {
                            entity_id,
                            from,
                            to,
                            duration,
                            easing,
                            loop_mode,
                            backwards,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_entity_insert_tween_scale(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)
        // Inserts TweenScale during collision handling
        engine.set(
            "collision_entity_insert_tween_scale",
            self.lua.create_function(
                |lua,
                 (
                    entity_id,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    duration,
                    easing,
                    loop_mode,
                    backwards,
                ): (u64, f32, f32, f32, f32, f32, String, String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::InsertTweenScale {
                            entity_id,
                            from_x,
                            from_y,
                            to_x,
                            to_y,
                            duration,
                            easing,
                            loop_mode,
                            backwards,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_entity_remove_tween_position(entity_id)
        // Removes TweenPosition during collision handling
        engine.set(
            "collision_entity_remove_tween_position",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RemoveTweenPosition { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_remove_tween_rotation(entity_id)
        // Removes TweenRotation during collision handling
        engine.set(
            "collision_entity_remove_tween_rotation",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RemoveTweenRotation { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_remove_tween_scale(entity_id)
        // Removes TweenScale during collision handling
        engine.set(
            "collision_entity_remove_tween_scale",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::RemoveTweenScale { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_set_rotation(entity_id, degrees)
        // Sets entity rotation during collision handling
        engine.set(
            "collision_entity_set_rotation",
            self.lua
                .create_function(|lua, (entity_id, degrees): (u64, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetRotation { entity_id, degrees });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_set_scale(entity_id, sx, sy)
        // Sets entity scale during collision handling
        engine.set(
            "collision_entity_set_scale",
            self.lua
                .create_function(|lua, (entity_id, sx, sy): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetScale { entity_id, sx, sy });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_add_force(entity_id, name, x, y, enabled)
        // Adds a force to entity during collision handling
        engine.set(
            "collision_entity_add_force",
            self.lua.create_function(
                |lua, (entity_id, name, x, y, enabled): (u64, String, f32, f32, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::AddForce {
                            entity_id,
                            name,
                            x,
                            y,
                            enabled,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_entity_remove_force(entity_id, name)
        // Removes a force from entity during collision handling
        engine.set(
            "collision_entity_remove_force",
            self.lua
                .create_function(|lua, (entity_id, name): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::RemoveForce { entity_id, name });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_set_force_enabled(entity_id, name, enabled)
        // Enables/disables a force during collision handling
        engine.set(
            "collision_entity_set_force_enabled",
            self.lua
                .create_function(|lua, (entity_id, name, enabled): (u64, String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetForceEnabled {
                            entity_id,
                            name,
                            enabled,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_set_force_value(entity_id, name, x, y)
        // Updates force value during collision handling
        engine.set(
            "collision_entity_set_force_value",
            self.lua
                .create_function(|lua, (entity_id, name, x, y): (u64, String, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetForceValue {
                            entity_id,
                            name,
                            x,
                            y,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_set_friction(entity_id, friction)
        // Sets entity friction during collision handling
        engine.set(
            "collision_entity_set_friction",
            self.lua
                .create_function(|lua, (entity_id, friction): (u64, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetFriction {
                            entity_id,
                            friction,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_set_max_speed(entity_id, max_speed)
        // Sets entity max speed during collision handling (nil to remove limit)
        engine.set(
            "collision_entity_set_max_speed",
            self.lua
                .create_function(|lua, (entity_id, max_speed): (u64, Option<f32>)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetMaxSpeed {
                            entity_id,
                            max_speed,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_freeze(entity_id)
        // Freezes entity during collision handling
        engine.set(
            "collision_entity_freeze",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::FreezeEntity { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_unfreeze(entity_id)
        // Unfreezes entity during collision handling
        engine.set(
            "collision_entity_unfreeze",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::UnfreezeEntity { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_set_speed(entity_id, speed)
        // Sets entity speed during collision handling
        engine.set(
            "collision_entity_set_speed",
            self.lua
                .create_function(|lua, (entity_id, speed): (u64, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetSpeed { entity_id, speed });
                    Ok(())
                })?,
        )?;

        // engine.collision_play_sound(sound_name)
        // Plays a sound effect during collision handling
        engine.set(
            "collision_play_sound",
            self.lua.create_function(|lua, id: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_audio_commands
                    .borrow_mut()
                    .push(AudioLuaCmd::PlaySound { id });
                Ok(())
            })?,
        )?;

        // engine.collision_set_integer(key, value)
        // Sets a global integer signal during collision handling
        engine.set(
            "collision_set_integer",
            self.lua
                .create_function(|lua, (key, value): (String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetInteger { key, value });
                    Ok(())
                })?,
        )?;

        // engine.collision_set_flag(flag)
        // Sets a global flag signal during collision handling
        engine.set(
            "collision_set_flag",
            self.lua.create_function(|lua, flag: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_signal_commands
                    .borrow_mut()
                    .push(SignalCmd::SetFlag { key: flag });
                Ok(())
            })?,
        )?;

        // engine.collision_clear_flag(flag)
        // Clears a global flag signal during collision handling
        engine.set(
            "collision_clear_flag",
            self.lua.create_function(|lua, flag: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearFlag { key: flag });
                Ok(())
            })?,
        )?;

        // engine.collision_set_scalar(key, value)
        // Sets a global scalar signal during collision handling
        engine.set(
            "collision_set_scalar",
            self.lua
                .create_function(|lua, (key, value): (String, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetScalar { key, value });
                    Ok(())
                })?,
        )?;

        // engine.collision_set_string(key, value)
        // Sets a global string signal during collision handling
        engine.set(
            "collision_set_string",
            self.lua
                .create_function(|lua, (key, value): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetString { key, value });
                    Ok(())
                })?,
        )?;

        // engine.collision_clear_scalar(key)
        // Clears a global scalar signal during collision handling
        engine.set(
            "collision_clear_scalar",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearScalar { key });
                Ok(())
            })?,
        )?;

        // engine.collision_clear_integer(key)
        // Clears a global integer signal during collision handling
        engine.set(
            "collision_clear_integer",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearInteger { key });
                Ok(())
            })?,
        )?;

        // engine.collision_clear_string(key)
        // Clears a global string signal during collision handling
        engine.set(
            "collision_clear_string",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearString { key });
                Ok(())
            })?,
        )?;

        // engine.collision_spawn() - Create a new entity builder for collision context
        // Returns a LuaEntityBuilder that queues spawns for processing after collision
        engine.set(
            "collision_spawn",
            self.lua
                .create_function(|_, ()| Ok(LuaEntityBuilder::new_collision()))?,
        )?;

        // engine.collision_clone(source_key) - Clone an entity in collision context
        // Returns a LuaEntityBuilder that clones the source entity and applies overrides
        engine.set(
            "collision_clone",
            self.lua
                .create_function(|_, source_key: String| Ok(LuaEntityBuilder::new_collision_clone(source_key)))?,
        )?;

        // engine.collision_phase_transition(entity_id, phase)
        // Request a phase transition for an entity during collision handling
        engine.set(
            "collision_phase_transition",
            self.lua
                .create_function(|lua, (entity_id, phase): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_phase_commands
                        .borrow_mut()
                        .push(PhaseCmd::TransitionTo { entity_id, phase });
                    Ok(())
                })?,
        )?;

        // engine.collision_set_camera(target_x, target_y, offset_x, offset_y, rotation, zoom)
        // Set the camera during collision handling (for camera shake, zoom effects, etc.)
        engine.set(
            "collision_set_camera",
            self.lua.create_function(
                |lua,
                 (target_x, target_y, offset_x, offset_y, rotation, zoom): (
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                )| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_camera_commands
                        .borrow_mut()
                        .push(CameraCmd::SetCamera2D {
                            target_x,
                            target_y,
                            offset_x,
                            offset_y,
                            rotation,
                            zoom,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_entity_freeze(entity_id) - Freeze entity during collision handling
        engine.set(
            "collision_entity_freeze",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::FreezeEntity { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_unfreeze(entity_id) - Unfreeze entity during collision handling
        engine.set(
            "collision_entity_unfreeze",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(EntityCmd::UnfreezeEntity { entity_id });
                Ok(())
            })?,
        )?;

        // engine.collision_entity_add_force(entity_id, name, x, y, enabled) - Add/update force during collision
        engine.set(
            "collision_entity_add_force",
            self.lua.create_function(
                |lua, (entity_id, name, x, y, enabled): (u64, String, f32, f32, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::AddForce {
                            entity_id,
                            name,
                            x,
                            y,
                            enabled,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_entity_set_force_enabled(entity_id, name, enabled) - Enable/disable force during collision
        engine.set(
            "collision_entity_set_force_enabled",
            self.lua
                .create_function(|lua, (entity_id, name, enabled): (u64, String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetForceEnabled {
                            entity_id,
                            name,
                            enabled,
                        });
                    Ok(())
                })?,
        )?;

        // engine.collision_entity_set_speed(entity_id, speed) - Set speed while maintaining velocity direction during collision
        engine.set(
            "collision_entity_set_speed",
            self.lua
                .create_function(|lua, (entity_id, speed): (u64, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(EntityCmd::SetSpeed { entity_id, speed });
                    Ok(())
                })?,
        )?;

        Ok(())
    }

    /// Registers animation functions in the `engine` table.
    fn register_animation_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.register_animation(id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped)
        // Registers an animation resource in the AnimationStore
        engine.set(
            "register_animation",
            self.lua.create_function(
                |lua,
                 (id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped): (
                    String,
                    String,
                    f32,
                    f32,
                    f32,
                    usize,
                    f32,
                    bool,
                )| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .animation_commands
                        .borrow_mut()
                        .push(AnimationCmd::RegisterAnimation {
                            id,
                            tex_key,
                            pos_x,
                            pos_y,
                            displacement,
                            frame_count,
                            fps,
                            looped,
                        });
                    Ok(())
                },
            )?,
        )?;

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
