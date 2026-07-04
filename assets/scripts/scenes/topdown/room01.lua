-- scenes/topdown/room01.lua
-- Top-down room scaffold with a minimal playable scene shell.

local M = {}

-- ─── Callbacks (local — injected into _G by main.lua) ───────────────────────

--- Called each frame when topdown_room01 scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_topdown_room01(input, dt)
	if input.digital.back.just_pressed then
		engine.change_scene("menu")
	end
end

-- ─── Callback registry ──────────────────────────────────────────────────────

M._callbacks = {
	on_update_topdown_room01 = on_update_topdown_room01,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn all entities for the top-down room scene.
function M.spawn()
	engine.log_debug("Spawning topdown room01 scene...")
	engine.stop_all_music()

	engine.set_render_size(640, 360)
	engine.set_render_target_filter("nearest")
	engine.set_vsync(true)
	engine.set_target_fps(120)
	engine.set_camera(0.0, 0.0, 320.0, 180.0, 0.0, 1.0)
	engine.set_background_color(18, 24, 30)
	engine.post_process_shader(nil)

	engine.spawn()
		:with_text("TOP-DOWN ROOM 01", "future", 28, 255, 255, 255, 255)
		:with_screen_position(16, 20)
		:with_zindex(1)
		:build()

	engine.spawn()
		:with_text("scene boilerplate", "arcade", 12, 180, 180, 180, 255)
		:with_screen_position(18, 58)
		:with_zindex(1)
		:build()

	engine.spawn()
		:with_text("Press Back to return to menu", "arcade", 12, 200, 200, 200, 255)
		:with_screen_position(18, 86)
		:with_zindex(1)
		:build()

	engine.log_debug("Topdown room01 scene entities queued!")
end

return M
