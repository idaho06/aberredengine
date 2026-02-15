//! Generator for `.luarc.json` — Lua Language Server configuration.
//!
//! Produces a `.luarc.json` file that configures the Lua Language Server for
//! editor autocompletion based on the engine's runtime metadata. The generated
//! config includes the `engine` global and points to the generated stubs file.

use crate::resources::lua_runtime::LuaRuntime;
use mlua::prelude::*;
use std::path::Path;

/// Generate `.luarc.json` content from the engine's Lua runtime metadata.
///
/// Validates that the `engine` global and `engine.__meta` table exist, then
/// builds the JSON configuration string.
pub fn generate_luarc(runtime: &LuaRuntime, stubs_filename: &str) -> Result<String, String> {
    let lua = runtime.lua();

    // Validate engine global and __meta exist
    let engine: LuaTable = lua
        .globals()
        .get("engine")
        .map_err(|e| format!("Failed to get engine table: {e}"))?;
    let _meta: LuaTable = engine
        .get("__meta")
        .map_err(|e| format!("Failed to get engine.__meta: {e}"))?;

    let content = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
        "runtime.version": "LuaJIT",
        "diagnostics.globals": ["engine"],
        "workspace.library": [stubs_filename],
        "completion.autoRequire": false
    });

    serde_json::to_string_pretty(&content)
        .map_err(|e| format!("Failed to serialize .luarc.json: {e}"))
}

/// Write the generated `.luarc.json` content to a file.
pub fn write_luarc(path: &Path, content: &str) -> Result<(), String> {
    std::fs::write(path, content)
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}
