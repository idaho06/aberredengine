# Aberred Engine - Lua Scripting API Documentation

Complete reference for game developers using Lua scripting in Aberred Engine.

## Table of Contents

- [Getting Started](#getting-started)
- [Script Execution Flow](#script-execution-flow)
- [Logging Functions](#logging-functions)
- [Input System](#input-system)
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
  - [Particle Emitter Component](#particle-emitter-component)
- [World Signals](#world-signals)
- [Entity Commands](#entity-commands)
- [Phase Control](#phase-control)
- [Collision Handling](#collision-handling)
- [Camera Control](#camera-control)
- [Group Tracking](#group-tracking)
- [Tilemap Rendering](#tilemap-rendering)
- [Complete Example: Player Paddle](#complete-example-player-paddle)
- [Tips and Best Practices](#tips-and-best-practices)
- [Debugging](#debugging)
- [License](#license)

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

-- Called every frame while this scene is active (runs at 60 FPS)
function on_update_level01(dt)
    -- dt: delta time in seconds

    -- Handle input for this scene
    if engine.is_action_back_just_pressed() then
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")
    end

    -- Most game logic goes in phase callbacks, not here

    -- PERFORMANCE WARNING: You can use entity, spawn, phase, and audio commands
    -- in on_update callbacks, but avoid spawning entities here when possible.
    -- This callback runs every frame (60 FPS), so spawning entities here can
    -- cause significant performance issues. Prefer spawning in on_switch_scene,
    -- phase callbacks, or collision callbacks instead.
end

return M
```

**Global Flags**:

- `"switch_scene"` - Set this flag to trigger a scene change (cleared by engine after processing)
- `"quit_game"` - Set this flag to exit the game (cleared by engine after processing)

### 3. Callback Command Processing

Different callbacks process different types of engine commands. Here's what commands are processed after each callback type:

| Callback | Processed Commands | Use Cases |
|----------|-------------------|-----------|
| `on_setup()` | Asset, Animation | Load textures, fonts, audio, tilemaps; register animations |
| `on_enter_play()` | Signal, Group | Initialize world signals; configure group tracking |
| `on_switch_scene(scene)` | Signal, Entity, Phase, Audio, Spawn, Group, Tilemap, Camera | **Most complete** - spawn entities; set signals; play audio; set camera |
| `on_update_<scene>(dt)` | Signal, Entity, Spawn, Phase, Audio, Camera | Per-frame logic - camera effects, but avoid spawning (see warning below) |
| Phase callbacks | Phase, Audio, Signal, Spawn, Entity, Camera | Transition phases; play sounds; spawn/modify entities; camera effects |
| Timer callbacks | Phase, Audio, Signal, Spawn, Entity, Camera | Same as phase callbacks |
| Collision callbacks | Entity, Signal, Audio, Spawn, Phase, Camera | Same capabilities as phase/timer callbacks; camera shake on impact |

**Processing Order by Callback**:

| Callback | Order |
|----------|-------|
| `on_setup()` | Asset → Animation |
| `on_enter_play()` | Signal → Group |
| `on_switch_scene(scene)` | Signal → Entity → Phase → Audio → Spawn → Group → Tilemap → Camera |
| `on_update_<scene>(dt)` | Signal → Entity → Spawn → Phase → Audio → Camera |
| Phase/Timer callbacks | Phase → Audio → Signal → Spawn → Entity → Camera |
| Collision callbacks | Entity → Signal → Audio → Spawn → Phase → Camera |

**Important: Collision Callbacks Use Separate Queues**

Collision callbacks process commands from their own dedicated queues, which are drained immediately after the callback returns. This ensures that entity modifications and spawns happen at the right time during collision resolution.

**Use collision-specific functions in collision callbacks:**

- `engine.collision_spawn()` instead of `engine.spawn()`
- `engine.collision_play_sound()` instead of `engine.play_sound()`
- `engine.collision_set_flag()` / `engine.collision_clear_flag()` instead of `engine.set_flag()` / `engine.clear_flag()`
- `engine.collision_set_integer()` instead of `engine.set_integer()`
- `engine.collision_phase_transition()` instead of `engine.phase_transition()`
- `engine.collision_set_camera()` instead of `engine.set_camera()`

**Entity commands in collision callbacks** also require the `collision_` prefix. Both APIs now have full parity - all entity commands available in the regular API have a `collision_` equivalent:

- `engine.collision_entity_set_position()` instead of `engine.entity_set_position()`
- `engine.collision_entity_set_velocity()` instead of `engine.entity_set_velocity()`
- `engine.collision_entity_despawn()` instead of `engine.entity_despawn()`
- `engine.collision_entity_signal_set_flag()` instead of `engine.entity_signal_set_flag()`
- `engine.collision_entity_signal_clear_flag()` instead of `engine.entity_signal_clear_flag()`
- `engine.collision_entity_signal_set_integer()` instead of `engine.entity_signal_set_integer()`
- `engine.collision_entity_signal_set_scalar()` instead of `engine.entity_signal_set_scalar()`
- `engine.collision_entity_signal_set_string()` instead of `engine.entity_signal_set_string()`
- `engine.collision_entity_insert_lua_timer()` instead of `engine.entity_insert_lua_timer()`
- `engine.collision_entity_remove_lua_timer()` instead of `engine.entity_remove_lua_timer()`
- `engine.collision_entity_insert_ttl()` instead of `engine.entity_insert_ttl()`
- `engine.collision_entity_insert_stuckto()` instead of `engine.entity_insert_stuckto()`
- `engine.collision_release_stuckto()` instead of `engine.release_stuckto()`
- `engine.collision_entity_set_animation()` instead of `engine.entity_set_animation()`
- `engine.collision_entity_restart_animation()` instead of `engine.entity_restart_animation()`
- `engine.collision_entity_set_rotation()` instead of `engine.entity_set_rotation()`
- `engine.collision_entity_set_scale()` instead of `engine.entity_set_scale()`
- `engine.collision_entity_insert_tween_position()` instead of `engine.entity_insert_tween_position()`
- `engine.collision_entity_insert_tween_rotation()` instead of `engine.entity_insert_tween_rotation()`
- `engine.collision_entity_insert_tween_scale()` instead of `engine.entity_insert_tween_scale()`
- `engine.collision_entity_remove_tween_position()` instead of `engine.entity_remove_tween_position()`
- `engine.collision_entity_remove_tween_rotation()` instead of `engine.entity_remove_tween_rotation()`
- `engine.collision_entity_remove_tween_scale()` instead of `engine.entity_remove_tween_scale()`
- `engine.collision_entity_add_force()` instead of `engine.entity_add_force()`
- `engine.collision_entity_remove_force()` instead of `engine.entity_remove_force()`
- `engine.collision_entity_set_force_enabled()` instead of `engine.entity_set_force_enabled()`
- `engine.collision_entity_set_force_value()` instead of `engine.entity_set_force_value()`
- `engine.collision_entity_set_friction()` instead of `engine.entity_set_friction()`
- `engine.collision_entity_set_max_speed()` instead of `engine.entity_set_max_speed()`
- `engine.collision_entity_freeze()` instead of `engine.entity_freeze()`
- `engine.collision_entity_unfreeze()` instead of `engine.entity_unfreeze()`
- `engine.collision_entity_set_speed()` instead of `engine.entity_set_speed()`

**What happens if you use the wrong function?** Commands won't be lost, but timing will be delayed:

- Using `engine.spawn()` in a collision callback → entity is created during the next `on_update` or phase/timer callback (1+ frames later)
- Using `engine.play_sound()` in a collision callback → sound plays during the next processing cycle
- Using `engine.entity_set_velocity()` in a collision callback → velocity change may happen after next collision is processed

**Performance Warning for `on_update` callbacks**:

- `on_update_<scene>` runs every frame (60 FPS)
- Entity/Spawn commands work but can cause performance issues
- **Avoid spawning entities in update loops** - prefer `on_switch_scene`, phase callbacks, or collision callbacks
- Use `on_update` primarily for input handling and signal updates

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

## Input System

Input is passed as a table argument to callbacks instead of being queried via functions. This provides a snapshot of all input state at the moment the callback is invoked.

### Input Table Structure

The `input` table has the following structure:

```lua
input = {
    digital = {
        up = { pressed = bool, just_pressed = bool, just_released = bool },
        down = { pressed = bool, just_pressed = bool, just_released = bool },
        left = { pressed = bool, just_pressed = bool, just_released = bool },
        right = { pressed = bool, just_pressed = bool, just_released = bool },
        action_1 = { pressed = bool, just_pressed = bool, just_released = bool },
        action_2 = { pressed = bool, just_pressed = bool, just_released = bool },
        back = { pressed = bool, just_pressed = bool, just_released = bool },
        special = { pressed = bool, just_pressed = bool, just_released = bool },
    },
    analog = {
        -- Reserved for future gamepad support
    }
}
```

### Digital Input States

Each digital button has three boolean properties:

- `pressed` - `true` if the button is currently held down
- `just_pressed` - `true` only on the frame when the button was first pressed
- `just_released` - `true` only on the frame when the button was released

### Input Mapping

| Input Name | Keyboard Keys |
|------------|---------------|
| `up` | W, Up Arrow |
| `down` | S, Down Arrow |
| `left` | A, Left Arrow |
| `right` | D, Right Arrow |
| `action_1` | Space |
| `action_2` | Left Shift |
| `back` | Escape |
| `special` | Enter |

### Usage Examples

```lua
-- Check if player is holding right
if input.digital.right.pressed then
    -- Move player right
end

-- Check if player just pressed jump (one-shot action)
if input.digital.action_1.just_pressed then
    -- Start jump
end

-- Check if player released the action button
if input.digital.action_1.just_released then
    -- End charged attack
end

-- Common pattern: Return to menu on ESC press
function on_update_level01(input, dt)
    if input.digital.back.just_pressed then
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")
    end
end
```

### Which Callbacks Receive Input

| Callback Type | Receives Input | Signature |
|---------------|----------------|-----------|
| Scene update | ✓ | `on_update_scenename(input, dt)` |
| Phase on_enter | ✓ | `callback(ctx, input)` |
| Phase on_update | ✓ | `callback(ctx, input, dt)` |
| Phase on_exit | ✗ | `callback(ctx)` |
| Timer | ✓ | `callback(ctx, input)` |
| Collision | ✗ | `callback(ctx)` |

**Note:** Phase and timer callbacks now receive an `EntityContext` (`ctx`) object instead of just `entity_id`. The context contains all entity component data. See [Phase Component](#phase-component) for details. Phase `on_exit` callbacks do not receive input because they are meant for housekeeping tasks only. Collision callbacks do not receive input as they are triggered by physics events, not player actions

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

### Entity Cloning

Clone an existing entity using `engine.clone(source_key)`. The source entity is looked up by its WorldSignals key (set via `:register_as()`). All components are cloned, and builder overrides win over the template's values. Animation is always reset to frame 0.

```lua
-- First, create a template entity (no position = won't render)
engine.spawn()
    :with_sprite("bullet", 8, 8, 4, 4)
    :with_collider(8, 8, 4, 4)
    :with_animation("bullet_anim")
    :register_as("bullet_template")
    :build()

-- Clone with overrides
engine.clone("bullet_template")
    :with_group("bullets")
    :with_position(x, y)
    :with_velocity(vx, vy)
    :with_ttl(3.0)
    :build()
```

**Key behaviors:**

- **Overrides win**: Builder values always override the template's components
- **All components cloned**: Every component implementing `Clone` or `Reflect` is copied
- **Animation reset**: Animation always starts at frame 0, even without override
- **New entity registered**: `:register_as()` stores the NEW cloned entity's ID, not the template

**When to use cloning:**

- Spawning many similar entities (bullets, particles, enemies)
- Entity templates with complex component setups
- Performance optimization (fewer builder calls than full spawn)

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

#### `:with_friction(friction)`

Set velocity damping on RigidBody (requires `:with_velocity()` first).

**Parameters:**

- `friction` - Damping factor (0.0 = no friction, ~5.0 = responsive, ~10.0 = heavy drag)

```lua
:with_velocity(0, 0)
:with_friction(5.0)  -- Responsive friction for player control
```

#### `:with_max_speed(max_speed)`

Set maximum velocity magnitude on RigidBody (requires `:with_velocity()` first).

**Parameters:**

- `max_speed` - Maximum speed in world units per second

```lua
:with_velocity(0, 0)
:with_max_speed(300.0)  -- Clamp speed to 300 units/sec
```

#### `:with_accel(name, x, y, enabled)`

Add a named acceleration force to RigidBody (requires `:with_velocity()` first).

Forces are accumulated each frame and applied to velocity. Multiple forces can be added with different names and toggled independently.

**Parameters:**

- `name` - Unique identifier for this force
- `x`, `y` - Acceleration in world units per second squared
- `enabled` - Whether this force is active (true/false)

```lua
:with_velocity(0, 0)
:with_accel("gravity", 0, 980, true)      -- Enabled gravity
:with_accel("thrust", 0, -500, false)     -- Disabled thrust (toggle later)
:with_accel("wind", 50, 0, true)          -- Enabled wind
```

#### `:with_frozen(frozen)`

Set frozen state on RigidBody (requires `:with_velocity()` first).

When frozen, the movement system skips all physics calculations for this entity. Position can still be modified externally (e.g., by StuckTo system or direct manipulation).

**Parameters:**

- `frozen` - Whether entity is frozen (true/false)

```lua
:with_velocity(300, -300)
:with_frozen(true)  -- Start frozen (e.g., ball stuck to paddle)
```

**Complete Physics Example:**

```lua
-- Entity with gravity, friction, and speed limit
engine.spawn()
    :with_group("player")
    :with_position(400, 300)
    :with_sprite("player", 32, 32, 16, 16)
    :with_velocity(0, 0)
    :with_friction(5.0)
    :with_max_speed(300.0)
    :with_accel("gravity", 0, 980, true)
    :with_accel("jump", 0, -1500, false)  -- Toggle on when jumping
    :build()
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

**EntityContext Structure:**

Phase and timer callbacks receive a rich context object (`ctx`) containing entity state:

```lua
ctx = {
    -- Core identity (always present)
    id = 12345678,           -- Entity ID (u64)

    -- Optional fields (nil if component not present)
    group = "player",        -- Entity group name
    pos = { x = 100, y = 200 },      -- World position (MapPosition)
    screen_pos = { x = 50, y = 100 }, -- Screen position (ScreenPosition)
    vel = { x = 0, y = 0 },          -- Velocity (RigidBody)
    speed_sq = 0,                    -- Squared speed (RigidBody)
    frozen = false,                  -- Frozen state (RigidBody)
    rotation = 0,                    -- Rotation in degrees
    scale = { x = 1, y = 1 },        -- Scale factors
    rect = { x = 90, y = 190, w = 20, h = 20 }, -- BoxCollider AABB

    -- Sprite info
    sprite = {
        tex_key = "player_sheet",
        flip_h = false,
        flip_v = false
    },

    -- Animation state
    animation = {
        key = "idle",
        frame_index = 0,
        elapsed = 0.0
    },

    -- Entity signals
    signals = {
        flags = { "active", "grounded" },  -- 1-indexed array
        integers = { hp = 3, score = 100 },
        scalars = { speed = 150.5 },
        strings = { state = "idle" }
    },

    -- Phase info (from LuaPhase)
    phase = "idle",          -- Current phase name
    time_in_phase = 1.5,     -- Seconds in current phase

    -- Only on on_enter callbacks
    previous_phase = "spawning", -- Previous phase (nil on initial enter)

    -- Timer info (from LuaTimer)
    timer = {
        duration = 5.0,
        elapsed = 2.3,
        callback = "on_timer_tick"
    }
}
```

**Phase Callback Signatures:**

```lua
-- Called when entering a phase
-- Returns: nil or phase_name (string) to transition to another phase
function player_idle_enter(ctx, input)
    -- ctx: EntityContext table with all component data
    -- ctx.previous_phase: string or nil (the phase we came from)
    -- input: input state table

    engine.log_info("Player " .. ctx.id .. " entered idle from " .. (ctx.previous_phase or "initial"))
    return nil  -- Stay in current phase
end

-- Called each frame while in phase
-- Returns: nil or phase_name (string) to transition to another phase
function player_idle_update(ctx, input, dt)
    -- ctx: EntityContext table
    -- ctx.time_in_phase: seconds in current phase
    -- input: input state table
    -- dt: delta time in seconds

    -- Option 1: Use return value to transition (preferred for same entity)
    if ctx.time_in_phase >= 2.0 then
        return "running"
    end

    -- Access input directly
    if input.digital.action_1.just_pressed then
        return "jumping"
    end

    return nil  -- Stay in current phase
end

-- Called when exiting a phase
-- Does NOT support return value (transition already decided)
function player_idle_exit(ctx)
    -- ctx: EntityContext table (no input parameter)
    engine.log_info("Player " .. ctx.id .. " exiting idle phase")
end
```

**Phase Transition Methods:**

There are two ways to trigger phase transitions:

1. **Return value (preferred for same entity):** Return a phase name string from `on_enter` or `on_update` callbacks.

   ```lua
   function my_update(ctx, input, dt)
       if some_condition then
           return "next_phase"  -- Transition to "next_phase"
       end
       return nil  -- Stay in current phase
   end
   ```

2. **`engine.phase_transition(entity_id, phase)`:** Call this function to transition any entity (including other entities).

   ```lua
   function my_update(ctx, input, dt)
       -- Transition self (works, but return value is cleaner)
       engine.phase_transition(ctx.id, "next_phase")

       -- Transition another entity (must use this method)
       local other_id = engine.get_entity("partner")
       if other_id then
           engine.phase_transition(other_id, "alert")
       end
   end
   ```

**Important notes:**

- Return values take precedence over `engine.phase_transition(ctx.id, ...)` for the same entity
- Returning the current phase name is ignored (no transition)
- Transitions happen on the next frame (not immediately)
- `on_exit` does NOT support return values (transition already decided)

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

#### `:with_tween_position_backwards()`

Start the position tween from the end and play it in reverse (requires `:with_tween_position()`).

```lua
:with_tween_position(0, -100, 0, 0, 2.0)
:with_tween_position_backwards()
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

#### `:with_tween_rotation_backwards()`

Start the rotation tween from the end and play it in reverse (requires `:with_tween_rotation()`).

```lua
:with_tween_rotation(0, 360, 3.0)
:with_tween_rotation_backwards()
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

#### `:with_tween_scale_backwards()`

Start scale tween from the end and play in reverse (requires `:with_tween_scale()`).

```lua
:with_tween_scale(0.5, 0.5, 1.0, 1.0, 1.0)  -- Normally: 0.5 -> 1.0
:with_tween_scale_backwards()  -- Now starts at 1.0, goes to 0.5
```

---

### Particle Emitter Component

The particle emitter component enables entities to spawn particles by cloning template entities at configurable rates, directions, and speeds.

#### `:with_particle_emitter(config)`

Add a particle emitter that spawns particles by cloning templates.

**Config Table Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `templates` | string[] | (required) | WorldSignals keys for template entities to clone |
| `shape` | string or table | `"point"` | `"point"` or `{type="rect", width=N, height=N}` |
| `offset` | table | `{x=0, y=0}` | Offset from entity position |
| `particles_per_emission` | integer | `1` | Particles spawned per emission event |
| `emissions_per_second` | number | `10.0` | Emission frequency |
| `emissions_remaining` | integer | `100` | Emissions before stopping (maximum 4294967295) |
| `arc` | table | `{0, 360}` | Direction arc in degrees (0° = up) |
| `speed` | table | `{50, 100}` | Speed range `{min, max}` for particles |
| `ttl` | number/table/nil | `nil` | TTL config for spawned particles |

**TTL Configuration:**

- `nil` - No TTL (particles live until manually despawned)
- `number` - Fixed TTL (e.g., `2.0` = all particles live 2 seconds)
- `{min=N, max=N}` - Random TTL within range

**Example - Smoke Trail:**

```lua
-- First, create a particle template (no position = won't render)
engine.spawn()
    :with_group("particle")
    :with_sprite("smoke", 8, 8, 4, 4)
    :with_friction(2.0)
    :register_as("smoke_particle")
    :build()

-- Create an emitter that spawns smoke particles
engine.spawn()
    :with_position(100, 100)
    :with_particle_emitter({
        templates = { "smoke_particle" },
        shape = "point",
        particles_per_emission = 3,
        emissions_per_second = 10,
        emissions_remaining = 100,
        arc = { -30, 30 },      -- 60° cone facing up
        speed = { 50, 100 },    -- Random speed between 50-100
        ttl = { min = 0.5, max = 1.0 },  -- Particles live 0.5-1.0 seconds
    })
    :build()
```

**Example - Explosion Burst:**

```lua
-- One-shot burst of particles
engine.spawn()
    :with_position(ctx.pos.x, ctx.pos.y)
    :with_particle_emitter({
        templates = { "spark_particle", "debris_particle" },
        shape = { type = "rect", width = 10, height = 10 },
        particles_per_emission = 20,
        emissions_per_second = 1000,  -- Instant burst
        emissions_remaining = 1,       -- Only emit once
        arc = { 0, 360 },              -- All directions
        speed = { 100, 300 },
        ttl = { min = 0.3, max = 0.8 },
    })
    :with_ttl(1.0)  -- Despawn emitter after 1 second
    :build()
```

**Key Behaviors:**

- **Template entities must be registered**: Use `:register_as(key)` on templates
- **Templates are cloned**: Each particle is a clone with position/velocity/rotation/TTL overrides
- **Template's RigidBody preserved**: Friction, max_speed, forces are kept from template
- **Coordinate system**: 0° points up, angles increase clockwise
- **Catch-up emission**: If dt is large, multiple emissions may occur per frame
- **Random selection**: Templates are chosen randomly from the list

---

### Additional Components

#### `:with_lua_timer(duration, callback)`

Add LuaTimer component that calls a Lua function after duration.

**Parameters:**

- `duration` - Time in seconds
- `callback` - Lua function name to call when timer expires

**Callback Signature:**

```lua
function callback_name(ctx, input)
    -- ctx: EntityContext table with all component data (see Phase Component section)
    -- ctx.id: entity ID (u64) - the entity that owns this timer
    -- ctx.timer: { duration, elapsed, callback } - timer info
    -- input: input state table
    -- Full access to engine API
end
```

**Example:**

```lua
-- Add timer during entity spawn
engine.spawn()
    :with_position(100, 100)
    :with_sprite("enemy", 32, 32, 16, 16)
    :with_lua_timer(3.0, "enemy_explode")
    :build()

-- Define the callback function
function enemy_explode(ctx, input)
    engine.log_info("Enemy " .. ctx.id .. " exploding at position " .. ctx.pos.x .. "," .. ctx.pos.y)
    engine.play_sound("explosion")
    engine.entity_despawn(ctx.id)
end
```

**Features:**

- Timer automatically resets after firing (repeats every `duration` seconds)
- Lua callback has full engine API access (spawn/despawn entities, play audio, modify signals, etc.)
- Can be added at spawn-time with `:with_lua_timer()` or at runtime with `engine.entity_insert_lua_timer()`

**See also:** `engine.entity_insert_lua_timer()` in the [Entity Commands](#entity-commands) section.

#### `:with_ttl(seconds)`

Add time-to-live component - entity automatically despawns after the specified duration.

**Parameters:**

- `seconds` (number): Time in seconds before entity despawns

**Example:**

```lua
-- Spawn a projectile that despawns after 5 seconds
engine.spawn()
    :with_position(100, 100)
    :with_sprite("bullet", 8, 8, 4, 4)
    :with_velocity(200, 0)
    :with_ttl(5.0)
    :build()
```

**Note:** Unlike LuaTimer, TTL has no callback - it's a "fire and forget" mechanism for temporary entities like projectiles, particles, or visual effects.

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

#### `engine.clear_scalar(key)`

Remove a scalar signal.

```lua
engine.clear_scalar("player_y")
```

#### `engine.clear_integer(key)`

Remove an integer signal.

```lua
engine.clear_integer("temp_score")
```

#### `engine.clear_string(key)`

Remove a string signal.

```lua
engine.clear_string("current_powerup")
```

#### `engine.set_entity(key, entity_id)`

Store an entity ID in world signals (alternative to `:register_as()`).

```lua
local spawned_id = -- ... get from spawn
engine.set_entity("special_enemy", spawned_id)
```

#### `engine.remove_entity(key)`

Remove an entity registration from world signals.

```lua
engine.remove_entity("special_enemy")
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

2. **From collision callbacks** - Entity IDs are provided in the collision context. Use `collision_entity_*` functions:

   ```lua
   function on_ball_brick(ctx)
       local ball_id = ctx.a.id    -- Ball entity ID
       local brick_id = ctx.b.id   -- Brick entity ID
       engine.collision_entity_despawn(brick_id)
   end
   ```

3. **From phase/timer callbacks** - The entity ID is available in the context object:

   ```lua
   function player_idle_enter(ctx, input)
       engine.entity_set_animation(ctx.id, "idle_anim")
   end
   ```

### `engine.entity_set_velocity(entity_id, vx, vy)`

Set entity's velocity (requires RigidBody component).

```lua
engine.entity_set_velocity(ball_id, 300, -300)
```

### `engine.entity_set_position(entity_id, x, y)`

Set entity's world position.

```lua
engine.entity_set_position(player_id, 400, 300)
```

### `engine.entity_despawn(entity_id)`

Remove an entity from the world.

```lua
engine.entity_despawn(enemy_id)
```

### `engine.entity_menu_despawn(entity_id)`

Remove a menu entity along with its items, cursor, and associated textures. This is the proper way to clean up menu entities as it handles all related resources.

**Parameters:**

- `entity_id` - Menu entity ID (entity with Menu component)

```lua
-- Clean up menu when switching scenes
local menu_id = engine.get_entity("main_menu")
if menu_id then
    engine.entity_menu_despawn(menu_id)
end
```

**Note:** This function cleans up:

- All menu item entities
- The cursor entity (if present)
- Generated menu item textures from TextureStore
- The menu entity itself

### `engine.entity_set_rotation(entity_id, degrees)`

Set entity's rotation in degrees.

```lua
engine.entity_set_rotation(player_id, 45)  -- Rotate 45 degrees
```

### `engine.entity_set_scale(entity_id, sx, sy)`

Set entity's scale (1.0 = normal size).

```lua
engine.entity_set_scale(boss_id, 2.0, 2.0)  -- Double size
engine.entity_set_scale(player_id, 0.5, 1.0)  -- Half width
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
engine.entity_signal_set_integer(player_id, "hp", 3)
engine.entity_signal_set_integer(enemy_id, "damage", 10)
```

### `engine.entity_signal_set_scalar(entity_id, key, value)`

Set floating-point signal on entity.

```lua
engine.entity_signal_set_scalar(player_id, "speed", 150.5)
```

### `engine.entity_signal_set_string(entity_id, key, value)`

Set string signal on entity.

```lua
engine.entity_signal_set_string(player_id, "state", "running")
```

### `engine.entity_insert_lua_timer(entity_id, duration, callback)`

Insert LuaTimer component on entity at runtime.

**Parameters:**

- `entity_id` - Entity to add timer to
- `duration` - Time in seconds before timer fires
- `callback` - Lua function name to call when timer expires

**Example:**

```lua
-- In a phase callback (regular context)
function player_hit_enter(ctx, input)
    -- Give player 10 seconds of invulnerability
    engine.entity_signal_set_flag(ctx.id, "invulnerable")
    engine.entity_insert_lua_timer(ctx.id, 10.0, "remove_invulnerability")
end

-- Timer callback
function remove_invulnerability(ctx, input)
    engine.entity_signal_clear_flag(ctx.id, "invulnerable")
    engine.play_sound("powerup_end")
    engine.log_info("Invulnerability expired for entity " .. ctx.id)
end
```

**Note:** The timer automatically repeats every `duration` seconds until the component is removed or the entity is despawned.

### `engine.entity_remove_lua_timer(entity_id)`

Remove LuaTimer component from an entity to stop the timer.

**Parameters:**

- `entity_id` - Entity to remove timer from

**Example:**

```lua
-- One-shot timer that removes itself after firing
function on_timer_title_test(ctx, input)
    engine.log_info("Timer fired once!")
    engine.play_sound("beep")

    -- Remove timer so it doesn't repeat
    engine.entity_remove_lua_timer(ctx.id)
end
```

### `engine.entity_insert_ttl(entity_id, seconds)`

Insert TTL component on entity at runtime.

**Parameters:**

- `entity_id` (integer): Target entity ID
- `seconds` (number): Time in seconds before entity despawns

**Example:**

```lua
-- Give an enemy 30 seconds to live
engine.entity_insert_ttl(enemy_id, 30.0)
```

### `engine.entity_insert_tween_position(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)`

Add or replace TweenPosition component at runtime to animate entity movement.

**Parameters:**

- `entity_id` - Entity to add tween to
- `from_x`, `from_y` - Starting position
- `to_x`, `to_y` - Target position
- `duration` - Animation duration in seconds
- `easing` - Easing function (string): "linear", "quad_in", "quad_out", "quad_in_out", "cubic_in", "cubic_out", "cubic_in_out"
- `loop_mode` - Loop behavior (string): "once", "loop", "ping_pong"
- `backwards` - Start the tween from the end and play in reverse (boolean, default false)

**Example:**

```lua
-- Make entity slide from left to right with smooth easing
local entity_id = engine.get_entity("player")
engine.entity_insert_tween_position(entity_id, -100, 0, 100, 0, 2.0, "quad_out", "once", false)
```

### `engine.entity_insert_tween_rotation(entity_id, from, to, duration, easing, loop_mode, backwards)`

Add or replace TweenRotation component at runtime to animate entity rotation.

**Parameters:**

- `entity_id` - Entity to add tween to
- `from` - Starting rotation in degrees
- `to` - Target rotation in degrees
- `duration` - Animation duration in seconds
- `easing` - Easing function (string): "linear", "quad_in", "quad_out", "quad_in_out", "cubic_in", "cubic_out", "cubic_in_out"
- `loop_mode` - Loop behavior (string): "once", "loop", "ping_pong"
- `backwards` - Start the tween from the end and play in reverse (boolean, default false)

**Example:**

```lua
-- Rotate entity 360 degrees continuously
local entity_id = engine.get_entity("spinner")
engine.entity_insert_tween_rotation(entity_id, 0, 360, 3.0, "linear", "loop", false)
```

### `engine.entity_insert_tween_scale(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)`

Add or replace TweenScale component at runtime to animate entity scaling.

**Parameters:**

- `entity_id` - Entity to add tween to
- `from_x`, `from_y` - Starting scale
- `to_x`, `to_y` - Target scale
- `duration` - Animation duration in seconds
- `easing` - Easing function (string): "linear", "quad_in", "quad_out", "quad_in_out", "cubic_in", "cubic_out", "cubic_in_out"
- `loop_mode` - Loop behavior (string): "once", "loop", "ping_pong"
- `backwards` - Start tween from end and play in reverse (boolean, default false)

**Example:**

```lua
-- Make entity pulse between normal and slightly larger
local entity_id = engine.get_entity("button")
engine.entity_insert_tween_scale(entity_id, 1.0, 1.0, 1.2, 1.2, 0.5, "quad_in_out", "ping_pong", false)
```

### `engine.entity_remove_tween_position(entity_id)`

Remove TweenPosition component from an entity to stop position animation.

**Parameters:**

- `entity_id` - Entity to remove tween from

**Example:**

```lua
-- Stop position animation when player takes damage
function on_player_hit(entity_id)
    engine.entity_remove_tween_position(entity_id)
end
```

### `engine.entity_remove_tween_rotation(entity_id)`

Remove TweenRotation component from an entity to stop rotation animation.

**Parameters:**

- `entity_id` - Entity to remove tween from

**Example:**

```lua
-- Stop rotation animation
local entity_id = engine.get_entity("spinner")
engine.entity_remove_tween_rotation(entity_id)
```

### `engine.entity_remove_tween_scale(entity_id)`

Remove TweenScale component from an entity to stop scale animation.

**Parameters:**

- `entity_id` - Entity to remove tween from

**Example:**

```lua
-- Stop scale pulsing when menu is closed
function on_menu_close()
    local button = engine.get_entity("start_button")
    engine.entity_remove_tween_scale(button)
end
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

### Physics Commands

The following commands manipulate the physics properties of entities with RigidBody components.

### `engine.entity_add_force(entity_id, name, x, y, enabled)`

Add or update a named acceleration force on an entity's RigidBody.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `name` - Unique force identifier
- `x`, `y` - Acceleration in world units per second squared
- `enabled` - Whether the force is active

```lua
-- Add gravity to an entity
engine.entity_add_force(player_id, "gravity", 0, 980, true)

-- Add jump force (disabled until player jumps)
engine.entity_add_force(player_id, "jump", 0, -1500, false)
```

### `engine.entity_remove_force(entity_id, name)`

Remove a named force entirely from an entity's RigidBody.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `name` - Force identifier to remove

```lua
-- Remove wind force when player enters shelter
engine.entity_remove_force(player_id, "wind")
```

### `engine.entity_set_force_enabled(entity_id, name, enabled)`

Enable or disable a specific force without removing it.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `name` - Force identifier
- `enabled` - Whether to enable (true) or disable (false) the force

```lua
-- Disable gravity when player is on ground
engine.entity_set_force_enabled(player_id, "gravity", false)

-- Re-enable gravity when player leaves ground
engine.entity_set_force_enabled(player_id, "gravity", true)

-- Enable jump force momentarily
engine.entity_set_force_enabled(player_id, "jump", true)
```

### `engine.entity_set_force_value(entity_id, name, x, y)`

Update the acceleration value of an existing force.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `name` - Force identifier
- `x`, `y` - New acceleration values

```lua
-- Increase gravity in a specific area
engine.entity_set_force_value(player_id, "gravity", 0, 1500)

-- Change wind direction
engine.entity_set_force_value(player_id, "wind", -100, 0)
```

### `engine.entity_set_friction(entity_id, friction)`

Set velocity damping on an entity's RigidBody.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `friction` - Damping factor (0.0 = no friction, ~5.0 = responsive, ~10.0 = heavy drag)

```lua
-- Apply ice physics (low friction)
engine.entity_set_friction(player_id, 0.5)

-- Apply mud physics (high friction)
engine.entity_set_friction(player_id, 15.0)
```

### `engine.entity_set_max_speed(entity_id, max_speed)`

Set or remove maximum velocity limit on an entity's RigidBody.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `max_speed` - Maximum speed in world units/sec, or `nil` to remove limit

```lua
-- Limit player speed
engine.entity_set_max_speed(player_id, 300.0)

-- Remove speed limit (power-up)
engine.entity_set_max_speed(player_id, nil)
```

### `engine.entity_freeze(entity_id)`

Freeze an entity, preventing the movement system from updating its physics.

When frozen, velocity and acceleration are not applied. Position can still be modified externally (e.g., by StuckTo system or direct manipulation).

**Parameters:**

- `entity_id` - Entity with RigidBody component

```lua
-- Freeze ball while stuck to paddle
engine.entity_freeze(ball_id)
```

### `engine.entity_unfreeze(entity_id)`

Unfreeze an entity, allowing the movement system to resume physics calculations.

**Parameters:**

- `entity_id` - Entity with RigidBody component

```lua
-- Release ball from paddle
engine.entity_unfreeze(ball_id)
```

### `engine.entity_set_speed(entity_id, speed)`

Set entity's speed while maintaining velocity direction. If velocity is zero, prints a warning and does nothing.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `speed` - New speed magnitude (in world units per second)

```lua
-- Increase ball speed after hitting brick
engine.entity_set_speed(ball_id, 400.0)

-- Slow down player
engine.entity_set_speed(player_id, 100.0)
```

**Note:** This function normalizes the current velocity and scales it by the new speed. If the entity has zero velocity (not moving), this is a no-op and a warning is printed to stderr.

**Complete Physics Example:**

```lua
-- Platform game player with multiple forces
function setup_player_physics(player_id)
    -- Add gravity (always enabled)
    engine.entity_add_force(player_id, "gravity", 0, 980, true)

    -- Add jump force (disabled by default)
    engine.entity_add_force(player_id, "jump", 0, -2000, false)

    -- Add movement force (controlled by input)
    engine.entity_add_force(player_id, "move", 0, 0, true)

    -- Set friction for responsive controls
    engine.entity_set_friction(player_id, 8.0)

    -- Limit max speed
    engine.entity_set_max_speed(player_id, 250.0)
end

function player_jump(player_id)
    -- Disable gravity, enable jump for a moment
    engine.entity_set_force_enabled(player_id, "gravity", false)
    engine.entity_set_force_enabled(player_id, "jump", true)

    -- Schedule re-enabling gravity after 0.1 seconds
    engine.entity_insert_lua_timer(player_id, 0.1, "restore_gravity")
end

function restore_gravity(ctx, input)
    engine.entity_set_force_enabled(ctx.id, "gravity", true)
    engine.entity_set_force_enabled(ctx.id, "jump", false)
    engine.entity_remove_lua_timer(ctx.id)
end

function player_move(player_id, direction)
    -- direction: -1 (left), 0 (stop), 1 (right)
    local move_accel = 500 * direction
    engine.entity_set_force_value(player_id, "move", move_accel, 0)
end
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
    local ball_speed_sq = ctx.a.speed_sq -- Squared speed (use math.sqrt for actual speed)
    local ball_rect = ctx.a.rect       -- { x, y, w, h }
    local ball_signals = ctx.a.signals -- { flags = {...}, integers = {...}, ... }

    local player_id = ctx.b.id         -- Entity B ID
    local player_pos = ctx.b.pos
    local player_vel = ctx.b.vel
    local player_speed_sq = ctx.b.speed_sq
    local player_rect = ctx.b.rect
    local player_signals = ctx.b.signals

    local sides = ctx.sides            -- Collision sides
    -- sides.a contains: "top", "bottom", "left", "right"
    -- sides.b contains: "top", "bottom", "left", "right"

    -- Manipulate entities using collision-specific functions
    engine.collision_entity_set_velocity(ball_id, new_vx, new_vy)
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

#### `engine.collision_set_scalar(key, value)`

Set global scalar signal during collision.

```lua
engine.collision_set_scalar("impact_force", 150.0)
```

#### `engine.collision_set_string(key, value)`

Set global string signal during collision.

```lua
engine.collision_set_string("last_collision", "ball_brick")
```

#### `engine.collision_clear_scalar(key)`

Remove a scalar signal during collision.

```lua
engine.collision_clear_scalar("temp_boost")
```

#### `engine.collision_clear_integer(key)`

Remove an integer signal during collision.

```lua
engine.collision_clear_integer("combo_count")
```

#### `engine.collision_clear_string(key)`

Remove a string signal during collision.

```lua
engine.collision_clear_string("active_effect")
```

#### `engine.collision_spawn()`

Create a new entity builder for spawning entities during collision. Returns a `CollisionEntityBuilder` with the same capabilities as the standard `EntityBuilder`.

All methods available on `engine.spawn()` are also available on `engine.collision_spawn()`. See [Entity Spawning](#entity-spawning) for the complete method reference.

```lua
function on_ball_brick(ctx)
    -- Spawn a particle effect at the brick's position
    engine.collision_spawn()
        :with_position(ctx.b.pos.x, ctx.b.pos.y)
        :with_sprite("particle", 8, 8, 4, 4)
        :with_group("particles")
        :with_velocity(0, -50)
        :with_lua_timer(0.5, "despawn_particle")
        :build()

    engine.collision_entity_despawn(ctx.b.id)
end

function despawn_particle(entity_id)
    -- Note: This timer callback runs in regular context, but entity_despawn
    -- doesn't exist in regular API. Use phase callbacks or spawn entities
    -- with a fixed lifetime instead.
end
```

#### `engine.collision_clone(source_key)`

Clone an existing entity during collision handling. The source entity is looked up by its WorldSignals key. All components are cloned, and builder overrides win over the template's values. Animation is always reset to frame 0.

All methods available on `engine.clone()` are also available on `engine.collision_clone()`. See [Entity Cloning](#entity-cloning) for details.

```lua
function on_enemy_hit(ctx)
    -- Clone a bullet template at the enemy's position
    engine.collision_clone("bullet_template")
        :with_position(ctx.a.pos.x, ctx.a.pos.y)
        :with_velocity(0, -200)
        :with_ttl(2.0)
        :build()
end
```

#### `engine.collision_phase_transition(entity_id, phase)`

Request a phase transition for an entity during collision handling. Useful for triggering state changes like "hurt" or "stunned" states when collisions occur.

**Parameters:**

- `entity_id` - Entity with LuaPhase component
- `phase` - Target phase name

```lua
function on_player_enemy(ctx)
    local player_id = ctx.a.id

    -- Transition player to hurt phase
    engine.collision_phase_transition(player_id, "hurt")

    -- Play hit sound
    engine.collision_play_sound("player_hit")
end
```

#### `engine.collision_entity_set_position(entity_id, x, y)`

Set an entity's world position during collision handling. Useful for teleporting entities or implementing push-back mechanics.

**Parameters:**

- `entity_id` - Entity with MapPosition component
- `x`, `y` - New world position

```lua
function on_player_teleporter(ctx)
    local player_id = ctx.a.id
    -- Teleport player to destination
    engine.collision_entity_set_position(player_id, 400, 300)
    engine.collision_play_sound("teleport")
end
```

#### `engine.collision_entity_set_velocity(entity_id, vx, vy)`

Set an entity's velocity during collision handling. Essential for implementing bounce physics and knockback effects.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `vx`, `vy` - New velocity components

```lua
function on_ball_wall(ctx)
    local ball_id = ctx.a.id
    local ball_vel = ctx.a.vel
    local sides = ctx.sides.a

    local new_vx = ball_vel.x
    local new_vy = ball_vel.y

    if sides.left or sides.right then
        new_vx = -new_vx
    end
    if sides.top or sides.bottom then
        new_vy = -new_vy
    end

    engine.collision_entity_set_velocity(ball_id, new_vx, new_vy)
end
```

#### `engine.collision_entity_despawn(entity_id)`

Remove an entity during collision handling. The entity will be despawned after the collision callback returns.

**Parameters:**

- `entity_id` - Entity to despawn

```lua
function on_bullet_enemy(ctx)
    local bullet_id = ctx.a.id
    local enemy_id = ctx.b.id

    -- Despawn both the bullet and the enemy
    engine.collision_entity_despawn(bullet_id)
    engine.collision_entity_despawn(enemy_id)

    -- Spawn explosion effect
    engine.collision_spawn()
        :with_position(ctx.b.pos.x, ctx.b.pos.y)
        :with_sprite("explosion", 32, 32, 16, 16)
        :with_animation("explosion_anim")
        :build()
end
```

#### `engine.collision_entity_signal_set_integer(entity_id, key, value)`

Set an integer signal on an entity's Signals component during collision handling.

**Parameters:**

- `entity_id` - Entity with Signals component
- `key` - Signal key
- `value` - Integer value

```lua
function on_ball_brick(ctx)
    local brick_id = ctx.b.id
    local brick_signals = ctx.b.signals

    local hp = brick_signals.integers and brick_signals.integers.hp or 1
    hp = hp - 1

    if hp <= 0 then
        engine.collision_entity_despawn(brick_id)
    else
        engine.collision_entity_signal_set_integer(brick_id, "hp", hp)
    end
end
```

#### `engine.collision_entity_signal_set_flag(entity_id, flag)`

Set a flag on an entity's Signals component during collision handling.

**Parameters:**

- `entity_id` - Entity with Signals component
- `flag` - Flag name

```lua
function on_player_powerup(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_signal_set_flag(player_id, "powered_up")
    engine.collision_entity_despawn(ctx.b.id)
end
```

#### `engine.collision_entity_signal_clear_flag(entity_id, flag)`

Clear a flag on an entity's Signals component during collision handling.

**Parameters:**

- `entity_id` - Entity with Signals component
- `flag` - Flag name

```lua
function on_player_debuff_zone(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_signal_clear_flag(player_id, "shield_active")
end
```

#### `engine.collision_entity_insert_stuckto(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)`

Attach an entity to another entity during collision handling.

**Parameters:**

- `entity_id` - Entity to attach
- `target_id` - Entity to follow
- `follow_x`, `follow_y` - Which axes to follow
- `offset_x`, `offset_y` - Position offset from target
- `stored_vx`, `stored_vy` - Velocity to restore when released

```lua
function on_ball_sticky_paddle(ctx)
    local ball_id = ctx.a.id
    local paddle_id = ctx.b.id

    -- Calculate offset (ball position relative to paddle)
    local offset_x = ctx.a.pos.x - ctx.b.pos.x

    -- Attach ball to paddle
    engine.collision_entity_insert_stuckto(
        ball_id, paddle_id,
        true, false,      -- Follow X only
        offset_x, -12,    -- Offset (centered X, above paddle)
        300, -300         -- Stored velocity for release
    )
end
```

#### `engine.collision_entity_freeze(entity_id)`

Freeze an entity during collision handling, preventing physics calculations.

**Parameters:**

- `entity_id` - Entity with RigidBody component

```lua
function on_ball_sticky_paddle(ctx)
    local ball_id = ctx.a.id
    -- Freeze ball when it hits a sticky paddle
    engine.collision_entity_freeze(ball_id)
end
```

#### `engine.collision_entity_unfreeze(entity_id)`

Unfreeze an entity during collision handling, resuming physics calculations.

**Parameters:**

- `entity_id` - Entity with RigidBody component

```lua
function on_ball_release(ctx)
    local ball_id = ctx.a.id
    engine.collision_entity_unfreeze(ball_id)
end
```

#### `engine.collision_entity_add_force(entity_id, name, x, y, enabled)`

Add or update a named acceleration force during collision handling.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `name` - Unique force identifier
- `x`, `y` - Acceleration in world units per second squared
- `enabled` - Whether the force is active

```lua
function on_player_wind_zone(ctx)
    local player_id = ctx.a.id
    -- Add wind force when entering wind zone
    engine.collision_entity_add_force(player_id, "wind", 200, 0, true)
end
```

#### `engine.collision_entity_set_force_enabled(entity_id, name, enabled)`

Enable or disable a specific force during collision handling.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `name` - Force identifier
- `enabled` - Whether to enable (true) or disable (false)

```lua
function on_player_ground(ctx)
    local player_id = ctx.a.id
    -- Disable gravity when landing on ground
    engine.collision_entity_set_force_enabled(player_id, "gravity", false)
end

function on_player_leave_ground(ctx)
    local player_id = ctx.a.id
    -- Re-enable gravity when leaving ground
    engine.collision_entity_set_force_enabled(player_id, "gravity", true)
end
```

#### `engine.collision_entity_set_speed(entity_id, speed)`

Set entity's speed while maintaining velocity direction during collision handling. If velocity is zero, prints a warning and does nothing.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `speed` - New speed magnitude (in world units per second)

```lua
function on_ball_speed_boost(ctx)
    local ball_id = ctx.a.id
    -- Increase ball speed when hitting speed booster
    engine.collision_entity_set_speed(ball_id, 500.0)
    engine.collision_play_sound("speed_up")
end
```

#### `engine.collision_release_stuckto(entity_id)`

Release entity from StuckTo attachment during collision handling, restoring stored velocity.

**Parameters:**

- `entity_id` - Entity with StuckTo component

```lua
function on_ball_release_trigger(ctx)
    local ball_id = ctx.a.id
    engine.collision_release_stuckto(ball_id)
    engine.collision_play_sound("launch")
end
```

#### `engine.collision_entity_signal_set_scalar(entity_id, key, value)`

Set a floating-point signal on an entity's Signals component during collision handling.

**Parameters:**

- `entity_id` - Entity with Signals component
- `key` - Signal key
- `value` - Float value

```lua
function on_player_speed_boost(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_signal_set_scalar(player_id, "speed_multiplier", 1.5)
end
```

#### `engine.collision_entity_signal_set_string(entity_id, key, value)`

Set a string signal on an entity's Signals component during collision handling.

**Parameters:**

- `entity_id` - Entity with Signals component
- `key` - Signal key
- `value` - String value

```lua
function on_player_color_change(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_signal_set_string(player_id, "color", "red")
end
```

#### `engine.collision_entity_insert_lua_timer(entity_id, duration, callback)`

Insert a LuaTimer component on an entity during collision handling.

**Parameters:**

- `entity_id` - Entity to add timer to
- `duration` - Time in seconds before timer fires
- `callback` - Lua function name to call when timer expires

```lua
function on_player_powerup(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_signal_set_flag(player_id, "powered_up")
    engine.collision_entity_insert_lua_timer(player_id, 10.0, "remove_powerup")
    engine.collision_entity_despawn(ctx.b.id)
end
```

#### `engine.collision_entity_remove_lua_timer(entity_id)`

Remove LuaTimer component from an entity during collision handling.

**Parameters:**

- `entity_id` - Entity to remove timer from

```lua
function on_timer_cancel_zone(ctx)
    local entity_id = ctx.a.id
    engine.collision_entity_remove_lua_timer(entity_id)
end
```

#### `engine.collision_entity_insert_ttl(entity_id, seconds)`

Insert TTL component on an entity during collision handling.

**Parameters:**

- `entity_id` (integer): Target entity ID
- `seconds` (number): Time in seconds before entity despawns

**Example:**

```lua
function on_bullet_hit(ctx)
    -- Bullet becomes a fading particle for 0.5 seconds
    engine.collision_entity_set_velocity(ctx.a.id, 0, 0)
    engine.collision_entity_set_animation(ctx.a.id, "bullet_fade")
    engine.collision_entity_insert_ttl(ctx.a.id, 0.5)
end
```

#### `engine.collision_entity_restart_animation(entity_id)`

Restart entity's current animation from frame 0 during collision handling.

**Parameters:**

- `entity_id` - Entity with Animation component

```lua
function on_player_hit(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_restart_animation(player_id)
end
```

#### `engine.collision_entity_set_animation(entity_id, animation_key)`

Change entity's animation during collision handling.

**Parameters:**

- `entity_id` - Entity with Animation component
- `animation_key` - Animation identifier (registered with `engine.register_animation()`)

```lua
function on_player_hit(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_set_animation(player_id, "player_hit")
end
```

#### `engine.collision_entity_insert_tween_position(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)`

Add or replace TweenPosition component during collision handling.

**Parameters:**

- `entity_id` - Entity to add tween to
- `from_x`, `from_y` - Starting position
- `to_x`, `to_y` - Target position
- `duration` - Animation duration in seconds
- `easing` - Easing function: "linear", "quad_in", "quad_out", "quad_in_out", "cubic_in", "cubic_out", "cubic_in_out"
- `loop_mode` - Loop behavior: "once", "loop", "ping_pong"
- `backwards` - Start the tween from the end and play in reverse (boolean, default false)

```lua
function on_player_bounce_pad(ctx)
    local player_id = ctx.a.id
    local pos = ctx.a.pos
    engine.collision_entity_insert_tween_position(player_id, pos.x, pos.y, pos.x, pos.y - 100, 0.5, "quad_out", "once", false)
end
```

#### `engine.collision_entity_insert_tween_rotation(entity_id, from, to, duration, easing, loop_mode, backwards)`

Add or replace TweenRotation component during collision handling.

**Parameters:**

- `entity_id` - Entity to add tween to
- `from` - Starting rotation in degrees
- `to` - Target rotation in degrees
- `duration` - Animation duration in seconds
- `easing` - Easing function
- `loop_mode` - Loop behavior
- `backwards` - Start the tween from the end and play in reverse (boolean, default false)

```lua
function on_player_spin_powerup(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_insert_tween_rotation(player_id, 0, 360, 1.0, "linear", "loop", false)
end
```

#### `engine.collision_entity_insert_tween_scale(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)`

Add or replace TweenScale component during collision handling.

**Parameters:**

- `entity_id` - Entity to add tween to
- `from_x`, `from_y` - Starting scale
- `to_x`, `to_y` - Target scale
- `duration` - Animation duration in seconds
- `easing` - Easing function
- `loop_mode` - Loop behavior
- `backwards` - Start tween from end and play in reverse (boolean, default false)

```lua
function on_player_grow_powerup(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_insert_tween_scale(player_id, 1.0, 1.0, 2.0, 2.0, 0.5, "quad_out", "once", false)
end
```

#### `engine.collision_entity_remove_tween_position(entity_id)`

Remove TweenPosition component during collision handling.

**Parameters:**

- `entity_id` - Entity to remove tween from

```lua
function on_player_stop_zone(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_remove_tween_position(player_id)
end
```

#### `engine.collision_entity_remove_tween_rotation(entity_id)`

Remove TweenRotation component during collision handling.

**Parameters:**

- `entity_id` - Entity to remove tween from

#### `engine.collision_entity_remove_tween_scale(entity_id)`

Remove TweenScale component during collision handling.

**Parameters:**

- `entity_id` - Entity to remove tween from

#### `engine.collision_entity_set_rotation(entity_id, degrees)`

Set entity's rotation during collision handling.

**Parameters:**

- `entity_id` - Entity to rotate
- `degrees` - Rotation angle in degrees

```lua
function on_player_orientation_trigger(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_set_rotation(player_id, 45)
end
```

#### `engine.collision_entity_set_scale(entity_id, sx, sy)`

Set entity's scale during collision handling.

**Parameters:**

- `entity_id` - Entity to scale
- `sx`, `sy` - Scale factors

```lua
function on_player_shrink_zone(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_set_scale(player_id, 0.5, 0.5)
end
```

#### `engine.collision_entity_remove_force(entity_id, name)`

Remove a named force from entity's RigidBody during collision handling.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `name` - Force identifier to remove

```lua
function on_player_leave_wind_zone(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_remove_force(player_id, "wind")
end
```

#### `engine.collision_entity_set_force_value(entity_id, name, x, y)`

Update the acceleration value of an existing force during collision handling.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `name` - Force identifier
- `x`, `y` - New acceleration values

```lua
function on_player_strong_wind_zone(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_set_force_value(player_id, "wind", 500, 0)
end
```

#### `engine.collision_entity_set_friction(entity_id, friction)`

Set velocity damping on entity's RigidBody during collision handling.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `friction` - Damping factor (0.0 = no friction, ~5.0 = responsive, ~10.0 = heavy drag)

```lua
function on_player_ice_zone(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_set_friction(player_id, 0.5)
end
```

#### `engine.collision_entity_set_max_speed(entity_id, max_speed)`

Set or remove maximum velocity limit during collision handling.

**Parameters:**

- `entity_id` - Entity with RigidBody component
- `max_speed` - Maximum speed in world units/sec, or `nil` to remove limit

```lua
function on_player_speed_limit_zone(ctx)
    local player_id = ctx.a.id
    engine.collision_entity_set_max_speed(player_id, 150.0)
end
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
        engine.collision_entity_despawn(brick_id)

        -- Award points
        local score = engine.get_integer("score") or 0
        engine.collision_set_integer("score", score + 100)
    else
        engine.collision_entity_signal_set_integer(brick_id, "hp", hp)
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

    engine.collision_entity_set_velocity(ball_id, new_vx, new_vy)
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

**Note:** Group names must not exceed 51 characters for optimal performance.

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

-- Phase callbacks (using EntityContext)
function player_sticky_enter(ctx, input)
    engine.entity_signal_set_flag(ctx.id, "sticky")
    engine.entity_set_animation(ctx.id, "vaus_glowing")
end

function player_sticky_update(ctx, input, dt)
    if ctx.time_in_phase >= 3.0 then
        engine.phase_transition(ctx.id, "glowing")
    end
end

function player_glowing_enter(ctx, input)
    engine.entity_signal_clear_flag(ctx.id, "sticky")
    engine.entity_set_animation(ctx.id, "vaus_glowing")
end

function player_hit_enter(ctx, input)
    engine.entity_set_animation(ctx.id, "vaus_hit")
end

function player_hit_update(ctx, input, dt)
    if ctx.time_in_phase >= 0.5 then
        engine.phase_transition(ctx.id, "glowing")
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
