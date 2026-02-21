-- scenes/birthday/card.lua
-- Birthday Card example — card scene STUB (implementation pending)
-- Scene name: "birthday_card"
--
-- TODO: Port from ../raquelhb15 project, copy assets, refactor callbacks

local M = {}

-- ─── Callbacks ───────────────────────────────────────────────────────────────

--- Called each frame when birthday_card scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_birthday_card(input, dt)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

-- ─── Callback registry ──────────────────────────────────────────────────────

M._callbacks = {
    on_update_birthday_card = on_update_birthday_card,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

function M.spawn()
    engine.log_info("Birthday card — implementation pending")

    engine.set_camera(0, 0, 320, 180, 0, 1)
    engine.post_process_shader(nil)

    -- Placeholder text
    engine.spawn()
        :with_text("BIRTHDAY CARD", "future", 48, 255, 200, 200, 255)
        :with_screen_position(8, 140)
        :with_zindex(1)
        :build()

    engine.spawn()
        :with_text("(Card scene)", "arcade", 12, 180, 180, 180, 255)
        :with_screen_position(10, 200)
        :with_zindex(1)
        :build()

    engine.spawn()
        :with_text("Press ESC to return", "arcade", 10, 120, 120, 120, 255)
        :with_screen_position(10, 340)
        :with_zindex(1)
        :build()

    -- Black background
    engine.spawn()
        :with_sprite("black", 64, 64, 0, 0)
        :with_position(0, 0)
        :with_zindex(-10)
        :with_scale(10, 10)
        :build()
end

return M
