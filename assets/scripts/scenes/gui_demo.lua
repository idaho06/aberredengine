-- scenes/gui_demo.lua
-- GuiWindow + a GuiOffset child GuiLabel + a real GuiButton, exercising the
-- Child Layout Model (gui_layout_system) and hit-test/click
-- (gui_hit_test_system + gui_interactable_click_observer). See
-- docs/gui-system-architecture.md for the full design.

local M = {}

-- Second window's shown/hidden ScreenPosition + slide-in/out duration —
-- shared between the Show/Hide callbacks below.
local WINDOW2_X = 10
local WINDOW2_SHOWN_Y = 265
local WINDOW2_HIDDEN_Y = 400
local WINDOW2_ANIM_DURATION = 1.0

-- "wave" signal: a 0.5 Hz / amplitude-100 sine wave, recomputed every frame
-- from accumulated dt (the engine exposes no direct elapsed-time getter to
-- Lua, see CLAUDE.md's input/signal API surface) and pushed into
-- WorldSignals — the standalone "Wave" GuiLabel below reads it back via
-- :with_gui_label_signal_binding("wave"), demonstrating the signal-bound
-- dynamic label feature with no per-frame Lua-side text manipulation.
local WAVE_FREQUENCY_HZ = 0.1
local WAVE_AMPLITUDE = 100.0
local wave_elapsed_time = 0.0

-- Tracks how window 2 was last shown so the Hide button can mirror it
-- symmetrically — slide back out if it slid in, vanish instantly if it
-- appeared instantly.
local SHOW_MODE_SLIDE = "slide"
local SHOW_MODE_INSTANT = "instant"
local window2_show_mode = SHOW_MODE_SLIDE

-- Tracks the sword attack cooldown so on_update can display remaining seconds.
-- Set to the random duration on click; decremented each frame; 0 = not on cooldown.
local sword_cooldown_secs = 0.0
-- Last value written to "char_sword_cooldown"; guards against redundant set_string
-- calls (each call clones the WorldSignals strings map even when value is unchanged).
local sword_cooldown_last_display = ""

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
        :with_gui_button(80, 20, "Slide", "on_slide_window2_clicked")
        :with_parent(ctx.id)
        :with_gui_offset(16, 80)
        :with_zindex(2)
        :build()

    -- Shows the second window instantly, by giving it a ScreenPosition
    -- directly instead of tweening one in.
    engine.spawn()
        :with_gui_button(80, 20, "Appear", "on_appear_window2_clicked")
        :with_parent(ctx.id)
        :with_gui_offset(16, 110)
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

    -- Disabled button using the "default" theme (not "compact", unlike its
    -- siblings here) — demonstrates a disabled GuiButton plus mixed themes
    -- within the same window.
    engine.spawn()
        :with_gui_button(180, 20, "Can't touch this!", "")
        :with_gui_button_disabled()
        :with_parent(ctx.id)
        :with_gui_offset(106, 60)
        :with_zindex(2)
        :build()
end

-- ─── Character window helpers ────────────────────────────────────────────────

--- Returns true and logs nothing when the game is still active; returns false
-- (after no-op) when the game-over flag is set. Used as a guard at the top
-- of every action callback to prevent double-fires after game ends.
local function char_game_active()
    return not engine.has_flag("char_game_over")
end

--- Re-evaluates which buttons should be enabled based on current signal state.
-- Called after any action that changes potions or gold.
-- Sword cooldown is NOT managed here — on_sword_cooldown_done handles that.
local function check_button_states()
    local potions = engine.get_integer("char_potions")
    local gold    = engine.get_integer("char_gold")

    local potion_id = engine.get_entity("char_potion")
    local sell_id   = engine.get_entity("char_sell")
    local buy_id    = engine.get_entity("char_buy")

    if potion_id then engine.entity_set_gui_disabled(potion_id, potions <= 0) end
    if sell_id   then engine.entity_set_gui_disabled(sell_id,   potions <= 0) end
    if buy_id    then engine.entity_set_gui_disabled(buy_id,    gold < 2)     end
end

--- Ends the game: sets game-over flag, displays a final message, stops both
-- timers, and disables all four interactive buttons.
local function end_game(message)
    engine.set_flag("char_game_over")
    engine.set_string("char_message", message)

    local timer_id  = engine.get_entity("char_enemy_timer")
    local potion_id = engine.get_entity("char_potion")
    local sword_id  = engine.get_entity("char_sword")
    local buy_id    = engine.get_entity("char_buy")
    local sell_id   = engine.get_entity("char_sell")

    if timer_id  then engine.entity_remove_lua_timer(timer_id)        end
    if sword_id  then engine.entity_remove_lua_timer(sword_id)        end
    if potion_id then engine.entity_set_gui_disabled(potion_id, true) end
    if sword_id  then engine.entity_set_gui_disabled(sword_id,  true) end
    if buy_id    then engine.entity_set_gui_disabled(buy_id,    true) end
    if sell_id   then engine.entity_set_gui_disabled(sell_id,   true) end
    sword_cooldown_secs = 0.0
    sword_cooldown_last_display = ""
    engine.set_string("char_sword_cooldown", "")
end

-- ─── Character window: child setup callbacks ─────────────────────────────────

--- Stores the enemy timer entity id and fires the first attack timer.
--- @param ctx EntityContext
local function on_char_enemy_timer_setup(ctx)
    engine.set_entity("char_enemy_timer", ctx.id)
    engine.entity_insert_lua_timer(ctx.id, math.random(5, 10), "on_enemy_attack")
end

-- ─── Character window: action callbacks ──────────────────────────────────────

--- Fires when the enemy attack timer ticks. Removes itself, deals damage,
-- then reschedules with a new random delay (variable interval).
--- @param ctx EntityContext
local function on_enemy_attack(ctx)
    engine.entity_remove_lua_timer(ctx.id)
    if not char_game_active() then return end

    local dmg = math.random(1, 10)
    local hp  = engine.get_integer("char_hp") - dmg
    engine.set_string("char_message", "Enemy dealt\n" .. dmg .. " damage!")

    if hp <= 0 then
        engine.set_integer("char_hp", 0)
        end_game("Defeated!\nEnemy wins.")
    else
        engine.set_integer("char_hp", hp)
        engine.entity_insert_lua_timer(ctx.id, math.random(5, 10), "on_enemy_attack")
    end
end

--- Use a potion: restore 5–10 HP (capped at 100), consume one potion.
local function on_potion_clicked()
    if not char_game_active() then return end
    local potions = engine.get_integer("char_potions")
    if potions <= 0 then return end

    local heal = math.random(5, 10)
    local hp   = math.min(engine.get_integer("char_hp") + heal, 100)
    engine.set_integer("char_hp",      hp)
    engine.set_integer("char_potions", potions - 1)
    engine.set_string("char_message",  "Recovered " .. heal .. " HP!")
    check_button_states()
end

--- Attack the enemy: deal 1–10 damage and start sword cooldown.
local function on_sword_clicked(evt)
    if not char_game_active() then return end
    local sword_id = evt.entity_id

    local cooldown = math.random(5, 8)
    sword_cooldown_secs = cooldown
    engine.entity_set_gui_disabled(sword_id, true)
    engine.entity_insert_lua_timer(sword_id, cooldown, "on_sword_cooldown_done")

    local dmg      = math.random(1, 10)
    local enemy_hp = engine.get_integer("char_enemy_hp") - dmg
    engine.set_string("char_message", "You dealt\n" .. dmg .. " damage!")

    if enemy_hp <= 0 then
        engine.set_integer("char_enemy_hp", 0)
        end_game("Victory!\nEnemy defeated.")
    else
        engine.set_integer("char_enemy_hp", enemy_hp)
    end
end

--- Re-enables the sword after its cooldown expires.
--- @param ctx EntityContext
local function on_sword_cooldown_done(ctx)
    engine.entity_remove_lua_timer(ctx.id)
    sword_cooldown_secs = 0.0
    sword_cooldown_last_display = ""
    engine.set_string("char_sword_cooldown", "")
    if char_game_active() then
        engine.entity_set_gui_disabled(ctx.id, false)
    end
end

--- Buy a potion for 2 gold.
local function on_char_buy_clicked()
    if not char_game_active() then return end
    local gold = engine.get_integer("char_gold")
    if gold < 2 then return end

    engine.set_integer("char_gold",    gold - 2)
    engine.set_integer("char_potions", engine.get_integer("char_potions") + 1)
    engine.set_string("char_message",  "Bought a potion for\n2 gold!")
    check_button_states()
end

--- Sell a potion for 1 gold.
local function on_char_sell_clicked()
    if not char_game_active() then return end
    local potions = engine.get_integer("char_potions")
    if potions <= 0 then return end

    engine.set_integer("char_potions", potions - 1)
    engine.set_integer("char_gold",    engine.get_integer("char_gold") + 1)
    engine.set_string("char_message",  "Sold a potion for\n1 gold!")
    check_button_states()
end

-- ─── Character window: main build callback ────────────────────────────────────

--- Spawns all children of the Character window one frame after the window
-- entity itself is created. Uses :with_lua_setup on interactive children so
-- their entity ids can be stored for later disable/enable calls.
--- @param ctx EntityContext
local function build_character_window(ctx)
    -- Title label
    engine.spawn()
        :with_gui_label(181, 20, "Character")
        :with_parent(ctx.id)
        :with_gui_offset(8, 10)
        :with_zindex(2)
        :build()

    -- Potion GuiImage — 2×2 atlas, 32×32 cells:
    -- Normal(0,0)  Hover(32,0)  Pressed/Selected(0,32)  Disabled(32,32)
    engine.spawn()
        :with_gui_image(32, 32, "gui-potion-btn", 0, 0, "on_potion_clicked")
        :with_gui_image_hover_offset(32, 0)
        :with_gui_image_pressed_offset(0, 32)
        :with_gui_image_disabled_offset(32, 32)
        :register_as("char_potion")
        :with_parent(ctx.id)
        :with_gui_offset(8, 36)
        :with_zindex(2)
        :build()

    -- Potion count label (signal-bound)
    engine.spawn()
        :with_gui_label(32, 20, "x3")
        :with_gui_label_signal_binding("char_potions")
        :with_gui_label_signal_binding_format("x{}")
        :with_gui_theme_key("compact")
        :with_parent(ctx.id)
        :with_gui_offset(46, 43)
        :with_zindex(2)
        :build()

    -- "+" buy button
    engine.spawn()
        :with_gui_button(28, 20, "+", "on_char_buy_clicked")
        :with_gui_theme_key("compact")
        :register_as("char_buy")
        :with_parent(ctx.id)
        :with_gui_offset(82, 43)
        :with_zindex(2)
        :build()

    -- "-" sell button
    engine.spawn()
        :with_gui_button(28, 20, "-", "on_char_sell_clicked")
        :with_gui_theme_key("compact")
        :register_as("char_sell")
        :with_parent(ctx.id)
        :with_gui_offset(113, 43)
        :with_zindex(2)
        :build()

    -- Sword GuiImage — same 2×2 atlas layout as the potion button
    engine.spawn()
        :with_gui_image(32, 32, "gui-sword-btn", 0, 0, "on_sword_clicked")
        :with_gui_image_hover_offset(32, 0)
        :with_gui_image_pressed_offset(0, 32)
        :with_gui_image_disabled_offset(32, 32)
        :register_as("char_sword")
        :with_parent(ctx.id)
        :with_gui_offset(8, 80)
        :with_zindex(2)
        :build()

    -- Sword cooldown countdown: shows "Xs" while the sword is on cooldown,
    -- empty when ready. Positioned to the right of the sword image.
    engine.spawn()
        :with_gui_label(40, 18, "")
        :with_gui_label_signal_binding("char_sword_cooldown")
        :with_gui_label_signal_binding_format("{}")
        :with_gui_theme_key("compact")
        :with_parent(ctx.id)
        :with_gui_offset(46, 88)
        :with_zindex(2)
        :build()

    -- Message label (signal-bound to "char_message"): starts blank, updated
    -- by each game action. Empty initial caption is fine here because
    -- spawn_themed_caption creates the DynamicText child whenever a signal
    -- binding is present, even for empty text.
    engine.spawn()
        :with_gui_label(181, 36, "")
        :with_gui_label_signal_binding("char_message")
        :with_gui_label_signal_binding_format("{}")
        :with_gui_theme_key("compact")
        :with_parent(ctx.id)
        :with_gui_offset(8, 120)
        :with_zindex(2)
        :build()

    -- HP stat label
    engine.spawn()
        :with_gui_label(181, 18, "HP: 100")
        :with_gui_label_signal_binding("char_hp")
        :with_gui_label_signal_binding_format("HP: {}")
        :with_gui_theme_key("compact")
        :with_parent(ctx.id)
        :with_gui_offset(8, 162)
        :with_zindex(2)
        :build()

    -- Gold stat label
    engine.spawn()
        :with_gui_label(181, 18, "Gold: 10")
        :with_gui_label_signal_binding("char_gold")
        :with_gui_label_signal_binding_format("Gold: {}")
        :with_gui_theme_key("compact")
        :with_parent(ctx.id)
        :with_gui_offset(8, 182)
        :with_zindex(2)
        :build()

    -- Enemy HP stat label
    engine.spawn()
        :with_gui_label(181, 18, "Enemy HP: 50")
        :with_gui_label_signal_binding("char_enemy_hp")
        :with_gui_label_signal_binding_format("Enemy HP: {}")
        :with_gui_theme_key("compact")
        :with_parent(ctx.id)
        :with_gui_offset(8, 202)
        :with_zindex(2)
        :build()
end

-- ─── Existing demo callbacks ──────────────────────────────────────────────────

--- Fired by gui_interactable_click_observer when the demo button is clicked.
local function on_gui_demo_button_clicked()
    engine.log_debug("GUI Demo button clicked!")
end

--- Shared lookup for window 2's entity, used by all three callbacks below.
-- Returns nil (after logging) if the entity isn't found.
local function find_window2_or_warn()
    local id = engine.get_entity("gui_demo_window2")
    if id == nil then
        engine.log_error("gui_demo_window2 entity not found!")
    end
    return id
end

--- Slides window 2 in from off-screen at the bottom. Inserts both
-- ScreenPosition (the window has none yet) and the tween in one call, via
-- engine.entity_insert_tween_screen_position.
local function on_slide_window2_clicked()
    if engine.has_flag("gui_demo_window2_visible") then
        return
    end
    local id = find_window2_or_warn()
    if id == nil then
        return
    end
    engine.entity_insert_tween_screen_position(
        id, WINDOW2_X, WINDOW2_HIDDEN_Y, WINDOW2_X, WINDOW2_SHOWN_Y,
        WINDOW2_ANIM_DURATION, "quad_out", "once", false, ""
    )
    window2_show_mode = SHOW_MODE_SLIDE
    engine.set_flag("gui_demo_window2_visible")
end

--- Shows window 2 instantly at its resting position, by giving it a
-- ScreenPosition directly instead of tweening one in.
local function on_appear_window2_clicked()
    if engine.has_flag("gui_demo_window2_visible") then
        return
    end
    local id = find_window2_or_warn()
    if id == nil then
        return
    end
    -- entity_set_screen_position only mutates an existing ScreenPosition, and
    -- window 2 starts with none — entity_insert_tween_screen_position with a
    -- zero duration inserts it and snaps in the same call instead.
    engine.entity_insert_tween_screen_position(
        id, WINDOW2_X, WINDOW2_SHOWN_Y, WINDOW2_X, WINDOW2_SHOWN_Y,
        0.0, "linear", "once", false, ""
    )
    window2_show_mode = SHOW_MODE_INSTANT
    engine.set_flag("gui_demo_window2_visible")
end

--- Hides window 2 the way it appeared — sliding back out if it slid in,
-- vanishing instantly if it appeared instantly.
local function on_hide_window2_clicked()
    if not engine.has_flag("gui_demo_window2_visible") then
        return
    end
    local id = find_window2_or_warn()
    if id == nil then
        return
    end
    if window2_show_mode == SHOW_MODE_INSTANT then
        engine.entity_remove_screen_position(id)
    else
        engine.entity_insert_tween_screen_position(
            id, WINDOW2_X, WINDOW2_SHOWN_Y, WINDOW2_X, WINDOW2_HIDDEN_Y,
            WINDOW2_ANIM_DURATION, "quad_in", "once", false, "on_hide_window2_tween_done"
        )
    end
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

    if sword_cooldown_secs > 0.0 then
        sword_cooldown_secs = sword_cooldown_secs - dt
        local display
        if sword_cooldown_secs <= 0.0 then
            sword_cooldown_secs = 0.0
            display = ""
        else
            display = math.ceil(sword_cooldown_secs) .. "s"
        end
        if display ~= sword_cooldown_last_display then
            engine.set_string("char_sword_cooldown", display)
            sword_cooldown_last_display = display
        end
    end

    wave_elapsed_time = wave_elapsed_time + dt
    local wave = WAVE_AMPLITUDE * math.sin(2.0 * math.pi * WAVE_FREQUENCY_HZ * wave_elapsed_time)
    -- Rounded to 1 decimal -- get_scalar's "{}" formatting has no precision
    -- control, so an unrounded f32 can render with many trailing digits.
    engine.set_scalar("wave", math.floor(wave * 10.0 + 0.5) / 10.0)
end

-- ─── Callback registry ──────────────────────────────────────────────────────

M._callbacks = {
    -- Existing demo window callbacks
    build_gui_demo_window      = build_gui_demo_window,
    build_gui_demo_window2     = build_gui_demo_window2,
    on_gui_demo_button_clicked = on_gui_demo_button_clicked,
    on_slide_window2_clicked   = on_slide_window2_clicked,
    on_appear_window2_clicked  = on_appear_window2_clicked,
    on_hide_window2_clicked    = on_hide_window2_clicked,
    on_hide_window2_tween_done = on_hide_window2_tween_done,
    on_update_gui_demo         = on_update_gui_demo,
    -- Character window setup callbacks
    build_character_window     = build_character_window,
    on_char_enemy_timer_setup  = on_char_enemy_timer_setup,
    -- Character window action callbacks
    on_enemy_attack            = on_enemy_attack,
    on_potion_clicked          = on_potion_clicked,
    on_sword_clicked           = on_sword_clicked,
    on_sword_cooldown_done     = on_sword_cooldown_done,
    on_char_buy_clicked        = on_char_buy_clicked,
    on_char_sell_clicked       = on_char_sell_clicked,
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

    -- Window 1 moved left (was x=220, y=105)
    engine.spawn()
        :with_gui_window(200, 150)
        :with_screen_position(10, 80)
        :with_zindex(0)
        :with_lua_setup("build_gui_demo_window")
        :build()

    engine.spawn()
        :with_text("GUI Demo - press Back to return", "arcade", 16, 200, 200, 200, 255)
        :with_screen_position(10, 18)
        :with_zindex(1)
        :build()

    -- Wave label moved to middle zone (was x=520, would overlap Character window)
    engine.spawn()
        :with_gui_label(110, 24, "0.0")
        :with_gui_label_signal_binding("wave")
        :with_gui_label_signal_binding_format("Wave: {}")
        :with_gui_theme_key("compact")
        :with_screen_position(225, 10)
        :with_zindex(2)
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

    -- ── Character window (mini-game) ──────────────────────────────────────
    -- Initialize all game signals so signal-bound labels show correct values
    -- from the first frame, even before the window's LuaSetup fires.
    engine.set_integer("char_hp",        30)
    engine.set_integer("char_gold",      10)
    engine.set_integer("char_potions",    3)
    engine.set_integer("char_enemy_hp",  50)
    engine.set_string("char_message",        "")
    engine.set_string("char_sword_cooldown", "")
    sword_cooldown_secs = 0.0
    sword_cooldown_last_display = ""
    engine.clear_flag("char_game_over")

    engine.spawn()
        :with_gui_window(197, 240)
        :with_screen_position(440, 30)
        :with_zindex(0)
        :with_lua_setup("build_character_window")
        :build()

    -- Enemy attack timer — a standalone entity (no components beyond LuaTimer)
    -- so its id can be stored and the timer removed cleanly on game over.
    -- on_char_enemy_timer_setup registers the id and starts the first tick.
    engine.spawn()
        :with_lua_setup("on_char_enemy_timer_setup")
        :build()

    engine.log_debug("GUI demo scene entities queued!")
end

return M
