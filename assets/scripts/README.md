# Aberred Engine - Lua Scripting API Documentation

Complete reference for game developers using Lua scripting in Aberred Engine.

## Table of Contents

- [Getting Started](#getting-started)
- [Script Execution Flow](#script-execution-flow)
- [Logging Functions](#logging-functions)
- [Input Functions](#input-functions)
- [Asset Loading](#asset-loading)
- [Audio Playback](#audio-playback)
- [Entity Spawning](#entity-spawning)
  - [Core Components](#core-components)
  - [Signal Components](#signal-components)
  - [Text Components](#text-components)
  - [Menu Components](#menu-components)
  - [Animation Components](#animation-components)
  - [Phase Component](#phase-component)
  - [Attachment Components](#attachment-components)
  - [Tween Components](#tween-components)
- [World Signals](#world-signals)
- [Entity Commands](#entity-commands)
- [Phase Control](#phase-control)
- [Collision Handling](#collision-handling)
- [Camera Control](#camera-control)
- [Group Tracking](#group-tracking)
- [Tilemap Rendering](#tilemap-rendering)

---

## Getting Started

Aberred Engine provides a comprehensive Lua API through the global `engine` table. All engine functions are accessed via this table (e.g., `engine.log()`, `engine.spawn()`).

### Project Structure

```
assets/scripts/
├── main.lua           # Entry point - loaded first by Rust
├── setup.lua          # Asset loading configuration
└── scenes/
    ├── menu.lua       # Menu scene spawning logic
    └── level01.lua    # Level 01 gameplay logic
```

---

## Script Execution Flow

### 1. `main.lua` - Entry Point

The engine loads `main.lua` first. This file must define specific callback functions:

```lua
-- Called during Setup game state to load all assets
function on_setup()
    engine.log_info("Loading assets...")
    -- Queue asset loading here
end

-- Called when entering Playing game state
function on_enter_play()
    engine.log_info("Game started!")
    return "Hello from Lua!"  -- Optional return value
end

-- Called when switching scenes
function on_switch_scene(scene_name)
    local scene = require("scenes." .. scene_name)
    if scene and scene.spawn then
        scene.spawn()
    end
end
```

**Note**: The `on_update(dt)` callback in `main.lua` is not called by the engine. Use per-scene update callbacks instead (see below).

### 2. Scene Scripts

Scene scripts in `scenes/` directory are loaded on-demand. Each scene module should export:
- A `spawn()` function - called when the scene is loaded
- An `on_update_<scenename>(dt)` function - called every frame when that scene is active

```lua
-- scenes/level01.lua
local M = {}

-- Called when switching to this scene
function M.spawn()
    engine.log_info("Spawning level 01...")
    -- Spawn entities here
end

-- Called every frame while this scene is active
function on_update_level01(dt)
    -- dt: delta time in seconds

    -- Handle input for this scene
    if engine.is_action_back_just_pressed() then
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")
    end

    -- Most game logic goes in phase callbacks, not here
end

return M
```

**Global Flags**:
- `"switch_scene"` - Set this flag to trigger a scene change (cleared by engine after processing)
- `"quit_game"` - Set this flag to exit the game (cleared by engine after processing)

---

## Logging Functions

### `engine.log(message)`
General purpose logging to stderr with "[Lua]" prefix.
```lua
engine.log("Hello from Lua!")
```

### `engine.log_info(message)`
Info level logging with "[Lua INFO]" prefix.
```lua
engine.log_info("Player spawned successfully")
```

### `engine.log_warn(message)`
Warning level logging with "[Lua WARN]" prefix.
```lua
engine.log_warn("Entity not found in world signals")
```

### `engine.log_error(message)`
Error level logging with "[Lua ERROR]" prefix.
```lua
engine.log_error("Failed to load asset: " .. path)
```

---

## Input Functions

Query the current input state from Lua. These functions read cached input values updated each frame.

### `engine.is_action_back_pressed()`
Returns `true` if the back/cancel button (ESC) is currently held down.
```lua
if engine.is_action_back_pressed() then
    -- ESC key is being held
end
```

### `engine.is_action_back_just_pressed()`
Returns `true` if the back/cancel button (ESC) was pressed this frame (not held from previous frame).
```lua
-- Common pattern: Return to menu on ESC press
function on_update_level01(dt)
    if engine.is_action_back_just_pressed() then
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")
    end
end
```

### `engine.is_action_confirm_pressed()`
Returns `true` if the confirm/action button (SPACE) is currently held down.
```lua
if engine.is_action_confirm_pressed() then
    -- Space bar is being held
end
```

### `engine.is_action_confirm_just_pressed()`
Returns `true` if the confirm/action button (SPACE) was pressed this frame.
```lua
if engine.is_action_confirm_just_pressed() then
    -- Player just pressed space - trigger action
end
```

**Input Mapping**:
- `action_back` → ESC key
- `action_confirm` → SPACE key (also used as `action_1`)

---

## Asset Loading

Assets are queued during `on_setup()` and loaded before entering the Playing state.

### `engine.load_texture(id, path)`
Load a texture from disk.
```lua
engine.load_texture("ball", "./assets/textures/ball_12.png")
engine.load_texture("vaus_sheet", "./assets/textures/vaus_sheet.png")
```

### `engine.load_font(id, path, size)`
Load a TrueType font with specified point size.
```lua
engine.load_font("arcade", "./assets/fonts/Arcade_Cabinet.ttf", 128)
engine.load_font("future", "./assets/fonts/Formal_Future.ttf", 64)
```

### `engine.load_music(id, path)`
Load a music track (supports XM tracker format).
```lua
engine.load_music("menu", "./assets/audio/menu_theme.xm")
engine.load_music("boss_fight", "./assets/audio/boss_fight.xm")
```

### `engine.load_sound(id, path)`
Load a sound effect (supports WAV format).
```lua
engine.load_sound("ping", "./assets/audio/ping.wav")
engine.load_sound("ding", "./assets/audio/ding.wav")
```

### `engine.load_tilemap(id, path)`
Load a tilemap from directory (requires PNG atlas and JSON metadata).
```lua
engine.load_tilemap("level01", "./assets/tilemaps/level01")
```

### `engine.register_animation(id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped)`
Register a frame-based sprite animation.

**Parameters:**
- `id` - Unique animation identifier
- `tex_key` - Texture to use as sprite sheet
- `pos_x`, `pos_y` - Starting pixel position in sprite sheet
- `displacement` - Pixel offset between frames (horizontal)
- `frame_count` - Number of frames in animation
- `fps` - Playback speed in frames per second
- `looped` - Whether animation loops (true/false)

```lua
-- 16-frame looping animation at 15fps
engine.register_animation("vaus_glowing", "vaus_sheet", 0, 0, 96, 16, 15, true)

-- 6-frame non-looping animation at 15fps
engine.register_animation("vaus_hit", "vaus_sheet", 0, 24, 96, 6, 15, false)
```

---

## Audio Playback

### `engine.play_music(id, looped)`
Play a music track. If `looped` is true, music repeats indefinitely.
```lua
engine.play_music("menu", true)        -- Loop menu music
engine.play_music("victory", false)    -- Play once
```

### `engine.play_sound(id)`
Play a sound effect.
```lua
engine.play_sound("ping")
engine.play_sound("explosion")
```

### `engine.stop_all_music()`
Stop all currently playing music.
```lua
engine.stop_all_music()
```

### `engine.stop_all_sounds()`
Stop all currently playing sound effects.
```lua
engine.stop_all_sounds()
```

---

## Entity Spawning

Entities are created using a fluent builder pattern starting with `engine.spawn()`.

### Basic Pattern

```lua
engine.spawn()
    :with_position(400, 300)
    :with_sprite("player", 32, 32, 16, 16)
    :with_zindex(10)
    :build()  -- Always call build() to finalize!
```

All `:with_*()` methods return the builder for chaining. Call `:build()` to finalize.

---

### Core Components

#### `:with_group(name)`
Set entity's collision/query group.
```lua
:with_group("player")
:with_group("enemy")
:with_group("bullet")
```

#### `:with_position(x, y)`
Set entity's world position.
```lua
:with_position(400, 300)
```

#### `:with_screen_position(x, y)`
Set entity's screen position (for UI elements that don't scroll with camera).
```lua
:with_screen_position(10, 10)  -- Top-left corner
```

#### `:with_sprite(tex_key, width, height, origin_x, origin_y)`
Add sprite component for rendering.

**Parameters:**
- `tex_key` - Texture identifier (from `load_texture()`)
- `width`, `height` - Sprite dimensions in pixels
- `origin_x`, `origin_y` - Pivot point within sprite

```lua
-- 96x24 sprite with origin at bottom-center
:with_sprite("vaus_sheet", 96, 24, 48, 24)

-- 12x12 sprite with origin at center
:with_sprite("ball", 12, 12, 6, 6)
```

#### `:with_sprite_offset(offset_x, offset_y)`
Offset sprite from entity position (requires `:with_sprite()`).
```lua
:with_sprite_offset(0, -5)
```

#### `:with_sprite_flip(flip_h, flip_v)`
Flip sprite horizontally and/or vertically (requires `:with_sprite()`).
```lua
:with_sprite_flip(true, false)  -- Flip horizontally only
```

#### `:with_zindex(z)`
Set rendering order (higher values render on top).
```lua
:with_zindex(10)   -- Game entities
:with_zindex(100)  -- UI elements
```

#### `:with_velocity(vx, vy)`
Add RigidBody component with initial velocity.
```lua
:with_velocity(300, -300)  -- Move diagonally
```

#### `:with_collider(width, height, origin_x, origin_y)`
Add BoxCollider for collision detection.
```lua
:with_collider(96, 24, 48, 24)
```

#### `:with_collider_offset(offset_x, offset_y)`
Offset collider from entity position (requires `:with_collider()`).
```lua
:with_collider_offset(0, -2)
```

#### `:with_rotation(degrees)`
Set entity rotation in degrees.
```lua
:with_rotation(45)
```

#### `:with_scale(sx, sy)`
Set entity scale (1.0 = normal size).
```lua
:with_scale(2.0, 2.0)  -- Double size
:with_scale(0.5, 1.0)  -- Half width
```

#### `:with_mouse_controlled(follow_x, follow_y)`
Make entity follow mouse position on specified axes.
```lua
:with_mouse_controlled(true, false)  -- Follow X only (paddle)
:with_mouse_controlled(true, true)   -- Follow X and Y (cursor)
```

#### `:with_persistent()`
Mark entity as persistent across scene changes.
```lua
:with_persistent()
```

---

### Signal Components

Signals are entity-local key-value storage for game state.

#### `:with_signals()`
Add empty Signals component.
```lua
:with_signals()
```

#### `:with_signal_scalar(key, value)`
Add a floating-point signal.
```lua
:with_signal_scalar("speed", 100.0)
:with_signal_scalar("health", 1.0)
```

#### `:with_signal_integer(key, value)`
Add an integer signal.
```lua
:with_signal_integer("hp", 3)
:with_signal_integer("points", 100)
```

#### `:with_signal_flag(key)`
Add a boolean flag (presence = true).
```lua
:with_signal_flag("sticky")
:with_signal_flag("invulnerable")
```

#### `:with_signal_string(key, value)`
Add a string signal.
```lua
:with_signal_string("color", "red")
```

---

### Text Components

#### `:with_text(content, font, font_size, r, g, b, a)`
Add dynamic text rendering.

**Parameters:**
- `content` - Text to display
- `font` - Font identifier (from `load_font()`)
- `font_size` - Font size in pixels
- `r, g, b, a` - RGBA color (0-255)

```lua
:with_text("GAME OVER", "future", 48, 255, 0, 0, 255)  -- Red text
:with_text("Score: 0", "arcade", 24, 255, 255, 255, 255)  -- White text
```

#### `:with_signal_binding(key)`
Bind text to a world signal (auto-updates) (requires `:with_text()`).
```lua
:with_text("0", "arcade", 24, 255, 255, 255, 255)
:with_signal_binding("score")  -- Text shows current score
```

#### `:with_signal_binding_format(format)`
Format signal value in text (use `{}` as placeholder).
```lua
:with_signal_binding("score")
:with_signal_binding_format("Score: {}")
```

---

### Menu Components

#### `:with_menu(items, origin_x, origin_y, font, font_size, item_spacing, use_screen_space)`
Create an interactive menu.

**Parameters:**
- `items` - Table of `{ id = "...", label = "..." }`
- `origin_x`, `origin_y` - Menu position
- `font` - Font identifier
- `font_size` - Font size
- `item_spacing` - Vertical spacing between items
- `use_screen_space` - If true, menu doesn't scroll with camera

```lua
:with_menu(
    {
        { id = "start", label = "Start Game" },
        { id = "quit", label = "Quit" }
    },
    400, 300,      -- Position
    "arcade", 32,  -- Font and size
    40,            -- Item spacing
    true           -- Screen space
)
```

#### `:with_menu_colors(normal_r, normal_g, normal_b, normal_a, selected_r, selected_g, selected_b, selected_a)`
Set menu text colors (requires `:with_menu()`).
```lua
:with_menu_colors(
    200, 200, 200, 255,  -- Normal: gray
    255, 255, 0, 255     -- Selected: yellow
)
```

#### `:with_menu_dynamic_text(dynamic)`
Enable dynamic text in menu items (requires `:with_menu()`).
```lua
:with_menu_dynamic_text(true)
```

#### `:with_menu_cursor(key)`
Specify cursor entity to display next to selected menu item (requires `:with_menu()`).

**Important:** You must create a separate cursor entity **before** creating the menu, and register it with `:register_as(key)` so the menu can reference it.

**Parameters:**
- `key` - World signal key where cursor entity ID is stored (via `:register_as()`)

```lua
-- Step 1: Create and register the cursor entity FIRST
engine.spawn()
    :with_screen_position(350, 280)
    :with_sprite("cursor", 16, 16, 8, 8)
    :with_zindex(101)
    :register_as("menu_cursor")  -- Store entity ID for menu to reference
    :build()

-- Step 2: Create menu that uses the registered cursor
engine.spawn()
    :with_menu(
        { { id = "start", label = "Start" } },
        400, 300, "arcade", 32, 40, true
    )
    :with_menu_cursor("menu_cursor")  -- Reference the cursor by its registration key
    :build()
```

**How it works:** The menu system will automatically move the cursor entity to align with the currently selected menu item.

#### `:with_menu_selection_sound(sound_key)`
Play sound when selection changes (requires `:with_menu()`).
```lua
:with_menu_selection_sound("menu_beep")
```

#### `:with_menu_action_set_scene(item_id, scene)`
Define scene switch action (requires `:with_menu()`).
```lua
:with_menu_action_set_scene("start", "level01")
```

#### `:with_menu_action_show_submenu(item_id, submenu)`
Define submenu action (requires `:with_menu()`).
```lua
:with_menu_action_show_submenu("options", "options_menu")
```

#### `:with_menu_action_quit(item_id)`
Define quit game action (requires `:with_menu()`).
```lua
:with_menu_action_quit("quit")
```

---

### Animation Components

#### `:with_animation(animation_key)`
Play a single animation (must be registered first).
```lua
:with_animation("vaus_glowing")
```

#### `:with_animation_controller(fallback_key)`
Add conditional animation playback system.
```lua
:with_animation_controller("idle")
```

#### `:with_animation_rule(condition_table, set_key)`
Add rule to AnimationController (requires `:with_animation_controller()`).

**Condition Types:**

**Flag Conditions:**
```lua
:with_animation_rule({ type = "has_flag", key = "running" }, "run_anim")
:with_animation_rule({ type = "lacks_flag", key = "grounded" }, "jump_anim")
```

**Scalar (Float) Conditions:**
```lua
-- Compare operators: "lt", "le", "gt", "ge", "eq", "ne"
:with_animation_rule(
    { type = "scalar_cmp", key = "speed", op = "gt", value = 50.0 },
    "run_anim"
)

-- Range check
:with_animation_rule(
    { type = "scalar_range", key = "speed", min = 5.0, max = 50.0, inclusive = true },
    "walk_anim"
)
```

**Integer Conditions:**
```lua
:with_animation_rule(
    { type = "integer_cmp", key = "hp", op = "le", value = 0 },
    "dead_anim"
)

:with_animation_rule(
    { type = "integer_range", key = "hp", min = 1, max = 3, inclusive = true },
    "low_hp_anim"
)
```

**Composite Conditions:**
```lua
-- ALL conditions must pass
:with_animation_rule(
    {
        type = "all",
        conditions = {
            { type = "has_flag", key = "grounded" },
            { type = "scalar_cmp", key = "speed", op = "gt", value = 10.0 }
        }
    },
    "run_anim"
)

-- ANY condition can pass
:with_animation_rule(
    {
        type = "any",
        conditions = {
            { type = "has_flag", key = "damaged" },
            { type = "integer_cmp", key = "hp", op = "le", value = 1 }
        }
    },
    "hurt_anim"
)

-- NOT (negate condition)
:with_animation_rule(
    {
        type = "not",
        condition = { type = "has_flag", key = "invisible" }
    },
    "visible_anim"
)
```

---

### Phase Component

Phases provide state machine behavior for entities.

#### `:with_phase(table)`
Add LuaPhase component with state machine definition.

```lua
:with_phase({
    initial = "idle",  -- Starting phase
    phases = {
        idle = {
            on_enter = "player_idle_enter",    -- Called when entering
            on_update = "player_idle_update",  -- Called each frame
            on_exit = "player_idle_exit"       -- Called when exiting
        },
        running = {
            on_enter = "player_running_enter",
            on_update = "player_running_update"
        }
    }
})
```

**Phase Callback Signatures:**

```lua
-- Called when entering a phase
function player_idle_enter(entity_id, previous_phase)
    -- entity_id: u64
    -- previous_phase: string or nil
end

-- Called each frame while in phase
function player_idle_update(entity_id, time_in_phase)
    -- entity_id: u64
    -- time_in_phase: f32 (seconds in current phase)

    if time_in_phase >= 2.0 then
        engine.phase_transition(entity_id, "running")
    end
end

-- Called when exiting a phase
function player_idle_exit(entity_id, next_phase)
    -- entity_id: u64
    -- next_phase: string
end
```

---

### Attachment Components

#### `:with_stuckto(target_entity_id, follow_x, follow_y)`
Attach entity to another entity's position.

**Important:** The target entity must be registered with `:register_as(key)` so you can retrieve its ID with `engine.get_entity(key)`.

**Parameters:**
- `target_entity_id` - Entity ID (from `engine.get_entity()`)
- `follow_x`, `follow_y` - Which axes to follow

```lua
-- First, spawn and register the target entity
engine.spawn()
    :with_group("player")
    :with_position(400, 700)
    :register_as("player")  -- Register so we can get its ID later
    :build()

-- Then, spawn the entity that will follow it
local player_id = engine.get_entity("player")
engine.spawn()
    :with_group("ball")
    :with_position(400, 650)
    :with_stuckto(player_id, true, false)  -- Follow player X only
    :build()
```

#### `:with_stuckto_offset(offset_x, offset_y)`
Set offset from target (requires `:with_stuckto()`).
```lua
:with_stuckto_offset(0, -30)  -- 30 pixels above target
```

#### `:with_stuckto_stored_velocity(vx, vy)`
Set velocity to restore when released (requires `:with_stuckto()`).
```lua
:with_stuckto_stored_velocity(300, -300)
```

**Complete Example:**
```lua
engine.spawn()
    :with_group("ball")
    :with_position(336, 650)
    :with_sprite("ball", 12, 12, 6, 6)
    :with_stuckto(player_id, true, false)     -- Follow player X
    :with_stuckto_offset(0, 0)                -- Centered on player
    :with_stuckto_stored_velocity(300, -300)  -- Launch velocity
    :build()

-- Later, release the ball
engine.release_stuckto(ball_id)  -- Restores velocity
```

---

### Tween Components

Tween components provide automated interpolation animations.

#### `:with_tween_position(from_x, from_y, to_x, to_y, duration)`
Animate position over time.
```lua
:with_tween_position(0, -100, 0, 0, 2.0)  -- Slide down over 2 seconds
```

#### `:with_tween_position_easing(easing)`
Set easing function (requires `:with_tween_position()`).

**Available Easings:** `"linear"`, `"quad_in"`, `"quad_out"`, `"quad_in_out"`, `"cubic_in"`, `"cubic_out"`, `"cubic_in_out"`

```lua
:with_tween_position_easing("quad_in_out")
```

#### `:with_tween_position_loop(loop_mode)`
Set loop behavior (requires `:with_tween_position()`).

**Loop Modes:** `"once"`, `"loop"`, `"ping_pong"`

```lua
:with_tween_position_loop("ping_pong")
```

#### `:with_tween_rotation(from, to, duration)`
Animate rotation over time.
```lua
:with_tween_rotation(0, 360, 3.0)  -- Full rotation in 3 seconds
```

#### `:with_tween_rotation_easing(easing)`
Set easing for rotation (requires `:with_tween_rotation()`).
```lua
:with_tween_rotation_easing("linear")
```

#### `:with_tween_rotation_loop(loop_mode)`
Set loop behavior for rotation (requires `:with_tween_rotation()`).
```lua
:with_tween_rotation_loop("loop")
```

#### `:with_tween_scale(from_x, from_y, to_x, to_y, duration)`
Animate scale over time.
```lua
:with_tween_scale(0, 0, 1, 1, 0.5)  -- Grow from nothing over 0.5 seconds
```

#### `:with_tween_scale_easing(easing)`
Set easing for scale (requires `:with_tween_scale()`).
```lua
:with_tween_scale_easing("quad_out")
```

#### `:with_tween_scale_loop(loop_mode)`
Set loop behavior for scale (requires `:with_tween_scale()`).
```lua
:with_tween_scale_loop("ping_pong")  -- Pulse effect
```

---

### Additional Components

#### `:with_timer(duration, signal)`
Add Timer component that fires after duration.

**Parameters:**
- `duration` - Time in seconds
- `signal` - TimerEvent signal name to emit

```lua
:with_timer(3.0, "powerup_expired")
```

#### `:with_lua_collision_rule(group_a, group_b, callback)`
Register collision callback between two groups.

**Parameters:**
- `group_a`, `group_b` - Entity group names
- `callback` - Lua function name to call on collision

```lua
:with_lua_collision_rule("ball", "player", "on_ball_player")
```

#### `:with_grid_layout(path, group, zindex)`
Spawn entities from JSON grid layout file.
```lua
:with_grid_layout("./assets/levels/bricks.json", "brick", 5)
```

---

### Entity Registration & Finalization

#### `:register_as(key)`
Store entity ID in world signals for later retrieval.
```lua
:register_as("player")

-- Later retrieve it:
local player_id = engine.get_entity("player")
```

#### `:build()`
**Required.** Finalize and queue entity for spawning.
```lua
:build()  -- Must be called at end of chain!
```

---

## World Signals

World signals provide global game state storage accessible from all scripts.

### Reading Signals

#### `engine.get_scalar(key) -> f32 or nil`
Read floating-point world signal.
```lua
local player_y = engine.get_scalar("player_y") or 700.0
```

#### `engine.get_integer(key) -> i32 or nil`
Read integer world signal.
```lua
local score = engine.get_integer("score") or 0
local lives = engine.get_integer("lives") or 3
```

#### `engine.get_string(key) -> string or nil`
Read string world signal.
```lua
local scene_name = engine.get_string("scene")
```

#### `engine.has_flag(key) -> bool`
Check if boolean flag is set.
```lua
if engine.has_flag("switch_scene") then
    -- Scene transition requested
end
```

#### `engine.get_entity(key) -> u64 or nil`
Retrieve entity ID registered with `:register_as()`.
```lua
local player_id = engine.get_entity("player")
if player_id then
    engine.entity_set_velocity(player_id, 100, 0)
end
```

#### `engine.get_group_count(group) -> i32 or nil`
Get entity count in tracked group.
```lua
local brick_count = engine.get_group_count("brick")
if brick_count == 0 then
    engine.log_info("Level cleared!")
end
```

### Writing Signals

#### `engine.set_scalar(key, value)`
Set floating-point world signal.
```lua
engine.set_scalar("player_y", 700.0)
engine.set_scalar("ball_speed", 350.0)
```

#### `engine.set_integer(key, value)`
Set integer world signal.
```lua
engine.set_integer("score", 1000)
engine.set_integer("lives", 3)
```

#### `engine.set_string(key, value)`
Set string world signal.
```lua
engine.set_string("scene", "menu")
engine.set_string("current_level", "level01")
```

#### `engine.set_flag(key)`
Set boolean flag to true.
```lua
engine.set_flag("switch_scene")
engine.set_flag("game_paused")
```

#### `engine.clear_flag(key)`
Clear boolean flag (set to false).
```lua
engine.clear_flag("switch_scene")
engine.clear_flag("game_paused")
```

---

## Entity Commands

Directly manipulate specific entities at runtime.

**Obtaining Entity IDs:**

All entity command functions require an `entity_id` parameter (type `u64`). You can obtain entity IDs in three ways:

1. **From registered entities** - Use `engine.get_entity(key)` for entities registered with `:register_as(key)`:
   ```lua
   local player_id = engine.get_entity("player")
   engine.entity_set_velocity(player_id, 100, 0)
   ```

2. **From collision callbacks** - Entity IDs are provided in the collision context:
   ```lua
   function on_ball_brick(ctx)
       local ball_id = ctx.a.id    -- Ball entity ID
       local brick_id = ctx.b.id   -- Brick entity ID
       engine.entity_despawn(brick_id)
   end
   ```

3. **From phase callbacks** - The entity ID is passed as the first parameter:
   ```lua
   function player_idle_enter(entity_id, previous_phase)
       engine.entity_set_animation(entity_id, "idle_anim")
   end
   ```

### `engine.entity_set_position(entity_id, x, y)`
Set entity's world position.
```lua
engine.entity_set_position(ball_id, 400, 300)
```

### `engine.entity_set_velocity(entity_id, vx, vy)`
Set entity's velocity (requires RigidBody component).
```lua
engine.entity_set_velocity(ball_id, 300, -300)
```

### `engine.entity_despawn(entity_id)`
Delete an entity.
```lua
engine.entity_despawn(brick_id)
```

### `engine.entity_signal_set_flag(entity_id, flag)`
Set flag on entity's Signals component.
```lua
engine.entity_signal_set_flag(player_id, "sticky")
```

### `engine.entity_signal_clear_flag(entity_id, flag)`
Clear flag on entity's Signals component.
```lua
engine.entity_signal_clear_flag(player_id, "sticky")
```

### `engine.entity_signal_set_integer(entity_id, key, value)`
Set integer signal on entity.
```lua
local hp = engine.get_entity_signal_integer(brick_id, "hp")
engine.entity_signal_set_integer(brick_id, "hp", hp - 1)
```

### `engine.entity_insert_timer(entity_id, duration, signal)`
Insert Timer component on entity.
```lua
engine.entity_insert_timer(player_id, 5.0, "invulnerability_expired")
```

### `engine.entity_insert_stuckto(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)`
Attach entity to another at runtime.

**Parameters:**
- `entity_id` - Entity to attach
- `target_id` - Target entity
- `follow_x`, `follow_y` - Which axes to follow
- `offset_x`, `offset_y` - Offset from target
- `stored_vx`, `stored_vy` - Velocity to restore on release

```lua
engine.entity_insert_stuckto(ball_id, player_id, true, false, 0, 0, 300, -300)
```

### `engine.release_stuckto(entity_id)`
Release entity from StuckTo, restore stored velocity as RigidBody.
```lua
engine.release_stuckto(ball_id)
```

### `engine.entity_set_animation(entity_id, animation_key)`
Change entity's animation.
```lua
engine.entity_set_animation(player_id, "vaus_glowing")
```

### `engine.entity_restart_animation(entity_id)`
Restart current animation from frame 0.
```lua
engine.entity_restart_animation(player_id)
```

---

## Phase Control

### `engine.phase_transition(entity_id, phase)`
Request phase transition for entity with LuaPhase component.
```lua
engine.phase_transition(player_id, "hit")
engine.phase_transition(ball_id, "moving")
```

---

## Collision Handling

### Registering Collision Rules

Use `:with_lua_collision_rule()` when spawning an entity:

```lua
engine.spawn()
    :with_lua_collision_rule("ball", "player", "on_ball_player")
    :build()
```

### Collision Callback Function

Define a global Lua function matching the callback name:

```lua
function on_ball_player(ctx)
    -- ctx contains collision information
    local ball_id = ctx.a.id           -- Entity A ID
    local ball_pos = ctx.a.pos         -- { x, y }
    local ball_vel = ctx.a.vel         -- { x, y }
    local ball_rect = ctx.a.rect       -- { x, y, w, h }
    local ball_signals = ctx.a.signals -- { flags = {...}, integers = {...}, ... }

    local player_id = ctx.b.id         -- Entity B ID
    local player_pos = ctx.b.pos
    local player_vel = ctx.b.vel
    local player_rect = ctx.b.rect
    local player_signals = ctx.b.signals

    local sides = ctx.sides            -- Collision sides
    -- sides.a contains: "top", "bottom", "left", "right"
    -- sides.b contains: "top", "bottom", "left", "right"

    -- Manipulate entities
    engine.entity_set_velocity(ball_id, new_vx, new_vy)
    engine.collision_play_sound("ping")
end
```

### Collision-Specific Commands

These functions are designed for use inside collision callbacks:

#### `engine.collision_play_sound(id)`
Play sound during collision.
```lua
engine.collision_play_sound("ping")
```

#### `engine.collision_set_integer(key, value)`
Set global integer signal during collision.
```lua
local score = engine.get_integer("score") or 0
engine.collision_set_integer("score", score + 100)
```

#### `engine.collision_set_flag(flag)`
Set global flag during collision.
```lua
engine.collision_set_flag("ball_hit_player")
```

#### `engine.collision_clear_flag(flag)`
Clear global flag during collision.
```lua
engine.collision_clear_flag("ball_hit_player")
```

### Example: Ball-Brick Collision

```lua
function on_ball_brick(ctx)
    local ball_id = ctx.a.id
    local ball_pos = ctx.a.pos
    local ball_vel = ctx.a.vel

    local brick_id = ctx.b.id
    local brick_signals = ctx.b.signals

    -- Get brick HP
    local hp = 1
    if brick_signals.integers and brick_signals.integers.hp then
        hp = brick_signals.integers.hp
    end

    -- Damage brick
    hp = hp - 1
    if hp <= 0 then
        engine.entity_despawn(brick_id)

        -- Award points
        local score = engine.get_integer("score") or 0
        engine.collision_set_integer("score", score + 100)
    else
        engine.entity_signal_set_integer(brick_id, "hp", hp)
    end

    -- Bounce ball
    local sides = ctx.sides.a
    local new_vx = ball_vel.x
    local new_vy = ball_vel.y

    if sides.top or sides.bottom then
        new_vy = -new_vy
    end
    if sides.left or sides.right then
        new_vx = -new_vx
    end

    engine.entity_set_velocity(ball_id, new_vx, new_vy)
    engine.collision_play_sound("ding")
end
```

---

## Camera Control

### `engine.set_camera(target_x, target_y, offset_x, offset_y, rotation, zoom)`
Configure 2D camera.

**Parameters:**
- `target_x`, `target_y` - World position to center on
- `offset_x`, `offset_y` - Screen-space offset
- `rotation` - Camera rotation in degrees
- `zoom` - Zoom level (1.0 = normal)

```lua
engine.set_camera(336, 384, 336, 384, 0.0, 1.0)
```

---

## Group Tracking

Entity groups can be tracked for counting.

### `engine.track_group(name)`
Enable tracking for a group.
```lua
engine.track_group("ball")
engine.track_group("brick")
```

### `engine.untrack_group(name)`
Disable tracking for a group.
```lua
engine.untrack_group("brick")
```

### `engine.clear_tracked_groups()`
Clear all tracked groups.
```lua
engine.clear_tracked_groups()
```

### `engine.has_tracked_group(name) -> bool`
Check if a group is tracked.
```lua
if engine.has_tracked_group("brick") then
    local count = engine.get_group_count("brick")
end
```

---

## Tilemap Rendering

### `engine.spawn_tiles(id)`
Spawn tiles from a loaded tilemap.
```lua
engine.load_tilemap("level01", "./assets/tilemaps/level01")
-- ... later, after assets loaded ...
engine.spawn_tiles("level01")
```

---

## Complete Example: Player Paddle

```lua
-- Register animations
engine.register_animation("vaus_glowing", "vaus_sheet", 0, 0, 96, 16, 15, true)
engine.register_animation("vaus_hit", "vaus_sheet", 0, 24, 96, 6, 15, false)

-- Spawn player with phase system
local player_y = 700
engine.spawn()
    :with_group("player")
    :with_position(400, player_y)
    :with_sprite("vaus_sheet", 96, 24, 48, 24)
    :with_animation("vaus_glowing")
    :with_collider(96, 24, 48, 24)
    :with_mouse_controlled(true, false)
    :with_zindex(10)
    :with_signals()
    :with_phase({
        initial = "sticky",
        phases = {
            sticky = {
                on_enter = "player_sticky_enter",
                on_update = "player_sticky_update"
            },
            glowing = {
                on_enter = "player_glowing_enter"
            },
            hit = {
                on_enter = "player_hit_enter",
                on_update = "player_hit_update"
            }
        }
    })
    :register_as("player")
    :build()

-- Phase callbacks
function player_sticky_enter(entity_id, previous_phase)
    engine.entity_signal_set_flag(entity_id, "sticky")
    engine.entity_set_animation(entity_id, "vaus_glowing")
end

function player_sticky_update(entity_id, time_in_phase)
    if time_in_phase >= 3.0 then
        engine.phase_transition(entity_id, "glowing")
    end
end

function player_glowing_enter(entity_id, previous_phase)
    engine.entity_signal_clear_flag(entity_id, "sticky")
    engine.entity_set_animation(entity_id, "vaus_glowing")
end

function player_hit_enter(entity_id, previous_phase)
    engine.entity_set_animation(entity_id, "vaus_hit")
end

function player_hit_update(entity_id, time_in_phase)
    if time_in_phase >= 0.5 then
        engine.phase_transition(entity_id, "glowing")
    end
end
```

---

## Tips and Best Practices

1. **Always call `:build()`** - Entity spawning won't complete without it.

2. **Register entities for later access** - Use `:register_as()` to store entity IDs.
   ```lua
   :register_as("player")
   -- Later:
   local player_id = engine.get_entity("player")
   ```

3. **Use world signals for global state** - Score, lives, scene name, etc.
   ```lua
   engine.set_integer("score", 0)
   engine.set_string("scene", "menu")
   ```

4. **Track groups for counting** - Enable tracking before using `get_group_count()`.
   ```lua
   engine.track_group("brick")
   local count = engine.get_group_count("brick")
   ```

5. **Phase systems for state machines** - Use phases instead of timers for complex behavior.

6. **Collision callbacks receive context** - Access both entities via `ctx.a` and `ctx.b`.

7. **Load assets in `on_setup()`** - Assets must be queued before entering Playing state.

8. **Scene scripts are lazy-loaded** - Only loaded when `on_switch_scene()` requires them.

---

## Debugging

Use logging functions liberally:

```lua
engine.log_info("Player spawned at: " .. tostring(player_id))
engine.log_warn("Entity not found: " .. entity_key)
engine.log_error("Failed to load texture: " .. path)
```

Check signal values:

```lua
local score = engine.get_integer("score")
engine.log_info("Current score: " .. tostring(score))
```

Verify entity IDs:

```lua
local player_id = engine.get_entity("player")
if not player_id then
    engine.log_error("Player entity not registered!")
end
```

---

## License

This documentation is part of the Aberred Engine project.
