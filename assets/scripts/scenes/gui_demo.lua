-- scenes/gui_demo.lua
-- GuiWindow + a GuiOffset child GuiLabel + a real GuiButton, exercising the
-- Child Layout Model (gui_layout_system) and hit-test/click
-- (gui_hit_test_system + gui_button_click_observer). See
-- docs/gui-system-architecture.md for the full design.

local M = {}

-- ─── Callbacks (local — injected into _G by main.lua) ───────────────────────

--- Fires the frame after the window spawns (ctx.id = window's real entity
-- id) — :with_parent() needs a live entity id, which a single Lua callback
-- can't get synchronously for an entity it just queued via :build(). See
-- docs/gui-system-architecture.md, "Window/children build order".
--- @param ctx EntityContext
local function build_gui_demo_window(ctx)
    -- GuiLabel's caption spawns in the same call as the label itself (no
    -- :with_lua_setup() round trip needed here), exactly like GuiButton's.
    engine.spawn()
        :with_gui_label(160, 24, "Hello, GUI!", "arcade")
        :with_parent(ctx.id)
        :with_gui_offset(16, 16)
        :with_zindex(2)
        :build()

    -- GuiButton's caption spawns in the same call as the button itself (no
    -- :with_lua_setup() round trip needed here) — see
    -- docs/gui-system-architecture.md's Components section.
    engine.spawn()
        :with_gui_button(100, 20, "Click Me", "arcade", "on_gui_demo_button_clicked")
        :with_parent(ctx.id)
        :with_gui_offset(16, 50)
        :with_zindex(2)
        :build()
end

--- Fired by gui_button_click_observer when the demo button is clicked.
local function on_gui_demo_button_clicked()
    engine.log_debug("GUI Demo button clicked!")
end

--- Called each frame when gui_demo scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_gui_demo(input, dt)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

-- ─── Callback registry ──────────────────────────────────────────────────────

M._callbacks = {
    build_gui_demo_window = build_gui_demo_window,
    on_gui_demo_button_clicked = on_gui_demo_button_clicked,
    on_update_gui_demo = on_update_gui_demo,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn all entities for the GUI demo scene.
function M.spawn()
    engine.log_debug("Spawning GUI demo scene...")

    engine.set_render_size(640, 360)
    engine.set_camera(0.0, 0.0, 0.0, 0.0, 0.0, 1.0)
    engine.set_background_color(20, 24, 30)

    -- Theme: bluewindow_6_6_6_6.png is 64x64 with 6px nine-patch borders on
    -- all sides (encoded in the filename). Set here, not in setup.lua's
    -- load_gui_demo(): this queues a RenderCmd, and RenderCmd's queue has
    -- "clear" policy (wiped by clear_all_commands() at the start of every
    -- switch_scene) — unlike AssetCmd's "preserve" queue, it would never
    -- survive being queued during on_setup(), before the first scene switch.
    engine.set_gui_theme_panel("gui-bluewindow", 0, 0, 64, 64, 6, 6, 6, 6)

    -- Button skin: button_atlas_8_8_8_8.png is a 128x128 2x2 grid of 64x64
    -- cells (top-left=normal, top-right=hover, bottom-left=pressed,
    -- bottom-right=disabled), 8px nine-patch borders on all sides (encoded
    -- in the filename, same convention as bluewindow_6_6_6_6.png above).
    engine.set_gui_theme_button("normal", "gui-button-atlas", 0, 0, 64, 64, 8, 8, 8, 8)
    -- engine.set_gui_theme_button("hover", "gui-button-atlas", 64, 0, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_button("pressed", "gui-button-atlas", 0, 64, 64, 64, 8, 8, 8, 8)
    -- engine.set_gui_theme_button("disabled", "gui-button-atlas", 64, 64, 64, 64, 8, 8, 8, 8)

    -- Label skin: label_6_6_6_6.png is 64x64 with 6px nine-patch borders on
    -- all sides (encoded in the filename, same convention as
    -- bluewindow_6_6_6_6.png above).
    engine.set_gui_theme_label("gui-label", 0, 0, 64, 64, 6, 6, 6, 6)

    engine.spawn()
        :with_gui_window(200, 150)
        :with_screen_position(220, 105)
        :with_zindex(0)
        :with_lua_setup("build_gui_demo_window")
        :build()

    engine.spawn()
        :with_text("GUI Demo - press Back to return", "arcade", 16, 200, 200, 200, 255)
        :with_screen_position(140, 20)
        :with_zindex(1)
        :build()

    engine.log_debug("GUI demo scene entities queued!")
end

return M
