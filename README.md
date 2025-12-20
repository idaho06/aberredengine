# Aberred Engine

A compact 2D game engine and sandbox with full Lua scripting support. Demonstrated as an Arkanoid-style breakout clone.

Built with:
- **Rust** (2024 edition) — core engine implementation
- **Lua 5.4/LuaJIT** — game logic scripting via mlua bindings
- **raylib** (v5.5) — windowing, input, 2D rendering
- **bevy_ecs** (v0.17) — Entity-Component-System architecture
- **crossbeam-channel** — lock-free audio thread communication

## Current status (2025-12-20)

- Playable Arkanoid demo with full Lua scripting (active development)
- Core subsystems implemented:
  - **Rendering**: sprite rendering with z-ordering, rotation, scale, camera transforms, frustum culling, and dynamic text
  - **Input**: keyboard and mouse input with action-based bindings (back/confirm actions)
  - **Movement**: velocity-based position integration (semi-implicit Euler)
  - **Collision**: AABB overlap detection with group-based callback rules and detailed side information
  - **Animation**: frame-based sprite animation with data-driven state machine and conditional rules
  - **Tweening**: position, rotation, and scale interpolation with multiple easing functions (linear, quad, cubic variants)
  - **Audio**: background thread with XM tracker music streaming and WAV sound effects
  - **Menus**: interactive menu system with keyboard navigation and scene switching
  - **Timers**: countdown timers with Lua callback support
  - **Phases**: state machine component with enter/update/exit lifecycle callbacks accessible from Lua
  - **Signals**: per-entity and global signal storage (scalars, integers, strings, flags, entities) for cross-system communication
  - **Signal Bindings**: reactive UI text updates bound to signal values
  - **Grid Layouts**: data-driven entity spawning from JSON definitions
  - **Entity Attachment**: attach entities to follow other entities (StuckTo) with offset support
  - **Tilemap**: JSON-based tilemap loading (Tilesetter format)
  - **2D Camera**: world/screen transforms with zoom and rotation
  - **Lua Scripting**: comprehensive API with 100+ engine functions, fluent entity builder, and scene-based callbacks
- ECS-driven architecture with 26 components:
  - Position: `MapPosition`, `ScreenPosition`
  - Rendering: `Sprite`, `DynamicText`, `ZIndex`, `Rotation`, `Scale`
  - Physics: `RigidBody`, `BoxCollider`
  - Animation: `Animation`, `AnimationController`
  - Input: `InputControlled`, `MouseControlled`
  - UI: `Menu`, `MenuActions`, `MenuItem`, `SignalBinding`
  - State: `Phase`, `LuaPhase`, `Signals`, `Timer`, `LuaTimer`, `StuckTo`
  - Collision: `CollisionRules`, `LuaCollision`
  - Utility: `Group`, `Persistent`, `GridLayout`
  - Tweening: `TweenPosition`, `TweenRotation`, `TweenScale`
- 21 game systems + background audio thread
- 15+ shared resources: `TextureStore`, `FontStore`, `AnimationStore`, `TilemapStore`, `LuaRuntime`, `Camera2DRes`, `ScreenSize`, `InputState`, `WorldTime`, `WorldSignals`, `TrackedGroups`, `AudioBridge`, `GameState`, `SystemsStore`, `DebugMode`
- Event system: `CollisionEvent`, `InputEvent`, `MenuSelectionEvent`, `TimerEvent`, `LuaTimerEvent`, `PhaseChangeEvent`, `GameStateChangedEvent`, `SwitchDebugEvent`, `AudioCmd`, `AudioMessage`
- Debug utilities: debug-mode toggle (F11), collision box visualization, entity signal display, and on-screen diagnostics
- Game state machine with setup, playing, paused, and quitting states
- Packaging: no installers; runnable via `cargo run`. Release builds available with `--release`.

Not yet implemented / TODO (high level):
- Shader support for sprites
- Automated tests and CI
- Cross-platform packaging and installers (currently tested on Linux)

## Lua Scripting

Game logic is defined in Lua scripts under `assets/scripts/`. The engine exposes a global `engine` table with functions for:

- **Asset Loading**: `load_texture`, `load_font`, `load_music`, `load_sound`, `load_tilemap`
- **Audio**: `play_music`, `play_sound`, `stop_all_music`, `stop_all_sounds`
- **Input**: `is_action_back_pressed`, `is_action_confirm_just_pressed`, etc.
- **Signals**: `set_scalar`, `get_integer`, `set_flag`, etc.
- **Entity Commands**: `entity_despawn`, `entity_set_position`, `entity_set_velocity`, `phase_transition`
- **Camera**: `set_camera` (position, offset, rotation, zoom)
- **Groups**: `track_group`, `get_group_count`

**Fluent Entity Builder:**
```lua
engine.spawn()
    :with_group("player")
    :with_position(400, 700)
    :with_sprite("vaus", 96, 12, 48, 6)
    :with_collider(96, 12, 48, 6)
    :with_velocity(0, 0)
    :with_zindex(10)
    :build()
```

**Scene Callbacks:**
- `on_setup()` — asset loading
- `on_enter_play()` — global signal initialization
- `on_switch_scene(name)` — scene setup and entity spawning
- `on_update_<scene>(dt)` — per-frame game logic

See `assets/scripts/README.md` for the full API reference.

## Repository layout (high-level)

- `src/` — engine source (~12,700 lines)
  - `main.rs`, `game.rs` — entry point, window setup, main loop
  - `components/` — 26 ECS component definitions
    - `animation.rs` — animation playback and rule-based controller
    - `boxcollider.rs` — AABB collision geometry
    - `collision.rs`, `luacollision.rs` — collision rules and callbacks
    - `dynamictext.rs` — runtime text rendering
    - `gridlayout.rs` — data-driven grid spawning
    - `group.rs` — entity grouping tags
    - `inputcontrolled.rs` — keyboard and mouse control
    - `mapposition.rs`, `screenposition.rs` — world/screen positioning
    - `menu.rs` — interactive menu components
    - `persistent.rs` — entities that survive scene changes
    - `phase.rs`, `luaphase.rs` — state machine with Rust/Lua callbacks
    - `rigidbody.rs` — velocity storage
    - `rotation.rs`, `scale.rs` — transform components
    - `signalbinding.rs` — binds UI text to signal values
    - `signals.rs` — per-entity signal storage
    - `sprite.rs` — 2D sprite rendering
    - `stuckto.rs` — attach entities to other entities
    - `timer.rs`, `luatimer.rs` — countdown timers with Lua support
    - `tween.rs` — animated interpolation
    - `zindex.rs` — render order
  - `resources/` — shared ECS resources
    - `animationstore.rs` — animation definitions
    - `audio.rs` — audio thread bridge (crossbeam channels)
    - `camera2d.rs` — 2D camera state
    - `debugmode.rs` — debug rendering toggle
    - `fontstore.rs`, `texturestore.rs`, `tilemapstore.rs` — asset caches
    - `gamestate.rs` — game state management
    - `group.rs` — tracked groups for entity counting
    - `input.rs` — keyboard/mouse state
    - `screensize.rs`, `worldtime.rs` — screen and timing
    - `systemsstore.rs` — dynamic system lookup
    - `worldsignals.rs` — global signals
    - `lua_runtime/` — Lua integration subsystem
      - `runtime.rs` — LuaRuntime, engine API registration
      - `commands.rs` — 30+ command types for Lua callbacks
      - `spawn_data.rs` — component serialization
      - `entity_builder.rs` — fluent entity builder
  - `systems/` — 21 game systems
    - `animation.rs` — animation updates
    - `audio.rs` — audio thread and message polling
    - `collision.rs` — overlap detection and event dispatch
    - `gamestate.rs` — state transitions
    - `gridlayout.rs` — spawns entities from grid layouts
    - `group.rs` — entity counting per group
    - `input.rs` — input polling
    - `inputsimplecontroller.rs`, `mousecontroller.rs` — input-to-velocity
    - `menu.rs` — menu spawning and interaction
    - `movement.rs` — position integration
    - `phase.rs`, `luaphase.rs` — phase state machine processing
    - `luatimer.rs` — Lua timer callbacks
    - `lua_commands.rs` — Lua command processing
    - `render.rs` — sprite and debug rendering
    - `signalbinding.rs` — updates text from signals
    - `stuckto.rs` — position attachment system
    - `time.rs` — world time and timer updates
    - `tween.rs` — tween animation
  - `events/` — event types and observers
    - `audio.rs` — audio commands and messages
    - `collision.rs` — collision events
    - `gamestate.rs` — state change events
    - `input.rs` — input action events
    - `menu.rs` — menu selection events
    - `phase.rs`, `luatimer.rs` — phase/timer events
    - `switchdebug.rs` — debug toggle
    - `timer.rs` — timer expiration events
- `assets/` — game content
  - `scripts/` — Lua game scripts (~800 lines)
    - `main.lua` — entry point, scene callbacks
    - `setup.lua` — asset loading configuration
    - `engine.lua` — LuaLS type definitions
    - `scenes/` — scene modules (menu.lua, level01.lua)
  - `textures/` — PNG sprites (12 files)
  - `audio/` — XM music (8 files), WAV sounds (3 files)
  - `fonts/` — TrueType fonts (2 files)
  - `tilemaps/` — tilemap data (JSON + PNG atlas)
- `Cargo.toml`, `Cargo.lock` — Rust manifest and lockfile

## Build and run

Prerequisites:
- Rust stable (rustup recommended). The project uses standard crates and raylib bindings; on most Linux systems the `raylib-sys` crate will build the native dependency automatically.

Quick start:

```fish
cargo run
```

For a release build:

```fish
cargo run --release
```

Generate documentation:

```fish
cargo doc --open
```

### System dependencies for Wayland

On Debian/Ubuntu-based systems, raylib (and the native `raylib-sys` bindings) may require several development packages to compile and link correctly when using Wayland/GL. The exact packages depend on your distribution and available renderers, but the following list is a good starting point on an `apt` based system:

```fish
sudo apt update
sudo apt install -y \
	build-essential pkg-config cmake \
	libx11-dev libxcursor-dev libxinerama-dev libxrandr-dev libxi-dev \
	libgl1-mesa-dev libegl1-mesa-dev libgbm-dev \
	libwayland-dev libwayland-egl1-mesa \
	libxkbcommon-dev \
	libasound2-dev libpulse-dev \
	libfreetype6-dev libjpeg-dev libpng-dev
```

## Notes

- VSync is enabled by default in the renderer to avoid busy-waiting the CPU.
- The project is intentionally small and experimental. Expect breaking changes while APIs stabilize.