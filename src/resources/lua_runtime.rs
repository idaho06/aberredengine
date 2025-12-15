//! Lua scripting runtime resource.
//!
//! This module provides a Lua interpreter that can be used to run game scripts.
//! The runtime exposes an `engine` table with functions that scripts can call
//! to interact with the game engine.
//!
//! # Example
//!
//! ```lua
//! -- From a Lua script
//! engine.log("Hello from Lua!")
//! engine.load_texture("ball", "assets/textures/ball.png")
//! engine.load_font("arcade", "assets/fonts/Arcade.ttf", 128)
//! engine.load_music("menu", "assets/audio/menu.xm")
//! engine.load_sound("ping", "assets/audio/ping.wav")
//! ```

use mlua::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;

/// Commands that Lua can queue for asset loading.
/// These are processed by Rust systems that have access to the necessary resources.
#[derive(Debug, Clone)]
pub enum AssetCmd {
    /// Load a texture from a file path
    LoadTexture { id: String, path: String },
    /// Load a font from a file path with a specific size
    LoadFont { id: String, path: String, size: i32 },
    /// Load a music track from a file path
    LoadMusic { id: String, path: String },
    /// Load a sound effect from a file path
    LoadSound { id: String, path: String },
    /// Load a tilemap from a directory path
    LoadTilemap { id: String, path: String },
}

/// Shared state accessible from Lua function closures.
/// This is stored in Lua's app_data and allows Lua functions to queue commands.
struct LuaAppData {
    asset_commands: RefCell<Vec<AssetCmd>>,
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
        });

        let runtime = Self { lua };
        runtime.register_base_api()?;
        runtime.register_asset_api()?;

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
