-- scenes/menu.lua
-- Menu scene entity spawning
-- Called when switching to the "menu" scene

local M = {}

--- Spawn all entities for the menu scene.
--- This demonstrates the entity spawning API.
function M.spawn()
    engine.log_info("Spawning menu scene entities from Lua...")

    -- Set camera centered at origin with screen offset at center
    -- target: (0, 0) -
    -- offset: (0, 0) - origin is at top-left corner
    local camera_offset_x = 0.0
    local camera_offset_y = 0.0
    engine.set_camera(0.0, 0.0, camera_offset_x, camera_offset_y, 0.0, 1.0)

    -- Spawn the background (scaled 3x)
    --[[     engine.spawn()
        :with_position(0, 0)
        :with_sprite("background", 672, 768, 336, 384) -- width, height, origin_x, origin_y
        :with_zindex(0)
        :with_scale(3, 3)
        :register_as("menu_background")
        :build()
 ]]
    -- stars particles
    engine.spawn()
        :with_sprite("stars01_sheet", 32, 32, 16, 16)
        :with_zindex(-5)
        :register_as("star_particle01")
        :build()

    -- Spawn the title
    engine.spawn()
        :with_text("DRIFTERS", "future", 64, 255, 255, 255, 255)
        :with_screen_position(8, 32)
        :with_zindex(1)
        :register_as("menu_title")
        :build()

    -- Spawn cursor first so the menu can reference it by key.
    engine.spawn()
        :with_sprite("cursor", 16, 16, 16 + 8, 0)
        :register_as("menu_cursor")
        :build()

    -- Spawn menu with Lua callback for selection handling
    engine.spawn()
        :with_group("main_menu")
        :with_menu(
            {
                { id = "start_game", label = "Start Game" },
                { id = "options",    label = "Options" },
                { id = "exit",       label = "Exit" },
            },
            16 + 8,
            64 + 16,
            "arcade",
            16,
            24,
            false
        )
    -- :with_menu_colors(255, 255, 0, 255, 255, 0, 0, 255)
    -- :with_menu_dynamic_text(true)
        :with_menu_cursor("menu_cursor")
        :with_menu_selection_sound("option")
        :with_menu_callback("on_main_menu_select")
    -- :with_menu_visible_count(5)
        :build()

    -- Play menu music
    -- engine.play_music("menu", true)

    -- Fill the background with black
    engine.spawn()
        :with_sprite("black", 64, 64, 0, 0)
        :with_position(0, 0)
        :with_zindex(-10)
        :with_scale(10, 10)
        :build()

    -- stars generator
    engine.spawn()
        :with_position(660, 360 / 2)
        :with_particle_emitter({
            templates = { "star_particle01" },
            shape = { type = "rect", width = 10, height = 360 },
            particles_per_emission = 1,
            emissions_per_second = 3,
            emissions_remaining = 4294967295,
            arc = { -90, -90 },
            speed = { 60, 60 },
            ttl = 15.0,
        })
        :build()

    --[[     engine.spawn()
        :with_position(100, 360 / 2)
        :with_sprite("explosion01_sheet", 64, 64, 32, 32)
        :with_animation("explosion01")
        :with_zindex(10)
    -- :with_signals()
    -- :register_as("explosion01") -- Store entity ID for cloning
        :build()

    engine.spawn()
        :with_position(200, 360 / 2)
        :with_sprite("explosion02_sheet", 32, 32, 16, 16)
        :with_animation("explosion02")
        :with_zindex(10)
    -- :with_signals()
    -- :register_as("explosion02") -- Store entity ID for cloning
        :build()

    engine.spawn()
        :with_position(300, 360 / 2)
        :with_sprite("explosion03_sheet", 16, 16, 8, 8)
        :with_animation("explosion03")
        :with_zindex(10)
    -- :with_signals()
    -- :register_as("explosion03") -- Store entity ID for cloning
        :build()
 ]]
    -- post-process shader
    -- engine.post_process_shader({ "wave" })
    -- engine.post_process_shader({ "bloom" })
    engine.post_process_shader(nil)

    engine.post_process_set_float("amplitude", 0.01)
    engine.post_process_set_float("length", 10.0)
    engine.post_process_set_float("speed", 2.0)


    engine.log_info("Menu scene entities queued!")
end

--- Callback function for main menu selection.
--- @param ctx table Context with menu_id (u64), item_id (string), item_index (integer)
function on_main_menu_select(ctx)
    engine.log_info("Menu selected: " .. ctx.item_id .. " (index " .. ctx.item_index .. ")")

    if ctx.item_id == "start_game" then
        engine.set_string("scene", "level01")
        engine.set_flag("switch_scene")
    elseif ctx.item_id == "options" then
        -- Options menu not implemented yet
        engine.log_info("Options menu not implemented")
    elseif ctx.item_id == "exit" then
        engine.set_flag("quit_game")
    end
end

--- Called each frame when menu scene is active.
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function on_update_menu(input, dt)
    -- Check for back button to quit game
    if input.digital.back.just_pressed then
        engine.set_flag("quit_game")
    end

    -- Note: Menu actions (scene switching) are handled by the menu system,
    -- so no additional logic is needed here for that.
end

--[[ function on_timer_title_test(ctx, input)
    -- Test callback for Lua timer on title entity
    engine.log_info("Title timer test callback triggered for entity ID: " .. tostring(ctx.id))
    -- Create another lua timer attached to the background entity as a demonstration
    local bg_entity = engine.get_entity("menu_background")
    if bg_entity then
        engine.entity_insert_lua_timer(bg_entity, 3.0, "on_timer_background_test")
    end
    -- remove this timer so it doesn't repeat
    engine.entity_remove_lua_timer(ctx.id)
end ]]

--[[ function on_timer_background_test(ctx, input)
    -- Test callback for Lua timer on background entity
    engine.log_info("Background timer test callback triggered for entity ID: " .. tostring(ctx.id))
    -- Add a tween scale effect to the background as a demonstration
    engine.entity_insert_tween_scale(ctx.id, 3.0, 3.0, 3.2, 3.2, 2.0, "cubic_in_out", "ping_pong")
    -- remove this timer so it doesn't repeat
    engine.entity_remove_lua_timer(ctx.id)
end ]]

return M
