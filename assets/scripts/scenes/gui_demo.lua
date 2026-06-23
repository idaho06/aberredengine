-- scenes/gui_demo.lua
-- GuiWindow + a GuiOffset child GuiLabel + a real GuiButton, exercising the
-- Child Layout Model (gui_layout_system) and hit-test/click
-- (gui_hit_test_system + gui_button_click_observer). See
-- docs/gui-system-architecture.md for the full design.

local M = {}

-- Second window's shown/hidden ScreenPosition + slide-in/out duration —
-- shared between the Show/Hide callbacks below.
local WINDOW2_X = 170
local WINDOW2_SHOWN_Y = 260
local WINDOW2_HIDDEN_Y = 400
local WINDOW2_ANIM_DURATION = 1.0

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
        :with_gui_label(160, 24, "Hello, GUI!")
        :with_parent(ctx.id)
        :with_gui_offset(16, 16)
        :with_zindex(2)
        :build()

    -- GuiButton's caption spawns in the same call as the button itself (no
    -- :with_lua_setup() round trip needed here) — see
    -- docs/gui-system-architecture.md's Components section.
    engine.spawn()
        :with_gui_button(100, 20, "Click Me", "on_gui_demo_button_clicked")
        :with_parent(ctx.id)
        :with_gui_offset(16, 50)
        :with_zindex(2)
        :build()

    -- Shows the second window (spawned with no ScreenPosition of its own —
    -- see build_gui_demo_window2) by sliding it in from off-screen.
    engine.spawn()
        :with_gui_button(80, 20, "Show", "on_show_window2_clicked")
        :with_parent(ctx.id)
        :with_gui_offset(16, 80)
        :with_zindex(2)
        :build()
end

--- Second window: spawned with no :with_screen_position() at all, so it has
-- no ScreenPosition component and is invisible from frame 0 (visibility is
-- presence/absence of ScreenPosition, per docs/gui-system-architecture.md).
-- on_show_window2_clicked is what first gives it a ScreenPosition.
--- @param ctx EntityContext
local function build_gui_demo_window2(ctx)
    -- Looked up by id from the Show/Hide button callbacks below, which run
    -- with a *different* entity's ctx (the button that was clicked, not this
    -- window) — engine.get_entity/set_entity is the existing mechanism for
    -- handing an entity id across unrelated callbacks.
    engine.set_entity("gui_demo_window2", ctx.id)

    -- Standalone styled DynamicText (not a themed GuiLabel caption) — a
    -- different font/size/color than the GuiTheme default, demonstrating the
    -- manual-child-entity escape hatch documented for GuiLabel/GuiButton.
    -- Embedded \n renders as a real line break (raylib's draw/measure text
    -- already handle it natively, no engine support needed).
    engine.spawn()
        :with_text("This is a different text style!\nAnd a new line here.", "future", 16, 255, 255, 0, 255)
        :with_parent(ctx.id)
        :with_gui_offset(16, 16)
        :with_zindex(2)
        :build()

    -- Mixed-theme demo: window 2's button uses the "compact" theme (set up
    -- in M.spawn() below) instead of "default" — themes are flat/explicit
    -- per widget, not inherited from the parent GuiWindow, so this call is
    -- required even though build_gui_demo_window2's own GuiWindow already
    -- set :with_gui_theme_key("compact"). See docs/gui-system-architecture.md,
    -- Roadmap item #2.
    engine.spawn()
        :with_gui_button(80, 20, "Hide", "on_hide_window2_clicked")
        :with_gui_theme_key("compact")
        :with_parent(ctx.id)
        :with_gui_offset(16, 60)
        :with_zindex(2)
        :build()
end

--- Fired by gui_button_click_observer when the demo button is clicked.
local function on_gui_demo_button_clicked()
    engine.log_debug("GUI Demo button clicked!")
end

--- Slides window 2 in from off-screen at the bottom. Inserts both
-- ScreenPosition (the window has none yet) and the tween in one call, via
-- engine.entity_insert_tween_screen_position.
local function on_show_window2_clicked()
    if engine.has_flag("gui_demo_window2_visible") then
        return
    end
    local id = engine.get_entity("gui_demo_window2")
    if id == nil then
        engine.log_error("gui_demo_window2 entity not found!")
        return
    end
    engine.entity_insert_tween_screen_position(
        id, WINDOW2_X, WINDOW2_HIDDEN_Y, WINDOW2_X, WINDOW2_SHOWN_Y,
        WINDOW2_ANIM_DURATION, "quad_out", "once", false, ""
    )
    engine.set_flag("gui_demo_window2_visible")
end

--- Slides window 2 back out, then removes its ScreenPosition once the
-- slide-out tween finishes — entity_insert_tween_screen_position's trailing
-- on_finished arg calls on_hide_window2_tween_done directly, no separate
-- LuaTimer needed to fake a completion signal.
local function on_hide_window2_clicked()
    if not engine.has_flag("gui_demo_window2_visible") then
        return
    end
    local id = engine.get_entity("gui_demo_window2")
    if id == nil then
        engine.log_error("gui_demo_window2 entity not found!")
        return
    end
    engine.entity_insert_tween_screen_position(
        id, WINDOW2_X, WINDOW2_SHOWN_Y, WINDOW2_X, WINDOW2_HIDDEN_Y,
        WINDOW2_ANIM_DURATION, "quad_in", "once", false, "on_hide_window2_tween_done"
    )
    engine.clear_flag("gui_demo_window2_visible")
end

--- Fired once the slide-out tween above finishes — removes ScreenPosition so
-- window 2 (and its children, via gui_layout_system's existing
-- hide-cascade) goes fully invisible again, not just off-screen.
local function on_hide_window2_tween_done(ctx)
    engine.entity_remove_screen_position(ctx.id)
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
    build_gui_demo_window2 = build_gui_demo_window2,
    on_gui_demo_button_clicked = on_gui_demo_button_clicked,
    on_show_window2_clicked = on_show_window2_clicked,
    on_hide_window2_clicked = on_hide_window2_clicked,
    on_hide_window2_tween_done = on_hide_window2_tween_done,
    on_update_gui_demo = on_update_gui_demo,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn all entities for the GUI demo scene.
function M.spawn()
    engine.log_debug("Spawning GUI demo scene...")

    engine.set_render_size(640, 360)
    engine.set_camera(0.0, 0.0, 0.0, 0.0, 0.0, 1.0)
    engine.set_background_color(20, 24, 30)

    -- GUI themes ("default" + "compact") are registered once in setup.lua's
    -- load_gui_demo(), called from on_setup() — gui_theme_commands is a
    -- "preserve"-policy queue (see queue_registry.rs), so theme setup
    -- belongs with the rest of this scene's asset loading rather than being
    -- re-asserted here every time the scene is entered.

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

    -- Reset in case this scene is being re-entered (the flag is a generic
    -- WorldSignals flag, not scene-scoped, and window 2 is freshly spawned
    -- with no ScreenPosition every time this function runs).
    engine.clear_flag("gui_demo_window2_visible")

    -- No :with_screen_position() here — window 2 starts fully invisible;
    -- on_show_window2_clicked is what gives it one. Uses the "compact"
    -- theme (registered alongside "default" in setup.lua's load_gui_demo())
    -- to demonstrate two named themes coexisting in the same scene.
    engine.spawn()
        :with_gui_window(300, 90)
        :with_gui_theme_key("compact")
        :with_zindex(0)
        :with_lua_setup("build_gui_demo_window2")
        :build()

    engine.log_debug("GUI demo scene entities queued!")
end

return M
