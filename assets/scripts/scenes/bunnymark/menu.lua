-- scenes/bunnymark/menu.lua
-- Bunnymark benchmark sub-menu — choose which variant to run.

local M = {}

-- ─── Callbacks ────────────────────────────────────────────────────────────────

--- Called when a menu item is selected.
--- @param ctx table  { menu_id, item_id, item_index }
local function on_bunnymark_menu_select(ctx)
    engine.log_debug("Bunnymark menu: " .. ctx.item_id)
    if ctx.item_id == "map_loop" then
        engine.change_scene("bunnymark_map_loop")
    elseif ctx.item_id == "screen_loop" then
        engine.change_scene("bunnymark_screen_loop")
    elseif ctx.item_id == "map_phase" then
        engine.change_scene("bunnymark_map_phase")
    elseif ctx.item_id == "screen_phase" then
        engine.change_scene("bunnymark_screen_phase")
    end
end

--- Called each frame while bunnymark_menu scene is active.
--- @param input InputSnapshot
local function on_update_bunnymark_menu(input)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

-- ─── Callback registry ────────────────────────────────────────────────────────

M._callbacks = {
    on_bunnymark_menu_select  = on_bunnymark_menu_select,
    on_update_bunnymark_menu  = on_update_bunnymark_menu,
}

-- ─── Spawn ────────────────────────────────────────────────────────────────────

function M.spawn()
    engine.log_debug("Spawning Bunnymark sub-menu...")

    engine.set_render_size(800, 450)
    engine.set_vsync(true)
    engine.set_target_fps(120)
    engine.set_camera(0, 0, 0, 0, 0.0, 1.0)
    engine.set_background_color(10, 10, 16)
    engine.post_process_shader(nil)

    -- Title
    engine.spawn()
        :with_text("BUNNYMARK", "future", 36, 255, 255, 255, 255)
        :with_screen_position(8, 20)
        :with_zindex(1)
        :build()

    -- Subtitle
    engine.spawn()
        :with_text("select variant", "arcade", 12, 160, 160, 160, 255)
        :with_screen_position(10, 68)
        :with_zindex(1)
        :build()

    -- Cursor
    engine.spawn()
        :with_sprite("cursor", 16, 16, 16 + 8, 0)
        :register_as("bunnymark_menu_cursor")
        :build()

    -- Variant menu
    engine.spawn()
        :with_group("bunnymark_menu")
        :with_menu(
            {
                { id = "map_loop",    label = "MapPosition  + Lua Loop" },
                { id = "screen_loop", label = "ScreenPosition + Lua Loop" },
                { id = "map_phase",   label = "MapPosition  + Phase Callback" },
                { id = "screen_phase",label = "ScreenPosition + Phase Callback" },
            },
            16 + 8,
            100,
            "arcade",
            16,
            24,
            false
        )
        :with_menu_cursor("bunnymark_menu_cursor")
        :with_menu_selection_sound("option")
        :with_menu_callback("on_bunnymark_menu_select")
        :build()

    engine.log_debug("Bunnymark sub-menu entities queued!")
end

return M
