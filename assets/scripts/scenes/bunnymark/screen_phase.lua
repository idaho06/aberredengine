-- scenes/bunnymark/screen_phase.lua
-- Bunnymark variant: ScreenPosition + per-entity Phase callback.
-- The engine calls on_update for each bunny entity individually each frame.
-- ScreenPosition sprites skip the z-sort pass in the render system.
-- Velocity state lives in a module-level table keyed by entity ID.

local M      = {}
local common = require("scenes.bunnymark.common")
local C      = common

-- ─── Module-level state ───────────────────────────────────────────────────────

local bunny_state       = {} -- [entity_id] = { x, y, vx, vy }
local pending           = {} -- { key, x, y, vx, vy } — spawned, ID not yet available
local bunny_key_counter = 0
local bunny_count       = 0  -- tracked separately (bunny_state table size)

-- ─── Phase callback (per entity) ─────────────────────────────────────────────

--- Called by the engine each frame for every bunny entity.
--- @param ctx EntityContext   { id, ... }
--- @param dt number  (second argument from engine is input, dropped here)
local function bunny_screen_phase_on_update(ctx, _, dt)
    local s = bunny_state[ctx.id]
    if not s then return end

    -- s.vy = s.vy + C.GRAVITY * dt
    s.x = s.x + s.vx * dt
    s.y = s.y + s.vy * dt

    if s.x + C.BUNNY_W > C.SCREEN_W then
        s.vx = -math.abs(s.vx)
        s.x  = C.SCREEN_W - C.BUNNY_W
    elseif s.x < 0 then
        s.vx = math.abs(s.vx)
        s.x  = 0
    end

    if s.y + C.BUNNY_H > C.SCREEN_H then
        s.vy = -math.abs(s.vy)
        s.y  = C.SCREEN_H - C.BUNNY_H
    elseif s.y < C.HUD_H then
        s.vy = math.abs(s.vy)
        s.y  = C.HUD_H
    end

    engine.entity_set_screen_position(ctx.id, s.x, s.y)
end

-- ─── Scene-level callback ─────────────────────────────────────────────────────

--- Called once per frame for scene orchestration: harvest IDs, spawn, HUD.
--- @param input InputSnapshot
--- @param dt number
local function on_update_bunnymark_screen_phase(input, dt)
    if input.digital.back.just_pressed then
        bunny_state       = {}
        pending           = {}
        bunny_key_counter = 0
        bunny_count       = 0
        engine.change_scene("bunnymark_menu")
        return
    end

    -- Phase 1: Harvest entity IDs for bunnies spawned last frame.
    for _, p in ipairs(pending) do
        local id = engine.get_entity(p.key)
        if id then
            bunny_state[id] = { x = p.x, y = p.y, vx = p.vx, vy = p.vy }
            bunny_count = bunny_count + 1
        end
    end
    pending = {}

    -- Phase 2: Spawn a new batch of bunnies at the cursor while action_1 is held.
    if input.digital.action_1.pressed then
        local mx = input.analog.mouse_x
        local my = input.analog.mouse_y
        for _ = 1, C.BUNNIES_PER_FRAME do
            bunny_key_counter  = bunny_key_counter + 1
            local key          = "sp" .. bunny_key_counter
            local vx, vy       = C.random_velocity()
            local r, g, b      = C.random_tint()
            engine.spawn()
                :with_sprite("bunnymark-raybunny", C.BUNNY_W, C.BUNNY_H, 0, 0)
                :with_screen_position(mx, my)
                :with_tint(r, g, b, 255)
                :with_zindex(0)
                :with_phase({
                    initial = "move",
                    phases  = { move = { on_update = "bunny_screen_phase_on_update" } },
                })
                :register_as(key)
                :build()
            pending[#pending + 1] = { key = key, x = mx, y = my, vx = vx, vy = vy }
        end
    end

    -- HUD
    engine.set_integer("bunny_count", bunny_count)
    if dt > 0 then
        engine.set_integer("bunny_fps", math.floor(1.0 / dt))
    end
end

-- ─── Callback registry ────────────────────────────────────────────────────────

M._callbacks = {
    on_update_bunnymark_screen_phase = on_update_bunnymark_screen_phase,
    bunny_screen_phase_on_update     = bunny_screen_phase_on_update,
}

-- ─── Spawn ────────────────────────────────────────────────────────────────────

function M.spawn()
    engine.log_info("Spawning Bunnymark ScreenPosition+Phase...")
    C.setup()
    C.spawn_hud("SCRN+PHASE")
    engine.log_info("Bunnymark ScreenPosition+Phase ready!")
end

return M
