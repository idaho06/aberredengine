# Aberred Engine

A minimal 2D game sandbox written in Rust using:
- raylib for windowing, input, and rendering
- bevy_ecs for an Entity-Component-System architecture
- rhai for future scripting

Key modules:
- components: MapPosition, Sprite, ZIndex
- resources: Camera2DRes, ScreenSize, TextureStore
- systems: render
- game::setup: loads textures, creates the camera, inserts resources, and spawns example entities

## Build and run
- Prerequisites: Rust (stable). On Linux, Raylib will build via the crate; Wayland is enabled by default.
- Run:
```bash
cargo run
```

VSync is enabled to avoid busy-waiting; the main loop renders using a 2D camera with