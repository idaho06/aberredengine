-- scenes/kraken/intro.lua
-- Kraken intro scene — empty black screen with back-to-menu support

local M = {}

-- ─── Callbacks (local — injected into _G by main.lua) ───────────────────────

--- Called each frame when kraken_intro scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_kraken_intro(input, dt)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

--- Called when the kraken mouth entity enters the "create_tentacles" phase.
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
local function on_create_tentacles_enter(ctx, input)
    engine.log_info("Kraken mouth entered create_tentacles phase!")
    -- Spawn tentacles here, or start a timer to spawn them after a delay.
    local distance = 110
    for i = 1, 8 do
        local angle = (i - 1) * (math.pi / 4) -- 8 tentacles evenly spaced
        local x = math.cos(angle) * distance  -- Distance from mouth center
        local y = math.sin(angle) * distance
        local rotation = math.deg(angle) + 90 -- 0° = up
        engine.spawn()
            :with_sprite("kraken-tentacle", 92, 92, 46, 92)
            :with_zindex(-1)
            :with_parent(ctx.id)
            :with_rotation(rotation)
            :with_position(x, y)
            :with_scale(0.80, 0.80)
            :with_signal_integer("depth", 10)
            :with_phase({
                initial = "spawn_tentacle",
                phases = {
                    spawn_tentacle = {
                        on_enter = "on_enter_spawn_tentacle",
                    }
                }
            })
            :build()
    end
end

--- Called when a tentacle enters the "spawn_tentacle" phase.
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
local function on_enter_spawn_tentacle(ctx, input)
    -- get depth signal and substract 1. If depth > 0, spawn a child tentacle with the new depth.
    local depth = 0
    if ctx.signals and ctx.signals.integers then
        depth = ctx.signals.integers["depth"] or 0
    end
    if depth > 0 then
        local distance = 80
        engine.spawn()
            :with_sprite("kraken-tentacle", 92, 92, 46, 92)
            :with_zindex(-1)
            :with_parent(ctx.id)
            :with_rotation(0)
            :with_position(0, -distance)
            :with_scale(0.80, 0.80)
            :with_signal_integer("depth", depth - 1)
            :with_phase({
                initial = "spawn_tentacle",
                phases = {
                    spawn_tentacle = {
                        on_enter = "on_enter_spawn_tentacle",
                    }
                }
            })
            :with_tween_rotation(16, -16, 5)
            :with_tween_rotation_easing("quad_in_out")
            :with_tween_rotation_loop("ping_pong")
            :build()
    end
end

-- ─── Callback registry ──────────────────────────────────────────────────────
-- Every function the engine calls by name must appear here.
-- Keys must exactly match the strings passed to the engine.

M._callbacks = {
    on_update_kraken_intro = on_update_kraken_intro,
    on_create_tentacles_enter = on_create_tentacles_enter,
    on_enter_spawn_tentacle = on_enter_spawn_tentacle,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn all entities for the kraken intro scene.
function M.spawn()
    engine.log_info("Spawning kraken intro scene...")

    -- Set render resolution (same as menu)
    engine.set_render_size(640, 360)

    -- Camera at origin, offset to center
    engine.set_camera(0.0, 0.0, 320.0, 180.0, 0.0, 1.0)

    -- Black background
    engine.set_background_color(0, 0, 0)

    -- No post-processing
    engine.post_process_shader(nil)

    -- Kraken mouth
    engine.spawn()
        :with_sprite("kraken-mouth", 256, 256, 128, 128)
        :with_zindex(0)
        :with_position(0, 0)
        :with_phase({
            initial = "create_tentacles",
            phases = {
                create_tentacles = {
                    on_enter = "on_create_tentacles_enter",
                    -- on_update = "on_create_tentacles_update",
                    -- on_exit = "on_create_tentacles_exit"
                },
                --moving = { on_enter = "on_moving_enter" }
            }
        })
        :with_scale(0.5, 0.5)
        :with_rotation(0)
        :with_tween_rotation(-16, 16, 5)
        :with_tween_rotation_easing("quad_in_out")
        :with_tween_rotation_loop("ping_pong")
        :register_as("mouth")
        :build()

    engine.log_info("Kraken intro scene entities queued!")
end

return M
