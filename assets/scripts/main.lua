-- main.lua
-- Entry point for game scripts
-- This file is loaded when the engine starts

engine.log("===========================================")
engine.log("  Aberred Engine - Lua Scripting Active!")
engine.log("===========================================")

-- Load modules
local setup = require("setup")

-- Game configuration table
game = {
    title = "Arkanoid Clone",
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

    -- Return a greeting that Rust can read
    return "Hello from Lua! Ready to play."
end

--- Called every frame during gameplay.
--- @param dt number Delta time in seconds
function on_update(dt)
    -- This will be implemented in a future phase
end

--- Called when switching scenes.
--- @param scene_name string The name of the scene to switch to
function on_switch_scene(scene_name)
    engine.log_info("Switching to scene: " .. scene_name)
end

engine.log("main.lua loaded successfully!")
