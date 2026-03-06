-- scenes/bunnymark/common.lua
-- Shared constants and helpers for all four bunnymark benchmark variants.

local M = {}

-- ─── Constants ────────────────────────────────────────────────────────────────

M.SCREEN_W          = 800
M.SCREEN_H          = 450
M.BUNNY_W           = 32
M.BUNNY_H           = 32
M.HUD_H             = 40    -- pixels reserved at top for HUD
M.BUNNIES_PER_FRAME = 100
M.GRAVITY           = 250.0 -- px/s^2 (matches original raylib example)

-- ─── Helpers ──────────────────────────────────────────────────────────────────

--- Shared scene setup: render size, camera, background, random seed, FPS.
function M.setup()
    engine.set_render_size(M.SCREEN_W, M.SCREEN_H)
    engine.set_vsync(false)
    engine.set_target_fps(0) -- 0 = unlimited
    engine.set_camera(0, 0, 0, 0, 0.0, 1.0)
    engine.set_background_color(20, 20, 20)
    engine.post_process_shader(nil)
    math.randomseed(os.time())
end

--- Spawn the four HUD entities plus a variant label at the top-right.
--- World signals "bunny_count" and "bunny_fps" must be set before calling.
--- @param label string Short variant name, e.g. "MAP+LOOP"
function M.spawn_hud(label)
    engine.set_integer("bunny_count", 0)
    engine.set_integer("bunny_fps",   0)

    engine.spawn()
        :with_text("BUNNIES:", "arcade", 12, 180, 180, 180, 255)
        :with_screen_position(10, 10)
        :with_zindex(10)
        :build()
    engine.spawn()
        :with_text("0", "arcade", 12, 255, 255, 255, 255)
        :with_screen_position(82, 10)
        :with_zindex(10)
        :with_signal_binding("bunny_count")
        :build()
    engine.spawn()
        :with_text("FPS:", "arcade", 12, 180, 180, 180, 255)
        :with_screen_position(10, 24)
        :with_zindex(10)
        :build()
    engine.spawn()
        :with_text("0", "arcade", 12, 255, 255, 255, 255)
        :with_screen_position(42, 24)
        :with_zindex(10)
        :with_signal_binding("bunny_fps")
        :build()
    -- Variant label, top-right corner
    engine.spawn()
        :with_text(label, "arcade", 10, 120, 200, 120, 255)
        :with_screen_position(M.SCREEN_W - #label * 7 - 4, 10)
        :with_zindex(10)
        :build()
end

--- Random initial velocity matching the original raylib bunnymark.
--- @return number vx, number vy
function M.random_velocity()
    return math.random() * 480 - 240, math.random() * 480 - 240
end

--- Random bunny tint.
--- @return number r, number g, number b
function M.random_tint()
    return math.random(50, 240), math.random(80, 240), math.random(100, 240)
end

return M
