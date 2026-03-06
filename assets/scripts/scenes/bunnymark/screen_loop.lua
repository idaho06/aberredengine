-- scenes/bunnymark/screen_loop.lua
-- Bunnymark variant: ScreenPosition + Lua loop.
-- All positions are stored in a Lua table and updated via entity_set_screen_position.
-- ScreenPosition sprites skip the z-sort pass in the render system.

local M      = {}
local common = require("scenes.bunnymark.common")
local C      = common

-- ─── Module-level state ───────────────────────────────────────────────────────

local bunnies           = {} -- { id, x, y, vx, vy }
local pending           = {} -- { key, x, y, vx, vy } — spawned, ID not yet available
local bunny_key_counter = 0

-- ─── Callbacks ────────────────────────────────────────────────────────────────

local function on_update_bunnymark_screen_loop(input, dt)
    if input.digital.back.just_pressed then
        bunnies           = {}
        pending           = {}
        bunny_key_counter = 0
        engine.change_scene("bunnymark_menu")
        return
    end

    -- Phase 1: Harvest entity IDs for bunnies spawned last frame.
    for _, p in ipairs(pending) do
        local id = engine.get_entity(p.key)
        if id then
            bunnies[#bunnies + 1] = { id = id, x = p.x, y = p.y, vx = p.vx, vy = p.vy }
        end
    end
    pending = {}

    -- Phase 2: Spawn a new batch of bunnies at the cursor while action_1 is held.
    if input.digital.action_1.pressed then
        local mx = input.analog.mouse_x
        local my = input.analog.mouse_y
        for _ = 1, C.BUNNIES_PER_FRAME do
            bunny_key_counter  = bunny_key_counter + 1
            local key          = "sl" .. bunny_key_counter
            local vx, vy       = C.random_velocity()
            local r, g, b      = C.random_tint()
            engine.spawn()
                :with_sprite("bunnymark-raybunny", C.BUNNY_W, C.BUNNY_H, 0, 0)
                :with_screen_position(mx, my)
                :with_tint(r, g, b, 255)
                :with_zindex(0)
                :register_as(key)
                :build()
            pending[#pending + 1] = { key = key, x = mx, y = my, vx = vx, vy = vy }
        end
    end

    -- Phase 3: Integrate and bounce all fully-tracked bunnies.
    for i = 1, #bunnies do
        local bn = bunnies[i]
        -- bn.vy = bn.vy + C.GRAVITY * dt
        bn.x  = bn.x + bn.vx * dt
        bn.y  = bn.y + bn.vy * dt

        if bn.x + C.BUNNY_W > C.SCREEN_W then
            bn.vx = -math.abs(bn.vx)
            bn.x  = C.SCREEN_W - C.BUNNY_W
        elseif bn.x < 0 then
            bn.vx = math.abs(bn.vx)
            bn.x  = 0
        end

        if bn.y + C.BUNNY_H > C.SCREEN_H then
            bn.vy = -math.abs(bn.vy)
            bn.y  = C.SCREEN_H - C.BUNNY_H
        elseif bn.y < C.HUD_H then
            bn.vy = math.abs(bn.vy)
            bn.y  = C.HUD_H
        end

        engine.entity_set_screen_position(bn.id, bn.x, bn.y)
    end

    -- HUD
    engine.set_integer("bunny_count", #bunnies)
    if dt > 0 then
        engine.set_integer("bunny_fps", math.floor(1.0 / dt))
    end
end

-- ─── Callback registry ────────────────────────────────────────────────────────

M._callbacks = {
    on_update_bunnymark_screen_loop = on_update_bunnymark_screen_loop,
}

-- ─── Spawn ────────────────────────────────────────────────────────────────────

function M.spawn()
    engine.log_info("Spawning Bunnymark ScreenPosition+Loop...")
    C.setup()
    C.spawn_hud("SCRN+LOOP")
    engine.log_info("Bunnymark ScreenPosition+Loop ready!")
end

return M
