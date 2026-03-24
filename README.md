# Aberred Engine

![Aberred Engine](aberred.ico)

A compact 2D game engine built in Rust with optional Lua scripting.

Aberred Engine currently ships as a multi-scene showcase containing a menu plus several example games and demos, including Asteroids, Arkanoid, a sidescroller, a birthday card, Kraken, and multiple Bunnymark variants.

Built with:

- **Rust** (edition 2024)
- **bevy_ecs** (0.18) for ECS
- **raylib** (5.5.1) for windowing, rendering, input, and audio integration
- **mlua** (0.11, LuaJIT) for optional Lua scripting

## What it includes

- Custom engine bootstrap via `EngineBuilder`
- Optional Lua-driven game logic through `assets/scripts/main.lua`
- Rust-native scene support via `SceneManager`
- Sprites, animation, text, menus, tweening, collision, timers, phases, signals, particles, shaders, and camera follow
- Parent-child hierarchy support using Bevy relationships plus `GlobalTransform2D`
- Generated Lua stubs for editor support in `assets/scripts/engine.lua`

## Lua scripting

Lua game content lives in `assets/scripts/`. The engine exposes a global `engine` table for asset loading, audio, signals, entity spawning, scene control, camera/shader commands, and more.

Main Lua entrypoints:

- `on_setup()`
- `on_enter_play()`
- `on_switch_scene(scene_name)`
- `on_update_<scene>(input, dt)`

Useful Lua docs:

- `assets/scripts/README.md` — full Lua API reference
- `assets/scripts/engine.lua` — generated EmmyLua stubs
- `assets/scripts/.luarc.json` — generated Lua language server config

Regenerate the generated Lua files with:

```bash
cargo run -- --create-lua-stubs
cargo run -- --create-luarc
```

## Rust-native usage

You can also use the engine without Lua:

- `cargo build --no-default-features`
- configure hooks with `EngineBuilder`
- or register scenes with `SceneManager`

See `RUST-GAME-GUIDE.md` for the Rust path.

## Repository layout

```plaintext
src/
├── main.rs                   # CLI entry point
├── lib.rs                    # Library crate exports
├── engine_app.rs             # EngineBuilder, world setup, schedule, main loop
├── components/               # ECS components
├── resources/                # ECS resources + lua_runtime/
├── systems/                  # Systems and observers
└── events/                   # Event and message types
assets/
├── scripts/                  # Lua entrypoint, setup, scenes, generated stubs
├── textures/                 # Art assets grouped by showcase
├── audio/                    # Audio assets grouped by showcase
├── shaders/                  # Fragment shaders
└── fonts/                    # Font assets
tests/                        # Integration tests
config.ini                    # Runtime configuration
```

## Build and run

Prerequisites:

- Rust stable (rustup recommended)
- On Linux, the native Raylib dependency may need system packages; see the Wayland section below

Quick start:

```bash
cargo run
```

Run tests:

```bash
cargo test
```

Build without Lua support:

```bash
cargo build --no-default-features
```

### System dependencies for Wayland

On Debian/Ubuntu-based systems, raylib (and the native `raylib-sys` bindings) may require several development packages to compile and link correctly when using Wayland/GL. The exact packages depend on your distribution and available renderers, but the following list is a good starting point on an `apt` based system:

```bash
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

- The default build enables Lua support.
- The generated Lua stubs should not be edited by hand.
- `engine_app.rs` is the main source of truth for engine startup and schedule ordering.
