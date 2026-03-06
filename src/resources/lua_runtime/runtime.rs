//! Lua runtime core implementation.
//!
//! This module contains the `LuaRuntime` struct which manages the Lua interpreter
//! and provides the `engine` table API to Lua scripts.

use super::commands::*;
use super::input_snapshot::InputSnapshot;
use super::spawn_data::*;
use crate::resources::worldsignals::SignalSnapshot;
use mlua::prelude::*;
use rustc_hash::FxHashSet;
use std::cell::RefCell;
use std::sync::Arc;

/// Cached game configuration snapshot for Lua to read.
pub(super) struct GameConfigSnapshot {
    pub fullscreen: bool,
    pub vsync: bool,
    pub target_fps: u32,
    pub render_width: u32,
    pub render_height: u32,
    pub background_r: u8,
    pub background_g: u8,
    pub background_b: u8,
}

impl Default for GameConfigSnapshot {
    fn default() -> Self {
        Self {
            fullscreen: false,
            vsync: false,
            target_fps: 60,
            render_width: 640,
            render_height: 360,
            background_r: 80,
            background_g: 80,
            background_b: 80,
        }
    }
}

/// Shared state accessible from Lua function closures.
/// This is stored in Lua's app_data and allows Lua functions to queue commands.
pub(super) struct LuaAppData {
    pub(super) asset_commands: RefCell<Vec<AssetCmd>>,
    pub(super) spawn_commands: RefCell<Vec<SpawnCmd>>,
    pub(super) audio_commands: RefCell<Vec<AudioLuaCmd>>,
    pub(super) signal_commands: RefCell<Vec<SignalCmd>>,
    pub(super) phase_commands: RefCell<Vec<PhaseCmd>>,
    pub(super) entity_commands: RefCell<Vec<EntityCmd>>,
    pub(super) group_commands: RefCell<Vec<GroupCmd>>,
    pub(super) tilemap_commands: RefCell<Vec<TilemapCmd>>,
    pub(super) camera_commands: RefCell<Vec<CameraCmd>>,
    pub(super) animation_commands: RefCell<Vec<AnimationCmd>>,
    pub(super) render_commands: RefCell<Vec<RenderCmd>>,
    /// Clone commands for regular context (scene setup, phase callbacks)
    pub(super) clone_commands: RefCell<Vec<CloneCmd>>,
    // Collision-scoped command queues (processed immediately after each collision callback)
    pub(super) collision_entity_commands: RefCell<Vec<EntityCmd>>,
    pub(super) collision_signal_commands: RefCell<Vec<SignalCmd>>,
    pub(super) collision_audio_commands: RefCell<Vec<AudioLuaCmd>>,
    pub(super) collision_spawn_commands: RefCell<Vec<SpawnCmd>>,
    pub(super) collision_phase_commands: RefCell<Vec<PhaseCmd>>,
    pub(super) collision_camera_commands: RefCell<Vec<CameraCmd>>,
    /// Clone commands for collision context (processed after collision callbacks)
    pub(super) collision_clone_commands: RefCell<Vec<CloneCmd>>,
    /// Cached world signal snapshot (read-only for Lua).
    /// Updated before calling Lua callbacks via `update_signal_cache()`.
    /// Using Arc allows cheap sharing without cloning all maps on every callback.
    pub(super) signal_snapshot: RefCell<Arc<SignalSnapshot>>,
    /// Cached tracked group names (read-only snapshot for Lua)
    pub(super) tracked_groups: RefCell<FxHashSet<String>>,
    /// Game config command queue
    pub(super) gameconfig_commands: RefCell<Vec<GameConfigCmd>>,
    /// Camera follow config command queue
    pub(super) camera_follow_commands: RefCell<Vec<CameraFollowCmd>>,
    /// Cached game configuration snapshot (read-only for Lua)
    pub(super) gameconfig_snapshot: RefCell<GameConfigSnapshot>,
    /// Input rebinding command queue
    pub(super) input_commands: RefCell<Vec<InputCmd>>,
    /// Cached input bindings snapshot (read-only for Lua: action_name → key_name)
    pub(super) bindings_snapshot: RefCell<std::collections::HashMap<String, String>>,
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
    world_pos: LuaRegistryKey,
    world_scale: LuaRegistryKey,
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
    pub world_pos: LuaTable,
    pub world_scale: LuaTable,
}

/// Resource holding the Lua interpreter state.
///
/// This is a `NonSend` resource because the Lua state is not thread-safe.
/// It should be initialized once at startup and reused throughout the game.
pub struct LuaRuntime {
    pub(super) lua: Lua,
    /// Pooled collision context tables for reuse across collisions.
    collision_ctx_pool: Option<CollisionCtxPool>,
    /// Pooled entity context tables for reuse across phase/timer callbacks.
    entity_ctx_pool: Option<EntityCtxPool>,
}

/// Converts an [`InputAction`] to its canonical Lua-facing string name.
///
/// These strings are what Lua passes to `engine.rebind_action()` and
/// `engine.get_binding()`.
pub(super) fn action_to_str(action: crate::events::input::InputAction) -> &'static str {
    use crate::events::input::InputAction;
    match action {
        InputAction::MainDirectionUp => "main_up",
        InputAction::MainDirectionDown => "main_down",
        InputAction::MainDirectionLeft => "main_left",
        InputAction::MainDirectionRight => "main_right",
        InputAction::SecondaryDirectionUp => "secondary_up",
        InputAction::SecondaryDirectionDown => "secondary_down",
        InputAction::SecondaryDirectionLeft => "secondary_left",
        InputAction::SecondaryDirectionRight => "secondary_right",
        InputAction::Back => "back",
        InputAction::Action1 => "action_1",
        InputAction::Action2 => "action_2",
        InputAction::Action3 => "action_3",
        InputAction::Special => "special",
        InputAction::ToggleDebug => "toggle_debug",
        InputAction::ToggleFullscreen => "toggle_fullscreen",
    }
}

/// Converts a canonical Lua action name string to an [`InputAction`].
pub fn action_from_str(s: &str) -> Option<crate::events::input::InputAction> {
    use crate::events::input::InputAction;
    match s {
        "main_up" => Some(InputAction::MainDirectionUp),
        "main_down" => Some(InputAction::MainDirectionDown),
        "main_left" => Some(InputAction::MainDirectionLeft),
        "main_right" => Some(InputAction::MainDirectionRight),
        "secondary_up" => Some(InputAction::SecondaryDirectionUp),
        "secondary_down" => Some(InputAction::SecondaryDirectionDown),
        "secondary_left" => Some(InputAction::SecondaryDirectionLeft),
        "secondary_right" => Some(InputAction::SecondaryDirectionRight),
        "back" => Some(InputAction::Back),
        "action_1" => Some(InputAction::Action1),
        "action_2" => Some(InputAction::Action2),
        "action_3" => Some(InputAction::Action3),
        "special" => Some(InputAction::Special),
        "toggle_debug" => Some(InputAction::ToggleDebug),
        "toggle_fullscreen" => Some(InputAction::ToggleFullscreen),
        _ => None,
    }
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
            gameconfig_commands: RefCell::new(Vec::new()),
            camera_follow_commands: RefCell::new(Vec::new()),
            gameconfig_snapshot: RefCell::new(GameConfigSnapshot::default()),
            input_commands: RefCell::new(Vec::new()),
            bindings_snapshot: RefCell::new(std::collections::HashMap::new()),
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
        runtime.register_camera_follow_api()?;
        runtime.register_collision_api()?;
        runtime.register_animation_api()?;
        runtime.register_render_api()?;
        runtime.register_gameconfig_api()?;
        runtime.register_input_api()?;
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
        let world_pos = lua.create_table()?;
        let world_scale = lua.create_table()?;

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
            world_pos: lua.create_registry_value(world_pos)?,
            world_scale: lua.create_registry_value(world_scale)?,
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
            world_pos: self.lua.registry_value(&pool.world_pos)?,
            world_scale: self.lua.registry_value(&pool.world_scale)?,
        })
    }

    /// Builds a Lua table representing the full input state for this frame.
    ///
    /// The returned table has the following shape:
    ///
    /// ```lua
    /// input = {
    ///     digital = {
    ///         up = { pressed = bool, just_pressed = bool, just_released = bool },
    ///         -- down, left, right, action_1/2/3, back, special,
    ///         -- main_up/down/left/right, secondary_up/down/left/right,
    ///         -- debug, fullscreen
    ///     },
    ///     analog = {
    ///         scroll_y      = number,  -- mouse wheel delta (positive=up)
    ///         mouse_x       = number,  -- game-space cursor X (0..render_width)
    ///         mouse_y       = number,  -- game-space cursor Y (0..render_height)
    ///         mouse_world_x = number,  -- world-space cursor X (after camera)
    ///         mouse_world_y = number,  -- world-space cursor Y (after camera)
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
        digital.set("action_3", create_button_table(&snapshot.digital.action_3)?)?;
        digital.set("back", create_button_table(&snapshot.digital.back)?)?;
        digital.set("special", create_button_table(&snapshot.digital.special)?)?;
        // Raw WASD (main directional)
        digital.set("main_up", create_button_table(&snapshot.digital.main_up)?)?;
        digital.set(
            "main_down",
            create_button_table(&snapshot.digital.main_down)?,
        )?;
        digital.set(
            "main_left",
            create_button_table(&snapshot.digital.main_left)?,
        )?;
        digital.set(
            "main_right",
            create_button_table(&snapshot.digital.main_right)?,
        )?;
        // Raw arrow keys (secondary directional)
        digital.set(
            "secondary_up",
            create_button_table(&snapshot.digital.secondary_up)?,
        )?;
        digital.set(
            "secondary_down",
            create_button_table(&snapshot.digital.secondary_down)?,
        )?;
        digital.set(
            "secondary_left",
            create_button_table(&snapshot.digital.secondary_left)?,
        )?;
        digital.set(
            "secondary_right",
            create_button_table(&snapshot.digital.secondary_right)?,
        )?;
        // Function keys
        digital.set("debug", create_button_table(&snapshot.digital.debug)?)?;
        digital.set(
            "fullscreen",
            create_button_table(&snapshot.digital.fullscreen)?,
        )?;

        // Create analog inputs table
        let analog = lua.create_table()?;
        analog.set("scroll_y", snapshot.analog.scroll_y)?;
        // Mouse position — game-space (letterbox-corrected, matches ScreenPosition)
        analog.set("mouse_x", snapshot.analog.mouse_x)?;
        analog.set("mouse_y", snapshot.analog.mouse_y)?;
        // Mouse position — world-space (after camera transform, matches MapPosition)
        analog.set("mouse_world_x", snapshot.analog.mouse_world_x)?;
        analog.set("mouse_world_y", snapshot.analog.mouse_world_y)?;

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
