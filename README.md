# Aberred Engine

A compact 2D game sandbox and engine prototype.

Built with:
- raylib (windowing, input, rendering) via the Rust bindings
- bevy_ecs for the Entity-Component-System architecture
- crossbeam-channel for lock-free audio thread communication

## Current status (2025-12-09)

- Playable prototype / engine scaffold (active development)
- Core subsystems implemented:
  - **Rendering**: sprite rendering with z-ordering, rotation, scale, and camera transforms
  - **Input**: keyboard and mouse input handling with configurable bindings
  - **Movement**: velocity-based position integration
  - **Collision**: AABB overlap detection with group-based callback rules
  - **Animation**: frame-based sprite animation with rule-driven state machine
  - **Tweening**: position, rotation, and scale interpolation with multiple easing functions
  - **Audio**: background thread with music streaming and sound effect playback
  - **Menus**: interactive menu system with scene switching and actions
  - **Timers**: countdown timers with event emission
  - **Phases**: state machine component with enter/update/exit callbacks for game logic
  - **Signals**: per-entity and global signal storage for cross-system communication
  - **Signal Bindings**: reactive UI text updates bound to signal values
  - **Grid Layouts**: data-driven entity spawning from JSON definitions
  - **Entity Attachment**: attach entities to follow other entities (StuckTo)
  - **Tilemap**: basic tilemap loading and storage
  - **2D Camera**: shared camera resource for world/screen transforms
- ECS-driven architecture with comprehensive component set:
  - Position: `MapPosition`, `ScreenPosition`
  - Rendering: `Sprite`, `DynamicText`, `ZIndex`, `Rotation`, `Scale`
  - Physics: `RigidBody`, `BoxCollider`
  - Animation: `Animation`, `AnimationController`
  - Input: `InputControlled`, `MouseControlled`
  - UI: `Menu`, `MenuActions`, `MenuItem`, `SignalBinding`
  - State: `Phase`, `Signals`, `Timer`, `StuckTo`
  - Utility: `Group`, `Persistent`, `GridLayout`
  - Tweening: `TweenPosition`, `TweenRotation`, `TweenScale`
- Resource containers: `TextureStore`, `FontStore`, `AnimationStore`, `TilemapStore`, `Camera2DRes`, `ScreenSize`, `InputState`, `WorldTime`, `WorldSignals`, `TrackedGroups`, `AudioBridge`, `GameState`, `SystemsStore`, `DebugMode`
- Event system: `CollisionEvent`, `InputEvent`, `MenuSelectionEvent`, `TimerEvent`, `PhaseChangeEvent`, `GameStateChangedEvent`, `SwitchDebugEvent`, `AudioCmd`, `AudioMessage`
- Debug utilities: debug-mode toggle (F11), collision box visualization, entity signal display, and on-screen diagnostics
- Game state machine with setup, playing, paused, and quitting states
- Packaging: no installers; runnable via `cargo run`. Release builds available with `--release`.

Not yet implemented / TODO (high level):
- Scripting integration (Lua) and a stable scripting API
- Shader support for sprites
- Automated tests and CI
- Cross-platform packaging and installers (currently tested on Linux)

## Repository layout (high-level)

- `src/` — engine source
  - `main.rs`, `game.rs` — entry point and game setup
  - `components/` — ECS component definitions
    - `animation.rs` — animation playback and rule-based controller
    - `boxcollider.rs` — AABB collision component
    - `collision.rs` — collision rules and callback context
    - `dynamictext.rs` — runtime text rendering
    - `gridlayout.rs` — data-driven grid spawning
    - `group.rs` — entity grouping tags
    - `inputcontrolled.rs` — keyboard and mouse control
    - `mapposition.rs`, `screenposition.rs` — world/screen positioning
    - `menu.rs` — interactive menu components
    - `persistent.rs` — entities that survive scene changes
    - `phase.rs` — state machine with enter/update/exit callbacks
    - `rigidbody.rs` — velocity storage
    - `rotation.rs`, `scale.rs` — transform components
    - `signalbinding.rs` — binds UI text to signal values
    - `signals.rs` — per-entity signal storage
    - `sprite.rs` — 2D sprite rendering
    - `stuckto.rs` — attach entities to other entities
    - `timer.rs` — countdown timer
    - `tween.rs` — animated interpolation
    - `zindex.rs` — render order
  - `resources/` — shared ECS resources
    - `animationstore.rs` — animation definitions
    - `audio.rs` — audio thread bridge
    - `camera2d.rs` — 2D camera
    - `debugmode.rs` — debug rendering toggle
    - `fontstore.rs`, `texturestore.rs`, `tilemapstore.rs` — asset stores
    - `gamestate.rs` — game state management
    - `group.rs` — tracked groups for entity counting
    - `input.rs` — keyboard state
    - `screensize.rs`, `worldtime.rs` — screen and timing
    - `systemsstore.rs` — dynamic system lookup
    - `worldsignals.rs` — global signals
  - `systems/` — game systems
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
    - `phase.rs` — phase state machine processing
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
    - `phase.rs` — phase transition events
    - `switchdebug.rs` — debug toggle
    - `timer.rs` — timer expiration events
- `assets/` — art, tilemaps, sounds, fonts
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