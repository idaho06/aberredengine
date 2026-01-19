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
8. [How to Add New Lua Commands](#how-to-add-new-lua-commands)
9. [Best Practices](#best-practices)

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
├── runtime.rs          # LuaRuntime struct, engine table API registration, context pools
├── commands.rs         # Command enums (EntityCmd, SignalCmd, etc.)
├── context.rs          # Entity context builder for Lua callbacks (pooled)
├── entity_builder.rs   # LuaEntityBuilder fluent API for spawning
├── input_snapshot.rs   # InputSnapshot for Lua callbacks
└── spawn_data.rs       # Data structures for spawn configuration
```

### Key Components

#### `LuaRuntime` (runtime.rs)

The main struct managing the Lua interpreter. It:

- Initializes the Lua state with MLua
- Registers the global `engine` table with all API functions
- Manages `LuaAppData` for command queuing
- Provides `drain_*_commands()` methods for Rust to retrieve queued commands
- Manages **context table pools** for collision and entity callbacks (see [Context Table Pooling](#context-table-pooling))

#### `LuaAppData` (runtime.rs)

Internal shared state accessible from Lua closures:

```rust
pub(super) struct LuaAppData {
    asset_commands: RefCell<Vec<AssetCmd>>,
    pub(super) spawn_commands: RefCell<Vec<SpawnCmd>>,
    audio_commands: RefCell<Vec<AudioLuaCmd>>,
    signal_commands: RefCell<Vec<SignalCmd>>,
    phase_commands: RefCell<Vec<PhaseCmd>>,
    entity_commands: RefCell<Vec<EntityCmd>>,
    group_commands: RefCell<Vec<GroupCmd>>,
    tilemap_commands: RefCell<Vec<TilemapCmd>>,
    camera_commands: RefCell<Vec<CameraCmd>>,
    animation_commands: RefCell<Vec<AnimationCmd>>,

    /// Clone commands for regular context (scene setup, phase callbacks)
    pub(super) clone_commands: RefCell<Vec<CloneCmd>>,

    // Collision-scoped command queues (processed immediately after each collision callback)
    collision_entity_commands: RefCell<Vec<EntityCmd>>,
    collision_signal_commands: RefCell<Vec<SignalCmd>>,
    collision_audio_commands: RefCell<Vec<AudioLuaCmd>>,
    pub(super) collision_spawn_commands: RefCell<Vec<SpawnCmd>>,
    collision_phase_commands: RefCell<Vec<PhaseCmd>>,
    collision_camera_commands: RefCell<Vec<CameraCmd>>,

    /// Clone commands for collision context (processed after collision callbacks)
    pub(super) collision_clone_commands: RefCell<Vec<CloneCmd>>,

    /// Cached world signal snapshot (read-only for Lua)
    signal_snapshot: RefCell<Arc<SignalSnapshot>>,
    /// Cached tracked group names (read-only snapshot for Lua)
    tracked_groups: RefCell<FxHashSet<String>>,
}
```

#### Command Enums (commands.rs)

Each command type is a Rust enum that encapsulates all data needed to perform an operation:

```rust
pub enum EntityCmd {
    SetVelocity { entity_id: u64, vx: f32, vy: f32 },
    Despawn { entity_id: u64 },
    SignalSetFlag { entity_id: u64, flag: String },
    // ... many more variants
}

pub enum SignalCmd {
    SetScalar { key: String, value: f32 },
    SetInteger { key: String, value: i32 },
    SetFlag { key: String },
    ClearFlag { key: String },
    SetString { key: String, value: String },
}
```

---

## Command Flow: Lua to ECS

### Step 1: Lua Calls Engine API

```lua
-- In a Lua script
engine.entity_set_velocity(ball_id, new_vx, new_vy)
engine.set_flag("switch_scene")
```

### Step 2: Command is Queued

The registered Lua function pushes a command to the appropriate queue:

```rust
// In runtime.rs, during register_entity_api()
engine.set(
    "entity_set_velocity",
    self.lua.create_function(|lua, (entity_id, vx, vy): (u64, f32, f32)| {
        lua.app_data_ref::<LuaAppData>()
            .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
            .entity_commands        // <- The queue to push to
            .borrow_mut()
            .push(EntityCmd::SetVelocity { entity_id, vx, vy });
        Ok(())
    })?,
)?;
```

### Step 3: Rust Drains Commands

After the Lua callback returns, Rust calls `drain_*_commands()`:

```rust
// In game.rs update()
for cmd in lua_runtime.drain_entity_commands() {
    // Process each command
}
```

### Step 4: Commands are Processed

The `process_*` functions in `lua_commands.rs` apply changes to the ECS:

```rust
// In lua_commands.rs
pub fn process_entity_commands(
    commands: &mut Commands,
    entity_commands: impl IntoIterator<Item = EntityCmd>,
    // ... query parameters
) {
    for cmd in entity_commands {
        match cmd {
            EntityCmd::SetVelocity { entity_id, vx, vy } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut rb) = rigid_bodies_query.get_mut(entity) {
                    rb.velocity = Vector2 { x: vx, y: vy };
                }
            }
            // ... handle other variants
        }
    }
}
```

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
| **Entity** | `EntityCmd` | Manipulate existing entities (velocity, position, signals, components) |
| **Spawn** | `SpawnCmd` | Create new entities with components |
| **Clone** | `CloneCmd` | Clone an entity registered in WorldSignals and apply builder overrides |
| **Signal** | `SignalCmd` | Modify global WorldSignals |
| **Audio** | `AudioLuaCmd` | Play/stop music and sounds |
| **Phase** | `PhaseCmd` | Trigger state machine transitions |
| **Camera** | `CameraCmd` | Configure the 2D camera |
| **Asset** | `AssetCmd` | Load textures, fonts, music (setup only) |
| **Group** | `GroupCmd` | Manage tracked entity groups |
| **Tilemap** | `TilemapCmd` | Spawn tiles from tilemap data |
| **Animation** | `AnimationCmd` | Register animation definitions |

In addition to the regular queues, most write APIs have a collision-scoped variant (prefixed with `collision_` or `collision_entity_`) that queues into collision-specific buffers.

---

## Current Lua API Index

This section is meant to stay in sync with the actual implementation.

- Source of truth for `engine.*`: `src/resources/lua_runtime/runtime.rs` (`engine.set(...)` registrations)
- Source of truth for `engine.spawn()/engine.clone()` builder methods: `src/resources/lua_runtime/entity_builder.rs`

### `engine` Table Functions

#### Logging

- `log`, `log_info`, `log_warn`, `log_error`

#### Assets

- `load_texture`, `load_font`, `load_music`, `load_sound`, `load_tilemap`

#### Spawning / Cloning

- `spawn`, `clone`

#### Audio

- `play_music`, `play_sound`, `stop_all_music`, `stop_all_sounds`

#### Global Signals (read)

- `get_scalar`, `get_integer`, `get_string`, `has_flag`, `get_group_count`, `get_entity`, `has_tracked_group`

#### Global Signals (write)

- `set_scalar`, `set_integer`, `set_string`, `set_flag`
- `clear_scalar`, `clear_integer`, `clear_string`, `clear_flag`
- `set_entity`, `remove_entity`

#### Groups

- `track_group`, `untrack_group`, `clear_tracked_groups`

#### Phase / Camera / Tilemap / Animation

- `phase_transition`
- `set_camera`
- `spawn_tiles`
- `register_animation`

#### Entity Commands

- `entity_set_position`, `entity_set_velocity`, `entity_set_speed`, `entity_set_rotation`, `entity_set_scale`
- `entity_add_force`, `entity_remove_force`, `entity_set_force_enabled`, `entity_set_force_value`
- `entity_set_friction`, `entity_set_max_speed`
- `entity_freeze`, `entity_unfreeze`
- `entity_set_animation`, `entity_restart_animation`
- `entity_insert_lua_timer`, `entity_remove_lua_timer`
- `entity_insert_ttl`
- `entity_insert_tween_position`, `entity_remove_tween_position`
- `entity_insert_tween_rotation`, `entity_remove_tween_rotation`
- `entity_insert_tween_scale`, `entity_remove_tween_scale`
- `entity_insert_stuckto`, `release_stuckto`
- `entity_signal_set_scalar`, `entity_signal_set_integer`, `entity_signal_set_string`, `entity_signal_set_flag`
- `entity_signal_clear_flag`
- `entity_despawn`, `entity_menu_despawn`

#### Collision Context Functions

- `collision_spawn`, `collision_clone`
- `collision_play_sound`
- `collision_phase_transition`
- `collision_set_camera`
- `collision_set_scalar`, `collision_set_integer`, `collision_set_string`, `collision_set_flag`
- `collision_clear_scalar`, `collision_clear_integer`, `collision_clear_string`, `collision_clear_flag`

#### Collision Entity Commands

- `collision_entity_set_position`, `collision_entity_set_velocity`, `collision_entity_set_speed`, `collision_entity_set_rotation`, `collision_entity_set_scale`
- `collision_entity_add_force`, `collision_entity_remove_force`, `collision_entity_set_force_enabled`, `collision_entity_set_force_value`
- `collision_entity_set_friction`, `collision_entity_set_max_speed`
- `collision_entity_freeze`, `collision_entity_unfreeze`
- `collision_entity_set_animation`, `collision_entity_restart_animation`
- `collision_entity_insert_lua_timer`, `collision_entity_remove_lua_timer`
- `collision_entity_insert_ttl`
- `collision_entity_insert_tween_position`, `collision_entity_remove_tween_position`
- `collision_entity_insert_tween_rotation`, `collision_entity_remove_tween_rotation`
- `collision_entity_insert_tween_scale`, `collision_entity_remove_tween_scale`
- `collision_entity_insert_stuckto`, `collision_release_stuckto`
- `collision_entity_signal_set_scalar`, `collision_entity_signal_set_integer`, `collision_entity_signal_set_string`, `collision_entity_signal_set_flag`
- `collision_entity_signal_clear_flag`
- `collision_entity_despawn`

### `LuaEntityBuilder` Methods

The builder returned by `engine.spawn()`, `engine.clone(source_key)`, `engine.collision_spawn()`, and `engine.collision_clone(source_key)` supports these methods:

```text
build
register_as
with_accel
with_animation
with_animation_controller
with_animation_rule
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
with_menu_colors
with_menu_cursor
with_menu_dynamic_text
with_menu_selection_sound
with_mouse_controlled
with_particle_emitter
with_persistent
with_phase
with_position
with_rotation
with_scale
with_screen_position
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
        initial = "playing",
        callbacks = {
            playing = "on_player_playing",
            hit = "on_player_hit",
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

### How it Works

1. `engine.spawn()` / `engine.clone(source_key)` returns a `LuaEntityBuilder` UserData object
2. Each `:with_*()` method modifies the internal `SpawnCmd` and returns `self`
3. `:build()` pushes a `SpawnCmd` (spawn mode) or a `CloneCmd` (clone mode) to the correct queue based on context (regular vs collision)
4. `:register_as(key)` stores the entity ID in WorldSignals after spawning

### Builder Implementation

```rust
// In entity_builder.rs
impl LuaUserData for LuaEntityBuilder {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("with_velocity", |_, this, (vx, vy): (f32, f32)| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.velocity_x = vx;
                rb.velocity_y = vy;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    velocity_x: vx,
                    velocity_y: vy,
                    ..RigidBodyData::default()
                });
            }
            Ok(this.clone())  // Return self for chaining
        });

        // build() - queue spawn or clone, regular or collision context
        methods.add_method("build", |lua, this, ()| {
            let app_data = lua
                .app_data_ref::<LuaAppData>()
                .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;

            match (this.mode, this.context) {
                (BuilderMode::Spawn, BuilderContext::Regular) => {
                    app_data.spawn_commands.borrow_mut().push(this.cmd.clone());
                }
                (BuilderMode::Spawn, BuilderContext::Collision) => {
                    app_data
                        .collision_spawn_commands
                        .borrow_mut()
                        .push(this.cmd.clone());
                }
                (BuilderMode::Clone, BuilderContext::Regular) => {
                    app_data.clone_commands.borrow_mut().push(CloneCmd {
                        source_key: this.source_key.clone().unwrap_or_default(),
                        overrides: this.cmd.clone(),
                    });
                }
                (BuilderMode::Clone, BuilderContext::Collision) => {
                    app_data.collision_clone_commands.borrow_mut().push(CloneCmd {
                        source_key: this.source_key.clone().unwrap_or_default(),
                        overrides: this.cmd.clone(),
                    });
                }
            }

            Ok(())
        });
    }
}
```

---

## Signal Snapshot System

Lua reads world state through a **cached snapshot**, not directly from ECS resources:

```rust
// Before calling Lua callbacks
lua_runtime.update_signal_cache(world_signals.snapshot());
lua_runtime.update_tracked_groups_cache(&tracked_groups);

// In Lua
local score = engine.get_integer("score")  -- Reads from cache
```

### Why Snapshots?

1. **Immutable reads**: Lua can't accidentally corrupt game state
2. **Consistency**: All reads within a callback see the same state
3. **Performance**: `Arc<SignalSnapshot>` is cheap to clone

### Updating the Cache

```rust
// In game.rs or system that calls Lua
lua_runtime.update_signal_cache(world_signals.snapshot());
lua_runtime.update_tracked_groups_cache(&tracked_groups);
```

### Input Data

Input is not read from the signal snapshot; it is passed to callbacks via a dedicated input table built from an `InputSnapshot`.

---

## Context Table Pooling

To minimize Lua table allocations in hot paths, the engine uses **table pooling** for callback context tables. Instead of creating new tables for each collision or entity callback, pre-allocated tables are stored in the Lua registry and reused.

### Why Pooling?

Without pooling, each callback would allocate many Lua tables:

- **Collision callbacks**: ~15-17 tables per collision (ctx, ctx.a, ctx.b, pos tables, vel tables, rect tables, signals, sides, etc.)
- **Entity callbacks** (phase/timer): ~10-14 tables per callback (ctx, pos, screen_pos, vel, scale, rect, sprite, animation, timer, signals)

In a game with frequent collisions or many entities with phase/timer components, this creates significant GC pressure.

### Pool Architecture

#### CollisionCtxPool (for collision callbacks)

```rust
struct CollisionCtxPool {
    ctx: LuaRegistryKey,
    entity_a: LuaRegistryKey,   // ctx.a
    entity_b: LuaRegistryKey,   // ctx.b

    pos_a: LuaRegistryKey,      // ctx.a.pos
    vel_a: LuaRegistryKey,      // ctx.a.vel
    rect_a: LuaRegistryKey,     // ctx.a.rect
    signals_a: LuaRegistryKey,  // ctx.a.signals

    pos_b: LuaRegistryKey,      // ctx.b.pos
    vel_b: LuaRegistryKey,      // ctx.b.vel
    rect_b: LuaRegistryKey,     // ctx.b.rect
    signals_b: LuaRegistryKey,  // ctx.b.signals

    sides_a: LuaRegistryKey,    // ctx.sides.a
    sides_b: LuaRegistryKey,    // ctx.sides.b
}
```

#### EntityCtxPool (for phase/timer callbacks)

```rust
struct EntityCtxPool {
    ctx: LuaRegistryKey,        // Root context table
    pos: LuaRegistryKey,        // ctx.pos
    screen_pos: LuaRegistryKey, // ctx.screen_pos
    vel: LuaRegistryKey,        // ctx.vel
    scale: LuaRegistryKey,      // ctx.scale
    rect: LuaRegistryKey,       // ctx.rect
    sprite: LuaRegistryKey,     // ctx.sprite
    animation: LuaRegistryKey,  // ctx.animation
    timer: LuaRegistryKey,      // ctx.timer
    signals: LuaRegistryKey,    // ctx.signals
}
```

### How It Works

1. **Initialization**: Pools are created once in `LuaRuntime::new()` via `create_collision_ctx_pool()` and `create_entity_ctx_pool()`

2. **Retrieval**: Before each callback, `get_collision_ctx_pool()` or `get_entity_ctx_pool()` fetches tables from the registry

3. **Population**: The context builder functions populate the pooled tables with current entity data:
   - Scalar values (id, speed_sq, rotation, etc.) are set directly
   - Optional fields are explicitly set to `nil` when absent (prevents stale data)
   - Variable-length data (signal maps) is still created fresh each time

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

- `runtime.rs`: Pool structs, `create_*_pool()`, `get_*_pool()` methods
- `context.rs`: `build_entity_context_pooled()` function
- `collision.rs`: Collision callback context population using pool

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
    
    /// Set the health value on an entity's Signals component
    SetHealth { entity_id: u64, health: f32 },
}
```

#### Step 2: Register Lua Function

In `src/resources/lua_runtime/runtime.rs`, add to `register_entity_api()`:

```rust
fn register_entity_api(&self) -> LuaResult<()> {
    let engine: LuaTable = self.lua.globals().get("engine")?;

    // ... existing functions ...

    // engine.entity_set_health(entity_id, health)
    engine.set(
        "entity_set_health",
        self.lua.create_function(|lua, (entity_id, health): (u64, f32)| {
            lua.app_data_ref::<LuaAppData>()
                .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                .entity_commands  // Use collision_entity_commands if for collision callbacks
                .borrow_mut()
                .push(EntityCmd::SetHealth { entity_id, health });
            Ok(())
        })?,
    )?;

    Ok(())
}
```

#### Step 3: Process the Command

In `src/systems/lua_commands.rs`, add to `process_entity_commands()`:

```rust
pub fn process_entity_commands(
    commands: &mut Commands,
    entity_commands: impl IntoIterator<Item = EntityCmd>,
    stuckto_query: &Query<&StuckTo>,
    signals_query: &mut Query<&mut Signals>,
    animation_query: &mut Query<&mut Animation>,
    rigid_bodies_query: &mut Query<&mut RigidBody>,
    positions_query: &mut Query<&mut MapPosition>,
) {
    for cmd in entity_commands {
        match cmd {
            // ... existing matches ...
            
            EntityCmd::SetHealth { entity_id, health } => {
                let entity = Entity::from_bits(entity_id);
                if let Ok(mut signals) = signals_query.get_mut(entity) {
                    signals.set_scalar("health", health);
                }
            }
        }
    }
}
```

#### Step 4: Update LSP Stubs (Optional but Recommended)

In `assets/scripts/engine.lua`:

```lua
--- Set the health value on an entity's Signals component.
--- @param entity_id integer The entity ID
--- @param health number The health value
function engine.entity_set_health(entity_id, health) end
```

#### Step 5: Document in README (Optional)

Add to `assets/scripts/README.md`:

```markdown
### engine.entity_set_health(entity_id, health)
Sets the "health" scalar signal on an entity.
- `entity_id`: Entity ID (u64)
- `health`: Health value (float)
```

---

### Adding a Completely New Command Type

If you need a new category of commands (e.g., `HealthCmd`):

#### Step 1: Define the Enum

```rust
// In commands.rs
#[derive(Debug, Clone)]
pub enum HealthCmd {
    SetEntityHealth { entity_id: u64, health: f32 },
    HealEntity { entity_id: u64, amount: f32 },
    DamageEntity { entity_id: u64, amount: f32 },
}
```

#### Step 2: Add Queue to LuaAppData

```rust
// In runtime.rs
pub(super) struct LuaAppData {
    // ... existing fields ...
    health_commands: RefCell<Vec<HealthCmd>>,
}
```

And initialize it in `LuaRuntime::new()`:

```rust
lua.set_app_data(LuaAppData {
    // ... existing fields ...
    health_commands: RefCell::new(Vec::new()),
});
```

#### Step 3: Add Drain Function

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

```rust
fn register_health_api(&self) -> LuaResult<()> {
    let engine: LuaTable = self.lua.globals().get("engine")?;

    engine.set(
        "heal_entity",
        self.lua.create_function(|lua, (entity_id, amount): (u64, f32)| {
            lua.app_data_ref::<LuaAppData>()
                .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                .health_commands
                .borrow_mut()
                .push(HealthCmd::HealEntity { entity_id, amount });
            Ok(())
        })?,
    )?;

    Ok(())
}
```

Don't forget to call `register_health_api()?` in `LuaRuntime::new()`.

#### Step 5: Create Processing Function

```rust
// In lua_commands.rs
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

```rust
// In game.rs update() or appropriate system
for cmd in lua_runtime.drain_health_commands() {
    process_health_commands(&mut signals_query, std::iter::once(cmd));
}
```

---

### Adding Entity Builder Methods

To add spawning capabilities:

#### Step 1: Add Data Structure (if needed)

```rust
// In spawn_data.rs
#[derive(Debug, Clone, Default)]
pub struct HealthData {
    pub initial_health: f32,
    pub max_health: f32,
}
```

#### Step 2: Add to SpawnCmd

```rust
// In spawn_data.rs
#[derive(Debug, Clone, Default)]
pub struct SpawnCmd {
    // ... existing fields ...
    pub health: Option<HealthData>,
}
```

#### Step 3: Add Builder Method

```rust
// In entity_builder.rs
impl LuaUserData for LuaEntityBuilder {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // ... existing methods ...
        
        methods.add_method_mut("with_health", |_, this, (initial, max): (f32, f32)| {
            this.cmd.health = Some(HealthData {
                initial_health: initial,
                max_health: max,
            });
            Ok(this.clone())
        });
    }
}
```

#### Step 4: Process During Spawn

```rust
// In lua_commands.rs process_spawn_command()
if let Some(health_data) = cmd.health {
    // Ensure Signals component exists
    if cmd.signals.is_none() {
        entity_commands.insert(Signals::default());
    }
    // Or configure a Health component if you have one
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
if let Ok(mut rb) = rigid_bodies_query.get_mut(entity) {
    rb.velocity = Vector2 { x: vx, y: vy };
}

// Better: Log for debugging
if let Ok(mut rb) = rigid_bodies_query.get_mut(entity) {
    rb.velocity = Vector2 { x: vx, y: vy };
} else {
    eprintln!("[Lua] Entity {:?} not found for SetVelocity", entity);
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

- `assets/scripts/engine.lua` - LSP stubs for autocomplete
- `assets/scripts/README.md` - Human-readable documentation
- `llm-context.md` - LLM context file

### 6. Test Your Commands

Add Lua test scripts to verify behavior:

```lua
-- Test entity_set_health
function test_set_health()
    local player_id = engine.get_entity("player")
    engine.entity_set_health(player_id, 100)
    engine.log("Health set to 100")
end
```

### 7. Consider Collision Context

If a command makes sense in collision callbacks, provide a `collision_*` variant:

```rust
// Regular context
engine.set("entity_set_health", /* pushes to entity_commands */);

// Collision context
engine.set("collision_entity_set_health", /* pushes to collision_entity_commands */);
```

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
2. Register Lua function in `runtime.rs`
3. Process command in `lua_commands.rs`
4. Optionally add builder method in `entity_builder.rs`
5. Update documentation files
