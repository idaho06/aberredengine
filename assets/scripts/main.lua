-- main.lua
-- Entry point for game scripts
-- This file is loaded when the engine starts

engine.log("===========================================")
engine.log("  Aberred Engine - Lua Scripting Active!")
engine.log("===========================================")

-- Load modules
local setup = require("setup")

-- Scene modules (loaded on demand)
local scenes = {}

--- Lazy-load a scene module.
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

    -- Try to load and spawn the scene
    local scene = get_scene(scene_name)
    if scene and scene.spawn then
        scene.spawn()
    else
        engine.log_info("No Lua spawning for scene '" .. scene_name .. "' (using Rust fallback)")
    end
end

engine.log("main.lua loaded successfully!")
