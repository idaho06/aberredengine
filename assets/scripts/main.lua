-- main.lua
-- Entry point for game scripts
-- This file is loaded when the engine starts
--
-- Global flags used by the engine:
-- - "switch_scene": Set to trigger scene switching (cleared by engine after processing)
-- - "quit_game": Set to exit the game (cleared by engine after processing)

engine.log("===========================================")
engine.log("  Aberred Engine - Lua Scripting Active!")
engine.log("===========================================")

-- Load modules
local setup = require("setup")
local utils = require("lib.utils")
local math_helpers = require("lib.math")

-- Expose common helpers globally for all scripts
Dump_value = utils.dump_value
Lerp = math_helpers.lerp
Lerp2 = math_helpers.lerp2
InvLerp = math_helpers.inv_lerp
Remap = math_helpers.remap

-- Scene modules (loaded on demand)
local scenes = {}

--- Lazy-load a scene module.
--- Used by `on_switch_scene` in this script to load scene logic as needed.
--- @param name string The scene name (e.g., "menu", "level01")
--- @return table|nil The scene module or nil if not found
local function get_scene(name)
    if not scenes[name] then
        local ok, mod = pcall(require, "scenes." .. name)
        if ok then
            scenes[name] = mod
        else
            engine.log_warn("Scene module 'scenes." .. name .. "' not found: " .. tostring(mod))
            return nil
        end
    end
    return scenes[name]
end

-- Game configuration table
local game = {
    title = "Asteroids Clone",
    version = "0.1.0",
    author = "Idaho",
}

--- Called during Setup state to load all assets.
--- This is called before entering the Playing state.
function on_setup()
    engine.log_info("on_setup() called from Lua!")
    setup.load_assets()
end

--- Called when the game enters the Playing state.
--- Use this to initialize game-specific Lua state.
function on_enter_play()
    engine.log_info("on_enter_play() called from Lua!")
    engine.log_info("Game: " .. game.title .. " v" .. game.version)
    engine.log_info("Lua scripting is working correctly.")

    -- Initialize game world signals
    engine.set_integer("score", 0)
    engine.set_integer("high_score", 0)
    engine.set_integer("lives", 3)
    engine.set_integer("level", 1)
    engine.set_string("scene", "menu")

    -- Return a greeting that Rust can read
    return "Hello from Lua! Ready to play."
end

--- Called when switching scenes.
--- @param scene_name string The name of the scene to switch to
function on_switch_scene(scene_name)
    engine.log_info("Switching to scene: " .. scene_name)

    -- Try to load and spawn the scene
    local scene = get_scene(scene_name)
    if scene and scene.spawn then
        scene.spawn()
    else
        engine.log_warn("No Lua `M.spawn` function for scene '" .. scene_name .. "' (using Rust fallback?)")
    end
end

engine.log("main.lua loaded successfully!")
