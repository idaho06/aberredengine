# Aberred Engine - Lua Interface Architecture

This document describes the Lua scripting interface architecture and provides a guide for developers who want to add new Lua commands to interact with ECS components.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Module Structure](#module-structure)
3. [Command Flow: Lua to ECS](#command-flow-lua-to-ecs)
4. [Command Types and Queues](#command-types-and-queues)
5. [Entity Builder Pattern](#entity-builder-pattern)
6. [Signal Snapshot System](#signal-snapshot-system)
7. [Context Table Pooling](#context-table-pooling)
8. [Meta Schema (`engine.__meta`)](#meta-schema-enginemeta)
9. [How to Add New Lua Commands](#how-to-add-new-lua-commands)
10. [Best Practices](#best-practices)

---

## Architecture Overview

The Aberred Engine uses a **deferred command pattern** for Lua-Rust integration. Lua scripts cannot directly modify ECS entities—instead, they queue commands that are processed by Rust systems after Lua callbacks return.

### High-Level Flow

```text
┌───────────────────────────────────────────────────────────────────────────────┐
│                             GAME LOOP                                         │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐         │
│   │   Lua Script    │───▶│  Command Queue  │───▶│  Rust Systems   │         │
│   │                 │     │  (LuaAppData)   │     │  (process_*)    │         │
│   └─────────────────┘     └─────────────────┘     └─────────────────┘         │
│          │                       │                      │                     │
│          │ engine.spawn()        │ SpawnCmd             │ Commands.spawn()    │
│          │ engine.set_flag()     │ SignalCmd            │ world_signals.set   │
│          │ engine.despawn()      │ EntityCmd            │ entity.despawn()    │
│          ▼                       ▼                      ▼                     │
│   ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐         │
│   │ Signal Snapshot │◀───│  WorldSignals   │◀───│   ECS World     │         │
│   │   (read-only)   │     │   (Resource)    │     │                 │         │
│   └─────────────────┘     └─────────────────┘     └─────────────────┘         │
│                                                                               │
└───────────────────────────────────────────────────────────────────────────────┘
```

### Why Deferred Commands?

1. **Thread Safety**: Lua is single-threaded; direct ECS access would require complex synchronization.
2. **Consistency**: Commands are processed at predictable points in the game loop.
3. **Error Handling**: Commands can be validated and errors reported cleanly.
4. **Performance**: Batch processing of commands is more efficient than immediate execution.

---

## Module Structure

The Lua runtime is organized in `src/resources/lua_runtime/`:

```text
src/resources/lua_runtime/
├── mod.rs              # Public exports
├── runtime.rs          # LuaRuntime struct, LuaAppData, pool types, GameConfigSnapshot
├── engine_api.rs       # API registration: register_*_api() methods, macros (register_cmd!, etc.), push_fn_meta()
├── command_queues.rs   # drain_*_commands() methods, clear_all_commands, cache updates
├── stub_meta.rs        # Stub metadata types, meta registration (types, enums, callbacks, builder meta)
├── commands.rs         # Command enums (EntityCmd, SignalCmd, CameraFollowCmd, InputCmd, etc.)
├── context.rs          # Entity context builder for Lua callbacks (pooled), snapshot types
├── entity_builder.rs   # LuaEntityBuilder fluent API for spawning/cloning
├── input_snapshot.rs   # InputSnapshot, DigitalInputs, AnalogInputs for Lua callbacks
└── spawn_data.rs       # Data structures for spawn configuration (SpawnCmd, component data structs)
```

Command processing lives in a separate submodule:

```text
src/systems/lua_commands/
├── mod.rs              # Re-exports, EntityCmdQueries/ContextQueries SystemParams, build_tween helper
├── context.rs          # build_entity_context: gathers ECS data → pooled Lua ctx table
├── entity_cmd.rs       # process_entity_commands: runtime entity manipulation (physics, signals, tweens, shaders, hierarchy)
├── spawn_cmd.rs        # process_spawn_command, process_clone_command: entity creation via apply_components()
└── parse.rs            # Animation condition parsing helpers
```

### Key Components

#### `LuaRuntime` (runtime.rs)

The main struct managing the Lua interpreter. It:

- Initializes the Lua state with MLua
- Delegates API registration to `register_*_api()` methods in `engine_api.rs`
- Manages `LuaAppData` for command queuing
- Manages **context table pools** for collision, entity, and input callbacks (see [Context Table Pooling](#context-table-pooling))
- Provides `get_function()` to resolve global Lua functions by name

#### `engine_api.rs`

Contains all `engine` table API registration. Uses three macros:

- `register_cmd!` — registers a single Lua function that pushes to a queue, with metadata
- `register_entity_cmds!` — batch-registers entity commands with a name prefix
- `define_entity_cmds!` — defines all entity commands once; called with `""` and `"collision_"` prefixes

And one helper function:

- `push_fn_meta()` — pushes function metadata to `engine.__meta.functions` (used for manually registered functions that don't go through `register_cmd!`)

#### `command_queues.rs`

Contains all `drain_*_commands()` methods and cache update functions (`update_signal_cache`, `update_bindings_cache`, etc.).

#### `LuaAppData` (runtime.rs)

Internal shared state accessible from Lua closures:

```rust
pub(super) struct LuaAppData {
    // Regular command queues
    asset_commands: RefCell<Vec<AssetCmd>>,
    spawn_commands: RefCell<Vec<SpawnCmd>>,
    audio_commands: RefCell<Vec<AudioLuaCmd>>,
    signal_commands: RefCell<Vec<SignalCmd>>,
    phase_commands: RefCell<Vec<PhaseCmd>>,
    entity_commands: RefCell<Vec<EntityCmd>>,
    group_commands: RefCell<Vec<GroupCmd>>,
    tilemap_commands: RefCell<Vec<TilemapCmd>>,
    camera_commands: RefCell<Vec<CameraCmd>>,
    animation_commands: RefCell<Vec<AnimationCmd>>,
    render_commands: RefCell<Vec<RenderCmd>>,
    clone_commands: RefCell<Vec<CloneCmd>>,
    gameconfig_commands: RefCell<Vec<GameConfigCmd>>,
    camera_follow_commands: RefCell<Vec<CameraFollowCmd>>,
    input_commands: RefCell<Vec<InputCmd>>,

    // Collision-scoped command queues (processed immediately after each collision callback)
    collision_entity_commands: RefCell<Vec<EntityCmd>>,
    collision_signal_commands: RefCell<Vec<SignalCmd>>,
    collision_audio_commands: RefCell<Vec<AudioLuaCmd>>,
    collision_spawn_commands: RefCell<Vec<SpawnCmd>>,
    collision_phase_commands: RefCell<Vec<PhaseCmd>>,
    collision_camera_commands: RefCell<Vec<CameraCmd>>,
    collision_clone_commands: RefCell<Vec<CloneCmd>>,

    // Cached snapshots (read-only for Lua)
    signal_snapshot: RefCell<Arc<SignalSnapshot>>,
    tracked_groups: RefCell<FxHashSet<String>>,
    gameconfig_snapshot: RefCell<GameConfigSnapshot>,
    bindings_snapshot: RefCell<HashMap<String, String>>,
}
```

#### Command Enums (commands.rs)

Each command type is a Rust enum that encapsulates all data needed to perform an operation. See [Command Types and Queues](#command-types-and-queues) for the full list.

---

## Command Flow: Lua to ECS

### Step 1: Lua Calls Engine API

```lua
-- In a Lua script
engine.entity_set_velocity(ball_id, new_vx, new_vy)
engine.set_flag("switch_scene")
```

### Step 2: Command is Queued

Most Lua functions are registered via the `register_cmd!` macro, which generates the closure, pushes to the correct queue, and registers metadata in `engine.__meta` — all in one declaration:

```rust
// In engine_api.rs — macro-based registration (typical pattern)
register_cmd!(engine, self.lua, meta_fns, "set_scalar", signal_commands,
    |(key, value)| (String, f32), SignalCmd::SetScalar { key, value },
    desc = "Set a world signal scalar value", cat = "signal",
    params = [("key", "string"), ("value", "number")]);
```

Entity commands are registered in bulk via `define_entity_cmds!`, which calls `register_entity_cmds!` under the hood. A single definition under `define_entity_cmds!` is invoked twice — once with `""` prefix for regular commands and once with `"collision_"` for collision commands:

```rust
// In engine_api.rs
define_entity_cmds!(engine, self.lua, meta_fns, "", entity_commands);
define_entity_cmds!(engine, self.lua, meta_fns, "collision_", collision_entity_commands);
```

For functions with non-push logic (reads, builders, validation), registration is manual with a separate `push_fn_meta()` call for metadata:

```rust
// Manual registration example (read function)
engine.set("get_scalar", self.lua.create_function(|lua, key: String| {
    let value = lua.app_data_ref::<LuaAppData>()
        .and_then(|data| data.signal_snapshot.borrow().scalars.get(&key).copied());
    Ok(value)
})?);
push_fn_meta(&self.lua, &meta_fns, "get_scalar", "Get a world signal scalar value", "signal",
    &[("key", "string")], Some("number?"));
```

### Step 3: Rust Drains Commands

After the Lua callback returns, Rust calls `drain_*_commands()` (defined in `command_queues.rs`):

```rust
// In lua_plugin.rs update()
for cmd in lua_runtime.drain_entity_commands() {
    // Process each command
}
```

### Step 4: Commands are Processed

The processing functions in `systems/lua_commands/` apply changes to the ECS. `process_entity_commands` takes an `EntityCmdQueries` SystemParam bundle and dispatches to sub-functions:

```rust
// In lua_commands/entity_cmd.rs
pub fn process_entity_commands(
    commands: &mut Commands,
    entity_commands: impl IntoIterator<Item = EntityCmd>,
    cmd_queries: &mut EntityCmdQueries,
    systems_store: &SystemsStore,
    anim_store: &AnimationStore,
) { ... }
```

Spawn and clone commands are processed via `process_spawn_command()` and `process_clone_command()` in `lua_commands/spawn_cmd.rs`. Both delegate to the shared `apply_components()` helper.

---

## Command Types and Queues

### Regular vs Collision Queues

The engine maintains **two sets** of command queues:

| Queue Type | When Processed | Use Case |
| ---------- | -------------- | -------- |
| Regular (`entity_commands`, etc.) | After phase/timer/update callbacks | Normal game logic |
| Collision (`collision_entity_commands`, etc.) | Immediately after each collision callback | Collision response |

This distinction matters because collision callbacks need immediate processing to ensure position corrections and velocity changes happen before the next collision is detected.

### Command Categories

| Category | Enum | Purpose |
| -------- | ---- | ------- |
| **Entity** | `EntityCmd` | Manipulate existing entities (velocity, position, signals, shaders, tweens, hierarchy, camera target) |
| **Spawn** | `SpawnCmd` | Create new entities with components |
| **Clone** | `CloneCmd` | Clone an entity registered in WorldSignals and apply builder overrides |
| **Signal** | `SignalCmd` | Modify global WorldSignals |
| **Audio** | `AudioLuaCmd` | Play/stop music and sounds (with optional pitch) |
| **Phase** | `PhaseCmd` | Trigger state machine transitions |
| **Camera** | `CameraCmd` | Set 2D camera target/offset/rotation/zoom directly |
| **CameraFollow** | `CameraFollowCmd` | Configure the camera follow system (mode, speed, zoom_lerp_speed, bounds, deadzone) |
| **Asset** | `AssetCmd` | Load textures, fonts, music, sounds, tilemaps, shaders (setup only) |
| **Group** | `GroupCmd` | Manage tracked entity groups |
| **Tilemap** | `TilemapCmd` | Spawn tiles from tilemap data |
| **Animation** | `AnimationCmd` | Register animation definitions |
| **Render** | `RenderCmd` | Configure post-process shaders and uniforms |
| **GameConfig** | `GameConfigCmd` | Runtime game settings (fullscreen, vsync, FPS, render size, background color) |
| **Input** | `InputCmd` | Runtime input rebinding (rebind action, add binding) |

In addition to the regular queues, most write APIs have a collision-scoped variant (prefixed with `collision_` or `collision_entity_`) that queues into collision-specific buffers.

---

## Current Lua API Index

This section is meant to stay in sync with the actual implementation.

- Source of truth for `engine.*`: `src/resources/lua_runtime/engine_api.rs` (`register_*_api()` methods)
- Source of truth for `engine.spawn()/engine.clone()` builder methods: `src/resources/lua_runtime/entity_builder.rs`

### `engine` Table Functions

#### Logging

- `log`, `log_info`, `log_warn`, `log_error`

#### Assets

- `load_texture`, `load_font`, `load_music`, `load_sound`, `load_tilemap`, `load_shader`

#### Spawning / Cloning

- `spawn`, `clone`

#### Audio

- `play_music`, `play_sound`, `play_sound_pitched`, `stop_all_music`, `stop_all_sounds`

#### Navigation

- `change_scene`, `quit`

#### Global Signals (read)

- `get_scalar`, `get_integer`, `get_string`, `has_flag`, `get_group_count`, `get_entity`, `has_tracked_group`

#### Global Signals (write)

- `set_scalar`, `set_integer`, `set_string`, `set_flag`
- `clear_scalar`, `clear_integer`, `clear_string`, `clear_flag`
- `set_entity`, `remove_entity`

#### Groups

- `track_group`, `untrack_group`, `clear_tracked_groups`

#### Phase / Tilemap / Animation

- `phase_transition`
- `spawn_tiles`
- `register_animation`

#### Camera

- `set_camera`

#### Camera Follow

- `camera_follow_enable`, `camera_follow_set_mode`, `camera_follow_set_deadzone`
- `camera_follow_set_easing`, `camera_follow_set_speed`, `camera_follow_set_spring`
- `camera_follow_set_offset`, `camera_follow_set_bounds`, `camera_follow_clear_bounds`
- `camera_follow_reset_velocity`, `camera_follow_set_zoom_speed`

#### Game Config

- `set_fullscreen`, `get_fullscreen`
- `set_vsync`, `get_vsync`
- `set_target_fps`, `get_target_fps`
- `set_render_size`, `get_render_size`
- `set_background_color`, `get_background_color`

#### Input Rebinding

- `rebind_action`, `add_binding`, `get_binding`

#### Post-Process Shaders

- `post_process_shader`
- `post_process_set_float`, `post_process_set_int`, `post_process_set_vec2`, `post_process_set_vec4`
- `post_process_clear_uniform`, `post_process_clear_uniforms`

#### Entity Commands

- `entity_set_position`, `entity_set_screen_position`, `entity_set_velocity`, `entity_set_speed`, `entity_set_rotation`, `entity_set_scale`
- `entity_add_force`, `entity_remove_force`, `entity_set_force_enabled`, `entity_set_force_value`
- `entity_set_friction`, `entity_set_max_speed`
- `entity_freeze`, `entity_unfreeze`
- `entity_set_animation`, `entity_restart_animation`, `entity_set_sprite_flip`
- `entity_insert_lua_timer`, `entity_remove_lua_timer`
- `entity_insert_ttl`
- `entity_insert_tween_position`, `entity_remove_tween_position`
- `entity_insert_tween_rotation`, `entity_remove_tween_rotation`
- `entity_insert_tween_scale`, `entity_remove_tween_scale`
- `entity_insert_stuckto`, `release_stuckto`
- `entity_signal_set_scalar`, `entity_signal_set_integer`, `entity_signal_set_string`, `entity_signal_set_flag`
- `entity_signal_clear_flag`
- `entity_despawn`, `entity_menu_despawn`
- `entity_set_shader`, `entity_remove_shader`
- `entity_shader_set_float`, `entity_shader_set_int`, `entity_shader_set_vec2`, `entity_shader_set_vec4`
- `entity_shader_clear_uniform`, `entity_shader_clear_uniforms`
- `entity_set_tint`, `entity_remove_tint`
- `entity_set_parent`, `entity_remove_parent`
- `entity_set_camera_target`, `entity_set_camera_target_zoom`, `entity_remove_camera_target`

#### Collision Context Functions

- `collision_spawn`, `collision_clone`
- `collision_play_sound`, `collision_play_sound_pitched`
- `collision_phase_transition`
- `collision_set_camera`
- `collision_set_scalar`, `collision_set_integer`, `collision_set_string`, `collision_set_flag`
- `collision_clear_scalar`, `collision_clear_integer`, `collision_clear_string`, `collision_clear_flag`

#### Collision Entity Commands

All `entity_*` commands have a `collision_entity_*` counterpart (auto-generated via `define_entity_cmds!`). These queue into collision-scoped buffers for immediate processing.

### `LuaEntityBuilder` Methods

The builder returned by `engine.spawn()`, `engine.clone(source_key)`, `engine.collision_spawn()`, and `engine.collision_clone(source_key)` supports these methods:

```text
build
register_as
with_accel
with_animation
with_animation_controller
with_animation_rule
with_camera_target
with_collider
with_collider_offset
with_friction
with_frozen
with_grid_layout
with_group
with_lua_collision_rule
with_lua_timer
with_max_speed
with_menu
with_menu_action_quit
with_menu_action_set_scene
with_menu_action_show_submenu
with_menu_callback
with_menu_colors
with_menu_cursor
with_menu_dynamic_text
with_menu_selection_sound
with_menu_visible_count
with_mouse_controlled
with_parent
with_particle_emitter
with_persistent
with_phase
with_position
with_rotation
with_scale
with_screen_position
with_shader
with_signal_binding
with_signal_binding_format
with_signal_flag
with_signal_integer
with_signal_scalar
with_signal_string
with_signals
with_sprite
with_sprite_flip
with_sprite_offset
with_stuckto
with_stuckto_offset
with_stuckto_stored_velocity
with_text
with_tint
with_ttl
with_tween_position
with_tween_position_backwards
with_tween_position_easing
with_tween_position_loop
with_tween_rotation
with_tween_rotation_backwards
with_tween_rotation_easing
with_tween_rotation_loop
with_tween_scale
with_tween_scale_backwards
with_tween_scale_easing
with_tween_scale_loop
with_velocity
with_zindex
```

---

## Entity Builder Pattern

The engine uses a fluent builder pattern for spawning entities from Lua:

```lua
engine.spawn()
    :with_group("player")
    :with_position(400, 700)
    :with_sprite("vaus", 48, 12, 24, 6)
    :with_velocity(0, 0)
    :with_collider(48, 12, 24, 6)
    :with_phase({
        initial = "idle",
        phases = {
            idle = {
                on_enter = "player_idle_on_enter",
                on_update = "player_idle_on_update"
                -- on_exit = "player_idle_on_exit"
            },
            running = {
                on_enter = "player_running_on_enter",
                on_update = "player_running_on_update",
                on_exit = "player_running_on_exit"
            }            
        }
    })
    :register_as("player")
    :build()
```

Cloning is supported via the same builder pattern:

```lua
engine.clone("some_template_key")
    :with_position(100, 200)
    :with_velocity(0, -120)
    :build()
```

### Menu Selection Callback

Menus can optionally invoke a Lua callback when an item is selected.

- Set the callback via `:with_menu_callback("callback_name")`.
- When a callback is set, `MenuActions` are ignored (the callback takes full control).

The callback receives three arguments:

```lua
-- entity_id = menu entity, item_id = string ID, item_index = 1-based index
function on_menu_select(entity_id, item_id, item_index)
    -- Use engine.* to queue commands
end
```

### How it Works

1. `engine.spawn()` / `engine.clone(source_key)` returns a `LuaEntityBuilder` UserData object
2. Each `:with_*()` method modifies the internal `SpawnCmd` and returns `self`
3. `:build()` pushes a `SpawnCmd` (spawn mode) or a `CloneCmd` (clone mode) to the correct queue based on context (regular vs collision)
4. `:register_as(key)` stores the entity ID in WorldSignals after spawning

### Builder Metadata

Builder methods are documented in `engine.__meta.classes` via `register_builder_meta()` in `stub_meta.rs`. All `with_*` methods, `register_as`, and `build` are listed with descriptions and parameter types for both `EntityBuilder` and `CollisionEntityBuilder` classes.

For builder methods that accept complex table arguments, a `schema` field on the param points to a type name in `engine.__meta.types`:

```lua
-- Example: with_phase's "table" param has schema = "PhaseDefinition"
local p = engine.__meta.classes.EntityBuilder.methods.with_phase.params[1]
assert(p.schema == "PhaseDefinition")
assert(engine.__meta.types.PhaseDefinition)  -- full type definition
```

Current schema mappings:
- `with_phase` → `"PhaseDefinition"`
- `with_particle_emitter` → `"ParticleEmitterConfig"`
- `with_animation_rule` → `"AnimationRuleCondition"`
- `with_menu` → `"MenuItem[]"`

When adding a new builder method that accepts a table, add a `schema_refs` entry in `register_builder_meta()`.

### Spawn Processing

Spawn and clone commands are processed via `process_spawn_command()` and `process_clone_command()` in `lua_commands/spawn_cmd.rs`. Both delegate to the shared `apply_components()` helper, which applies all component data from `SpawnCmd` to the entity. This ensures spawn and clone have identical component support.

`apply_components()` is split into focused sub-functions:
- `apply_transform_components()` — position, screen position, rotation, scale, parent, stuckto, camera target
- `apply_physics_components()` — rigidbody, collider
- `apply_render_components()` — sprite, zindex, shader, tint
- `apply_animation_components()` — animation, animation controller, tweens
- `apply_signal_components()` — signals, signal bindings
- `apply_behavior_components()` — phase, lua timer, lua collision rule
- `apply_ui_components()` — text, menu, grid layout, mouse controlled
- `apply_particle_emitter()` — particle emitter setup with template resolution

---

## Camera Follow System

The camera follow system allows the camera to automatically track an entity marked with `CameraTarget`.

### Lua API

**Configuration** (called from `on_enter_play` or `on_switch_scene`):

```lua
engine.camera_follow_enable(true)
engine.camera_follow_set_mode("lerp")       -- "instant", "lerp", "smooth_damp"
engine.camera_follow_set_speed(5.0)
engine.camera_follow_set_easing("ease_out") -- "linear", "ease_out", "ease_in", "ease_in_out"
engine.camera_follow_set_offset(0, -20)
engine.camera_follow_set_bounds(0, 0, 2000, 1000) -- world-space bounds
```

**Deadzone mode:**

```lua
engine.camera_follow_set_deadzone(32, 24) -- sets mode to deadzone with half-dimensions
```

**Spring mode:**

```lua
engine.camera_follow_set_mode("smooth_damp")
engine.camera_follow_set_spring(80.0, 8.0) -- stiffness, damping
```

**Marking an entity as the camera target (with optional zoom):**

```lua
-- Via builder: priority=10, zoom in to 2x when this target wins
engine.spawn()
    :with_camera_target(10, 2.0)
    :build()

-- At runtime
engine.entity_set_camera_target(entity_id, 10)
engine.entity_set_camera_target_zoom(entity_id, 2.0)  -- update zoom independently
engine.entity_remove_camera_target(entity_id)
```

**Zoom interpolation speed:**

```lua
engine.camera_follow_set_zoom_speed(5.0)  -- default; higher = faster zoom transition
```

The camera lerps `Camera2D.zoom` toward the winning target's `CameraTarget.zoom` every frame using `EaseOut`, at the rate set by `zoom_lerp_speed`. This is independent of the position follow mode.

### Command Enum

```rust
pub enum CameraFollowCmd {
    Enable { enabled: bool },
    SetMode { mode: String },
    SetDeadzone { half_w: f32, half_h: f32 },
    SetEasing { easing: String },
    SetSpeed { speed: f32 },
    SetSpring { stiffness: f32, damping: f32 },
    SetOffset { x: f32, y: f32 },
    SetBounds { x: f32, y: f32, w: f32, h: f32 },
    ClearBounds,
    ResetVelocity,
    SetZoomSpeed { speed: f32 },
}
```

---

## Input Rebinding

Lua can rebind input actions at runtime via the input API.

### Lua API

```lua
-- Replace all bindings for an action
engine.rebind_action("action_1", "z")

-- Add an extra binding (multi-bind)
engine.add_binding("action_1", "space")

-- Read current first binding (snapshot, visible next frame)
local key = engine.get_binding("action_1") -- "z" or nil
```

### Valid Action Names

`main_up`, `main_down`, `main_left`, `main_right`, `secondary_up`, `secondary_down`, `secondary_left`, `secondary_right`, `back`, `action_1`, `action_2`, `action_3`, `special`, `toggle_debug`, `toggle_fullscreen`

### Valid Key Strings

Single lowercase letters `a`-`z`, digits `0`-`9`, `space`, `enter`/`return`, `escape`/`esc`, arrow keys (`up`, `down`, `left`, `right`), modifiers (`lshift`/`rshift`/`lctrl`/`rctrl`/`lalt`/`ralt`), `f1`-`f12`, `mouse_left`, `mouse_right`, `mouse_middle`.

### Command Enum

```rust
pub enum InputCmd {
    Rebind { action: String, key: String },
    AddBinding { action: String, key: String },
}
```

---

## Parent-Child Hierarchy

Entities can be organized into parent-child hierarchies for transform propagation.

### Lua API

**At spawn time:**

```lua
engine.spawn()
    :with_parent(parent_id)
    :with_position(10, 0) -- local offset from parent
    :build()
```

**At runtime:**

```lua
engine.entity_set_parent(child_id, parent_id)
engine.entity_remove_parent(child_id) -- snaps to current world position
```

### Notes

- `GlobalTransform2D` is computed automatically by `propagate_transforms` system.
- Use `ComputeInitialGlobalTransform` EntityCommand after setting `ChildOf` on a newly spawned entity to avoid a one-frame world-origin flash.
- `ChildOf` entities skip the `StuckTo` system (hierarchy takes precedence).
- Entity context exposes `ctx.world_pos`, `ctx.world_rotation`, `ctx.world_scale`, and `ctx.parent_id` in phase/timer callbacks.

---

## Signal Snapshot System

Lua reads world state through a **cached snapshot**, not directly from ECS resources:

```rust
// Before calling Lua callbacks
lua_runtime.update_signal_cache(world_signals.snapshot());
lua_runtime.update_tracked_groups_cache(&tracked_groups);
```

```lua
-- In Lua
local score = engine.get_integer("score")  -- Reads from cache
```

### Why Snapshots?

1. **Immutable reads**: Lua can't accidentally corrupt game state
2. **Consistency**: All reads within a callback see the same state
3. **Performance**: `Arc<SignalSnapshot>` is cheap to clone

### Additional Snapshots

Beyond signal snapshots, the engine also caches:
- **GameConfig snapshot** — fullscreen, vsync, fps, render size, background color (read via `get_fullscreen()`, `get_render_size()`, etc.)
- **Bindings snapshot** — current input bindings (read via `get_binding()`)

These are updated before Lua callbacks run and ensure consistent reads.

### Input Data

Input is not read from the signal snapshot; it is passed to callbacks via a dedicated input table built from an `InputSnapshot` using pooled tables.

---

## Context Table Pooling

To minimize Lua table allocations in hot paths, the engine uses **table pooling** for callback context tables. Instead of creating new tables for each collision or entity callback, pre-allocated tables are stored in the Lua registry and reused.

### Why Pooling?

Without pooling, each callback would allocate many Lua tables:

- **Collision callbacks**: ~15-17 tables per collision (ctx, ctx.a, ctx.b, pos tables, vel tables, rect tables, signals, sides, etc.)
- **Entity callbacks** (phase/timer): ~10-14 tables per callback (ctx, pos, screen_pos, vel, scale, rect, sprite, animation, timer, signals)
- **Input tables**: digital/analog subtables reused across all callbacks each frame

In a game with frequent collisions or many entities with phase/timer components, this creates significant GC pressure.

### Pool Architecture

Three pool types are maintained:

- **CollisionCtxPool** — for collision callbacks (ctx.a, ctx.b, sides, subtables)
- **EntityCtxPool** — for phase/timer callbacks (ctx, pos, vel, scale, rect, etc.)
- **InputCtxPool** — for the input table passed to all callbacks (digital, analog subtables)

### How It Works

1. **Initialization**: Pools are created once in `LuaRuntime::new()` via `create_*_pool()` functions
2. **Retrieval**: Before each callback, `get_*_pool()` fetches tables from the registry
3. **Population**: Context builder functions populate the pooled tables with current entity data — optional fields are explicitly set to `nil` when absent (prevents stale data)
4. **Reuse**: The same tables are reused for the next callback

### What Gets Pooled vs Created Fresh

| Data Type | Pooled? | Reason |
| --------- | ------- | ------ |
| Fixed-structure tables (ctx, pos, vel, rect, etc.) | Yes | Same structure every time |
| Scalar/numeric values | N/A | Set directly on pooled tables |
| Signal inner maps (flags, integers, scalars, strings) | No | Variable keys per entity |
| Collision side arrays | Cleared & repopulated | Variable length |

### Important: No Persistent References

**Lua scripts must NOT store references to context tables or their subtables for later use.** The tables are reused and values will be overwritten on the next callback.

```lua
-- BAD: Don't do this!
local saved_pos = ctx.pos  -- This reference will have wrong values later

-- GOOD: Copy the values you need
local saved_x = ctx.pos.x
local saved_y = ctx.pos.y
```

### Implementation Files

- `runtime.rs`: Pool structs (`CollisionCtxPool`, `EntityCtxPool`, `InputCtxPool`), `create_*_pool()`, `get_*_pool()` methods
- `lua_runtime/context.rs`: `build_entity_context_pooled()` — low-level Lua table writer
- `lua_commands/context.rs`: `build_entity_context()` — ECS-facing adapter that gathers component data and calls `build_entity_context_pooled()`

---

## Meta Schema (`engine.__meta`)

The `engine.__meta` table provides a complete, introspectable API contract for the Lua interface. It is populated during `LuaRuntime::new()` and can be used for automated stub generation, documentation, and drift protection tests.

### Structure

```lua
engine.__meta = {
    functions  = { ... },  -- All engine.* function signatures
    classes    = { ... },  -- EntityBuilder / CollisionEntityBuilder method signatures
    types      = { ... },  -- Type shape definitions (table schemas)
    enums      = { ... },  -- Valid string literal value sets
    callbacks  = { ... },  -- Well-known callback signatures the engine invokes
}
```

### `__meta.types` — Type Shape Definitions

Each entry describes a Lua table shape with typed fields. Registered by `register_types_meta()` in `stub_meta.rs`.

```lua
engine.__meta.types["EntityContext"] = {
    description = "Entity state passed to phase/timer callbacks",
    fields = {
        { name = "id",    type = "integer",  optional = false, description = "Entity ID" },
        { name = "pos",   type = "Vector2",  optional = true },
        { name = "phase", type = "string",   optional = true },
        -- ...
    }
}
```

Current types: `Vector2`, `Rect`, `SpriteInfo`, `AnimationInfo`, `TimerInfo`, `SignalSet`, `EntityContext`, `CollisionEntity`, `CollisionSides`, `CollisionContext`, `DigitalButtonState`, `DigitalInputs`, `InputSnapshot`, `PhaseCallbacks`, `PhaseDefinition`, `ParticleEmitterConfig`, `MenuItem`, `AnimationRuleCondition`.

### `__meta.enums` — String Literal Value Sets

Each entry lists the valid string values for a domain concept. Registered by `register_enums_meta()` in `stub_meta.rs`.

```lua
engine.__meta.enums["Easing"] = {
    description = "Tween easing function",
    values = { "linear", "quad_in", "quad_out", "quad_in_out",
               "cubic_in", "cubic_out", "cubic_in_out" }
}
```

Current enums: `Easing`, `LoopMode`, `BoxSide`, `ComparisonOp`, `ConditionType`, `EmitterShape`, `TtlSpec`, `Category`.

### `__meta.callbacks` — Engine-Invoked Callback Signatures

Each entry documents a global Lua function the engine calls, including parameter types, return types, and context. Registered by `register_callbacks_meta()` in `stub_meta.rs`.

```lua
engine.__meta.callbacks["phase_on_enter"] = {
    description = "Called when entering a phase",
    params  = { { name = "ctx", type = "EntityContext" },
                { name = "input", type = "InputSnapshot" } },
    returns = { type = "string?" },
    context = "play",
    note    = "Return phase name to trigger transition"
}
```

Current callbacks: `on_setup`, `on_enter_play`, `on_switch_scene`, `on_update_<scene>`, `phase_on_enter`, `phase_on_update`, `phase_on_exit`, `timer_callback`, `collision_callback`, `menu_callback`.

### Drift Protection

Tests in `tests/engine_tick_integration.rs` verify the meta schema stays in sync with the implementation:

- `meta_types_table_is_populated` — all type entries have `description` + `fields` with `name`/`type`/`optional`
- `meta_enums_table_is_populated` — hard-coded expected values for `Easing`, `LoopMode`, `BoxSide`, `Category`
- `meta_callbacks_table_is_populated` — all callback entries have `params` with correct shapes
- `meta_functions_complete` — comprehensive function list + collision/entity command parity check
- `meta_builder_methods_have_schema_refs` — schema references point to existing types

When adding new Rust types, easing functions, callback conventions, or API functions, update the corresponding `register_*_meta()` method in `stub_meta.rs`. If you don't, these tests will fail.

---

## How to Add New Lua Commands

This section provides step-by-step instructions for adding new Lua commands.

### Example: Adding `entity_set_health`

Let's add a command that sets a "health" scalar on an entity's Signals component.

#### Step 1: Add Command Variant

In `src/resources/lua_runtime/commands.rs`:

```rust
pub enum EntityCmd {
    // ... existing variants ...
    SetHealth { entity_id: u64, health: f32 },
}
```

#### Step 2: Register Lua Function

In `src/resources/lua_runtime/engine_api.rs`, add the entry to the `define_entity_cmds!` macro body. This single entry auto-registers both the regular (`entity_set_health`) and collision (`collision_entity_set_health`) variants, along with metadata:

```rust
// Inside define_entity_cmds! macro body in engine_api.rs
("entity_set_health",
    |(entity_id, health)| (u64, f32),
    EntityCmd::SetHealth { entity_id, health },
    desc = "Set entity health signal",
    params = [("entity_id", "integer"), ("health", "number")]),
```

For non-entity commands (signals, audio, etc.), use `register_cmd!` directly in the appropriate `register_*_api()` method:

```rust
// In the relevant register_*_api() method in engine_api.rs
register_cmd!(engine, self.lua, meta_fns, "set_health", health_commands,
    |(entity_id, health)| (u64, f32), HealthCmd::SetHealth { entity_id, health },
    desc = "Set entity health", cat = "entity",
    params = [("entity_id", "integer"), ("health", "number")]);
```

#### Step 3: Process the Command

In `src/systems/lua_commands/entity_cmd.rs`, add the match arm to the appropriate sub-function (or create a new one):

```rust
EntityCmd::SetHealth { entity_id, health } => {
    let entity = Entity::from_bits(entity_id);
    if let Ok(mut signals) = cmd_queries.signals.get_mut(entity) {
        signals.set_scalar("health", health);
    }
}
```

#### Step 4: Update Meta Schema (If Applicable)

If the new command introduces new string literal values (e.g. a new easing mode or condition type), update `register_enums_meta()` in `stub_meta.rs`. If it introduces a new callback convention, update `register_callbacks_meta()`. If it accepts a complex table argument, add a type definition in `register_types_meta()`.

#### Step 5: Update LSP Stubs (Optional but Recommended)

In `assets/scripts/engine.lua`:

```lua
--- Set the health value on an entity's Signals component.
--- @param entity_id integer The entity ID
--- @param health number The health value
function engine.entity_set_health(entity_id, health) end
```

> **Note**: Because entity commands are registered via `define_entity_cmds!`, the collision-prefixed variant (`collision_entity_set_health`) is automatically available — no extra registration step is needed. Metadata for `engine.__meta` is also generated automatically by the macro.

---

### Adding a Completely New Command Type

If you need a new category of commands (e.g., `HealthCmd`):

#### Step 1: Define the Enum

In `commands.rs`:

```rust
#[derive(Debug, Clone)]
pub enum HealthCmd {
    SetEntityHealth { entity_id: u64, health: f32 },
    HealEntity { entity_id: u64, amount: f32 },
}
```

#### Step 2: Add Queue to LuaAppData

In `runtime.rs`, add the field to `LuaAppData`:

```rust
pub(super) struct LuaAppData {
    // ... existing fields ...
    health_commands: RefCell<Vec<HealthCmd>>,
}
```

And initialize it in `LuaRuntime::new()`.

#### Step 3: Add Drain Function

In `command_queues.rs`:

```rust
impl LuaRuntime {
    pub fn drain_health_commands(&self) -> Vec<HealthCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.health_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }
}
```

#### Step 4: Register API Functions

Use `register_cmd!` macro in a new `register_health_api()` method in `engine_api.rs`:

```rust
pub(super) fn register_health_api(&self) -> LuaResult<()> {
    let engine: LuaTable = self.lua.globals().get("engine")?;
    let meta: LuaTable = engine.get("__meta")?;
    let meta_fns: LuaTable = meta.get("functions")?;

    register_cmd!(engine, self.lua, meta_fns, "heal_entity", health_commands,
        |(entity_id, amount)| (u64, f32), HealthCmd::HealEntity { entity_id, amount },
        desc = "Heal an entity by amount", cat = "health",
        params = [("entity_id", "integer"), ("amount", "number")]);

    Ok(())
}
```

Don't forget to call `self.register_health_api()?` in `LuaRuntime::new()`.

#### Step 5: Create Processing Function

In `lua_commands/mod.rs` (or a new sub-file if complex):

```rust
pub fn process_health_commands(
    health_query: &mut Query<&mut Signals>,
    commands: impl IntoIterator<Item = HealthCmd>,
) {
    for cmd in commands {
        match cmd {
            HealthCmd::HealEntity { entity_id, amount } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = health_query.get_mut(entity) {
                    let current = signals.get_scalar("health").unwrap_or(0.0);
                    signals.set_scalar("health", current + amount);
                }
            }
            // ... other variants
        }
    }
}
```

#### Step 6: Call from Game Loop

In `lua_plugin.rs` (or the appropriate system):

```rust
for cmd in lua_runtime.drain_health_commands() {
    process_health_commands(&mut signals_query, std::iter::once(cmd));
}
```

---

### Adding Entity Builder Methods

To add spawning capabilities:

#### Step 1: Add Data Structure (if needed)

In `spawn_data.rs`:

```rust
#[derive(Debug, Clone, Default)]
pub struct HealthData {
    pub initial_health: f32,
    pub max_health: f32,
}
```

#### Step 2: Add to SpawnCmd

In `spawn_data.rs`:

```rust
pub struct SpawnCmd {
    // ... existing fields ...
    pub health: Option<HealthData>,
}
```

#### Step 3: Add Builder Method

In `entity_builder.rs`, inside `impl LuaUserData for LuaEntityBuilder`:

```rust
methods.add_method_mut("with_health", |_, this, (initial, max): (f32, f32)| {
    this.cmd.health = Some(HealthData {
        initial_health: initial,
        max_health: max,
    });
    Ok(this.clone())  // Return self for chaining
});
```

#### Step 4: Register Builder Metadata

In `register_builder_meta()` in `stub_meta.rs`, add the method to the `builder_methods` array:

```rust
let builder_methods: &[(&str, &str, &[(&str, &str)])] = &[
    // ... existing entries ...
    ("with_health", "Set initial and max health", &[("initial", "number"), ("max", "number")]),
];
```

This populates `engine.__meta.classes.EntityBuilder.methods` and `engine.__meta.classes.CollisionEntityBuilder.methods`.

#### Step 5: Process During Spawn

In `lua_commands/spawn_cmd.rs`, inside `apply_components()` (or the appropriate sub-function):

```rust
if let Some(health_data) = cmd.health {
    // Add Health component or set signals
}
```

---

## Best Practices

### 1. Keep Commands Small and Focused

Each command should do one thing. If you need multiple operations, use multiple commands:

```rust
// Good: Separate concerns
EntityCmd::SetVelocity { entity_id, vx, vy },
EntityCmd::SetPosition { entity_id, x, y },

// Avoid: Combining unrelated operations
EntityCmd::SetPositionAndVelocity { ... }  // Too broad
```

### 2. Use Appropriate Queue

- **Regular queues**: For phase callbacks, timer callbacks, update callbacks
- **Collision queues**: For collision callbacks (immediate processing needed)

### 3. Handle Missing Entities Gracefully

```rust
// Good: Silent failure if entity doesn't exist
if let Ok(mut rb) = cmd_queries.rigid_bodies.get_mut(entity) {
    rb.velocity = Vector2 { x: vx, y: vy };
}
```

### 4. Entity IDs are u64

Bevy's `Entity` type is not directly usable in Lua. Always convert:

```rust
// Rust to Lua: entity.to_bits()
// Lua to Rust: Entity::from_bits(entity_id)
```

### 5. Document Lua API

Always update:

- `assets/scripts/engine.lua` - LSP stubs for autocomplete (regenerate via `cargo run -- --create-lua-stubs`)
- `assets/scripts/README.md` - Human-readable documentation

### 6. Consider Collision Context

For entity commands, the `define_entity_cmds!` macro automatically registers both regular and collision variants from a single definition — no manual duplication needed.

For other command types, provide a separate `collision_*` registration using `register_cmd!` with the collision-scoped queue:

```rust
// Regular context
register_cmd!(engine, self.lua, meta_fns, "play_sound", audio_commands,
    |id| String, AudioLuaCmd::PlaySound { id },
    desc = "Play a sound effect", cat = "audio",
    params = [("id", "string")]);

// Collision context
register_cmd!(engine, self.lua, meta_fns, "collision_play_sound", collision_audio_commands,
    |id| String, AudioLuaCmd::PlaySound { id },
    desc = "Play a sound effect (collision context)", cat = "collision",
    params = [("id", "string")]);
```

### 7. Registration Patterns Summary

| What | Macro/Function | File |
| ---- | -------------- | ---- |
| Entity commands (with auto collision variants) | `define_entity_cmds!` macro | `engine_api.rs` |
| Simple push-to-queue functions | `register_cmd!` macro | `engine_api.rs` |
| Functions with custom logic (reads, validation) | Manual `engine.set()` + `push_fn_meta()` | `engine_api.rs` |
| Builder methods | `methods.add_method_mut()` in `LuaUserData` impl | `entity_builder.rs` |
| Builder metadata | Add to `builder_methods` array in `register_builder_meta()` | `stub_meta.rs` |
| Type/enum/callback metadata | `register_types_meta()` / `register_enums_meta()` / `register_callbacks_meta()` | `stub_meta.rs` |

---

## Summary

The Lua interface follows these principles:

1. **Deferred Execution**: Commands are queued, not executed immediately
2. **Type Safety**: Rust enums ensure valid command structures
3. **Separation of Concerns**: Commands are defined, registered, and processed in different modules
4. **Read-Write Split**: Lua reads from cached snapshots, writes via command queues
5. **Context Awareness**: Collision callbacks have separate queues for immediate processing

To add new commands:

1. Add variant to appropriate command enum in `commands.rs`
2. Register Lua function in `engine_api.rs` (use `register_cmd!` macro for push-to-queue, or add to `define_entity_cmds!` for entity commands)
3. Add drain method in `command_queues.rs` (if new command type)
4. Process command in `lua_commands/` (entity_cmd.rs, spawn_cmd.rs, or mod.rs)
5. Optionally add builder method in `entity_builder.rs` + update `register_builder_meta()` in `stub_meta.rs`
6. Update `register_types_meta()` / `register_enums_meta()` / `register_callbacks_meta()` in `stub_meta.rs` if the command introduces new types, enum values, or callback conventions
7. Update documentation files
