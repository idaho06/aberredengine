-- filepath: /home/idaho/Projects/aberredengine/assets/scripts/scenes/sidescroller/level01.lua
-- scenes/sidescroller/level01.lua
-- Sidescroller level 01 scene

local M = {}

-- ─── Callbacks (local — injected into _G by main.lua) ───────────────────────

--- Called each frame when sidescroller level01 scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_sidescroller_level01(input, dt)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

--- Called when entering the running phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_running_on_enter(ctx, input)
    engine.log_info("Player started running!")
    engine.entity_set_animation(ctx.id, "sidescroller-char_red_run")
end

--- Called each frame while in the running phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_running_on_update(ctx, input, dt)
    if input.digital.left.pressed or input.digital.right.pressed then
        -- Keep running
    else
        -- Transition to walking
        return "walking"
    end
end

--- Called when entering the walking phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_walking_on_enter(ctx, input)
    engine.log_info("Player started walking!")
    engine.entity_set_animation(ctx.id, "sidescroller-char_red_walk")
end

--- Called each frame while in the walking phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_walking_on_update(ctx, input, dt)
    if input.digital.left.pressed or input.digital.right.pressed then
        -- Transition to running
        return "running"
    end
end

-- ─── Callback registry ──────────────────────────────────────────────────────
-- Every function the engine calls by name must appear here.
-- Keys must exactly match the strings passed to the engine.

M._callbacks = {
    on_update_sidescroller_level01 = on_update_sidescroller_level01,
    player_running_on_enter = player_running_on_enter,
    player_running_on_update = player_running_on_update,
    player_walking_on_enter = player_walking_on_enter,
    player_walking_on_update = player_walking_on_update,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn all entities for the sidescroller level01 scene.
function M.spawn()
    engine.log_info("Spawning sidescroller level01 scene...")

    -- Set render resolution
    engine.set_render_size(640, 360)

    -- Set camera
    engine.set_camera(0, 0, 640 / 2, 360 / 2, 0.0, 1.0) -- target to 0,0, centered, no rotation, default zoom

    -- Set background color
    engine.set_background_color(20, 20, 30)

    -- Clear post-processing shader
    engine.post_process_shader(nil)

    -- Spawn player character
    engine.spawn()
        :with_sprite("sidescroller-char_red_1_sheet", 56, 56, 56 / 2, 56)
        :with_animation("sidescroller-char_red_walk")
        :with_zindex(0)
        :with_position(0, 0)
        :with_group("player")
        :with_signal_flag("on_ground")
        :with_signal_flag("facing_right")
        :with_phase({
            initial = "walking",
            phases = {
                running = {
                    on_enter = "player_running_on_enter",
                    on_update = "player_running_on_update"
                    -- on_exit = "player_running_on_exit"
                },
                walking = {
                    on_enter = "player_walking_on_enter",
                    on_update = "player_walking_on_update"
                    -- on_exit = "player_walking_on_exit"
                }
            }
        })
        :register_as("player")
        :build()

    engine.log_info("Sidescroller level01 scene entities queued!")
end

return M
