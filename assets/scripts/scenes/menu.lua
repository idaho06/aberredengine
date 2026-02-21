-- scenes/menu.lua
-- Main showcase menu — select which example to run
-- Called when switching to the "menu" scene

local M = {}

-- ─── Callbacks (local — injected into _G by main.lua) ───────────────────────

--- Callback function for showcase menu selection.
--- @param ctx table Context with menu_id (u64), item_id (string), item_index (integer)
local function on_showcase_menu_select(ctx)
    engine.log_info("Menu selected: " .. ctx.item_id .. " (index " .. ctx.item_index .. ")")

    if ctx.item_id == "asteroids" then
        engine.change_scene("asteroids_level01")
    elseif ctx.item_id == "arkanoid" then
        engine.change_scene("arkanoid_level01")
    elseif ctx.item_id == "birthday" then
        engine.change_scene("birthday_intro")
    elseif ctx.item_id == "exit" then
        engine.quit()
    end
end

--- Called each frame when menu scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_menu(input, dt)
    if input.digital.back.just_pressed then
        engine.quit()
    end
end

-- ─── Callback registry ──────────────────────────────────────────────────────
-- Every function the engine calls by name must appear here.
-- Keys must exactly match the strings passed to the engine.

M._callbacks = {
    on_showcase_menu_select = on_showcase_menu_select,
    on_update_menu          = on_update_menu,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn all entities for the showcase menu scene.
function M.spawn()
    engine.log_info("Spawning showcase menu scene...")

    -- Set render resolution for menu
    engine.set_render_size(640, 360)

    -- Camera at origin, top-left
    engine.set_camera(0.0, 0.0, 0.0, 0.0, 0.0, 1.0)

    -- Stars particle templates
    engine.spawn()
        :with_sprite("asteroids-stars01_sheet", 32, 32, 16, 16)
        :with_zindex(-5)
        :register_as("star_particle01")
        :build()
    engine.spawn()
        :with_sprite("asteroids-stars01_sheet", 32, 32, 16, 16)
        :with_sprite_offset(32, 0)
        :with_zindex(-5)
        :register_as("star_particle02")
        :build()
    engine.spawn()
        :with_sprite("asteroids-stars01_sheet", 32, 32, 16, 16)
        :with_sprite_offset(0, 32)
        :with_zindex(-5)
        :register_as("star_particle03")
        :build()

    -- Title
    engine.spawn()
        :with_text("ABERRED ENGINE", "future", 48, 255, 255, 255, 255)
        :with_screen_position(8, 24)
        :with_zindex(1)
        :build()

    -- Subtitle
    engine.spawn()
        :with_text("showcase", "arcade", 12, 180, 180, 180, 255)
        :with_screen_position(10, 76)
        :with_zindex(1)
        :build()

    -- Cursor for menu
    engine.spawn()
        :with_sprite("cursor", 16, 16, 16 + 8, 0)
        :register_as("menu_cursor")
        :build()

    -- Showcase menu
    engine.spawn()
        :with_group("main_menu")
        :with_menu(
            {
                { id = "asteroids", label = "Asteroids" },
                { id = "arkanoid",  label = "Arkanoid" },
                { id = "birthday",  label = "Birthday Card" },
                { id = "exit",      label = "Exit" },
            },
            16 + 8,
            100,
            "arcade",
            16,
            24,
            false
        )
        :with_menu_cursor("menu_cursor")
        :with_menu_selection_sound("option")
        :with_menu_callback("on_showcase_menu_select")
        :build()

    -- Black background fill
    engine.spawn()
        :with_sprite("black", 64, 64, 0, 0)
        :with_position(0, 0)
        :with_zindex(-10)
        :with_scale(10, 10)
        :build()

    -- Stars particle emitter
    engine.spawn()
        :with_position(660, 360 / 2)
        :with_particle_emitter({
            templates = { "star_particle01", "star_particle02", "star_particle03" },
            shape = { type = "rect", width = 10, height = 360 },
            particles_per_emission = 1,
            emissions_per_second = 3,
            emissions_remaining = 4294967295,
            arc = { -90, -90 },
            speed = { 60, 60 },
            ttl = 15.0,
        })
        :build()

    -- No post-processing on menu
    engine.post_process_shader(nil)

    engine.log_info("Showcase menu scene entities queued!")
end

return M
