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

return M
