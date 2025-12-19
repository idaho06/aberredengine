-- scenes/menu.lua
-- Menu scene entity spawning
-- Called when switching to the "menu" scene

local M = {}

--- Spawn all entities for the menu scene.
--- This demonstrates the entity spawning API.
function M.spawn()
    engine.log_info("Spawning menu scene entities from Lua...")

    -- Set camera centered at origin with screen offset at center
    -- target: (0, 0) - center of the menu scene
    -- offset: center of screen (672x768 window)
    local camera_offset_x = 336.0 -- 672 / 2
    local camera_offset_y = 384.0 -- 768 / 2
    engine.set_camera(0.0, 0.0, camera_offset_x, camera_offset_y, 0.0, 1.0)

    -- Spawn the background (scaled 3x)
    engine.spawn()
        :with_position(0, 0)
        :with_sprite("background", 672, 768, 336, 384) -- width, height, origin_x, origin_y
        :with_zindex(0)
        :with_scale(3, 3)
        :register_as("menu_background")
        :build()

    -- Spawn the animated title
    -- Starts at y=384, tweens to y=-220 over 2 seconds
    -- Rotates back and forth between -10 and 10 degrees
    -- Scales between 0.9 and 1.1
    engine.spawn()
        :with_group("title")
        :with_position(0, 384)
        :with_sprite("title", 672, 198, 336, 99) -- width, height, origin_x, origin_y
        :with_zindex(1)
        :with_rotation(0)
        :with_scale(1, 1)
        :with_tween_position(0, 384, 0, -220, 2.0)
        :with_tween_position_easing("quad_out")
        :with_tween_position_loop("once")
        :with_tween_rotation(-10, 10, 2.0)
        :with_tween_rotation_easing("quad_in_out")
        :with_tween_rotation_loop("ping_pong")
        :with_tween_scale(0.9, 0.9, 1.1, 1.1, 1.0)
        :with_tween_scale_easing("quad_in_out")
        :with_tween_scale_loop("ping_pong")
        :with_lua_timer(4.0, "on_timer_title_test") -- example timer callback
        :build()

    -- Spawn cursor first so the menu can reference it by key.
    engine.spawn()
        :with_sprite("cursor", 48, 48, 56, 0)
        :register_as("menu_cursor")
        :build()

    -- Spawn menu (Menu + MenuActions)
    engine.spawn()
        :with_group("main_menu")
        :with_menu(
            {
                { id = "start_game", label = "Start Game" },
                { id = "options",    label = "Options" },
                { id = "exit",       label = "Exit" },
            },
            250,
            350,
            "arcade",
            48,
            64,
            true
        )
        :with_menu_colors(255, 255, 0, 255, 255, 255, 255, 255)
        :with_menu_dynamic_text(true)
        :with_menu_cursor("menu_cursor")
        :with_menu_selection_sound("option")
        :with_menu_action_set_scene("start_game", "level01")
        :with_menu_action_show_submenu("options", "options")
        :with_menu_action_quit("exit")
        :build()

    -- Play menu music
    engine.play_music("menu", true)

    engine.log_info("Menu scene entities queued!")
end

--- Called each frame when menu scene is active.
--- @param dt number Delta time in seconds
function on_update_menu(dt)
    -- Check for back button to quit game
    if engine.is_action_back_just_pressed() then
        engine.set_flag("quit_game")
    end

    -- Note: Menu actions (scene switching) are handled by the menu system,
    -- so no additional logic is needed here for that.
end

function on_timer_title_test(entity_id)
    -- Test callback for Lua timer on title entity_id
    engine.log_info("Title timer test callback triggered for entity ID: " .. tostring(entity_id))
    -- Create another lua timer attached to the background entity as a demonstration
    local bg_entity = engine.get_entity("menu_background")
    if bg_entity then
        engine.entity_insert_lua_timer(bg_entity, 3.0, "on_timer_background_test")
    end
    -- remove this timer so it doesn't repeat
    engine.entity_remove_lua_timer(entity_id)
end

function on_timer_background_test(entity_id)
    -- Test callback for Lua timer on background entity_id
    engine.log_info("Background timer test callback triggered for entity ID: " .. tostring(entity_id))
    -- Add a tween scale effect to the background as a demonstration
    engine.entity_insert_tween_scale(entity_id, 3.0, 3.0, 3.2, 3.2, 2.0, "cubic_in_out", "ping_pong")
    -- remove this timer so it doesn't repeat
    engine.entity_remove_lua_timer(entity_id)
end

return M
