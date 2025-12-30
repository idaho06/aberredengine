![Aberred Engine](aberred.ico)
# Aberred Engine

A compact 2D game engine and sandbox with full Lua scripting support. Demonstrated as an Arkanoid-style breakout clone.

Built with:
- **Rust** (2024 edition) — core engine implementation
- **Lua 5.4/LuaJIT** — game logic scripting via mlua bindings
- **raylib** (v5.5) — windowing, input, 2D rendering
- **bevy_ecs** (v0.17) — Entity-Component-System architecture
- **crossbeam-channel** — lock-free audio thread communication

## Current status (2025-12-29)

- Playable Arkanoid demo with full Lua scripting (active development)
- Core subsystems implemented:
  - **Rendering**: sprite rendering with z-ordering, rotation, scale, camera transforms, letterboxing, and dynamic text with cached sizing
  - **Input**: keyboard and mouse input with action-based bindings (back/confirm/special actions), mouse position corrected for letterboxing
  - **Movement**: acceleration-based physics with named forces, friction, max speed limiting, and entity freezing
  - **Collision**: AABB overlap detection with group-based callback rules, detailed side information, and immediate command processing
  - **Animation**: frame-based sprite animation with data-driven state machine and conditional rules (AnimationController)
  - **Tweening**: position, rotation, and scale interpolation with multiple easing functions and loop modes
  - **Audio**: background thread with XM tracker music streaming and WAV sound effects
  - **Menus**: interactive menu system with keyboard navigation, scene switching, and submenu support
  - **Timers**: countdown timers with both signal-based and Lua callback support
  - **Phases**: state machine component with enter/update/exit lifecycle callbacks accessible from Lua
  - **Signals**: per-entity and global signal storage (scalars, integers, strings, flags, entities, group counts) for cross-system communication
  - **Signal Bindings**: reactive UI text updates bound to signal values with format strings
  - **Grid Layouts**: data-driven entity spawning from JSON definitions
  - **Entity Attachment**: attach entities to follow other entities (StuckTo) with offset and stored velocity support
  - **Tilemap**: JSON-based tilemap loading (Tilesetter format)
  - **2D Camera**: world/screen transforms with zoom and rotation
  - **Fullscreen**: toggle fullscreen mode (F10) with render target scaling
  - **Lua Scripting**: comprehensive API with 100+ engine functions, fluent entity builder, and scene-based callbacks
- ECS-driven architecture with 27 components:
  - Position: `MapPosition`, `ScreenPosition`
  - Rendering: `Sprite`, `DynamicText`, `ZIndex`, `Rotation`, `Scale`
  - Physics: `RigidBody` (velocity, friction, max_speed, named acceleration forces, frozen), `BoxCollider`
  - Animation: `Animation`, `AnimationController`
  - Input: `InputControlled`, `AccelerationControlled`, `MouseControlled`
  - UI: `Menu`, `SignalBinding`
  - State: `Phase`, `LuaPhase`, `Signals`, `Timer`, `LuaTimer`, `StuckTo`
  - Collision: `CollisionRule`, `LuaCollisionRule`
  - Utility: `Group`, `Persistent`, `GridLayout`
  - Tweening: `TweenPosition`, `TweenRotation`, `TweenScale`
- 22+ game systems + background audio thread
- 18+ shared resources: `TextureStore`, `FontStore`, `AnimationStore`, `TilemapStore`, `LuaRuntime`, `Camera2D`, `ScreenSize`, `WindowSize`, `RenderTarget`, `InputState`, `WorldTime`, `WorldSignals`, `SignalSnapshot`, `TrackedGroups`, `AudioBridge`, `GameState`, `NextGameState`, `SystemsStore`, `DebugMode`, `FullScreen`
- Event system: `CollisionEvent`, `InputAction`, `MenuSelection`, `TimerEvent`, `LuaTimerEvent`, `PhaseTransition`, `GameStateTransition`, `DebugToggle`, `FullScreenToggle`, `AudioCmd`, `AudioMessage`
- Debug utilities: debug-mode toggle (F11), fullscreen toggle (F10), collision box visualization, entity signal display, and on-screen diagnostics
- Game state machine with setup, playing, paused, and quitting states
- Packaging: no installers; runnable via `cargo run`. Release builds available with `--release`.

Not yet implemented / TODO (high level):
- Shader support for sprites
- Automated tests and CI
- Cross-platform packaging and installers (currently tested on Linux)

## Lua Scripting

Game logic is defined in Lua scripts under `assets/scripts/`. The engine exposes a global `engine` table with functions for:

- **Asset Loading** (on_setup only): `load_texture`, `load_font`, `load_music`, `load_sound`, `load_tilemap`, `register_animation`
- **Audio**: `play_music`, `play_sound`, `stop_all_music`, `stop_all_sounds`
- **Input**: `is_action_back_pressed`, `is_action_back_just_pressed`, `is_action_confirm_pressed`, `is_action_confirm_just_pressed`
- **World Signals**: `set_scalar`, `get_scalar`, `set_integer`, `get_integer`, `set_string`, `get_string`, `set_flag`, `has_flag`, `clear_flag`, `get_entity`, `get_group_count`
- **Entity Commands**: `entity_set_velocity`, `entity_set_position`, `entity_despawn`, `entity_set_rotation`, `entity_set_scale`, `entity_freeze`, `entity_unfreeze`, `entity_set_speed`, `entity_set_friction`, `entity_set_max_speed`
- **Entity Forces**: `entity_add_force`, `entity_remove_force`, `entity_set_force_enabled`, `entity_set_force_value`
- **Entity Signals**: `entity_signal_set_flag`, `entity_signal_clear_flag`, `entity_signal_set_scalar`, `entity_signal_set_string`, `entity_signal_set_integer`
- **Entity Attachment**: `entity_insert_stuckto`, `release_stuckto`
- **Entity Animation**: `entity_set_animation`, `entity_restart_animation`
- **Entity Timers**: `entity_insert_lua_timer`, `entity_remove_lua_timer`, `entity_insert_timer`
- **Entity Tweening**: `entity_insert_tween_position`, `entity_insert_tween_rotation`, `entity_insert_tween_scale`, `entity_remove_tween_position`, `entity_remove_tween_rotation`, `entity_remove_tween_scale`
- **Phase Control**: `phase_transition`
- **Camera**: `set_camera` (target, offset, rotation, zoom)
- **Groups**: `track_group`, `untrack_group`, `clear_tracked_groups`, `has_tracked_group`
- **Tilemap**: `spawn_tiles`
- **Collision Context Commands**: `collision_spawn`, `collision_play_sound`, `collision_set_integer`, `collision_set_flag`, `collision_clear_flag`, `collision_phase_transition`, `collision_set_camera`, `collision_entity_freeze`, `collision_entity_unfreeze`, `collision_entity_add_force`, `collision_entity_set_force_enabled`, `collision_entity_set_speed`

**Fluent Entity Builder:**
```lua
engine.spawn()
    :with_group("player")
    :with_position(400, 700)
    :with_sprite("vaus", 96, 12, 48, 6)
    :with_collider(96, 12, 48, 6)
    :with_velocity(0, 0)
    :with_friction(0.9)
    :with_max_speed(500)
    :with_accel("input", 0, 0, true)
    :with_zindex(10)
    :with_phase({ phases = {...}, initial = "idle" })
    :register_as("player")
    :build()
```

**Scene Callbacks:**
- `on_setup()` — asset loading
- `on_enter_play()` — global signal initialization
- `on_switch_scene(name)` — scene setup and entity spawning
- `on_update_<scene>(dt)` — per-frame game logic

See `assets/scripts/README.md` for the full API reference.

## Repository layout (high-level)

- `src/` — engine source
  - `main.rs` — app entry, ECS world setup, main loop, system schedule
  - `game.rs` — GameState logic, scene switching, Lua callbacks
  - `components/` — 27 ECS component definitions
    - `animation.rs` — Animation playback state + AnimationController
    - `boxcollider.rs` — BoxCollider (AABB collision shape)
    - `collision.rs` — CollisionRule, collision observer context
    - `luacollision.rs` — LuaCollisionRule for Lua callbacks
    - `dynamictext.rs` — Text rendering component with cached size
    - `gridlayout.rs` — JSON grid spawning
    - `group.rs` — Entity grouping tag
    - `inputcontrolled.rs` — InputControlled, AccelerationControlled, MouseControlled
    - `mapposition.rs` — MapPosition (world-space position)
    - `screenposition.rs` — ScreenPosition (UI/screen-space position)
    - `menu.rs` — Interactive menu
    - `persistent.rs` — Survive scene transitions
    - `phase.rs` — Rust phase state machine
    - `luaphase.rs` — Lua-based phase state machine
    - `rigidbody.rs` — Velocity, friction, max_speed, named accel forces, frozen
    - `rotation.rs` — Rotation in degrees
    - `scale.rs` — 2D scale
    - `signalbinding.rs` — Bind text to world signals
    - `signals.rs` — Per-entity signals (scalars/ints/flags/strings)
    - `sprite.rs` — Sprite rendering (tex_key, offset, origin, flip)
    - `stuckto.rs` — Attach entity to another
    - `timer.rs` — Countdown timer
    - `luatimer.rs` — Lua callback timer
    - `tween.rs` — TweenPosition, TweenRotation, TweenScale
    - `zindex.rs` — Render order
  - `resources/` — shared ECS resources
    - `animationstore.rs` — Animation definitions
    - `audio.rs` — AudioBridge channels
    - `camera2d.rs` — Camera2D config
    - `debugmode.rs` — Debug render toggle
    - `fullscreen.rs` — FullScreen marker resource
    - `fontstore.rs` — Font cache (non-send)
    - `texturestore.rs` — Texture cache
    - `tilemapstore.rs` — Tilemap layouts
    - `gamestate.rs` — GameState enum + NextGameState
    - `group.rs` — TrackedGroups set
    - `input.rs` — InputState cached keyboard (F10=fullscreen, F11=debug)
    - `screensize.rs` — Game's internal render resolution
    - `windowsize.rs` — Actual window dimensions (for letterboxing)
    - `rendertarget.rs` — RenderTarget for fixed-resolution rendering
    - `worldtime.rs` — Delta time, time scale
    - `systemsstore.rs` — Named system lookup
    - `worldsignals.rs` — Global signal storage + SignalSnapshot
    - `lua_runtime/` — Lua integration subsystem
      - `runtime.rs` — LuaRuntime, engine table API registration
      - `commands.rs` — EntityCmd, CollisionEntityCmd, SpawnCmd, etc.
      - `entity_builder.rs` — LuaEntityBuilder, LuaCollisionEntityBuilder fluent API
      - `spawn_data.rs` — SpawnComponentData structures
  - `systems/` — 22+ game systems
    - `animation.rs` — Frame advancement + rule evaluation (AnimationController)
    - `audio.rs` — Audio thread bridge
    - `collision.rs` — AABB detection, Lua callback dispatch
    - `dynamictext_size.rs` — Cache DynamicText bounding box sizes
    - `gamestate.rs` — State transition check
    - `gridlayout.rs` — Grid entity spawning
    - `group.rs` — Group counting
    - `input.rs` — Poll keyboard state
    - `inputsimplecontroller.rs` — Input→velocity
    - `inputaccelerationcontroller.rs` — Input→acceleration
    - `mousecontroller.rs` — Mouse position tracking (with letterbox correction)
    - `menu.rs` — Menu spawn/input
    - `movement.rs` — Physics: accel→vel→pos, friction, max_speed
    - `phase.rs` — Rust phase callbacks
    - `luaphase.rs` — Lua phase callbacks
    - `luatimer.rs` — Lua timer processing
    - `lua_commands.rs` — Process EntityCmd/CollisionEntityCmd/SpawnCmd
    - `render.rs` — Raylib drawing, camera, debug overlays, letterboxing
    - `signalbinding.rs` — Update bound text
    - `stuckto.rs` — StuckTo entity following
    - `time.rs` — WorldTime update, timer ticks
    - `tween.rs` — Tween animation systems (position/rotation/scale)
  - `events/` — event types and observers
    - `audio.rs` — AudioCmd, AudioMessage
    - `collision.rs` — CollisionEvent
    - `gamestate.rs` — GameStateTransition
    - `input.rs` — InputAction events
    - `menu.rs` — MenuSelection
    - `phase.rs` — PhaseTransition
    - `timer.rs` — TimerEvent
    - `luatimer.rs` — LuaTimerEvent
    - `switchdebug.rs` — DebugToggle (F11)
    - `switchfullscreen.rs` — FullScreen toggle event + observer (F10)
- `assets/` — game content
  - `scripts/` — Lua game scripts
    - `main.lua` — Entry: on_setup, on_enter_play, on_switch_scene
    - `setup.lua` — Asset loading helpers
    - `engine.lua` — LSP autocomplete stubs
    - `README.md` — Lua API documentation
    - `scenes/` — scene modules (menu.lua, level01.lua)
  - `textures/` — PNG sprites
  - `audio/` — XM music, WAV sounds
  - `fonts/` — TrueType fonts
  - `tilemaps/` — tilemap data (JSON + PNG atlas)
- `Cargo.toml`, `Cargo.lock` — Rust manifest and lockfile
- `llm-context.md` — Machine-readable context for AI assistants

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