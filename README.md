# Aberred Engine

![Aberred Engine](aberred.ico)

A compact 2D game engine with full Lua scripting support. Currently demonstrated as "DRIFTERS", an Asteroids-style arcade game.

Built with:

- **Rust** (2024 edition) — core engine
- **bevy_ecs** (0.18) — Entity-Component-System architecture
- **raylib** (5.5.1) — windowing, input, 2D rendering
- **mlua** (0.11, LuaJIT) — game logic scripting
- **fastrand** — RNG for particle systems

## Current status (2026-02-19)

Playable loop: menu → level01 asteroids prototype with ship (idle/propulsion phases), drifting asteroids, tiled space background, and laser firing.

**Core subsystems:**

- Rendering (sprites, z-ordering, rotation, scale, camera, dynamic text)
- Physics (velocity, friction, max speed, named acceleration forces)
- Collision (AABB with group-based Lua callbacks)
- Animation (frame-based with rule-driven state machine)
- Tweening (position/rotation/scale with easing)
- Audio (background thread, WAV sounds)
- Menus (keyboard navigation, scene switching)
- Phases (Lua state machine with enter/update/exit callbacks)
- Signals (per-entity and global: scalars, integers, strings, flags, entities)
- Entity cloning (template-based spawning with overrides)
- Particle emitter (WIP — configurable shape, arc, speed, TTL)
- TTL (time-to-live auto-despawn)

**ECS architecture:**

- 28 components, 23 systems, 15+ resources
- Debug mode (F11): collision boxes, entity signals, diagnostics

**TODO:**

- ~~Shader support~~
- _Automated tests and CI_ Work in progress
- _Cross-platform packaging_ Linux and Windows

## Lua Scripting

Game logic is defined in `assets/scripts/`. The engine exposes a global `engine` table with 100+ functions for asset loading, audio, signals, entity commands, camera, and groups.

```lua
-- Fluent entity builder example
engine.spawn()
    :with_group("ship")
    :with_position(320, 180)
    :with_sprite("ship_sheet", 32, 32, 16, 16)
    :with_phase({ idle = { on_enter = "ship_idle_enter", on_update = "ship_idle_update" } })
    :with_collider(24, 24, 12, 12)
    :register_as("player_ship")
    :build()
```

**Callbacks:** `on_setup()`, `on_enter_play()`, `on_switch_scene(name)`, `on_update_<scene>(input, dt)`

See `assets/scripts/README.md` for the full API reference (78k+ lines).

## Repository layout

```plaintext
src/
├── main.rs, game.rs          # Entry point, main loop, scene callbacks
├── components/               # 28 ECS components
├── systems/                  # 23 game systems
├── resources/                # Shared resources + lua_runtime/
└── events/                   # Event types and observers
assets/
├── scripts/                  # Lua: main.lua, setup.lua, scenes/
├── textures/                 # Space/asteroid PNG sprites
├── audio/                    # WAV sounds
└── fonts/                    # TTF fonts
config.ini                    # Runtime configuration
```

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

- The project is intentionally small and experimental. Expect breaking changes while APIs stabilize.
