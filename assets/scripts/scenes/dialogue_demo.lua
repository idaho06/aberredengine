-- scenes/dialogue_demo.lua
-- Dialogue demo: one NPC running a JSON dialogue tree (the gate guard) and a
-- second tree defined inline as a Lua table (the innkeeper), both registered
-- in setup.lua's load_dialogue_demo().
--
-- The NPC spawns with :with_dialogue("guard") plus the three dialogue
-- callbacks. The callbacks read the plain values they need out of the
-- dialogue tables and drive screen-space text through WorldSignals
-- (dlg_speaker, dlg_text, dlg_hint), because Lua has no runtime text
-- setter. The top-right inventory (keys, swords, coins) binds to the
-- integers the dialogue conditions and effects read and write.
--
-- Every confirm goes through one engine menu component (registered as
-- "dialogue_menu") with a text cursor, spawned once with an empty item list:
-- the engine handles up/down, confirm and the normal/selected colours itself.
-- Every dialogue node, and the end of the dialogue, replaces its items with
-- engine.entity_menu_set_items: a choices node lists its choices, each locked
-- choice a disabled item that up/down skips and the menu draws dimmed, except
-- a locked choice labelled "hide_when_locked" in the dialogue data, which the
-- menu leaves out until it unlocks (the player sees the closed gate but has to
-- find the key before "I found the key." appears). A choice item's id carries
-- the choice's declared index, because hidden choices shift the menu positions.
-- A plain node lists one "Continue" item, and an ended dialogue lists the two
-- NPCs to talk to. A node that re-enters itself (the guard's "I found the key."
-- loops back to "greet", which unlocks "Open the gate!") reopens the menu on the
-- item the player just confirmed instead of on the first one.
-- The scene's update reads no action keys: the menu confirms
-- during input resolution, ahead of the dialogue system in the same tick,
-- so a second reader of the same press would act on the node the press just
-- entered.
-- Back cancels the dialogue and returns to the menu.
--
-- Placeholders: lines and choices hold templates ({player_name}, {self.apples},
-- {shop_grain:.1}); the engine keeps them raw and the script fills them in with
-- engine.format_text. The node-entered callback formats with its own ctx.signals,
-- so {self.apples} is right at entry, and keeps a copy of that table. on_update
-- formats the line again every tick with the copy, which keeps the world values
-- ({player_name}, the shop_grain counter) live while {self.apples} stays as it was
-- at entry. A live per-NPC value would live in a world signal instead.

local M = {}

local SCREEN_W, SCREEN_H = 640, 360

local GRAIN_START = 5.0 -- kg of grain in the shop at scene start
local GRAIN_PER_SECOND = 0.1 -- grain the shop sells per second

-- State of the line on screen: the raw template, a copy of the `ctx.signals`
-- that was passed to the node-entered callback (ctx is pooled, so the callback
-- copies what it keeps), and the last text written to "dlg_text".
local raw_text = ""
local entity_signals = nil
local shown_text = ""
local grain = GRAIN_START

-- Id of the menu item the player confirmed last; a node that re-enters itself
-- reopens the menu on it.
local last_pick_id = nil

-- ─── Choice menu ────────────────────────────────────────────────────────────

--- Spawns the dialogue choice menu, registered as "dialogue_menu", with an
--- empty item list; every dialogue node replaces the items in place with
--- engine.entity_menu_set_items. The engine handles up/down (wrapping) and
--- confirm itself and draws the normal/selected colours.
local function spawn_choice_menu()
    engine.spawn()
        :with_menu({}, 16, 244, "default", 14, 16, true)
        :with_menu_colors(140, 200, 255, 255, 255, 214, 110, 255)
        :with_menu_cursor_text("→")
        :with_lua_menu_selected("dialogue_demo_menu_selected")
        :register_as("dialogue_menu")
        :build()
end

--- Replaces the items of the menu registered as "dialogue_menu", when it
--- exists. `selected` (optional, 0-based) is the item to select instead of the
--- first enabled one.
local function set_menu_items(items, selected)
    local menu = engine.get_entity("dialogue_menu")
    if menu then
        engine.entity_menu_set_items(menu, items, selected)
    end
end

--- 0-based position of the item with id `id` in `items`, or nil.
local function position_of(items, id)
    for i, item in ipairs(items) do
        if item.id == id then
            return i - 1
        end
    end
    return nil
end

--- True when the dialogue choice `choice` carries the dialogue label `label`.
local function has_label(choice, label)
    for _, name in ipairs(choice.labels) do
        if name == label then
            return true
        end
    end
    return false
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

--- Copies the sequence and the three maps of an entity `signals` table.
--- A callback's `ctx.signals` is pooled and must not be kept, so a callback
--- that wants the values later keeps this copy.
local function copy_signals(signals)
    local copy = { flags = {}, integers = {}, scalars = {}, strings = {} }
    for i, flag in ipairs(signals.flags or {}) do
        copy.flags[i] = flag
    end
    for _, kind in ipairs({ "integers", "scalars", "strings" }) do
        for key, value in pairs(signals[kind] or {}) do
            copy[kind][key] = value
        end
    end
    return copy
end

--- Shows `text` in the "dlg_text" line, writing the signal only when it changed.
local function show_text(text)
    if text ~= shown_text then
        shown_text = text
        engine.set_string("dlg_text", text)
    end
end

--- Clears the speaker/text/hint signals and forgets the raw line.
local function clear_dialogue_signals()
    raw_text = ""
    entity_signals = nil
    shown_text = ""
    last_pick_id = nil
    engine.set_string("dlg_speaker", "")
    engine.set_string("dlg_text", "")
    engine.set_string("dlg_hint", "")
end

--- Resets the story signals the dialogue trees read and write when the scene
--- starts: the guard's key and gate flags, visit count and the player's inventory
--- (0 keys, 2 swords, 0 coins), the player's name and the shop's grain counter.
--- Restarting a dialogue keeps them.
local function reset_story_state()
    engine.set_string("player_name", "Idaho06")
    grain = GRAIN_START
    engine.set_scalar("shop_grain", grain)
    engine.remove_flag("has_key")
    engine.remove_flag("gate_open")
    engine.set_integer("visits", 0)
    engine.set_integer("keys", 0)
    engine.set_integer("swords", 2)
    engine.set_integer("coins", 0)
end

--- Clears the speaker/text/hint signals and attaches the dialogue registered
--- under `key` to the NPC, restarting the conversation.
local function restart_dialogue(key)
    local npc = engine.get_entity("dialogue_npc")
    if not npc then
        return
    end
    clear_dialogue_signals()
    engine.entity_insert_dialogue(npc, key)
end

-- ─── Callbacks (local — injected into _G by main.lua) ───────────────────────

--- Dialogue node-entered callback: keeps the raw line and a copy of `ctx.signals`
--- for the per-tick re-format, writes the speaker and the formatted line to the
--- signal-driven text, replaces the menu's items with the node's choices
--- (formatted once here; a locked choice a disabled item, or no item when it is
--- labelled "hide_when_locked") or with one "Continue" item on a node without
--- any choices, and writes the hint line. A node that re-enters itself keeps the
--- selection on the item the player confirmed (its id is stable across the
--- rebuild, its position is not).
--- @param ctx EntityContext
--- @param dialogue DialogueLine
local function dialogue_demo_node_entered(ctx, dialogue)
    engine.set_string("dlg_speaker", dialogue.speaker or "")
    raw_text = dialogue.text or ""
    entity_signals = copy_signals(ctx.signals)
    show_text(engine.format_text(raw_text, ctx.signals))
    local items = {}
    for _, choice in ipairs(dialogue.choices) do
        if choice.available or not has_label(choice, "hide_when_locked") then
            items[#items + 1] = {
                id = "choice_" .. choice.index,
                label = engine.format_text(choice.text or "", ctx.signals),
                enabled = choice.available and true or false,
            }
        end
    end
    if #dialogue.choices == 0 then
        items[1] = { id = "continue", label = "Continue" }
    end
    local reentered = dialogue.previous == dialogue.node
    set_menu_items(items, reentered and last_pick_id and position_of(items, last_pick_id) or nil)
    engine.set_string("dlg_hint", "up/down: select - action_1: confirm - back: leave")
end

--- Menu-selected callback: a choice pick chooses the dialogue choice whose
--- declared index its item id carries, "continue" advances, and an ended-menu
--- pick restarts a conversation.
--- @param ctx table Context with menu_id, item_id (string) and item_index (0-based)
local function dialogue_demo_menu_selected(ctx)
    last_pick_id = ctx.item_id
    local npc = engine.get_entity("dialogue_npc")
    if not npc then
        return
    elseif ctx.item_id == "continue" then
        engine.entity_dialogue_advance(npc)
    elseif ctx.item_id == "talk_guard" then
        restart_dialogue("guard")
    elseif ctx.item_id == "talk_innkeeper" then
        restart_dialogue("innkeeper")
    else
        -- Hidden choices shift the menu positions, so ctx.item_index is not the
        -- declared index the dialogue takes: the item id carries it.
        local declared = tonumber(ctx.item_id:match("^choice_(%d+)$"))
        if declared then
            engine.entity_dialogue_choose(npc, declared)
        end
    end
end

--- Dialogue choice-selected callback: logs the pick.
--- @param ctx EntityContext
--- @param dialogue DialogueChoiceSelected
local function dialogue_demo_choice_selected(ctx, dialogue)
    engine.log_info("dialogue_demo: picked '" .. dialogue.text .. "' (index " .. dialogue.index ..
        ") on node '" .. dialogue.node .. "'")
end

--- Dialogue ended callback: replaces the menu's items with the two
--- conversations to start (the innkeeper is disabled until the guard has
--- opened the gate) and writes the end reason as the hint.
--- @param ctx EntityContext
--- @param dialogue DialogueEnded
local function dialogue_demo_ended(ctx, dialogue)
    set_menu_items({
        { id = "talk_guard", label = "Talk to the guard" },
        {
            id = "talk_innkeeper",
            label = "Talk to the innkeeper",
            enabled = engine.has_flag("gate_open"),
        },
    })
    engine.set_string("dlg_hint", "ended: " .. dialogue.reason .. " - up/down: select - action_1: confirm")
end

--- Called each tick while the scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_dialogue_demo(input, dt)
    -- The shop sells grain over time; the line re-formats with the new value.
    grain = math.max(0.0, grain - GRAIN_PER_SECOND * dt)
    engine.set_scalar("shop_grain", grain)
    if raw_text ~= "" then
        show_text(engine.format_text(raw_text, entity_signals))
    end

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

    clear_dialogue_signals()
    reset_story_state()

    -- The choice menu spawns before the NPC, so its registration exists when
    -- the NPC's first dialogue node replaces its items.
    spawn_choice_menu()

    -- The NPC runs the guard dialogue from the moment it spawns. Its own
    -- signals hold the apples that the innkeeper's {self.apples} reads.
    engine.spawn()
        :with_signal_integer("apples", 6)
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
