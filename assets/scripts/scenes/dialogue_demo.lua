-- scenes/dialogue_demo.lua
-- Dialogue demo: one NPC running a JSON dialogue tree (the gate guard) and a
-- second tree defined inline as a Lua table (the innkeeper), both registered
-- in setup.lua's load_dialogue_demo().
--
-- The NPC spawns with :with_dialogue("guard") plus the three dialogue
-- callbacks. The callbacks copy the plain values they need out of the
-- dialogue tables into module state and drive screen-space text through
-- WorldSignals (dlg_speaker, dlg_text, dlg_hint), because Lua has no runtime
-- text setter. The top-right inventory (keys, swords, coins) binds to the
-- integers the dialogue conditions and effects read and write.
--
-- Every confirm goes through an engine menu component (registered as
-- "dialogue_menu") that handles up/down, confirm and the normal/selected
-- colours itself: a choices node lists its choices (locked ones keep the item
-- id "locked" and a "(locked) " label prefix), a plain node lists one
-- "Continue" item, and an ended dialogue lists the two NPCs to talk to. The
-- scene's update reads no action keys: the menu confirms during input
-- resolution, ahead of the dialogue system in the same tick, so a second
-- reader of the same press would act on the node the press just entered.
-- The menu's items are fixed at spawn, so it is despawned and respawned on
-- every node. Back cancels the dialogue and returns to the menu.

local M = {}

local SCREEN_W, SCREEN_H = 640, 360

-- Module state, reset by M.spawn(). The ctx and dialogue callback tables are
-- never stored: the callbacks copy the plain values they need.
local menu_items  -- 1-based array of { id, label } for the current menu

-- ─── Choice menu ────────────────────────────────────────────────────────────

--- Despawns the current choice menu, when present.
local function despawn_choice_menu()
    local menu = engine.get_entity("dialogue_menu")
    if menu then
        engine.entity_menu_despawn(menu)
    end
end

--- Spawns the menu for the current node from the copied menu_items. The
--- engine handles up/down (wrapping) and confirm itself and draws the
--- normal/selected colours; the items are fixed at spawn, so the menu is
--- respawned per node.
local function spawn_choice_menu()
    engine.spawn()
        :with_menu(menu_items, 28, 244, "default", 14, 16, true)
        :with_menu_colors(140, 200, 255, 255, 255, 214, 110, 255)
        :with_lua_menu_selected("dialogue_demo_menu_selected")
        :register_as("dialogue_menu")
        :build()
end

--- Despawns the current choice menu when present and spawns a new one from
--- the copied menu_items.
local function respawn_choice_menu()
    despawn_choice_menu()
    spawn_choice_menu()
end

-- ─── Signal-driven lines ────────────────────────────────────────────────────

--- Screen-space text line bound to the WorldSignals value `key`; `format`
--- (optional) wraps it, `{}` standing for the value.
local function signal_line(key, x, y, r, g, b, format)
    local line = engine.spawn()
        :with_text("", "default", 14, r, g, b, 255)
        :with_screen_position(x, y)
        :with_z_index(10)
        :with_signal_binding(key)
    if format then
        line = line:with_signal_binding_format(format)
    end
    line:build()
end

--- Clears the speaker/text/hint signals.
local function clear_dialogue_signals()
    engine.set_string("dlg_speaker", "")
    engine.set_string("dlg_text", "")
    engine.set_string("dlg_hint", "")
end

--- Resets the story signals the dialogue trees read and write when the scene
--- starts: the guard's key and gate flags, visit count and the player's inventory
--- (0 keys, 2 swords, 0 coins). Restarting a dialogue keeps them.
local function reset_story_state()
    engine.remove_flag("has_key")
    engine.remove_flag("gate_open")
    engine.set_integer("visits", 0)
    engine.set_integer("keys", 0)
    engine.set_integer("swords", 2)
    engine.set_integer("coins", 0)
end

--- Clears the speaker/text/hint signals, despawns the choice menu and attaches
--- the dialogue registered under `key` to the NPC, restarting the conversation.
local function restart_dialogue(key)
    local npc = engine.get_entity("dialogue_npc")
    if not npc then
        return
    end
    clear_dialogue_signals()
    despawn_choice_menu()
    engine.entity_insert_dialogue(npc, key)
end

-- ─── Callbacks (local — injected into _G by main.lua) ───────────────────────

--- Dialogue node-entered callback: copies the line and the choices' plain
--- values into module state, writes the signal-driven lines and respawns the
--- menu: the node's choices, or one "Continue" item on a node without any.
--- @param ctx EntityContext
--- @param dialogue DialogueLine
local function dialogue_demo_node_entered(ctx, dialogue)
    engine.set_string("dlg_speaker", dialogue.speaker or "")
    engine.set_string("dlg_text", dialogue.text or "")
    menu_items = {}
    for _, choice in ipairs(dialogue.choices) do
        local id = "open"
        local label = choice.text or ""
        if not choice.available then
            id = "locked"
            label = "(locked) " .. label
        end
        menu_items[#menu_items + 1] = { id = id, label = label }
    end
    if #menu_items == 0 then
        menu_items[1] = { id = "continue", label = "Continue" }
    end
    respawn_choice_menu()
    engine.set_string("dlg_hint", "up/down: select - action_1: confirm - back: leave")
end

--- Menu-selected callback: an open pick chooses the matching dialogue choice,
--- "continue" advances, an ended-menu pick restarts a conversation, and a
--- locked pick is refused by respawning the menu.
--- @param ctx table Context with menu_id, item_id (string) and item_index (0-based)
local function dialogue_demo_menu_selected(ctx)
    local npc = engine.get_entity("dialogue_npc")
    if ctx.item_id == "locked" then
        -- A confirm deactivates a menu for good and Lua cannot reactivate it,
        -- so the refused pick is answered by respawning the menu.
        respawn_choice_menu()
    elseif not npc then
        return
    elseif ctx.item_id == "continue" then
        engine.entity_dialogue_advance(npc)
    elseif ctx.item_id == "talk_guard" then
        restart_dialogue("guard")
    elseif ctx.item_id == "talk_innkeeper" then
        restart_dialogue("innkeeper")
    else
        engine.entity_dialogue_choose(npc, ctx.item_index)
    end
end

--- Dialogue choice-selected callback: logs the pick.
--- @param ctx EntityContext
--- @param dialogue DialogueChoiceSelected
local function dialogue_demo_choice_selected(ctx, dialogue)
    engine.log_info("dialogue_demo: picked '" .. dialogue.text .. "' (index " .. dialogue.index ..
        ") on node '" .. dialogue.node .. "'")
end

--- Dialogue ended callback: respawns the menu with the two conversations to
--- start (the innkeeper is locked until the guard has opened the gate) and
--- writes the end reason as the hint.
--- @param ctx EntityContext
--- @param dialogue DialogueEnded
local function dialogue_demo_ended(ctx, dialogue)
    local innkeeper = { id = "talk_innkeeper", label = "Talk to the innkeeper" }
    if not engine.has_flag("gate_open") then
        innkeeper = { id = "locked", label = "(locked) Talk to the innkeeper" }
    end
    menu_items = {
        { id = "talk_guard", label = "Talk to the guard" },
        innkeeper,
    }
    respawn_choice_menu()
    engine.set_string("dlg_hint", "ended: " .. dialogue.reason .. " - up/down: select - action_1: confirm")
end

--- Called each tick while the scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_dialogue_demo(input, dt)
    if input.digital.back.just_pressed then
        local npc = engine.get_entity("dialogue_npc")
        if npc then
            engine.entity_dialogue_cancel(npc)
        end
        engine.request_scene("menu")
    end
end

M._callbacks = {
    on_update_dialogue_demo       = on_update_dialogue_demo,
    dialogue_demo_node_entered    = dialogue_demo_node_entered,
    dialogue_demo_menu_selected   = dialogue_demo_menu_selected,
    dialogue_demo_choice_selected = dialogue_demo_choice_selected,
    dialogue_demo_ended           = dialogue_demo_ended,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn the dialogue demo scene.
function M.spawn()
    engine.log_debug("Spawning dialogue demo scene...")
    engine.stop_all_music()
    engine.set_render_size(SCREEN_W, SCREEN_H)
    engine.set_background_color(20, 24, 30)
    engine.post_process_shader(nil)

    menu_items = {}

    clear_dialogue_signals()
    reset_story_state()

    -- The NPC runs the guard dialogue from the moment it spawns.
    engine.spawn()
        :with_dialogue("guard")
        :with_lua_dialogue_node_entered("dialogue_demo_node_entered")
        :with_lua_dialogue_choice_selected("dialogue_demo_choice_selected")
        :with_lua_dialogue_ended("dialogue_demo_ended")
        :register_as("dialogue_npc")
        :build()

    -- Screen-space lines, each bound to a WorldSignals string.
    signal_line("dlg_speaker", 16, 196, 255, 214, 110)
    signal_line("dlg_text", 16, 216, 235, 235, 235)
    signal_line("dlg_hint", 16, 336, 130, 130, 130)

    -- Inventory, top right: the integers the dialogue conditions and effects use.
    signal_line("keys", 536, 8, 255, 214, 110, "Keys: {}")
    signal_line("swords", 536, 24, 255, 214, 110, "Swords: {}")
    signal_line("coins", 536, 40, 255, 214, 110, "Coins: {}")

    engine.log_debug("Dialogue demo scene entities queued!")
end

return M
