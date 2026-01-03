# Aberred Engine - Lua Interface Architecture

This document describes the Lua scripting interface architecture and provides a guide for developers who want to add new Lua commands to interact with ECS components.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Module Structure](#module-structure)
3. [Command Flow: Lua to ECS](#command-flow-lua-to-ecs)
4. [Command Types and Queues](#command-types-and-queues)
5. [Entity Builder Pattern](#entity-builder-pattern)
6. [Signal Snapshot System](#signal-snapshot-system)
7. [How to Add New Lua Commands](#how-to-add-new-lua-commands)
8. [Best Practices](#best-practices)

---

## Architecture Overview

The Aberred Engine uses a **deferred command pattern** for Lua-Rust integration. Lua scripts cannot directly modify ECS entities—instead, they queue commands that are processed by Rust systems after Lua callbacks return.

### High-Level Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                             GAME LOOP                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐         │
│   │   Lua Script    │───▶│  Command Queue  │───▶│  Rust Systems   │         │
│   │                 │    │  (LuaAppData)   │    │  (process_*)    │         │
│   └─────────────────┘    └─────────────────┘    └─────────────────┘         │
│          │                       │                      │                    │
│          │ engine.spawn()        │ SpawnCmd             │ Commands.spawn()   │
│          │ engine.set_flag()     │ SignalCmd            │ world_signals.set  │
│          │ engine.despawn()      │ EntityCmd            │ entity.despawn()   │
│          ▼                       ▼                      ▼                    │
│   ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐         │
│   │ Signal Snapshot │◀───│  WorldSignals   │◀───│   ECS World     │         │
│   │   (read-only)   │    │   (Resource)    │    │                 │         │
│   └─────────────────┘    └─────────────────┘    └─────────────────┘         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Why Deferred Commands?

1. **Thread Safety**: Lua is single-threaded; direct ECS access would require complex synchronization.
2. **Consistency**: Commands are processed at predictable points in the game loop.
3. **Error Handling**: Commands can be validated and errors reported cleanly.
4. **Performance**: Batch processing of commands is more efficient than immediate execution.

---

## Module Structure

The Lua runtime is organized in `src/resources/lua_runtime/`:

```
src/resources/lua_runtime/
├── mod.rs              # Public exports
├── runtime.rs          # LuaRuntime struct, engine table API registration
├── commands.rs         # Command enums (EntityCmd, SignalCmd, etc.)
├── entity_builder.rs   # LuaEntityBuilder fluent API for spawning
└── spawn_data.rs       # Data structures for spawn configuration
```

### Key Components

#### `LuaRuntime` (runtime.rs)
The main struct managing the Lua interpreter. It:
- Initializes the Lua state with MLua
- Registers the global `engine` table with all API functions
- Manages `LuaAppData` for command queuing
- Provides `drain_*_commands()` methods for Rust to retrieve queued commands

#### `LuaAppData` (runtime.rs)
Internal shared state accessible from Lua closures:
```rust
pub(super) struct LuaAppData {
    // Regular command queues (processed after callbacks return)
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
    
    // Collision-scoped queues (processed immediately after collision callbacks)
    collision_entity_commands: RefCell<Vec<EntityCmd>>,
    collision_signal_commands: RefCell<Vec<SignalCmd>>,
    collision_audio_commands: RefCell<Vec<AudioLuaCmd>>,
    collision_spawn_commands: RefCell<Vec<SpawnCmd>>,
    collision_phase_commands: RefCell<Vec<PhaseCmd>>,
    collision_camera_commands: RefCell<Vec<CameraCmd>>,
    
    // Cached read-only data (updated before Lua callbacks)
    signal_snapshot: RefCell<Arc<SignalSnapshot>>,
    tracked_groups: RefCell<FxHashSet<String>>,
    input_action_back_pressed: RefCell<bool>,
    // ...more input cache fields
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
|------------|---------------|----------|
| Regular (`entity_commands`, etc.) | After phase/timer/update callbacks | Normal game logic |
| Collision (`collision_entity_commands`, etc.) | Immediately after each collision callback | Collision response |

This distinction matters because collision callbacks need immediate processing to ensure position corrections and velocity changes happen before the next collision is detected.

### Command Categories

| Category | Enum | Purpose |
|----------|------|---------|
| **Entity** | `EntityCmd` | Manipulate existing entities (velocity, position, signals, components) |
| **Spawn** | `SpawnCmd` | Create new entities with components |
| **Signal** | `SignalCmd` | Modify global WorldSignals |
| **Audio** | `AudioLuaCmd` | Play/stop music and sounds |
| **Phase** | `PhaseCmd` | Trigger state machine transitions |
| **Camera** | `CameraCmd` | Configure the 2D camera |
| **Asset** | `AssetCmd` | Load textures, fonts, music (setup only) |
| **Group** | `GroupCmd` | Manage tracked entity groups |
| **Tilemap** | `TilemapCmd` | Spawn tiles from tilemap data |
| **Animation** | `AnimationCmd` | Register animation definitions |

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

### How it Works

1. `engine.spawn()` returns a `LuaEntityBuilder` UserData object
2. Each `:with_*()` method modifies the internal `SpawnCmd` and returns `self`
3. `:build()` pushes the `SpawnCmd` to the spawn queue
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

        methods.add_method_mut("build", |lua, this, ()| {
            lua.app_data_ref::<LuaAppData>()
                .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                .spawn_commands
                .borrow_mut()
                .push(this.cmd.clone());
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
lua_runtime.update_input_cache(&input);

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
```

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
