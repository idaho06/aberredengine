# Aberred Engine

A compact 2D game sandbox and engine prototype.

Built with:
- raylib (windowing, input, rendering) via the Rust bindings
- bevy_ecs for the Entity-Component-System architecture
- rhai planned for scripting (work-in-progress)

## Current status (2025-09-07)

- Playable prototype / engine scaffold (active development)
- Core subsystems implemented: rendering, input, movement, collision events, animation system, tilemap loading, and a 2D camera resource
- Basic audio support present (playback resources and systems for simple sounds/music)
- ECS-driven: components include `MapPosition`, `Sprite`, `RigidBody`, `BoxCollider`, `ZIndex`, `Animation` and grouping utilities
- Resource containers: `TextureStore`, `AnimationStore`, `Camera2DRes`, `ScreenSize`, `Input` and `WorldTime`
- Systems: input handling, movement, collision detection/dispatch, animation updates, audio, render pass, and timing
- Debug utilities: debug-mode toggle, collision events, and simple on-screen diagnostics
- Packaging: no installers; runnable via `cargo run`. Release builds available with `--release`.

Not yet implemented / TODO (high level):
- Full scripting integration (rhai) and a stable scripting API
- Automated tests and CI
- Cross-platform packaging and installers (currently tested on Linux)

## Repository layout (high-level)

- `src/` — engine source
	- `main.rs`, `game.rs` — entry & setup
	- `components/` — component definitions (sprite, collider, rigidbody, etc.)
	- `resources/` — shared resources (texture/animation stores, camera, input)
	- `systems/` — game systems (render, input, movement, collision, animation, time)
	- `events/` — lightweight event types (collision, debug toggles)
- `assets/` — art, tilemaps, sounds, fonts used by examples
- `Cargo.toml`, `Cargo.lock` — Rust manifest and lockfile

## Build and run

Prerequisites:
- Rust stable (rustup recommended). The project uses standard crates and raylib bindings; on most Linux systems the `raylib-sys` crate will build the native dependency automatically.

Quick start (use your shell; example shown for fish):

```fish
cargo run
```

For a release build:

```fish
cargo run --release
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