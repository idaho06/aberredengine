//! Lua runtime core implementation.
//!
//! This module contains the `LuaRuntime` struct which manages the Lua interpreter
//! and provides the `engine` table API to Lua scripts.

use super::commands::*;
use super::entity_builder::LuaEntityBuilder;
use super::spawn_data::*;
use mlua::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::RefCell;

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
    // Collision-scoped command queues (processed immediately after each collision callback)
    collision_entity_commands: RefCell<Vec<CollisionEntityCmd>>,
    collision_signal_commands: RefCell<Vec<SignalCmd>>,
    collision_audio_commands: RefCell<Vec<AudioLuaCmd>>,
    /// Cached world signal values (read-only snapshot for Lua)
    /// These are updated before calling Lua callbacks
    signal_scalars: RefCell<FxHashMap<String, f32>>,
    signal_integers: RefCell<FxHashMap<String, i32>>,
    signal_strings: RefCell<FxHashMap<String, String>>,
    signal_flags: RefCell<FxHashSet<String>>,
    group_counts: RefCell<FxHashMap<String, u32>>,
    /// Cached entity IDs (as u64 from Entity::to_bits())
    signal_entities: RefCell<FxHashMap<String, u64>>,
    /// Cached tracked group names (read-only snapshot for Lua)
    tracked_groups: RefCell<FxHashSet<String>>,
}

/// Resource holding the Lua interpreter state.
///
/// This is a `NonSend` resource because the Lua state is not thread-safe.
/// It should be initialized once at startup and reused throughout the game.
pub struct LuaRuntime {
    lua: Lua,
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
            collision_entity_commands: RefCell::new(Vec::new()),
            collision_signal_commands: RefCell::new(Vec::new()),
            collision_audio_commands: RefCell::new(Vec::new()),
            signal_scalars: RefCell::new(FxHashMap::default()),
            signal_integers: RefCell::new(FxHashMap::default()),
            signal_strings: RefCell::new(FxHashMap::default()),
            signal_flags: RefCell::new(FxHashSet::default()),
            group_counts: RefCell::new(FxHashMap::default()),
            signal_entities: RefCell::new(FxHashMap::default()),
            tracked_groups: RefCell::new(FxHashSet::default()),
        });

        let runtime = Self { lua };
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
                    .and_then(|data| data.signal_scalars.borrow().get(&key).copied());
                Ok(value)
            })?,
        )?;

        // engine.get_integer(key) -> integer or nil
        engine.set(
            "get_integer",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_integers.borrow().get(&key).copied());
                Ok(value)
            })?,
        )?;

        // engine.get_string(key) -> string or nil
        engine.set(
            "get_string",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_strings.borrow().get(&key).cloned());
                Ok(value)
            })?,
        )?;

        // engine.has_flag(key) -> boolean
        engine.set(
            "has_flag",
            self.lua.create_function(|lua, key: String| {
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.signal_flags.borrow().contains(&key))
                    .unwrap_or(false);
                Ok(has)
            })?,
        )?;

        // engine.get_group_count(group) -> integer or nil
        engine.set(
            "get_group_count",
            self.lua.create_function(|lua, group: String| {
                let count = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.group_counts.borrow().get(&group).copied());
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
                    .and_then(|data| data.signal_entities.borrow().get(&key).copied());
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

        // engine.entity_set_position(entity_id, x, y)
        // Sets the position of an entity during collision handling
        engine.set(
            "entity_set_position",
            self.lua
                .create_function(|lua, (entity_id, x, y): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SetPosition { entity_id, x, y });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_velocity(entity_id, vx, vy)
        // Sets the velocity of an entity during collision handling
        engine.set(
            "entity_set_velocity",
            self.lua
                .create_function(|lua, (entity_id, vx, vy): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SetVelocity { entity_id, vx, vy });
                    Ok(())
                })?,
        )?;

        // engine.entity_despawn(entity_id)
        // Despawns an entity during collision handling
        engine.set(
            "entity_despawn",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(CollisionEntityCmd::Despawn { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_signal_set_integer(entity_id, key, value)
        // Sets an integer signal on an entity during collision handling
        engine.set(
            "entity_signal_set_integer",
            self.lua
                .create_function(|lua, (entity_id, key, value): (u64, String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SignalSetInteger {
                            entity_id,
                            key,
                            value,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_signal_set_flag(entity_id, flag)
        // Sets a flag signal on an entity during collision handling
        engine.set(
            "entity_signal_set_flag",
            self.lua
                .create_function(|lua, (entity_id, flag): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SignalSetFlag { entity_id, flag });
                    Ok(())
                })?,
        )?;

        // engine.entity_signal_clear_flag(entity_id, flag)
        // Clears a flag signal on an entity during collision handling
        engine.set(
            "entity_signal_clear_flag",
            self.lua
                .create_function(|lua, (entity_id, flag): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SignalClearFlag { entity_id, flag });
                    Ok(())
                })?,
        )?;

        // engine.entity_insert_timer(entity_id, duration, signal)
        // Inserts a timer component on an entity during collision handling
        engine.set(
            "entity_insert_timer",
            self.lua.create_function(
                |lua, (entity_id, duration, signal): (u64, f32, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::InsertTimer {
                            entity_id,
                            duration,
                            signal,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.entity_insert_stuckto(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)
        // Inserts a StuckTo component on an entity during collision handling
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
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::InsertStuckTo {
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
    pub fn drain_collision_entity_commands(&self) -> Vec<CollisionEntityCmd> {
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

    /// Updates the cached world signal values that Lua can read.
    /// Call this before invoking Lua callbacks so they have fresh data.
    pub fn update_signal_cache(
        &self,
        scalars: &FxHashMap<String, f32>,
        integers: &FxHashMap<String, i32>,
        strings: &FxHashMap<String, String>,
        flags: &FxHashSet<String>,
        group_counts: &FxHashMap<String, u32>,
        entities: &FxHashMap<String, u64>,
    ) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            *data.signal_scalars.borrow_mut() = scalars.clone();
            *data.signal_integers.borrow_mut() = integers.clone();
            *data.signal_strings.borrow_mut() = strings.clone();
            *data.signal_flags.borrow_mut() = flags.clone();
            *data.group_counts.borrow_mut() = group_counts.clone();
            *data.signal_entities.borrow_mut() = entities.clone();
        }
    }

    /// Updates the cached tracked groups that Lua can read.
    /// Call this before invoking Lua callbacks so they have fresh data.
    pub fn update_tracked_groups_cache(&self, groups: &FxHashSet<String>) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            *data.tracked_groups.borrow_mut() = groups.clone();
        }
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
