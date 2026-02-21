-- main.lua
-- Entry point for the Aberred Engine showcase
-- This file is loaded when the engine starts
--
-- Global flags used by the engine:
-- - "switch_scene": Set to trigger scene switching (cleared by engine after processing)
-- - "quit_game": Set to exit the game (cleared by engine after processing)
--
-- ============================================================================
-- CALLBACK INJECTION SYSTEM
-- ============================================================================
--
-- The engine resolves Lua callbacks by name in the global environment (_G).
-- For example, when a collision rule references "on_ball_hit", the engine calls
-- _G["on_ball_hit"](ctx). This means if two scene files both define a global
-- function with the same name, the last one loaded wins — causing bugs.
--
-- To prevent naming conflicts between examples, each scene module stores its
-- callbacks as LOCAL functions and lists them in M._callbacks:
--
--     local M = {}
--     local function my_collision_handler(ctx) ... end
--     local function on_update_example_level01(input, dt) ... end
--
--     M._callbacks = {
--         my_collision_handler        = my_collision_handler,
--         on_update_example_level01   = on_update_example_level01,
--     }
--
-- When switching scenes, main.lua:
--   1. Removes the previous scene's callbacks from _G
--   2. Injects the new scene's callbacks into _G
--   3. Calls scene.spawn() to set up entities
--
-- RULES FOR SCENE AUTHORS:
--   - Every function the engine will call by name MUST appear in M._callbacks.
--     This includes: scene update (on_update_<scene_name>), collision callbacks,
--     phase callbacks (on_enter/on_update/on_exit), and timer callbacks.
--   - The KEY in M._callbacks must exactly match the string you pass to the
--     engine (e.g. in :with_lua_collision_rule() or :with_phase()).
--   - Define all callbacks as LOCAL functions — never use plain `function name()`.
--   - Module-level local variables (counters, state) are fine and stay private.
-- ============================================================================

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

-- ============================================================================
-- Scene Registry
-- ============================================================================
-- Maps scene names to their require paths.
-- The scene name is what you pass to engine.change_scene().
-- The require path is the Lua module path for the scene file.
--
-- To add a new example, add one entry here and create the scene module.
local scene_registry = {
    menu               = "scenes.menu",
    asteroids_level01  = "scenes.asteroids.level01",
    arkanoid_level01   = "scenes.arkanoid.level01",
    birthday_intro     = "scenes.birthday.intro",
    birthday_card      = "scenes.birthday.card",
}

-- Loaded scene modules (cached to avoid re-requiring)
local scene_cache = {}

-- Keys currently injected into _G by the active scene
local active_callbacks = {}

--- Remove all callbacks registered by the previous scene from _G.
local function clear_callbacks()
    for name, _ in pairs(active_callbacks) do
        _G[name] = nil
    end
    active_callbacks = {}
end

--- Inject a scene module's _callbacks table into _G.
--- After this call, the engine can find them by name.
--- @param scene_mod table The scene module (must have _callbacks field)
local function inject_callbacks(scene_mod)
    if type(scene_mod._callbacks) ~= "table" then return end
    for name, fn in pairs(scene_mod._callbacks) do
        _G[name] = fn
        active_callbacks[name] = true
    end
end

--- Load a scene module by name using the registry.
--- Modules are cached after first load.
--- @param name string The scene name (e.g., "asteroids_level01")
--- @return table|nil The scene module or nil if not found
local function get_scene(name)
    local path = scene_registry[name]
    if not path then
        engine.log_warn("No registry entry for scene: " .. name)
        return nil
    end
    if not scene_cache[name] then
        local ok, mod = pcall(require, path)
        if ok then
            scene_cache[name] = mod
        else
            engine.log_warn("Failed to load scene '" .. path .. "': " .. tostring(mod))
            return nil
        end
    end
    return scene_cache[name]
end

-- Game configuration table
local game = {
    title = "Aberred Engine Showcase",
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

    -- Initialize world signals
    engine.set_integer("score", 0)
    engine.set_integer("high_score", 0)
    engine.set_integer("lives", 3)
    engine.set_integer("level", 1)
    engine.set_string("scene", "menu")

    return "Hello from Lua! Ready to play."
end

--- Called when switching scenes.
--- Clears previous callbacks, injects new ones, then spawns the scene.
--- @param scene_name string The name of the scene to switch to
function on_switch_scene(scene_name)
    engine.log_info("Switching to scene: " .. scene_name)

    -- 1. Remove previous scene's callbacks from _G
    clear_callbacks()

    -- 2. Load the scene module
    local scene = get_scene(scene_name)
    if scene then
        -- 3. Inject new scene's callbacks into _G
        inject_callbacks(scene)
        -- 4. Spawn scene entities
        if scene.spawn then
            scene.spawn()
        end
    else
        engine.log_warn("No scene module for '" .. scene_name .. "'")
    end
end

engine.log("main.lua loaded successfully!")
