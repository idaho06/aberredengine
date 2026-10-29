-- scenes/world_bench.lua
-- Deterministic world draw-order benchmark: static world sprites (and texts)
-- whose positions and ZIndex come from a fixed-seed integer LCG, so every
-- build renders identical frames. Pick a load profile with PROFILE below.

local M = {}

-- ─── Configuration ────────────────────────────────────────────────────────────

--- "sprites_flat"      N sprites, one shared ZIndex.
--- "sprites_scattered" N sprites, scattered ZIndex.
--- "mixed_scattered"   N sprites + M texts, ZIndex scattered across both kinds.
local PROFILE      = "mixed_scattered"
local SPRITE_COUNT = 10000
local TEXT_COUNT   = 1000

local SCREEN_W = 800
local SCREEN_H = 450
local SPRITE_W = 32
local SPRITE_H = 32
local TEXT_W   = 24 -- conservative width of the 2-glyph label at size 12
local TEXT_H   = 12
local Z_LEVELS = 100
local LCG_SEED = 12345

-- ─── Deterministic generator ──────────────────────────────────────────────────

-- 32-bit LCG (Numerical Recipes constants). Products stay below 2^53, so the
-- arithmetic is exact in both integer and float Lua numbers.
local lcg_state = LCG_SEED

--- @param n integer
--- @return integer value in [0, n)
local function next_below(n)
    lcg_state = (lcg_state * 1664525 + 1013904223) % 4294967296
    return math.floor(lcg_state / 65536) % n
end

-- ─── Callbacks ────────────────────────────────────────────────────────────────

local function on_update_world_bench(input, _dt)
    if input.digital.back.just_pressed then
        engine.request_scene("menu")
    end
end

M._callbacks = {
    on_update_world_bench = on_update_world_bench,
}

-- ─── Spawn ────────────────────────────────────────────────────────────────────

local function spawn_sprite(z)
    engine.spawn()
        :with_sprite("bunnymark-raybunny", SPRITE_W, SPRITE_H, 0, 0)
        :with_position(next_below(SCREEN_W - SPRITE_W), next_below(SCREEN_H - SPRITE_H))
        :with_z_index(z)
        :build()
end

local function spawn_text(z)
    engine.spawn()
        :with_text("BN", "default", 12, 255, 255, 255, 255)
        :with_position(next_below(SCREEN_W - TEXT_W), next_below(SCREEN_H - TEXT_H))
        :with_z_index(z)
        :build()
end

function M.spawn()
    engine.log_debug("Spawning world draw-order bench: " .. PROFILE)
    engine.stop_all_music()

    engine.set_render_size(SCREEN_W, SCREEN_H)
    engine.set_vsync(false)
    engine.set_target_fps(0) -- 0 = unlimited; the benchmark reads Tracy spans
    engine.set_camera(0, 0, 0, 0, 0.0, 1.0)
    engine.set_background_color(20, 20, 20)
    engine.post_process_shader(nil)

    lcg_state = LCG_SEED
    local flat = PROFILE == "sprites_flat"
    for _ = 1, SPRITE_COUNT do
        spawn_sprite(flat and 0 or next_below(Z_LEVELS))
    end
    if PROFILE == "mixed_scattered" then
        for _ = 1, TEXT_COUNT do
            spawn_text(next_below(Z_LEVELS))
        end
    end
end

return M
