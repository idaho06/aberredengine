-- filepath: /home/idaho/Projects/aberredengine/assets/scripts/scenes/bunnymark/level01.lua
-- scenes/bunnymark/level01.lua
-- Bunnymark benchmark scene — replicates the raylib bunnymark example.
--
-- Hold action_1 (Space or left mouse button) to spawn bunnies at the cursor.
-- Each batch of 100 bunnies spawns per frame while the button is held.
-- Bunnies bounce off the screen edges under gravity.

local M                 = {}

-- ─── Constants ───────────────────────────────────────────────────────────────

local SCREEN_W          = 800
local SCREEN_H          = 450
local BUNNY_W           = 32 -- raybunny.png is 32x32
local BUNNY_H           = 32
local BUNNIES_PER_FRAME = 100
local GRAVITY           = 250.0 -- px/s^2 (matches the original example)
local HUD_H             = 40    -- pixels reserved at top for HUD
local VSYNC             = false -- vsync on by default to cap FPS and reduce CPU/GPU load
local TARGET_FPS        = 1000  -- target FPS when vsync is off; adjust as needed for testing

-- ─── Module-level state ───────────────────────────────────────────────────────

local bunnies           = {} -- { id, x, y, vx, vy } — fully tracked
local pending           = {} -- { key, x, y, vx, vy } — spawned, ID not yet available
local bunny_key_counter = 0  -- monotonic counter for unique register_as keys

-- ─── Callbacks ───────────────────────────────────────────────────────────────

--- Called each frame while the bunnymark scene is active.
--- @param input InputSnapshot
--- @param dt number Delta time in seconds
local function on_update_bunnymark(input, dt)
    if input.digital.back.just_pressed then
        -- Reset state so it's clean if we return later
        bunnies = {}
        pending = {}
        bunny_key_counter = 0
        engine.change_scene("menu")
        return
    end

    -- Phase 1: Harvest entity IDs for bunnies spawned last frame.
    -- Entities are created by the ECS after the Lua update runs, so their
    -- IDs become available one frame after the :build() call.
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
        for _ = 1, BUNNIES_PER_FRAME do
            bunny_key_counter = bunny_key_counter + 1
            local key         = "b" .. bunny_key_counter
            local vx          = math.random() * 480 - 240 -- -240..+240 px/s
            local vy          = math.random() * 480 - 240
            local red         = math.random(50, 240)
            local green       = math.random(80, 240)
            local blue        = math.random(100, 240)
            engine.spawn()
                :with_sprite("bunnymark-raybunny", BUNNY_W, BUNNY_H, 0, 0)
                :with_position(mx, my)
                :with_tint(red, green, blue, 255)
                :with_zindex(0)
                :register_as(key)
                :build()
            pending[#pending + 1] = { key = key, x = mx, y = my, vx = vx, vy = vy }
        end
    end

    -- Phase 3: Integrate and bounce all fully-tracked bunnies.
    for i = 1, #bunnies do
        local bn = bunnies[i]
        bn.vy    = bn.vy + GRAVITY * dt
        bn.x     = bn.x + bn.vx * dt
        bn.y     = bn.y + bn.vy * dt

        -- Horizontal walls
        if bn.x + BUNNY_W > SCREEN_W then
            bn.vx = -math.abs(bn.vx)
            bn.x  = SCREEN_W - BUNNY_W
        elseif bn.x < 0 then
            bn.vx = math.abs(bn.vx)
            bn.x  = 0
        end

        -- Vertical walls (floor has slight damping, ceiling is the HUD boundary)
        if bn.y + BUNNY_H > SCREEN_H then
            bn.vy = -math.abs(bn.vy) * 0.85
            bn.y  = SCREEN_H - BUNNY_H
        elseif bn.y < HUD_H then
            bn.vy = math.abs(bn.vy)
            bn.y  = HUD_H
        end

        engine.entity_set_position(bn.id, bn.x, bn.y)
    end

    -- HUD: update world signals read by signal-bound text entities
    engine.set_integer("bunny_count", #bunnies)
    if dt > 0 then
        engine.set_integer("bunny_fps", math.floor(1.0 / dt))
    end
end

-- ─── Callback registry ───────────────────────────────────────────────────────

M._callbacks = {
    on_update_bunnymark = on_update_bunnymark,
}

-- ─── Spawn ───────────────────────────────────────────────────────────────────

--- Spawn all entities for the bunnymark scene.
function M.spawn()
    engine.log_info("Spawning Bunnymark scene...")

    engine.set_render_size(SCREEN_W, SCREEN_H)
    engine.set_vsync(VSYNC)
    engine.set_target_fps(TARGET_FPS)
    -- Camera: world origin = screen top-left, so world(x,y) == screen(x,y).
    engine.set_camera(0, 0, 0, 0, 0.0, 1.0)
    engine.set_background_color(20, 20, 20)
    engine.post_process_shader(nil)

    -- Seed random for varied colours and velocities each run
    math.randomseed(os.time())

    -- World signals used by the HUD signal bindings
    engine.set_integer("bunny_count", 0)
    engine.set_integer("bunny_fps", 0)

    -- HUD — static labels
    engine.spawn()
        :with_text("BUNNIES:", "arcade", 12, 180, 180, 180, 255)
        :with_screen_position(10, 10)
        :with_zindex(10)
        :build()

    engine.spawn()
        :with_text("FPS:", "arcade", 12, 180, 180, 180, 255)
        :with_screen_position(10, 24)
        :with_zindex(10)
        :build()

    -- HUD — live values (bound to world signals)
    engine.spawn()
        :with_text("0", "arcade", 12, 255, 255, 255, 255)
        :with_screen_position(82, 10)
        :with_zindex(10)
        :with_signal_binding("bunny_count")
        :build()

    engine.spawn()
        :with_text("0", "arcade", 12, 255, 255, 255, 255)
        :with_screen_position(42, 24)
        :with_zindex(10)
        :with_signal_binding("bunny_fps")
        :build()

    engine.log_info("Bunnymark scene entities queued!")
end

return M
