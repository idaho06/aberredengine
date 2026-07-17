# Aberred Engine — Rust Game Developer Guide

This guide explains how to build a 2D game in pure Rust using the Aberred Engine as a library dependency, without Lua scripting.

## 1. What the Engine Provides

Aberred Engine is a 2D game engine built on **Bevy ECS 0.19** and **sola-raylib 6.2**. It handles the main loop, windowing, rendering, and a full ECS system schedule. You supply game-specific logic via hook functions.

Built-in systems:

- **Rendering** — sprites, text, per-entity shaders, post-process shader chains, camera, letterboxing
- **Physics** — velocity, friction, max speed, named acceleration forces, freeze/unfreeze
- **Collision** — AABB detection with group-based rules and callback dispatch
- **Audio** — music and sound playback via a background thread bridge
- **Input** — keyboard polling with `just_pressed`/`just_released` tracking
- **Menus** — scrollable interactive menus with selection callbacks
- **Animation** — frame-based sprite animation with controller rules
- **Particles** — emitter system with templates, shapes, arcs, speed ranges
- **Scene management** — named scenes with enter/update/exit, GUI, and world-draw callbacks plus auto-despawn
- **Tweens** — position, rotation, and scale interpolation with easing and loop modes
- **Timers** — repeating countdown timers with function-pointer callbacks
- **Phase state machines** — per-entity state machines with enter/update/exit callbacks
- **Parent-child hierarchy** — recursive transform propagation (position, rotation, scale)

---

## 2. Project Setup

### Cargo.toml

Add the engine as a dependency with `default-features = false` to disable Lua support:

```toml
[package]
name = "my_game"
version = "0.1.0"
edition = "2024"

[dependencies]
aberredengine = { path = "../aberredengine", default-features = false }
```

For a git dependency:

```toml
[dependencies]
aberredengine = { git = "https://github.com/user/aberredengine.git", default-features = false }
```

Setting `default-features = false` disables the `lua` feature flag. This removes all mlua/LuaJIT dependencies, the Lua runtime, and all Lua-specific components and systems. Your binary will have zero Lua overhead.

### Recommended directory layout

```
my_game/
├── Cargo.toml
├── config.ini                 # Engine configuration (required at startup; missing keys use defaults)
├── src/
│   ├── main.rs                # EngineBuilder entry point
│   └── scenes/
│       ├── mod.rs             # Re-exports scene modules
│       ├── menu.rs            # Menu scene callbacks
│       └── level01.rs         # Gameplay scene callbacks
└── assets/
    ├── textures/              # PNG images
    ├── fonts/                 # TTF fonts
    ├── audio/                 # WAV/OGG sounds and music
    └── shaders/               # GLSL fragment shaders (.fs)
```

### config.ini

The engine reads `config.ini` at startup for window and rendering settings. The file must exist at startup, but individual missing or invalid keys fall back to safe defaults.

> **Alternative:** Use `EngineBuilder::config_str(content)` to supply the INI content as a `&'static str` instead of a file. This is useful for tests or games that embed their configuration. When `.config_str()` is called, the file at `.config()` path is not read.

```rust
EngineBuilder::new()
    .config_str("[render]\nwidth = 320\nheight = 180\n[window]\nwidth = 960\nheight = 540\n")
    // …
```

```ini
[render]
width = 640                    ; Internal render resolution width
height = 360                   ; Internal render resolution height
background_color = 0,2,4       ; Background clear color (R,G,B 0-255)

[window]
width = 1280                   ; Window width in pixels
height = 720                   ; Window height in pixels
target_fps = 120               ; Target frames per second
vsync = true                   ; Enable vertical sync
fullscreen = false             ; Start in fullscreen mode

[simulation]
hz = 240                       ; Logic thread's sim tick rate (see Threading Model, Section 3)
; snapshot_hz = 60              ; Optional; defaults to [window] target_fps

[audio]
hz = 100                       ; Audio thread's tick rate
```

---

## 3. EngineBuilder

The engine owns the main loop. You configure it through `EngineBuilder` and supply game logic via hook functions. There are two approaches depending on whether your game has multiple scenes.

### Threading Model: What Your Code Can Access

The engine runs **three separate ECS worlds on three threads**: render (the main thread — owns the raylib window and GPU resources), logic/sim (a spawned thread — this is where **all your game code runs**), and audio (a spawned thread, talked to via message queues you already use for sound/music). Understanding this is required to use the rest of this guide correctly.

- **Every hook and callback you write — `on_setup`, `on_enter_play`, `on_update`, scene `on_enter`/`on_update`/`on_exit`, systems added via `.add_system()`/`.configure_schedule()`, `Timer`/`Phase`/`CollisionRule` callbacks, and `.add_observer()` observers — runs on the LOGIC thread**, once per **sim tick**. Sim ticks happen at a configurable rate (`[simulation] hz` in `config.ini`, default 240) **decoupled from your render frame rate** — a sim tick is not the same thing as a rendered frame. `dt` is the real elapsed time since the previous tick (not a fixed constant), and sim ticks keep running even during a render stall.
- **`gui_callback` and `world_draw_callback` are the one exception** — they run on the RENDER thread, inside the render pass itself. This is why their signatures look different from everything else (read-only signal snapshots instead of live, mutable `WorldSignals` — see below).
- **A direct consequence**: logic-thread code (everything in the first bullet) can **never** take `RaylibAccess`, `NonSend<FontStore>`, `NonSend<ShaderStore>`, or `Res<TextureStore>` as a system parameter — those resources only exist in the render world. Requesting one from a logic-side system panics when the engine initializes its schedule. There is no escape hatch to register a custom system on the render thread. This is why asset loading (Section 4) goes through a message queue instead of calling `rl.load_texture(...)` directly inside `setup()`.

### Approach A — SceneManager (recommended for multi-scene games)

Register named scenes with enter/update/exit callbacks plus optional GUI and world-space draw callbacks. The engine handles despawning non-persistent entities on scene transitions and dispatching to the correct scene's callbacks.

```rust
use aberredengine::engine_app::EngineBuilder;
use aberredengine::systems::scene_dispatch::SceneDescriptor;

mod scenes;

fn main() -> Result<(), String> {
    EngineBuilder::new()
        .config("config.ini")
        .title("My Game")
        .on_setup(scenes::load_assets)
        .add_scene("menu", SceneDescriptor {
            on_enter:     scenes::menu::enter,
            on_update:    Some(scenes::menu::update),
            on_exit:      None,
            gui_callback: None,
            world_draw_callback: None,
        })
        .add_scene("level01", SceneDescriptor {
            on_enter:     scenes::level01::enter,
            on_update:    Some(scenes::level01::update),
            on_exit:      Some(scenes::level01::exit),
            gui_callback: None,
            world_draw_callback: None,
        })
        .initial_scene("menu")
        .try_run()
}

```

Scene callback signatures:

```rust
use aberredengine::systems::GameCtx;
use aberredengine::systems::scene_dispatch::WorldDraw;
use aberredengine::resources::appstate::AppState;
use aberredengine::resources::render::fontstore::FontStore;
use aberredengine::resources::input::InputState;
use aberredengine::resources::screensize::ScreenSize;
use aberredengine::resources::render::texturestore::TextureStore;
use aberredengine::resources::worldsignals::SignalSnapshot;
use aberredengine::resources::signal_intents::SignalIntents;
use raylib::prelude::Camera2D;

// Called once when the scene becomes active (logic thread)
fn enter(ctx: &mut GameCtx) { /* spawn entities, set signals */ }

// Called once per sim tick while the scene is active (logic thread)
fn update(ctx: &mut GameCtx, dt: f32, input: &InputState) { /* per-tick logic */ }

// Called once when leaving the scene, before entities are despawned (logic thread)
fn exit(ctx: &mut GameCtx) { /* cleanup */ }

// Called every render frame to draw ImGui widgets — Rust-only, optional, RENDER thread
// Signature must match: fn(&Ui, &SignalSnapshot, &mut SignalIntents, &TextureStore, &FontStore, &AppState)
fn my_gui(
    ui: &aberredengine::imgui::Ui,
    signals: &SignalSnapshot,
    intents: &mut SignalIntents,
    textures: &TextureStore,
    fonts: &FontStore,
    app_state: &AppState,
) { /* draw widgets, queue signal writes, read typed state */ }

// Called every render frame inside begin_mode2D in world space — Rust-only, optional, RENDER thread
// Signature must match: fn(&mut dyn WorldDraw, &Camera2D, &ScreenSize, &AppState, &SignalSnapshot)
fn my_world_draw(
    draw: &mut dyn WorldDraw,
    camera: &Camera2D,
    screen: &ScreenSize,
    app_state: &AppState,
    signals: &SignalSnapshot,
) { /* draw world overlays, read camera/screen/app state */ }
```

To trigger a scene transition from within a scene callback, set the target scene name and flag in `WorldSignals`. The engine's `scene_switch_poll` system (registered automatically by `EngineBuilder::add_scene()`) picks up the flag each frame and triggers the transition.

```rust
fn update(ctx: &mut GameCtx, _dt: f32, _input: &InputState) {
    if some_condition() {
        ctx.world_signals.set_string("scene", "level01".to_string());
        ctx.world_signals.set_flag("switch_scene");
    }
}
```

> **Tip:** Scene transitions can be triggered from callbacks by setting the `"switch_scene"` flag on `WorldSignals` — the engine polls this automatically. For menu-driven transitions, `MenuAction::SetScene` handles the switch internally. See [Section 6.2](#62-triggering-scene-transitions) for details.

### ImGui GUI callback (Rust-only)

`gui_callback` lets a scene draw an ImGui overlay every frame — useful for editors, debug tools, and dev GUIs. It runs whether or not F11 debug mode is active, inside the same ImGui frame as the debug panels.

**This callback runs on the render thread** (see [Threading Model](#threading-model-what-your-code-can-access) above) — unlike every other hook in this guide, which runs on the logic thread. That's why it can't take a live `&mut WorldSignals`: the render thread never holds one. Instead it receives a read-only snapshot and a write-queue:

- `&aberredengine::imgui::Ui` for drawing widgets
- `&SignalSnapshot` — a read-only, frame-stale-by-one-tick copy of `WorldSignals`. Read its fields **directly** (`signals.scalars.get("key")`, `signals.flags.contains("key")`, etc.) — `SignalSnapshot` has no getter methods, unlike `WorldSignals`.
- `&mut SignalIntents` — queue writes here (`intents.set_flag("key")`, `intents.set_scalar("key", 1.0)`, etc. — the method names mirror `WorldSignals`' setters). Queued writes are applied to the live `WorldSignals` on the logic thread at the start of its next sim tick — not immediately.
- `&TextureStore` for texture previews
- `&FontStore` for font access (e.g. measuring text)
- `&AppState` for richer Rust-only typed snapshots/view-models produced by systems or scene callbacks

`AppState` is inserted automatically by the engine and stores one value per Rust type. Use newtypes when you need two values of the same underlying type.

```rust
use aberredengine::imgui;
use aberredengine::resources::appstate::AppState;
use aberredengine::resources::render::fontstore::FontStore;
use aberredengine::resources::render::texturestore::TextureStore;
use aberredengine::resources::worldsignals::SignalSnapshot;
use aberredengine::resources::signal_intents::SignalIntents;

#[derive(Clone)]
struct EditorPanelState {
    active_tool: String,
}

fn editor_gui(
    ui: &imgui::Ui,
    _signals: &SignalSnapshot,
    intents: &mut SignalIntents,
    _textures: &TextureStore,
    _fonts: &FontStore,
    app_state: &AppState,
) {
    // Read typed state written by on_update or ECS systems
    let tool = app_state
        .get::<EditorPanelState>()
        .map(|s| s.active_tool.as_str())
        .unwrap_or("select");

    ui.text(format!("Active tool: {tool}"));

    if let Some(_mb) = ui.begin_main_menu_bar() {
        if let Some(_file) = ui.begin_menu("File") {
            if ui.menu_item("Save") {
                intents.set_flag("gui:action:file:save"); // consumed by on_update next sim tick
            }
        }
    }
}

fn editor_update(ctx: &mut GameCtx, _dt: f32, _input: &InputState) {
    ctx.app_state.insert(EditorPanelState {
        active_tool: "place".to_string(),
    });

    if ctx.world_signals.take_flag("gui:action:file:save") {
        // handle save
    }
}
```

Register it on the descriptor:

```rust
.add_scene("editor", SceneDescriptor {
    on_enter:     editor_enter,
    on_update:    Some(editor_update),
    on_exit:      None,
    gui_callback: Some(editor_gui),
    world_draw_callback: None,
})
```

> **Convention:** prefix all GUI signal keys with `"gui:"` to avoid collisions with game signals. Use `"gui:action:<verb>"` for flags set by the GUI and consumed by `on_update`, and `"gui:state:<name>"` for values set by `on_update` and read by the GUI.

### World-space draw callback (Rust-only)

`world_draw_callback` lets a scene draw world-space overlays every frame inside the render pass's `begin_mode2D` block. Use it for things like debug paths, editor gizmos, selection boxes, or navigation links that should follow the active camera transform.

**Like `gui_callback`, this runs on the render thread** (see [Threading Model](#threading-model-what-your-code-can-access)) — its signal parameter is a read-only `&SignalSnapshot`, not a live `&WorldSignals`.

The callback receives:

- `&mut dyn WorldDraw` with a minimal object-safe drawing API
- `&Camera2D` for the active render camera
- `&ScreenSize` for the internal game resolution
- `&AppState` for typed Rust-only snapshots
- `&SignalSnapshot` for read-only signal access (direct field access — `signals.flags.contains("key")` — no getter methods; there's no write side here, `world_draw_callback` only draws)

```rust
use aberredengine::resources::appstate::AppState;
use aberredengine::resources::screensize::ScreenSize;
use aberredengine::resources::worldsignals::SignalSnapshot;
use aberredengine::systems::scene_dispatch::WorldDraw;
use raylib::prelude::{Camera2D, Color, Vector2};

fn editor_world_draw(
    draw: &mut dyn WorldDraw,
    _camera: &Camera2D,
    _screen: &ScreenSize,
    _app_state: &AppState,
    _signals: &SignalSnapshot,
) {
    draw.draw_line_v(
        Vector2::new(-32.0, 0.0),
        Vector2::new(32.0, 0.0),
        Color::GREEN,
    );
    draw.draw_line(-16, -16, 16, 16, Color::YELLOW);
}
```

Register it on the descriptor:

```rust
.add_scene("editor", SceneDescriptor {
    on_enter:            editor_enter,
    on_update:           Some(editor_update),
    on_exit:             None,
    gui_callback:        Some(editor_gui),
    world_draw_callback: Some(editor_world_draw),
})
```

### Approach B — Raw hooks (single-scene or full manual control)

For single-scene games or when you need full control over scene transitions, use the four hook methods directly:

```rust
use aberredengine::engine_app::EngineBuilder;

fn main() -> Result<(), String> {
    EngineBuilder::new()
        .config("config.ini")
        .title("My Game")
        .on_setup(my_setup)
        .on_enter_play(my_enter_play)
        .on_update(my_update)
        .on_switch_scene(my_switch_scene)
        .try_run()
}
```

### Startup error handling

Prefer `EngineBuilder::try_run()` in Rust applications. It returns `Result<(), String>` for startup failures such as invalid builder configuration, missing `config.ini`, render-target creation failures, Lua runtime creation failures, and missing required built-in system registrations.

`EngineBuilder::run()` is still available as a convenience wrapper, but it only logs startup failures internally and does not return them to your `main` function.

Each hook is a standard Bevy ECS system — it receives queries and resources as parameters. For example:

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::resources::input::InputState;

fn my_update(signals: ResMut<WorldSignals>, input: Res<InputState>) {
    if input.action_1.just_pressed {
        // ...
    }
}
```

### Game lifecycle

```
Setup ──→ Playing ──→ Quitting
              ↑   ↓
          scene switches
```

1. **Setup** — The engine calls the `setup` hook once, on the logic thread. Load assets here (textures, fonts, sounds, shaders, animations) — see [Section 4](#4-loading-assets) for how texture/font/shader loading works.
2. **Playing** — The engine transitions to playing, calls `enter_play` (or the initial scene's `on_enter`), then runs `update` (or `on_update`) once per sim tick.
3. **Scene switches** — When `WorldSignals` has the `"switch_scene"` flag set, the engine calls the `switch_scene` hook (or the SceneManager's exit→enter sequence).
4. **Quitting** — When `WorldSignals` has the `"quit_game"` flag set or the window is closed, the engine shuts down.

### Builder method reference

| Method | Description |
|--------|-------------|
| `.config(path)` | Path to `config.ini` (default: `"config.ini"`) |
| `.config_str(content)` | Load INI config from an embedded `&'static str` instead of a file. Takes precedence over `.config(path)`. Useful for tests or games that ship with bundled defaults. |
| `.title(name)` | Window title (overrides config) |
| `.on_setup(system)` | Asset loading hook (called during `Setup` state, on the logic thread) |
| `.on_enter_play(system)` | Called once when transitioning to `Playing` |
| `.on_update(system)` | Runs once per sim tick while `Playing` — single system only. A sim tick is not the same as a render frame; see [Threading Model](#threading-model-what-your-code-can-access). |
| `.on_switch_scene(system)` | Called when a scene transition is requested |
| `.add_scene(name, descriptor)` | Register a named scene (SceneManager path) |
| `.initial_scene(name)` | Which scene starts first (required with `.add_scene()`) |
| `.add_system(system)` | Add an extra per-sim-tick system. Same auto-constraints as `.on_update()` (`run_if(state_is_playing)`, ordered alongside script-update systems). Can be called multiple times. |
| `.configure_schedule(closure)` | Add systems to the same schedule `.add_system()` targets, with full ordering control — no auto-constraints applied. Use this for custom ordering relative to the engine's own systems (via `SimSet` or `.after()`/`.before()`). |
| `.add_observer(observer_fn)` | Register a persistent observer for a custom or engine event. |
| `.try_run()` | Start the engine and return `Result<(), String>` on startup failure. Recommended for Rust `main`. |
| `.run()` | Convenience wrapper around `.try_run()` that logs startup failures and returns `()`. |

**Conflict rules:** `.add_scene()` cannot be combined with `.on_switch_scene()`, `.on_enter_play()`, or `.with_lua()` — the SceneManager owns those hooks, and a Lua game drives scenes from `main.lua`'s scene registry instead. `.add_scene()` also requires `.initial_scene(...)`, and that name must match a scene actually registered via `.add_scene()` — a missing or misspelled `.initial_scene(...)` is a startup error, and so is calling `.initial_scene(...)` with no `.add_scene()` calls at all. `.with_lua()` also conflicts with any explicit `.on_setup()`/`.on_enter_play()`/`.on_update()`/`.on_switch_scene()` call, in either order — it installs its own four hooks, and mixing in your own is ambiguous. Use `.on_setup()` for asset loading in the SceneManager approach. With `.try_run()`, all of these are returned as startup errors instead of panicking; `.run()` prints the error to stderr and exits with a nonzero status instead of failing silently.

### Custom systems and observers

These builder methods let you register multiple independent ECS systems and event observers alongside the existing hooks. **All of them run on the logic thread**, on the same schedule as every other per-tick system in this guide — see [Threading Model](#threading-model-what-your-code-can-access). None of them can take `RaylibAccess`/`NonSend<FontStore>`/`NonSend<ShaderStore>`/`Res<TextureStore>`; that always panics at schedule-init time regardless of which of these methods registered the system.

#### `.add_system(system)` — multiple per-sim-tick systems

Registers a Bevy ECS system that runs once per sim tick while `Playing`. Same automatic constraints as `.on_update()`: `run_if(state_is_playing)`, ordered alongside the engine's own script-update systems. Can be called multiple times.

```rust
EngineBuilder::new()
    .config("config.ini")
    .on_setup(load_assets)
    .add_system(tilemap_load_system)   // checks a signal each tick, then queues a load
    .add_system(tilemap_save_system)   // independent second system
    .add_scene("editor", /* … */)
    .initial_scene("editor")
    .try_run()
    .expect("engine startup failed");
```

The system signature is a standard Bevy ECS system. Since it runs on the logic thread, it cannot touch GL resources directly — queue a `RenderAssetCmd` instead (see [Section 4](#4-loading-assets)):

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::events::render_assets::RenderAssetCmd;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::resources::texturefilter::TextureFilter;

fn tilemap_load_system(
    mut world_signals: ResMut<WorldSignals>,
    mut asset_cmds: MessageWriter<RenderAssetCmd>,
) {
    let Some(path) = world_signals.remove_string("pending_load_path") else {
        return; // nothing to do this tick
    };
    asset_cmds.write(RenderAssetCmd::Texture {
        id: path.clone(),
        path,
        filter: TextureFilter::Nearest,
    });
    // spawn tile entities, etc. — the texture itself loads asynchronously
}
```

> **When to use `.add_system()` vs scene callbacks:** Scene callbacks (`on_enter`, `on_update`) receive `&mut GameCtx`, which already covers most per-tick needs (commands, common queries, `WorldSignals`, `AppState`, audio). Reach for `.add_system()` when you need ECS system params `GameCtx` doesn't expose (arbitrary `Query`s, `MessageWriter<RenderAssetCmd>`, etc.), or when the logic should run independent of which scene is active.

#### `.configure_schedule(closure)` — full ordering control

For systems that need custom ordering relative to engine systems, or that must run outside the `Playing` state, pass a closure receiving `&mut Schedule`. This targets the exact same schedule as `.add_system()` — all logic-thread, once per sim tick:

```rust
use aberredengine::systems::movement::movement;
use aberredengine::systems::camera_follow::camera_follow_system;

EngineBuilder::new()
    .configure_schedule(|schedule| {
        schedule.add_systems(
            undo_system
                .run_if(state_is_playing)
                .after(movement)
                .before(camera_follow_system),
        );
    })
    // …
    .try_run()
    .expect("engine startup failed");
```

Engine system functions are `pub` and importable from `aberredengine::systems::*`. Use them directly as `.after()` / `.before()` arguments — but only systems that live on this same logic-thread schedule; render-thread systems like `render_system` are never reachable from here, ordering relative to them is not expressible. No automatic `run_if` or `after` constraints are applied — you control everything.

#### `.add_observer(observer_fn)` — persistent event observers

Registers a Bevy ECS observer that fires when a specific event is triggered. The observer survives scene transitions (spawned with the `Persistent` component) — it is always active, not tied to a specific scene.

**Define a custom event:**

```rust
use aberredengine::bevy_ecs;
use aberredengine::bevy_ecs::prelude::Event;

#[derive(Event)]
struct TilemapLoaded {
    pub path: String,
}
```

If you derive `Event` in a downstream game crate, bring the re-exported crate itself into scope as `bevy_ecs` first. The derive macro expands using a `bevy_ecs::...` path, so importing only items from `aberredengine::bevy_ecs::prelude` is not enough.

**Define the observer function** — first parameter must be `On<E>`:

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::bevy_ecs::observer::On;

fn on_tilemap_loaded(
    trigger: On<TilemapLoaded>,
    mut world_signals: ResMut<WorldSignals>,
) {
    let path = &trigger.event().path;
    world_signals.set_string("last_loaded_tilemap", path.clone());
    log::info!("Tilemap loaded: {}", path);
}
```

**Register it with the builder:**

```rust
EngineBuilder::new()
    .add_observer(on_tilemap_loaded)
    // …
    .try_run()
    .expect("engine startup failed");
```

**Trigger the event from any system or scene callback:**

```rust
// From a Bevy ECS system:
fn my_system(mut commands: Commands) {
    commands.trigger(TilemapLoaded { path: "maps/level01.json".into() });
}

// From a scene callback (via GameCtx):
fn my_enter(ctx: &mut GameCtx) {
    ctx.commands.trigger(TilemapLoaded { path: "maps/intro.json".into() });
}
```

> You can also observe engine-defined events: `CollisionEvent`, `TimerEvent`, `InputEvent`, `GameStateChangedEvent`, `WindowResizedEvent`, `AudioCmd`, etc.

#### Scene-scoped (transient) observers

Observers registered with `.add_observer()` are always active. For observers that should only fire within a specific scene, spawn them from the scene's `on_enter` callback **without** the `Persistent` component:

```rust
use aberredengine::bevy_ecs::observer::Observer;

fn editor_enter(ctx: &mut GameCtx) {
    // This observer lives only until the next scene switch.
    // clean_all_entities (called on scene transition) despawns it automatically.
    ctx.commands.spawn(Observer::new(on_tile_selected));
}

fn on_tile_selected(trigger: On<TileSelectedEvent>, /* params */) {
    // only fires while the editor scene is active
}
```

This is the standard pattern for scene-scoped behaviour in the engine — no special API needed.

---

## 4. Loading Assets

The setup hook is a standard Bevy ECS system running on the **logic thread** (see [Threading Model](#threading-model-what-your-code-can-access)). That means it **cannot** take `RaylibAccess`, `NonSendMut<FontStore>`, `NonSendMut<ShaderStore>`, or `ResMut<TextureStore>` — those resources exist only in the render world, on the render thread. Requesting any of them from `setup()` (or any other logic-side system) panics when the engine builds its schedule.

Instead, texture/font/shader loading is **queued** from the logic thread and **performed** on the render thread: take `MessageWriter<RenderAssetCmd>` and write a `RenderAssetCmd` variant. The render thread's `process_render_asset_cmds` system drains the queue, does the actual GL load, and reports back — so the load itself is asynchronous relative to the tick that requested it.

```rust
use aberredengine::events::render_assets::RenderAssetCmd;
use aberredengine::resources::animationstore::{AnimationStore, AnimationResource};
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::raylib::prelude::*;
use aberredengine::protocol::audio::AudioCmd;
use std::sync::Arc;

fn setup(
    mut next_state: ResMut<NextGameState>,
    mut anim_store: ResMut<AnimationStore>,
    mut asset_cmds: MessageWriter<RenderAssetCmd>,
    mut audio: MessageWriter<AudioCmd>,
) {
    // ... queue asset loads here (see subsections below) ...
}
```

Audio loading uses the same message-queue pattern (`MessageWriter<AudioCmd>`) — see the Audio subsection below.

### What's pre-inserted vs. what you must create

| Resource | Where it lives | Accessible from your logic-side code? |
|----------|-----------------|----------------------------------------|
| `FontStore` / `ShaderStore` / `TextureStore` / `RenderTarget` | Render world only | **No** — request `MessageWriter<RenderAssetCmd>` instead |
| `FontMetricsStore` / `TextureDimsStore` | Logic world, pre-inserted | Yes — `Res<FontMetricsStore>` / `Res<TextureDimsStore>`; populated asynchronously after a load completes (see below) |
| `AnimationStore` | Logic world, pre-inserted | Yes — `ResMut<AnimationStore>` |
| `Camera2DRes` | Logic world, pre-inserted (pre-set to center offset) | Yes — `ResMut<Camera2DRes>` |

### Textures

Queue a load with `RenderAssetCmd::Texture`, passing the desired `TextureFilter` (use `TextureFilter::Nearest` for pixel art, `Bilinear`/`Trilinear`/`Anisotropic*` for smoothly scaled/rotated sprites):

```rust
use aberredengine::events::render_assets::RenderAssetCmd;
use aberredengine::resources::texturefilter::TextureFilter;

asset_cmds.write(RenderAssetCmd::Texture {
    id: "player".to_string(),
    path: "assets/textures/player.png".to_string(),
    filter: TextureFilter::Nearest,
});
asset_cmds.write(RenderAssetCmd::Texture {
    id: "background".to_string(),
    path: "assets/textures/background.png".to_string(),
    filter: TextureFilter::Nearest,
});
```

Keys (`id`) are arbitrary strings you'll reference later in `Sprite` components — you can spawn a `Sprite` referencing `"player"` in the very same tick that queued the load; it just won't have anything to draw until the render thread's GL upload lands (typically the next tick or two).

**The load-then-use gap:** if your own logic-side code needs the texture's *dimensions* before the render thread has replied (e.g. to size a collider off a spritesheet), you can't just call `.insert()` and read it back synchronously anymore. Query `TextureDimsStore` instead, and tolerate `None` until the reply arrives:

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::sprite::Sprite;
use aberredengine::components::zindex::ZIndex;
use aberredengine::raylib::prelude::Vector2;
use aberredengine::resources::texturedims::TextureDimsStore;
use aberredengine::resources::worldsignals::WorldSignals;
use std::sync::Arc;

fn spawn_player_once_texture_ready(
    dims: Res<TextureDimsStore>,
    mut world_signals: ResMut<WorldSignals>,
    mut commands: Commands,
) {
    if world_signals.has_flag("player_spawned") {
        return;
    }
    // `.get()` returns None until the render thread's TextureLoaded reply
    // lands — usually a tick or two after the RenderAssetCmd::Texture was
    // queued, never in the same tick.
    let Some((width, height)) = dims.get("player") else {
        return; // still waiting — try again next tick
    };

    commands.spawn((
        MapPosition::new(100.0, 200.0),
        Sprite {
            tex_key: Arc::from("player"),
            width: width as f32,
            height: height as f32,
            offset: Vector2::zero(),
            origin: Vector2 { x: width as f32 * 0.5, y: height as f32 * 0.5 },
            flip_h: false,
            flip_v: false,
        },
        ZIndex(1.0),
    ));
    world_signals.set_flag("player_spawned");
}
```

Register this as an extra system (`.add_system(spawn_player_once_texture_ready)`) — it no-ops every tick until the texture is ready, then spawns once and stops. If you don't need the exact pixel dimensions (e.g. you already know them, or don't need a pixel-perfect collider), you can just spawn the `Sprite` with hardcoded `width`/`height` immediately and skip `TextureDimsStore` entirely — the render thread will draw it correctly as soon as the GL upload lands, with no gap-handling needed on your side.

`TextureFilter::ALL` lists all six filter variants (useful for building filter pickers); `TextureFilter::as_str()`/`FromStr` round-trip a filter to/from its config string (`"nearest"`, `"bilinear"`, etc.).

To load a texture from an in-memory-encoded buffer (e.g. a PNG embedded in your binary via `include_bytes!`) instead of a file path, use `RenderAssetCmd::TextureFromMemory`:

```rust
asset_cmds.write(RenderAssetCmd::TextureFromMemory {
    id: "intro_logo".to_string(),
    ext: ".png".to_string(), // leading dot, matches raylib's file-type hint
    bytes: include_bytes!("../assets/textures/intro_logo.png").to_vec(),
    filter: TextureFilter::Nearest,
});
```

Same `LogicMsg::TextureLoaded` reply and `TextureDimsStore` population as the path-based `Texture` command — the load-then-use gap and dimension-query pattern above apply identically.

To remove a previously-loaded texture (dropping its GPU handle), use `RenderAssetCmd::RemoveTexture { key }`. The render thread also reports `LogicMsg::TextureRemoved { key }`, which prunes the logic-side `TextureDimsStore` entry automatically — no manual cleanup needed on your side.

To rename an already-loaded texture's key or change its sampling filter without reloading from disk, use `RenderAssetCmd::RenameTexture { old_key, new_key }` / `RenderAssetCmd::SetTextureFilter { key, filter }` — both are in-place `TextureStore` mutations (a key move / a single `SetTextureFilter` GL call), not a fresh load. `RenameTexture` reports `LogicMsg::TextureRenamed { old_key, new_key }`, which moves the `TextureDimsStore` entry to the new key automatically; `SetTextureFilter` has no logic-side effect to mirror. Both no-op with a warning if the key isn't loaded.

### Fonts

Queue a load with `RenderAssetCmd::Font`. Mipmap generation is handled internally by the render thread's loader (`process_render_asset_cmds`) for every font it loads — you don't need to do anything extra for smooth scaled/rotated text.

```rust
asset_cmds.write(RenderAssetCmd::Font {
    id: "arcade".to_string(),
    path: "assets/fonts/arcade.ttf".to_string(),
    size: 32,
    skip_if_loaded: false, // true = don't reload if "arcade" is already loaded
});
```

To measure text (e.g. to size UI elements) without touching the render-world `FontStore`, read `FontMetricsStore` — populated asynchronously the same way `TextureDimsStore` is, so tolerate a missing key the first tick or two after queuing the load:

```rust
use aberredengine::resources::fontmetrics::FontMetricsStore;

fn measure_label(fonts: Res<FontMetricsStore>) {
    if let Some(metrics) = fonts.0.get("arcade") {
        let size = metrics.measure_text("Hello!", 32.0, 1.0);
        // ... use size.x / size.y ...
    }
}
```

To remove a previously-loaded font (dropping its GPU handle and `FontStore` metadata), use `RenderAssetCmd::RemoveFont { key }`. The render thread also reports `LogicMsg::FontRemoved { key }`, which prunes the logic-side `FontMetricsStore` entry automatically.

To rename an already-loaded font's key in place (no reload, no glyph-atlas regeneration), use `RenderAssetCmd::RenameFont { old_key, new_key }`. It reports `LogicMsg::FontRenamed { old_key, new_key }`, which moves the `FontMetricsStore` entry to the new key automatically. No-ops with a warning if `old_key` isn't loaded.

### Audio (sounds and music)

Audio is loaded asynchronously via the `MessageWriter<AudioCmd>` channel. The audio thread processes commands in the background:

```rust
// Load a sound effect
audio.write(AudioCmd::LoadFx {
    id: "jump".to_string(),
    path: "assets/audio/jump.wav".to_string(),
});

// Load background music
audio.write(AudioCmd::LoadMusic {
    id: "bgm".to_string(),
    path: "assets/audio/music.ogg".to_string(),
});
```

Sounds and music are played later via the same channel (e.g., `AudioCmd::PlayFx { id: "jump".into() }`). See the `AudioCmd` enum for the full command set: `PlayMusic`, `StopMusic`, `PauseMusic`, `ResumeMusic`, `VolumeMusic`, `PlayFxPitched`, etc.

### Shaders

Queue a load with `RenderAssetCmd::Shader`. `vs_path`/`fs_path` mirror raylib's `load_shader(vertex, fragment)` — pass `None` for a path to use raylib's default for that stage:

```rust
asset_cmds.write(RenderAssetCmd::Shader {
    id: "glow".to_string(),
    vs_path: None, // default vertex shader
    fs_path: Some("assets/shaders/glow.fs".to_string()),
});
```

There's no synchronous "did it load, is it valid" result available to logic-side code — the render thread's loader logs an error and simply doesn't register the shader if the file is missing or fails validation. Reference the shader by its `id` key from an `EntityShader` component as usual; a failed load just means nothing renders through that shader key.

To load a shader from in-memory source strings instead of file paths (e.g. shaders embedded via `include_str!`), use `RenderAssetCmd::ShaderFromMemory`:

```rust
asset_cmds.write(RenderAssetCmd::ShaderFromMemory {
    id: "glitch".to_string(),
    vs_src: None, // default vertex shader
    fs_src: Some(include_str!("../assets/shaders/glitch.fs").to_string()),
});
```

Same `None`-per-stage convention as `RenderAssetCmd::Shader`, and the same "no synchronous success/failure signal" caveat applies — a failed compile just logs an error and the shader key stays unregistered.

### Animations

Animations are pure data — no Raylib calls needed. `AnimationStore` is pre-inserted by the engine. Request it as `ResMut<AnimationStore>` and populate it with `AnimationResource` entries:

```rust
anim_store.animations.insert("player_idle".to_string(), AnimationResource {
    tex_key: Arc::from("player"),              // must match a TextureStore key
    position: Vector2 { x: 0.0, y: 0.0},      // base offset in spritesheet
    horizontal_displacement: 32.0,             // per-frame X step (= frame width)
    vertical_displacement: 0.0,                // non-zero enables row-wrapping
    frame_count: 4,                            // number of frames
    fps: 8.0,                                  // playback speed
    looped: true,                              // restart after last frame
});

anim_store.animations.insert("player_run".to_string(), AnimationResource {
    tex_key: Arc::from("player"),
    position: Vector2 { x: 0.0, y: 64.0 },    // second row of spritesheet
    horizontal_displacement: 32.0,
    vertical_displacement: 0.0,
    frame_count: 6,
    fps: 12.0,
    looped: true,
});
```

### Tilemaps

Tilemaps use the **Tilesetter 2.1.0** export format: a directory containing a `.png` tileset texture and a `.txt` JSON data file, both named after the directory.

```
assets/tilemaps/level01/
├── level01.png    # tileset texture (atlas)
└── level01.txt    # JSON: { tile_size, map_width, map_height, layers: [{ name, positions: [{ x, y, id }] }] }
```

Spawn a tilemap by attaching the `TileMap` component to any entity. `tilemap_spawn_system` reacts to `Added<TileMap>`, loads the PNG + JSON from disk, and spawns all tile entities as `ChildOf` children of the root entity. The entire tilemap then moves, scales, and rotates as one unit:

```rust
use aberredengine::components::tilemap::TileMap;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::scale::Scale;

// Minimal — tiles appear at world origin (default MapPosition inserted automatically)
commands.spawn(TileMap::new("assets/tilemaps/level01"));

// Positioned and scaled
commands.spawn((
    TileMap::new("assets/tilemaps/level01"),
    MapPosition::new(100.0, 200.0),
    Scale::new(2.0, 2.0),
));
```

Move the whole tilemap at runtime by updating the root entity's `MapPosition`:

```rust
commands.entity(tilemap_root).insert(MapPosition::new(new_x, new_y));
```

From **Lua**, use the entity builder:

```lua
engine.spawn()
    :with_tilemap("./assets/tilemaps/level01")
    :with_position(0, 0)  -- optional, defaults to (0, 0)
    :build()
```

The texture is stored in `TextureStore` keyed by path stem and deduplicated — two `TileMap` entities pointing to the same directory share one GPU texture. Tile entities are in `Group("tiles")` and get `ZIndex` values automatically based on layer order (first layer most negative, last layer least negative). Use `Group("tiles")` in collision rules to match tile entities.

> **Note:** `load_tilemap` and `spawn_tiles` remain available as low-level utilities for advanced use cases where manual control of the load/spawn cycle is needed.

### Camera

`Camera2DRes` is pre-inserted by the engine with `target` at the origin and `offset` at half the render resolution (center-screen). If you need a different initial position, request `ResMut<Camera2DRes>` and overwrite it — use `ScreenSize` (a logic-side resource) rather than a live raylib handle for the resolution, since `RaylibAccess` isn't available here:

```rust
use aberredengine::resources::screensize::ScreenSize;

fn setup_camera(mut camera: ResMut<Camera2DRes>, screen: Res<ScreenSize>) {
    camera.0 = Camera2D {
        target: Vector2 { x: 0.0, y: 0.0 },
        offset: Vector2 {
            x: screen.w as f32 * 0.5,
            y: screen.h as f32 * 0.5,
        },
        rotation: 0.0,
        zoom: 1.0,
    };
}
```

`offset` is the screen point the camera looks through. `target` is the world position it looks at.

### Complete setup example

```rust
fn setup(
    mut next_state: ResMut<NextGameState>,
    mut anim_store: ResMut<AnimationStore>,
    mut asset_cmds: MessageWriter<RenderAssetCmd>,
    mut audio: MessageWriter<AudioCmd>,
) {
    // Textures — queued, loaded asynchronously on the render thread
    asset_cmds.write(RenderAssetCmd::Texture {
        id: "player".to_string(),
        path: "assets/textures/player.png".to_string(),
        filter: TextureFilter::Nearest,
    });

    // Fonts — mipmap generation is handled internally by the render thread
    asset_cmds.write(RenderAssetCmd::Font {
        id: "arcade".to_string(),
        path: "assets/fonts/arcade.ttf".to_string(),
        size: 32,
        skip_if_loaded: false,
    });

    // Audio — same message-queue pattern
    audio.write(AudioCmd::LoadFx { id: "jump".into(), path: "assets/audio/jump.wav".into() });
    audio.write(AudioCmd::LoadMusic { id: "bgm".into(), path: "assets/audio/music.ogg".into() });

    // Shaders
    asset_cmds.write(RenderAssetCmd::Shader {
        id: "glow".to_string(),
        vs_path: None,
        fs_path: Some("assets/shaders/glow.fs".to_string()),
    });

    // Animations (AnimationStore is pre-inserted, logic-owned — just populate it)
    anim_store.animations.insert("player_idle".into(), AnimationResource {
        tex_key: Arc::from("player"),
        position: Vector2 { x: 0.0, y: 0.0 },
        horizontal_displacement: 32.0,
        vertical_displacement: 0.0,
        frame_count: 4,
        fps: 8.0,
        looped: true,
    });

    // Transition to Playing state — required, or the game stays in Setup forever
    next_state.set(GameStates::Playing);
}
```

Entities that reference `"player"`/`"arcade"`/`"glow"` can be spawned right away in `on_enter_play`/the initial scene's `on_enter` — they just won't render anything until the render thread's uploads land a tick or two later. Use the `TextureDimsStore`/`FontMetricsStore` pattern from the Textures/Fonts subsections above only if your own logic needs the actual dimensions/metrics before then.

---

## 5. Spawning Entities

Entities are spawned with `commands.spawn((component_tuple))` — the standard Bevy ECS pattern. You build entities by composing components as a tuple.

### Example 1: Sprite entity

A minimal visible entity needs a position, a sprite, a draw order, and optionally a group:

```rust
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::sprite::Sprite;
use aberredengine::components::zindex::ZIndex;
use aberredengine::components::group::Group;
use aberredengine::raylib::prelude::*;
use std::sync::Arc;

ctx.commands.spawn((
    MapPosition::new(100.0, 200.0),
    Sprite {
        tex_key: Arc::from("player"),
        width: 32.0,
        height: 32.0,
        offset: Vector2::zero(),
        origin: Vector2 { x: 16.0, y: 16.0 }, // center pivot
        flip_h: false,
        flip_v: false,
    },
    ZIndex(1.0),
    Group::new("player"),
));
```

### Example 2: Physics entity

Add `RigidBody`, `BoxCollider`, and `AccelerationControlled` for a player character with momentum-based movement:

```rust
use aberredengine::components::rigidbody::RigidBody;
use aberredengine::components::boxcollider::BoxCollider;
use aberredengine::components::inputcontrolled::AccelerationControlled;

ctx.commands.spawn((
    MapPosition::new(100.0, 200.0),
    Sprite {
        tex_key: Arc::from("player"),
        width: 32.0,
        height: 32.0,
        offset: Vector2::zero(),
        origin: Vector2 { x: 16.0, y: 16.0 },
        flip_h: false,
        flip_v: false,
    },
    ZIndex(1.0),
    Group::new("player"),
    RigidBody::with_physics(5.0, Some(300.0)),  // friction=5.0, max_speed=300
    BoxCollider::new(28.0, 30.0)
        .with_origin(Vector2 { x: 16.0, y: 16.0 })
        .with_offset(Vector2 { x: 2.0, y: 2.0 }),
    AccelerationControlled::symmetric(800.0),    // 800 units/s² in all directions
));
```

### Example 3: UI text with signal binding

Screen-space text that auto-updates from `WorldSignals`:

```rust
use aberredengine::components::screenposition::ScreenPosition;
use aberredengine::components::dynamictext::DynamicText;
use aberredengine::components::signalbinding::SignalBinding;

ctx.commands.spawn((
    ScreenPosition::new(10.0, 10.0),
    DynamicText::new("0", "arcade", 16.0, Color::WHITE),
    SignalBinding::new("score").with_format("Score: {}"),
    ZIndex(100.0),
));
```

When `WorldSignals` has a value for key `"score"`, the text automatically updates to `"Score: 42"` (or whatever the value is).

### Component constructor quick reference

| Component | Constructor |
|-----------|-------------|
| `MapPosition` | `MapPosition::new(x, y)` |
| `ScreenPosition` | `ScreenPosition::new(x, y)` |
| `Sprite` | `Sprite { tex_key: Arc::from("key"), width, height, offset, origin, flip_h, flip_v }` |
| `RigidBody` | `RigidBody::new()` or `RigidBody::with_physics(friction, max_speed)` |
| `BoxCollider` | `BoxCollider::new(w, h).with_origin(v).with_offset(v)` |
| `Animation` | `Animation::new("anim_key")` |
| `AnimationController` | `AnimationController::new("fallback_key").with_rule(condition, "key")` |
| `Group` | `Group::new("name")` |
| `ZIndex` | `ZIndex(f32)` |
| `Rotation` | `Rotation { degrees: f32 }` |
| `Scale` | `Scale::new(sx, sy)` |
| `Tint` | `Tint::new(r, g, b, a)` — values are `u8` (0–255) |
| `Persistent` | `Persistent` — tag, survives scene transitions |
| `Ttl` | `Ttl::new(seconds)` — auto-despawn after duration |
| `DynamicText` | `DynamicText::new(text, font_key, size, color)` |
| `SignalBinding` | `SignalBinding::new("key").with_format("Score: {}")` |
| `Signals` | `Signals::default()` — per-entity signal bag |
| `InputControlled` | `InputControlled { up_velocity, down_velocity, left_velocity, right_velocity }` |
| `AccelerationControlled` | `AccelerationControlled::symmetric(accel)` |
| `MouseControlled` | `MouseControlled { follow_x: true, follow_y: true }` |
| `Timer` | `Timer::rust(duration_secs, callback)` — use `::rust()` for Rust callbacks; see §7.1 |
| `Phase` | `Phase::new("initial_phase", phases)` where `phases: FxHashMap<String, PhaseCallbackFns>` |
| `CollisionRule` | `CollisionRule::rust("group_a", "group_b", callback)` — use `::rust()` for Rust callbacks; see §7.3 |
| `LuaOnAnimationEnd` | `LuaOnAnimationEnd::new("fn_name")` — `[feature=lua]`; fires once when non-looped animation finishes; Rust counterpart is `AnimationFinishedEvent` |
| `Tween<MapPosition>` | `Tween::new(MapPosition::from_vec(from), MapPosition::from_vec(to), duration)` |
| `Tween<Rotation>` | `Tween::new(Rotation { degrees: from }, Rotation { degrees: to }, duration)` |
| `Tween<Scale>` | `Tween::new(Scale::new(from_x, from_y), Scale::new(to_x, to_y), duration)` |
| `Tween<ScreenPosition>` | `Tween::new(ScreenPosition::new(from_x, from_y), ScreenPosition::new(to_x, to_y), duration)` |
| `GuiWindow` | `GuiWindow::new(w, h)` — `theme_key` defaults to `"default"`; override with `.with_theme_key("my_theme")` |
| `GuiButton` | `GuiButton::new(width, height, "Caption")` — add `.with_theme_key(key)` to use a non-default theme |
| `GuiLabel` | `GuiLabel::new(width, height, "Text")` — add `.with_signal_binding(key)` / `.with_signal_binding_format("fmt {}") ` to bind text to `WorldSignals` |
| `GuiImage` | `GuiImage::new(width, height, "tex_key", offset_x, offset_y)` — add `.with_offset_hover(x, y)` / `.with_offset_pressed(x, y)` / `.with_offset_disabled(x, y)` for per-state atlas offsets |
| `GuiProgressBar` | `GuiProgressBar::new(w, h, value, max)` — add `.with_direction(ProgressBarDirection)`, `.with_signal_binding(key)`, `.with_theme_key(key)`; requires `ScreenPosition` + `ZIndex` |
| `Shadow` | `Shadow::new(dx, dy, r, g, b, a)` or `Shadow::default_color(dx, dy)` — pre-pass shadow for `Sprite` and `DynamicText` entities; see §7.7 |
| `GuiInteractable` | `GuiInteractable::rust(width, height, callback)` — use `::rust()` for Rust callbacks; see §7.6 |
| `GuiOffset` | `GuiOffset(Vector2::new(x, y))` — position relative to a `ChildOf` parent |

### Tween components in Rust

Tweens are represented by a single generic component: `Tween<T>`.

Use the target component type as `T`:

- `Tween<MapPosition>` for position animation
- `Tween<Rotation>` for rotation animation
- `Tween<Scale>` for scale animation
- `Tween<ScreenPosition>` for screen-space (UI) position animation

`EngineBuilder` registers the built-in tween systems for these four component types automatically, so in normal game code you only need to spawn the tween component itself.

**Position tween example:**

```rust
use aberredengine::components::mapposition::MapPosition;
use aberredengine::components::tween::{Easing, LoopMode, Tween};
use aberredengine::raylib::prelude::Vector2;

ctx.commands.spawn((
    MapPosition::new(0.0, 0.0),
    Tween::new(
        MapPosition::from_vec(Vector2 { x: 0.0, y: 0.0 }),
        MapPosition::from_vec(Vector2 { x: 200.0, y: 120.0 }),
        1.5,
    )
    .with_easing(Easing::CubicOut)
    .with_loop_mode(LoopMode::PingPong),
));
```

**Rotation tween example:**

```rust
use aberredengine::components::rotation::Rotation;
use aberredengine::components::tween::Tween;

ctx.commands.spawn((
    Rotation { degrees: 0.0 },
    Tween::new(
        Rotation { degrees: 0.0 },
        Rotation { degrees: 360.0 },
        2.0,
    ),
));
```

**Scale tween example:**

```rust
use aberredengine::components::scale::Scale;
use aberredengine::components::tween::Tween;

ctx.commands.spawn((
    Scale::new(1.0, 1.0),
    Tween::new(
        Scale::new(1.0, 1.0),
        Scale::new(1.5, 0.75),
        0.75,
    )
    .with_backwards(),
));
```

**Screen-position tween example (UI):**

```rust
use aberredengine::components::screenposition::ScreenPosition;
use aberredengine::components::tween::Tween;

ctx.commands.spawn((
    ScreenPosition::new(-200.0, 50.0),
    Tween::new(
        ScreenPosition::new(-200.0, 50.0),
        ScreenPosition::new(20.0, 50.0),
        0.4,
    ),
));
```

> **Important:** The generic parameter must match the component you want the engine to animate. For example, use `Tween<MapPosition>` with `MapPosition`, not `Tween<Vector2>`. The tween systems query concrete ECS component types, not raw value types.

### Spawning context: GameCtx vs. raw hooks

In **scene callbacks**, use `ctx.commands` to spawn entities:

```rust
fn enter(ctx: &mut GameCtx) {
    ctx.commands.spawn(( /* ... */ ));
}
```

In **raw hooks**, use `Commands` as a system parameter directly:

```rust
fn my_enter_play(mut commands: Commands) {
    commands.spawn(( /* ... */ ));
}
```

Both are standard Bevy `Commands` — the API is identical.

---

## 6. Scene Management Deep Dive

Section 3 introduced `SceneManager` at the API level. This section covers internals and practical patterns.

### 6.1 What happens during a scene switch

When the `scene_switch_system` runs, it performs these steps in order:

1. **Despawn non-persistent entities** — every entity *without* the `Persistent` component is despawned
2. **Clear entity registrations** — non-persistent entity refs stored in `WorldSignals` are removed
3. **Clear group tracking** — `TrackedGroups::clear()` and `WorldSignals` group counts are wiped
4. **Read target scene** — reads `WorldSignals["scene"]` for the target scene name (defaults to `"menu"` if unset)
5. **Call `on_exit` on previous scene** — if there was an active scene with an `on_exit` callback, it fires
6. **Write `previous_scene`** — the old active scene name is stored in `WorldSignals["previous_scene"]`
7. **Set active scene** — updates `SceneManager.active_scene` to the new scene name
8. **Call `on_enter` on new scene** — fires the new scene's `on_enter` callback, which typically spawns entities and sets up initial state

### 6.2 Triggering scene transitions

Scene transitions work by running the `scene_switch_system` as a one-shot system via `commands.run_system()`. The system is registered in `SystemsStore` under the key `"switch_scene"` when you use `EngineBuilder::add_scene()`.

**Approaches:**

**1. Menu-driven (recommended):** Use `MenuAction::SetScene("level01")` — the menu system calls `commands.run_system()` internally via `dispatch_menu_action`.

**2. Flag-based from scene callbacks:** Set the target scene name and the `"switch_scene"` flag on `WorldSignals`. The engine's `scene_switch_poll` system (registered automatically by `EngineBuilder::add_scene()`) picks up the flag each sim tick and triggers the transition:

```rust
fn update(ctx: &mut GameCtx, _dt: f32, _input: &InputState) {
    if player_reached_exit(ctx) {
        ctx.world_signals.set_string("scene", "level02".to_string());
        ctx.world_signals.set_flag("switch_scene");
    }
}
```

### 6.3 Persistent entities

The `Persistent` tag component (`src/components/persistent.rs`) marks entities that survive scene switches. During a transition, `scene_switch_system` despawns everything *without* `Persistent`.

Typical uses:

- **Score UI** — a `DynamicText` + `SignalBinding` that displays the score across all scenes
- **Collision rules** — `CollisionRule` entities are regular entities and will be despawned on scene switch unless they have `Persistent`
- **Global state entities** — entities carrying `Signals` or custom components that hold cross-scene state

```rust
ctx.commands.spawn((
    ScreenPosition::new(10.0, 10.0),
    DynamicText::new("0", "arcade", 16.0, Color::WHITE),
    SignalBinding::new("score").with_format("Score: {}"),
    ZIndex(100.0),
    Persistent,  // survives scene switches
));
```

> **Bevy 0.19 gotcha:** Bevy backs each `Resource` with an internal entity carrying an `IsResource` marker
> component. If you write a custom system (via `.add_system()` or `.configure_schedule()`) that scans for
> "all entities without `Persistent`" — e.g. your own cleanup/reset logic — a bare
> `Query<Entity, Without<Persistent>>` will also match these internal resource entities and despawn them.
> Use `aberredengine::components::persistent::CleanableEntity` instead, the same query filter the engine's
> own scene-switch cleanup uses internally:
>
> ```rust
> use aberredengine::components::persistent::CleanableEntity;
>
> fn my_cleanup(query: Query<Entity, CleanableEntity>, mut commands: Commands) {
>     for entity in &query {
>         commands.entity(entity).despawn();
>     }
> }
> ```

### 6.4 Group tracking across scenes

`TrackedGroups` (`src/resources/group.rs`) is a resource holding a set of group names to count. The engine's `update_group_counts_system` publishes entity counts for each tracked group to `WorldSignals` every sim tick.

```rust
// In your scene's on_enter callback:
fn enter(ctx: &mut GameCtx) {
    // Assume tracked_groups is accessed via a separate system or passed in
    // For scene callbacks, use world_signals directly to read counts
}
```

Key behaviors:

- `TrackedGroups::add_group("enemies")` registers a group for counting
- The engine publishes `"group_count:enemies"` to `WorldSignals` each frame
- **Cleared on scene switch** — group tracking is wiped by `scene_switch_system`. Re-register groups in your scene's `on_enter` callback
- Bind a `SignalBinding::new("group_count:enemies")` to auto-display the count in UI text

### 6.5 Per-sim-tick scene updates

The `scene_update_system` runs once per sim tick while a scene is active — not once per rendered frame; see [Threading Model](#threading-model-what-your-code-can-access) for why those differ. It looks up the active scene in `SceneManager`, and if it has an `on_update` callback, calls it:

```rust
fn update(ctx: &mut GameCtx, dt: f32, input: &InputState) {
    // dt = world_time.delta (real elapsed time since the last sim tick, in seconds)
    // input = current keyboard state (just_pressed, active, just_released)
    // Use ctx to read/write ECS state once per sim tick
}
```

The `dt` parameter is `WorldTime.delta` — the real time elapsed since the last sim tick, in seconds (not a render-frame time, and not a fixed constant). Use it for frame-rate-independent logic (e.g., `speed * dt`).

---

## 7. Gameplay Systems

The engine provides four major gameplay systems: **timers**, **phase state machines**, **collision rules**, and **menus**. Each follows the same pattern: a **component** attached to an entity, a **callback type** (Rust function pointer), and a **context SystemParam** providing full ECS access.

All callback types — timers, phases, collisions, menus, and scene callbacks — receive `&mut GameCtx` (`src/systems/game_ctx.rs`), which provides commands, mutable/write queries, read-only queries, and key resources including `world_signals`, `app_state`, `audio`, `world_time`, `config`, `post_process`, `camera_follow`, and `input_bindings`. `GameCtx` runs on the logic thread and has **no direct texture access** — if a callback needs texture data, load it via `RenderAssetCmd` and read back dimensions from `TextureDimsStore` (see [Section 4](#4-loading-assets)). Callbacks have full ECS access otherwise.

### 7.1 Timers

**Source:** `src/components/timer.rs`, `src/systems/timer.rs`

`Timer` is a repeating countdown component. When `elapsed >= duration`, it fires a `TimerEvent` and resets by subtracting `duration` (not zeroing) for timing accuracy.

**Callback signature:**

```rust
use aberredengine::systems::GameCtx;
use aberredengine::resources::input::InputState;

type TimerCallback = fn(Entity, &mut GameCtx, &InputState);
```

**Creating a timer:**

```rust
use aberredengine::components::timer::Timer;

// Spawn an entity with a 2-second repeating timer
ctx.commands.spawn((
    MapPosition::new(0.0, 0.0),
    Timer::rust(2.0, on_timer_fire),
));

fn on_timer_fire(entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
    // This fires every 2 seconds
    ctx.world_signals.set_string("timer_count", "fired!".to_string());
}
```

> **Note:** Use `Timer::rust(duration, callback)` for Rust callbacks. `Timer<C>` is generic — `Timer::rust()` forces the parameter to the concrete `TimerCallback` fn-pointer type that `Query<(Entity, &mut Timer)>` expects. If you use the generic `Timer::new()` with a plain function reference, Rust infers a unique function-item type that the query can never match, and the timer silently never fires.

**One-shot pattern:** Timers always repeat. To make a one-shot timer, despawn the entity in the callback:

```rust
ctx.commands.spawn((
    MapPosition::new(0.0, 0.0),
    Timer::rust(5.0, one_shot_callback),
));

fn one_shot_callback(entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
    // Do the one-time action
    ctx.audio.write(AudioCmd::PlayFx { id: "explosion".into() });
    // Then despawn to prevent future fires
    ctx.commands.entity(entity).despawn();
}
```

### 7.2 Phase State Machines

**Source:** `src/components/phase.rs`, `src/systems/phase.rs`

`Phase` is a per-entity state machine. Each entity has a current phase (a string label) and a map of phase names to callback function pointers.

`on_update` runs once per sim tick (configurable `[simulation] hz` in `config.ini`, default 240 — see [Threading Model](#threading-model-what-your-code-can-access)), with `dt` as the real elapsed time since the last tick, and keeps ticking through render stalls. This can mean more updates than rendered frames if `hz` exceeds your display's refresh rate — treat one-shot effects (playing a sound on enter, etc.) as guarded by `on_enter`/edge conditions, not by assuming one call per visible frame.

**Callback signatures:**

```rust
use aberredengine::systems::GameCtx;

// Called when entering a phase. Return Some("phase") to immediately chain-transition.
type PhaseEnterFn = fn(Entity, &mut GameCtx, &InputState) -> Option<String>;

// Called once per sim tick while in a phase. Return Some("phase") to transition.
type PhaseUpdateFn = fn(Entity, &mut GameCtx, &InputState, f32) -> Option<String>;

// Called when exiting a phase. No return — the transition is already committed.
type PhaseExitFn = fn(Entity, &mut GameCtx);
```

**Creating a phase state machine:**

```rust
use aberredengine::components::phase::{Phase, PhaseCallbackFns};
use rustc_hash::FxHashMap;

let mut phases = FxHashMap::default();

phases.insert("idle".to_string(), PhaseCallbackFns {
    on_enter: Some(idle_enter),
    on_update: Some(idle_update),
    on_exit: None,
});

phases.insert("jumping".to_string(), PhaseCallbackFns {
    on_enter: Some(jumping_enter),
    on_update: Some(jumping_update),
    on_exit: Some(jumping_exit),
});

phases.insert("falling".to_string(), PhaseCallbackFns {
    on_enter: None,
    on_update: Some(falling_update),
    on_exit: None,
});

ctx.commands.spawn((
    MapPosition::new(100.0, 200.0),
    // ... sprite, rigidbody, etc ...
    Phase::new("idle", phases),
));
```

`PhaseCallbackFns` derives `Default` (all callbacks `None`), so you can use `..Default::default()` when only some callbacks are needed:

```rust
// Equivalent to the "falling" entry above — only on_update is set
phases.insert("falling".to_string(), PhaseCallbackFns {
    on_update: Some(falling_update),
    ..Default::default()
});
```

**Phase fields:**

| Field | Type | Description |
|-------|------|-------------|
| `current` | `String` | Current phase label |
| `previous` | `Option<String>` | Phase before the last transition |
| `next` | `Option<String>` | Set to request a transition |
| `time_in_phase` | `f32` | Seconds since entering current phase |

**External transitions:** Set `phase.next = Some("new_phase".to_string())` from outside the phase system to request a transition. The `phase_system` processes this on the next frame.

**Example callbacks:**

```rust
fn idle_enter(_entity: Entity, _ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
    None // stay in idle
}

fn idle_update(entity: Entity, ctx: &mut GameCtx, input: &InputState, _dt: f32) -> Option<String> {
    if input.action_1.just_pressed {
        // Apply jump velocity
        if let Ok(mut rb) = ctx.rigid_bodies.get_mut(entity) {
            rb.velocity.y = -400.0;
        }
        return Some("jumping".to_string());
    }
    None
}

fn jumping_enter(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
    ctx.audio.write(AudioCmd::PlayFx { id: "jump".into() });
    None
}

fn jumping_update(entity: Entity, ctx: &mut GameCtx, _input: &InputState, _dt: f32) -> Option<String> {
    if let Ok(rb) = ctx.rigid_bodies.get(entity) {
        if rb.velocity.y > 0.0 {
            return Some("falling".to_string());
        }
    }
    None
}

fn jumping_exit(_entity: Entity, _ctx: &mut GameCtx) {
    // cleanup if needed
}

fn falling_update(entity: Entity, ctx: &mut GameCtx, _input: &InputState, _dt: f32) -> Option<String> {
    // Transition back to idle when landing (detected by some condition)
    if let Ok(rb) = ctx.rigid_bodies.get(entity) {
        if rb.velocity.y == 0.0 {
            return Some("idle".to_string());
        }
    }
    None
}
```

### 7.3 Collision Rules

**Source:** `src/components/collision.rs`, `src/systems/rust_collision.rs`, `src/systems/collision_detector.rs`

`CollisionRule` defines how collisions between two entity groups are handled. Rules are spawned as their own entities.

**Callback signature:**

```rust
use aberredengine::systems::GameCtx;

type CollisionCallback = fn(Entity, Entity, &BoxSides, &BoxSides, &mut GameCtx);
```

- `Entity, Entity` — the two colliding entities, ordered to match `group_a` and `group_b`
- `BoxSides = SmallVec<[BoxSide; 4]>` — which sides are colliding for each entity
- `BoxSide` variants: `Left`, `Right`, `Top`, `Bottom`

**Detection pipeline:**

1. `collision_detector` system iterates all entity pairs with `MapPosition` + `BoxCollider`
2. Uses AABB overlap via `BoxCollider::as_rectangle()` + `check_collision_recs()`
3. On overlap, triggers a `CollisionEvent`
4. `rust_collision_observer` receives the event, looks up `Group` names, finds a matching `CollisionRule`, computes collision sides, and calls the callback

**Bidirectional matching:** A rule for `("ball", "brick")` matches regardless of which entity is `ball` vs `brick`. The observer reorders entities so the first argument always corresponds to `group_a` and the second to `group_b`.

**Creating a collision rule:**

```rust
use aberredengine::components::collision::{CollisionRule, BoxSide};
use aberredengine::components::persistent::Persistent;

ctx.commands.spawn((
    CollisionRule::rust("ball", "brick", ball_brick_collision),
    Persistent, // survive scene switches
));
```

> **Note:** Use `CollisionRule::rust(group_a, group_b, callback)` for Rust callbacks. `CollisionRule<C>` is generic — `CollisionRule::rust()` forces the parameter to the concrete `CollisionCallback` fn-pointer type that `Query<&CollisionRule>` expects. If you use the generic `CollisionRule::new()` with a plain function reference, Rust infers a unique function-item type that the query can never match, and the callback silently never fires.

> **Note:** `CollisionRule` entities are regular entities — they get despawned on scene switch unless marked `Persistent`.

**Example callback — ball/brick collision with side-based reflection:**

```rust
fn ball_brick_collision(
    ball: Entity,
    brick: Entity,
    ball_sides: &BoxSides,
    _brick_sides: &BoxSides,
    ctx: &mut GameCtx,
) {
    // Despawn the brick
    ctx.commands.entity(brick).despawn();
    ctx.audio.write(AudioCmd::PlayFx { id: "break".into() });

    // Reflect ball velocity based on collision side
    if let Ok(mut rb) = ctx.rigid_bodies.get_mut(ball) {
        for side in ball_sides.iter() {
            match side {
                BoxSide::Top | BoxSide::Bottom => rb.velocity.y = -rb.velocity.y,
                BoxSide::Left | BoxSide::Right => rb.velocity.x = -rb.velocity.x,
            }
        }
    }
}
```

### 7.4 Menus

**Source:** `src/components/menu.rs`, `src/systems/menu.rs`

`Menu` is a component that creates an interactive, navigable menu. Spawn it on an entity and the engine handles rendering, input, scrolling, and selection dispatch.

**Constructor:**

```rust
use aberredengine::raylib::prelude::*;
use aberredengine::components::menu::{Menu, MenuActions, MenuAction};

let menu = Menu::new(
    &[("start", "Start Game"), ("options", "Options"), ("quit", "Quit")],
    Vector2 { x: 100.0, y: 80.0 }, // origin position
    "arcade",                        // font key
    24.0,                            // font size
    30.0,                            // item spacing (pixels)
    true,                            // use_screen_space
);
```

**Builder methods:**

| Method | Description |
|--------|-------------|
| `.with_colors(normal, selected)` | Set normal and selected item colors |
| `.with_selection_sound("key")` | Play a sound on selection change |
| `.with_on_rust_callback(fn)` | Set a Rust callback for selection |
| `.with_visible_count(n)` | Limit visible items (enables scrolling) |
| `.with_cursor(entity)` | Attach a cursor entity to the selection |

**Two selection handling approaches:**

**1. MenuActions (declarative):** Attach a `MenuActions` component alongside the `Menu`. Each item ID maps to an action:

```rust
let actions = MenuActions::new()
    .with("start", MenuAction::SetScene("level01".to_string()))
    .with("options", MenuAction::SetScene("options_menu".to_string()))
    .with("quit", MenuAction::QuitGame);

ctx.commands.spawn((menu, actions));
```

`MenuAction` variants:

| Variant | Effect |
|---------|--------|
| `SetScene(String)` | Triggers a scene switch (calls `commands.run_system()` internally) |
| `QuitGame` | Transitions to quitting state |
| `ShowSubMenu(String)` | Sets a signal for sub-menu display (TODO) |
| `Noop` | Does nothing |

**2. Rust callback:** For custom logic, use `.with_on_rust_callback()`:

```rust
use aberredengine::components::menu::MenuRustCallback;
use aberredengine::systems::GameCtx;

fn on_menu_select(menu_entity: Entity, item_id: &str, item_index: usize, ctx: &mut GameCtx) {
    match item_id {
        "start" => {
            ctx.world_signals.set_string("scene", "level01".to_string());
            ctx.world_signals.set_flag("switch_scene");
        }
        "quit" => {
            ctx.world_signals.set_flag("quit_game");
        }
        _ => {}
    }
}

ctx.commands.spawn((
    menu.with_on_rust_callback(on_menu_select),
));
```

**Callback priority:** When an item is selected, the engine checks in order:

1. **Lua callback** (`on_select_callback`) — only with `lua` feature
2. **Rust callback** (`on_rust_callback`)
3. **MenuActions** (declarative)

The first match wins; later options are skipped.

**Navigation:** Up/down arrows move selection. `action_1` or `action_2` confirms. With `.with_visible_count(n)`, the menu shows at most `n` items at a time with bounded navigation and auto-scrolling.

**Complete menu example:**

```rust
fn enter(ctx: &mut GameCtx) {
    let menu = Menu::new(
        &[("play", "Play"), ("quit", "Quit")],
        Vector2 { x: 200.0, y: 150.0 },
        "arcade",
        32.0,
        40.0,
        true,
    )
    .with_colors(Color::GRAY, Color::WHITE)
    .with_selection_sound("menu_move");

    let actions = MenuActions::new()
        .with("play", MenuAction::SetScene("level01".to_string()))
        .with("quit", MenuAction::QuitGame);

    ctx.commands.spawn((menu, actions));
}
```

### 7.5 Animation Finished Event

**Source:** `src/events/animation.rs`, `src/systems/lua_animation_finished.rs`

`AnimationFinishedEvent` is triggered **once** by the animation system on the frame a non-looped animation first reaches its final frame. Looped animations never trigger it. It is not re-triggered on subsequent frames even though the entity stays on the last frame.

**Event struct:**

```rust
pub struct AnimationFinishedEvent {
    pub entity: Entity,
}
```

**Observing from Rust** — register a persistent observer with `EngineBuilder::add_observer`:

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::bevy_ecs::observer::On;
use aberredengine::events::animation::AnimationFinishedEvent;

fn on_anim_done(
    trigger: On<AnimationFinishedEvent>,
    mut commands: Commands,
) {
    // Despawn the entity whose animation just finished
    commands.entity(trigger.event().entity).despawn();
}

// Register in main:
EngineBuilder::new()
    .add_observer(on_anim_done)
    // …
```

**Lua consumers:** attach `LuaOnAnimationEnd::new("fn_name")` to the entity (or use `:with_on_animation_end("fn_name")` in the Lua spawn builder). The Lua callback signature is `fn(ctx, input)` — the same as timer and phase callbacks.

### 7.6 Tween Finished Event

**Source:** `src/events/tween.rs`, `src/systems/tween.rs`

`TweenFinishedEvent<T>` is triggered **once** by the tween system for a given `Tween<T>` the frame it stops playing — either a `LoopMode::Once` tween reaching its end, or a zero-duration tween snapping immediately. `LoopMode::Loop` and `LoopMode::PingPong` tweens never trigger it, since they never stop playing on their own.

**Event struct (generic over the tweened component type):**

```rust
pub struct TweenFinishedEvent<T: TweenValue> {
    pub entity: Entity,
}
```

**Observing from Rust** — register a persistent observer per tweened type with `EngineBuilder::add_observer`. The engine already runs one monomorphized tween system per `T` (`MapPosition`, `Rotation`, `Scale`, `ScreenPosition`), so register one observer per `T` you care about:

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::bevy_ecs::observer::On;
use aberredengine::components::mapposition::MapPosition;
use aberredengine::events::tween::TweenFinishedEvent;

fn on_move_tween_done(
    trigger: On<TweenFinishedEvent<MapPosition>>,
    mut commands: Commands,
) {
    // Despawn the entity whose move tween just finished
    commands.entity(trigger.event().entity).despawn();
}

// Register in main:
EngineBuilder::new()
    .add_observer(on_move_tween_done)
    // …
```

**Lua consumers:** attach `LuaOnTweenFinished<T>::new("fn_name")` to the entity (or use the matching
`:with_tween_{position,rotation,scale,screen_position}_on_finished("fn_name")` builder method). The Lua
callback signature is `fn(ctx, input)` — the same as the animation-finished and timer/phase callbacks.

### 7.7 GUI Widgets

**Source:** `src/components/{guiwindow,guibutton,guilabel,guiimage,guiinteractable,guioffset}.rs`, `src/resources/guitheme.rs`, `src/systems/{gui_spawn,gui_layout,gui_hit_test,gui_interactable_click}.rs`, `src/events/gui_interactable.rs`

The engine provides a themed, nine-patch-skinned in-game GUI widget system — panels, buttons, labels, and
clickable images. It is plain ECS components and systems, fully usable from pure Rust; all of its systems
(`gui_button_spawn_system`, `gui_label_spawn_system`, `gui_image_spawn_system`, `gui_layout_system`,
`gui_hit_test_system`, `gui_interactable_click_observer`) are registered automatically by `EngineBuilder`
regardless of the `lua` feature — there's nothing extra to wire up.

This is distinct from the `gui_callback` ImGui overlay covered in Section 3 — ImGui is for editor/debug
tooling rendered every frame via a callback; this widget system is for in-game UI made of regular entities
that participate in the normal render/collision/hierarchy pipeline (positioned with `ScreenPosition`,
parented with `ChildOf`, hidden by removing `ScreenPosition`, etc.).

**Core components:**

| Component | Shape | Notes |
|-----------|-------|-------|
| `GuiWindow` | `GuiWindow::new(w, h)` — `theme_key: Arc<str>` defaults to `"default"`, override with `.with_theme_key(key)` | Themed panel. Visibility = presence/absence of `ScreenPosition` on the same entity. |
| `GuiButton` | `GuiButton::new(w, h, "Caption")` — `theme_key: Arc<str>` defaults to `"default"`, override with `.with_theme_key(key)` | Self-contained; `gui_button_spawn_system` reacts one frame later to insert a `GuiInteractable` (via `insert_if_new`) and, unless the caption is empty, spawn a themed caption `DynamicText` child. |
| `GuiLabel` | `GuiLabel::new(w, h, "Text")` — `theme_key: Arc<str>` defaults to `"default"`, override with `.with_theme_key(key)`; add `.with_signal_binding(key)` / `.with_signal_binding_format("fmt {}")` to drive text from `WorldSignals` | Same caption-child pattern as `GuiButton`, minus any interaction — never hit-tested. |
| `GuiImage` | `GuiImage::new(w, h, "tex_key", offset_x, offset_y)` — add `.with_offset_hover(x, y)` / `.with_offset_pressed(x, y)` / `.with_offset_disabled(x, y)` for per-state atlas offsets | `gui_image_spawn_system` inserts a co-located `GuiInteractable` + `Sprite` (same entity, no child). `gui_image_state_sync_system` re-resolves `Sprite.offset` from `GuiInteractable.state` every sim tick automatically — no game code needed. |
| `GuiProgressBar` | `GuiProgressBar::new(w, h, value, max)` — `theme_key: Arc<str>` defaults to `"default"`, override with `.with_theme_key(key)`; `.with_direction(ProgressBarDirection)`, `.with_signal_binding(key)` for `WorldSignals` auto-update | No spawn system — rendered directly. Requires `ScreenPosition` + `ZIndex`. `value` clamped to `[0, max]`. `direction` variants: `Horizontal` (default, left→right), `HorizontalReversed`, `Vertical` (bottom→top), `VerticalReversed`. |
| `Shadow` | `Shadow::new(dx, dy, r, g, b, a)` or `Shadow::default_color(dx, dy)` (50% transparent black) | Pre-pass shadow drawn at entity position + offset before the main sprite/text draw. Bypasses entity shaders. Works for both world-space and screen-space entities (`Sprite` and `DynamicText`). |
| `GuiInteractable` | `GuiInteractable::rust(w, h, callback)` | Shared hit-test/click runtime state (`Normal`/`Hovered`/`Pressed`/`Disabled`). Use the `::rust()` coercion constructor — mirrors `CollisionRule::rust`/`Timer::rust` — for a Rust fn-pointer callback: `GuiRustCallback = fn(Entity, &mut GameCtx)`. |
| `GuiOffset` | `GuiOffset(Vector2::new(x, y))` | A child widget's position relative to its `ChildOf` parent. `gui_layout_system` resolves it into the child's `ScreenPosition` every sim tick; `ChildOf` is used for lifecycle (cascade despawn) only, not positioning. |

> **Important:** `GuiButton`/`GuiImage`'s spawn systems use `insert_if_new` for the `GuiInteractable` they
> add — they will not overwrite one you pre-spawned. **To get a Rust callback, you must spawn
> `GuiInteractable::rust(...)` yourself in the same bundle as `GuiButton`/`GuiImage`.** If you only spawn
> `GuiButton`/`GuiImage` alone, the inserted default `GuiInteractable` has no Rust callback wired — only the
> Lua `callback_name`-based dispatch path, which no-ops with no matching Lua function present.

**Theming:** Themes are stored in `GuiThemeStore` — a `HashMap<Arc<str>, GuiTheme>` pre-inserted by the engine. Each widget carries a `theme_key: Arc<str>` (default `"default"`) that is resolved against `GuiThemeStore` at render time. Set up themes in your setup system before spawning any widgets:

```rust
use aberredengine::bevy_ecs::prelude::ResMut;
use aberredengine::raylib::prelude::{Color, Rectangle};
use aberredengine::resources::guitheme::{GuiButtonSkin, GuiNinePatch, GuiThemeStore};
use std::sync::Arc;

fn setup_gui_theme(mut theme_store: ResMut<GuiThemeStore>) {
    let theme = theme_store.themes.entry(Arc::from("default")).or_default();
    theme.panel = GuiNinePatch {
        tex_key: "gui_panel".into(),
        source: Rectangle::new(0.0, 0.0, 64.0, 64.0),
        left: 6,
        top: 6,
        right: 6,
        bottom: 6,
    };
    theme.button = Some(GuiButtonSkin {
        normal: GuiNinePatch {
            tex_key: "gui_button".into(),
            source: Rectangle::new(0.0, 0.0, 32.0, 32.0),
            left: 4,
            top: 4,
            right: 4,
            bottom: 4,
        },
        ..Default::default() // hover/pressed/disabled fall back to normal if unset
    });
    theme.font = "main_font".into();
    theme.font_size = 16.0;
    theme.text_color = Color::WHITE;

    // Optional: drop shadows. panel_shadow applies to all nine-patch backgrounds;
    // text_shadow is inserted as a Shadow component on spawned caption DynamicText children.
    // theme.panel_shadow = Some(Shadow::default_color(2.0, 2.0));
    // theme.text_shadow = Some(Shadow::default_color(1.0, 1.0));
}
```

`GuiNinePatch { tex_key, source, left, top, right, bottom }` maps 1:1 onto raylib's `NPatchInfo`. `tex_key`
must already be loaded into `TextureStore` (see Section 4). `theme.font` defaults to an empty key — if it's
still unset when a non-empty caption is about to spawn, the engine logs an `error!`; the caption entity still
spawns, it just renders no visible glyphs.

**`GuiTheme` fields of note:**
- `panel: GuiNinePatch` — background patch for `GuiWindow`, `GuiLabel`, `GuiProgressBar`.
- `button: Option<GuiButtonSkin>` — four nine-patches (`normal`/`hover`/`pressed`/`disabled`); unset states fall back to `normal`. Also has four optional per-state shadows (`shadow`, `hover_shadow`, `pressed_shadow`, `disabled_shadow`); unset states fall back to `shadow` (normal), which itself falls back to `panel_shadow`.
- `label: Option<GuiNinePatch>` — separate background for `GuiLabel` (falls back to `panel` if unset).
- `progress_bar: Option<GuiProgressBarSkin>` — `track: Option<GuiNinePatch>` (full-size background, optional) and `fill: GuiNinePatch` (scaled to `value/max`).
- `panel_shadow: Option<Shadow>` — drop shadow drawn behind all nine-patch backgrounds.
- `text_shadow: Option<Shadow>` — `Shadow` component inserted on spawned caption `DynamicText` children.

**Multi-theme UIs:** insert additional entries and use `.with_theme_key("my_theme")` on any widget:

```rust
let hud_theme = theme_store.themes.entry(Arc::from("hud")).or_default();
hud_theme.panel = GuiNinePatch { tex_key: "hud_panel".into(), /* … */ };
hud_theme.font = "hud_font".into();

// Spawn a widget using the "hud" theme
ctx.commands.spawn((
    GuiWindow::new(200.0, 40.0).with_theme_key("hud"),
    ScreenPosition::new(10.0, 10.0),
    ZIndex(5.0),
));
```

A missing/unregistered `theme_key` skips the themed background (caption/sprite still renders) and logs a warning once per widget via `GuiThemeWarnCache`.

**Complete example — a panel with one clickable button:**

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::raylib::prelude::Vector2;
use aberredengine::components::guibutton::GuiButton;
use aberredengine::components::guiinteractable::GuiInteractable;
use aberredengine::components::guioffset::GuiOffset;
use aberredengine::components::guiwindow::GuiWindow;
use aberredengine::components::screenposition::ScreenPosition;
use aberredengine::components::zindex::ZIndex;
use aberredengine::systems::GameCtx;

fn on_start_clicked(_entity: Entity, ctx: &mut GameCtx) {
    ctx.world_signals.set_flag("start_pressed");
}

fn spawn_menu_panel(ctx: &mut GameCtx) {
    let panel = ctx
        .commands
        .spawn((
            GuiWindow::new(200.0, 100.0),
            ScreenPosition::new(50.0, 50.0),
            ZIndex(10.0),
        ))
        .id();

    ctx.commands.spawn((
        GuiButton::new(120.0, 32.0, "Start"),
        GuiInteractable::rust(120.0, 32.0, on_start_clicked),
        ChildOf(panel),
        GuiOffset(Vector2::new(40.0, 34.0)),
        ZIndex(10.0),
    ));
}
```

The button entity needs no `ScreenPosition` set directly — `gui_layout_system` supplies it each frame from
the parent's `ScreenPosition` plus `GuiOffset`.

**Click events:** clicks dispatch primarily through the per-widget `GuiInteractable.on_rust_callback` /
`on_click_callback` (Lua name) shown above. For cross-cutting logic that doesn't belong to one specific
widget (analytics, a UI click sound), you can additionally observe `GuiInteractableClickEvent { entity }`
(`src/events/gui_interactable.rs`) the same way as `AnimationFinishedEvent`/`TweenFinishedEvent<T>` above —
register it once with `EngineBuilder::add_observer`; it fires for any `GuiInteractable`-carrying widget
(`GuiButton` or `GuiImage`) on a press-then-release-inside.

---

## 8. Engine Resources Quick Reference

All resources are accessed as Bevy ECS system parameters. Use `Res<T>` / `ResMut<T>` for Send resources, `NonSend<T>` / `NonSendMut<T>` for main-thread-only resources. Scene callbacks access most of these through `GameCtx` fields.

**Read this table by thread, not just by Send/NonSend** — see [Threading Model](#threading-model-what-your-code-can-access). The tables below are split into logic-thread resources (everything your `setup`/`on_update`/scene callbacks/custom systems can request) and render-thread-only resources (things only `process_render_asset_cmds`/`render_system` touch — requesting these from logic-side code panics at schedule-init time, full stop, regardless of `Res`/`NonSend`).

### Logic-thread resources (Send) — what your game code can use

| Resource | Access | Purpose |
|----------|--------|---------|
| `WorldTime` | `Res` | `elapsed`, `delta` (real elapsed time since the last **sim tick**, not a render frame), `time_scale`, `frame_count` |
| `WorldSignals` | `ResMut` | Global cross-system communication (scalars, integers, strings, flags, entities) |
| `AppState` | `ResMut` | Rust-only typed state store keyed by Rust type; useful for GUI/editor snapshots and view-models |
| `TrackedGroups` | `ResMut` | Group names to count — engine publishes counts to `WorldSignals` each sim tick |
| `ScreenSize` | `Res` | Internal render resolution (`w`, `h`) — inserted independently on both the logic and render threads, always in sync |
| `WindowSize` | `Res` | OS window dimensions (`w`, `h`), has `calculate_letterbox()` and `window_to_game_pos()` |
| `GameConfig` | `ResMut` | Loaded from `config.ini` — all render/window/simulation/audio settings |
| `GameConfigDefaults` | `Res` | Read-only snapshot (`.0: GameConfig`) of `GameConfig` as loaded at startup, before any runtime mutation — use to restore a field to its loaded default (e.g. window title after a map override) without needing your own capture-resource |
| `InputState` | `Res` | Input state — digital fields are `BoolState { active, just_pressed, just_released }`; analog fields (`scroll_y`, `mouse_x/y`, `mouse_world_x/y`) are `f32` |
| `InputBindings` | `ResMut` | Runtime key/mouse binding map (`InputAction` → `Vec<InputBinding>`). Modify to rebind actions at runtime — takes effect the very next sim tick. |
| `GameState` | `Res` | Current state: `None → Setup → Playing → Quitting` |
| `NextGameState` | `ResMut` | Request state transitions with `.set(GameStates::Playing)` |
| `PostProcessShader` | `ResMut` | Shader chain + uniforms (reserved: `uTime`, `uDeltaTime`, `uResolution`, `uFrame`, `uWindowResolution`, `uLetterbox`) |
| `CameraFollowConfig` | `ResMut` | Camera-follow behavior (mode, easing, zoom speed, bounds, offsets) |
| `DebugOverlayConfig` | `ResMut` | F11 debug overlay toggles for colliders, signals, bounds, and crosshairs |
| `SystemsStore` | `Res` | Named system registry for `commands.run_system()` |
| `SceneManager` | `Res` | Scene registry (only present with `.add_scene()`) |
| `Camera2DRes` | `ResMut` | 2D camera (target, offset, zoom, rotation) |
| `AnimationStore` | `Res` / `ResMut` | Animation definitions |
| `GuiThemeStore` | `ResMut` | Named GUI theme registry (`FxHashMap<Arc<str>, GuiTheme>`); each theme holds panel/button/label/progress_bar nine-patches, font settings, and optional shadows; see §7.7 |
| `GuiInputState` | `Res` | `click_consumed_this_frame: bool` — set by `gui_hit_test_system` when any `GuiInteractable` absorbs a click; reset each frame |
| `FontMetricsStore` | `Res` | CPU-side glyph measurement, keyed like `FontStore`. Populated asynchronously after a `RenderAssetCmd::Font` load completes — see [Section 4](#4-loading-assets). |
| `TextureDimsStore` | `Res` | Pixel `(width, height)` per loaded texture key, via `.get(key)`/`.width(key)`. Populated asynchronously after a `RenderAssetCmd::Texture` load completes — see [Section 4](#4-loading-assets). |

### Render-thread-only resources — inaccessible from your game code

These exist only in the render world. `RaylibAccess`/`NonSend<FontStore>`/`NonSend<ShaderStore>`/`Res<TextureStore>` as a parameter on any `setup`/`on_update`/scene-callback/`.add_system()`/`.configure_schedule()` system panics at schedule-init time — there is no way around this from a custom Rust system. Listed here for completeness (e.g. if you're reading engine source), not because you can request them:

| Resource | Access (render-side only) | Purpose |
|----------|---------------------------|---------|
| `RaylibHandle` / `RaylibThread` | via `RaylibAccess` SystemParam | Raylib context |
| `FontStore` | `NonSendMut` | Loaded fonts by key (GPU-bound) |
| `ShaderStore` | `NonSendMut` | Loaded shaders with cached uniform locations |
| `TextureStore` | `Res` / `ResMut` | Loaded textures by key (GPU-bound) |
| `RenderTarget` | `NonSendMut` | Internal framebuffer |

### Render-thread-only resources used by `gui_callback`/`world_draw_callback`

Since those two callbacks are the one exception that runs render-side (see [Threading Model](#threading-model-what-your-code-can-access) and [Section 3](#imgui-gui-callback-rust-only)), they're handed these instead of the live `WorldSignals`:

| Resource | Purpose |
|----------|---------|
| `SignalSnapshot` | Read-only, one-tick-stale copy of `WorldSignals`. **No getter methods** — read fields directly: `signals.scalars.get(key)`, `signals.flags.contains(key)`, `signals.integers`/`.strings`/`.entities`/`.group_counts` the same way. |
| `SignalIntents` | Queue of pending `WorldSignals` writes. Setter methods mirror `WorldSignals`' own: `.set_flag(key)`, `.set_scalar(key, v)`, `.set_integer(key, v)`, `.set_string(key, v)`, `.clear_flag(key)`. Applied to the live `WorldSignals` at the start of the logic thread's next sim tick. |

### Developer-inserted resources

| Resource | Access | Purpose |
|----------|--------|---------|
| `DebugMode` | marker resource | Presence enables debug overlays |
| `FullScreen` | marker resource (render-thread-only) | Presence enables fullscreen |

### WorldSignals API

`WorldSignals` is the most-used resource. It provides typed key-value storage for cross-system communication.

**Scalars (`f32`):**

| Method | Signature |
|--------|-----------|
| `set_scalar` | `(&mut self, key: impl Into<String>, value: f32)` |
| `get_scalar` | `(&self, key: &str) -> Option<f32>` |
| `clear_scalar` | `(&mut self, key: &str) -> Option<f32>` |

**Integers (`i32`):**

| Method | Signature |
|--------|-----------|
| `set_integer` | `(&mut self, key: impl Into<String>, value: i32)` |
| `get_integer` | `(&self, key: &str) -> Option<i32>` |
| `clear_integer` | `(&mut self, key: &str) -> Option<i32>` |

**Strings:**

| Method | Signature |
|--------|-----------|
| `set_string` | `(&mut self, key: impl Into<String>, value: impl Into<String>)` |
| `get_string` | `(&self, key: &str) -> Option<&String>` |
| `remove_string` | `(&mut self, key: &str) -> Option<String>` |

**Flags (presence-based booleans):**

| Method | Signature |
|--------|-----------|
| `set_flag` | `(&mut self, key: impl Into<String>)` |
| `has_flag` | `(&self, key: &str) -> bool` |
| `clear_flag` | `(&mut self, key: &str)` |
| `take_flag` | `(&mut self, key: &str) -> bool` — returns `true` and clears the flag if present; `false` if absent. Equivalent to `has_flag` + `clear_flag` in one lookup. Preferred in `on_update` to consume a GUI action flag. |

**Entities:**

| Method | Signature |
|--------|-----------|
| `set_entity` | `(&mut self, key: impl Into<String>, entity: Entity)` |
| `get_entity` | `(&self, key: &str) -> Option<&Entity>` |
| `remove_entity` | `(&mut self, key: &str) -> Option<Entity>` |

**Group counts** (stored as integers with `"group_count:"` prefix):

| Method | Signature |
|--------|-----------|
| `set_group_count` | `(&mut self, group_name: &str, count: i32)` |
| `get_group_count` | `(&self, group_name: &str) -> Option<i32>` |
| `clear_group_counts` | `(&mut self)` |

`WorldSignals` intentionally stays limited to those primitive/value-like channels. For richer Rust-only typed data, use `AppState` instead.

### AppState API

`AppState` is a Rust-only typed store keyed by `TypeId`. The engine inserts it automatically at startup. It stores one value per Rust type, so `insert::<T>` replaces any previous `T`.

| Method | Signature |
|--------|-----------|
| `insert` | `(&mut self, value: T) where T: Any + Send + Sync + 'static` |
| `get::<T>` | `(&self) -> Option<&T>` |
| `get_mut::<T>` | `(&mut self) -> Option<&mut T>` |
| `remove::<T>` | `(&mut self) -> Option<T>` |
| `contains::<T>` | `(&self) -> bool` |

Use `AppState` for richer GUI/editor snapshots and view-models that do not belong in the Lua-visible signal bus. If you need two values of the same underlying type, wrap them in newtypes.

```rust
use aberredengine::imgui;
use aberredengine::resources::appstate::AppState;
use aberredengine::resources::render::fontstore::FontStore;
use aberredengine::resources::render::texturestore::TextureStore;
use aberredengine::resources::worldsignals::SignalSnapshot;
use aberredengine::resources::signal_intents::SignalIntents;
use aberredengine::bevy_ecs::prelude::ResMut;

#[derive(Clone)]
struct InspectorSnapshot {
    selected_name: String,
}

// ECS system writes typed state (logic thread)
fn inspector_system(mut app_state: ResMut<AppState>) {
    app_state.insert(InspectorSnapshot {
        selected_name: "Player".to_string(),
    });
}

// GUI callback reads it (render thread — see Threading Model)
fn inspector_gui(
    ui: &imgui::Ui,
    _signals: &SignalSnapshot,
    _intents: &mut SignalIntents,
    _textures: &TextureStore,
    _fonts: &FontStore,
    app_state: &AppState,
) {
    if let Some(snapshot) = app_state.get::<InspectorSnapshot>() {
        ui.text(format!("Selected: {}", snapshot.selected_name));
    }
}
```

### InputState key bindings

Each digital field is a `BoolState { active, just_pressed, just_released }`. Hardware assignments live in `InputBindings`, not in `BoolState`. Analog fields are plain `f32`.

**Digital fields (`BoolState`):**

| Field | Default binding | Description |
|-------|-----------------|-------------|
| `maindirection_up` | W | WASD up |
| `maindirection_down` | S | WASD down |
| `maindirection_left` | A | WASD left |
| `maindirection_right` | D | WASD right |
| `secondarydirection_up` | Up arrow | Alternative up |
| `secondarydirection_down` | Down arrow | Alternative down |
| `secondarydirection_left` | Left arrow | Alternative left |
| `secondarydirection_right` | Right arrow | Alternative right |
| `action_1` | Space, mouse left | Primary action |
| `action_2` | Enter, mouse right | Secondary action |
| `action_3` | Mouse middle | Tertiary action (no keyboard default) |
| `action_back` | Escape | Back/cancel |
| `action_special` | F12 | Special action |
| `mode_debug` | F11 | Debug toggle |
| `fullscreen_toggle` | F10 | Fullscreen toggle |

**Analog fields (`f32`):**

| Field | Description |
|-------|-------------|
| `scroll_y` | Mouse wheel delta this frame. Positive = up, negative = down. |
| `mouse_x` | Cursor X in game/render-target space (letterbox-corrected, 0..render_width). |
| `mouse_y` | Cursor Y in game/render-target space (letterbox-corrected, 0..render_height). |
| `mouse_world_x` | Cursor X in world-space (after camera transform, matches `MapPosition`). |
| `mouse_world_y` | Cursor Y in world-space (after camera transform, matches `MapPosition`). |

### InputBindings resource

`InputBindings` (`src/resources/input_bindings.rs`) maps logical `InputAction` variants to a `Vec<InputBinding>`, supporting multiple hardware bindings per action (e.g. W and Up arrow both trigger `main_up`).

```rust
use aberredengine::resources::input_bindings::{InputBindings, InputBinding, InputAction};

// InputBinding variants:
InputBinding::Keyboard(KeyboardKey)       // a keyboard key
InputBinding::MouseButton(MouseButton)    // a mouse button
```

Key binding strings accepted by the Lua API (also useful as reference): `a`–`z`, `0`–`9`, `space`, `enter`/`return`, `escape`/`esc`, `up`/`down`/`left`/`right`, `lshift`/`rshift`/`lctrl`/`rctrl`/`lalt`/`ralt`, `f1`–`f12`, `mouse_left`, `mouse_right`, `mouse_middle`.

---

## 9. The config.ini File

Section 2 showed the basics. This is the complete reference.

### Complete key reference

**`[render]` section:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `width` | `u32` | `640` | Internal render width |
| `height` | `u32` | `360` | Internal render height |
| `background_color` | `R,G,B` | `80,80,80` | Background clear color (0–255 per channel) |
| `pixel_snap_camera` | `bool` | `true` | Snap the camera target/view-rect to integer pixels each frame, avoiding sprite atlas bleeding. Disable for games with smooth rotation/zoom (e.g. asteroids-style). Also toggleable at runtime via `config.pixel_snap_camera` or the Lua `engine.set_pixel_snap_camera(bool)` / `get_pixel_snap_camera()` API. |
| `render_target_filter` | `string` | `"nearest"` | Sampling filter for the render-target-to-window blit. Values: `"nearest"` (default, sharp pixel art), `"bilinear"`, `"trilinear"`, `"anisotropic_4x"`, `"anisotropic_8x"`, `"anisotropic_16x"`. Unrecognized values warn and fall back to `"nearest"`. Changeable at runtime via `ResMut<GameConfig>`. |

**`[window]` section:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `width` | `u32` | `1280` | Window width |
| `height` | `u32` | `720` | Window height |
| `target_fps` | `u32` | `120` | Target FPS |
| `vsync` | `bool` | `true` | Vertical sync |
| `fullscreen` | `bool` | `false` | Start fullscreen |
| `title` | `string` | `"Aberred Engine"` | Window title |

**`[simulation]` section:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `hz` | `f64` | `240` | Logic thread's sim tick rate — see [Threading Model](#threading-model-what-your-code-can-access). Read once at startup; a runtime `GameConfig` change has no effect. Clamped to `[15, 1000]`, out-of-range values warn and clamp rather than error. |
| `snapshot_hz` | `f64` | `[window] target_fps` if set, else `60` | Rate at which the logic thread publishes render state to the render thread. Re-resolved from `target_fps` whenever this key isn't set explicitly. Same clamp range as `hz`. |

**`[audio]` section:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `hz` | `f64` | `100` | Audio thread's tick rate. Read once at startup; same clamp range as `[simulation] hz`. |

### Parsing behavior

- **`config_str()`** — alternative to a file: pass INI content as a `&'static str`. The file path is ignored when this is set.
- **Missing file** -> startup error from `EngineBuilder::try_run()`
- **Unreadable or malformed INI** -> startup error from `EngineBuilder::try_run()`
- **Missing key** -> default for that key
- **Invalid value** -> silently ignored, default used for that key
- **Booleans** — case-insensitive: `true`/`false`, `yes`/`no`, `on`/`off`
- **`background_color`** — comma-separated `R,G,B` integers (e.g., `80,80,80`)

If you want the engine defaults with no custom settings, commit an otherwise empty `config.ini` file and only add keys you want to override.

### Runtime modification

`GameConfig` is mutable at runtime via `ResMut<GameConfig>`:

```rust
fn my_system(mut config: ResMut<GameConfig>) {
    config.set_render_size(1280, 720);
    config.set_window_size(1920, 1080);
    config.save_to_file().expect("Failed to save config");
}
```

The engine detects changes and applies them — render size changes recreate the framebuffer, vsync/fps changes apply immediately. Call `config.save_to_file()` to persist runtime changes back to disk.

> **Note:** `save_to_file()` currently persists only the `[render]`/`[window]` sections — changes to `[simulation]`/`[audio]` keys are read once at startup and are not written back to disk by this call.

---

## 10. Building and Running

### Build requirements

**All platforms:**

- Rust stable (edition 2024)
- CMake 3.10+ (for raylib compilation)
- C/C++ compiler (gcc/clang on Linux, MSVC on Windows)

**Linux (Debian/Ubuntu):**

```bash
sudo apt install build-essential pkg-config cmake \
  libx11-dev libxcursor-dev libxinerama-dev libxrandr-dev libxi-dev \
  libgl1-mesa-dev libegl1-mesa-dev libgbm-dev \
  libwayland-dev libwayland-egl1-mesa libxkbcommon-dev \
  libasound2-dev libpulse-dev libfreetype6-dev libjpeg-dev libpng-dev
```

**Windows:**

- Visual Studio with CMake + Windows SDK
- LLVM for Windows (in PATH)

### Building

```bash
cargo build                    # Debug build
cargo build --release          # Release build (recommended for playtesting)
```

First build takes ~5–15 minutes (compiles raylib from source via cmake). Incremental builds are fast.

### Running

```bash
cargo run                      # Run debug build
cargo run --release            # Run release build
RUST_LOG=info cargo run        # With engine logging
```

Working directory matters — `config.ini` and `assets/` are loaded relative to where you run the binary.

### Feature flags

| Flag | Default | Effect |
|------|---------|--------|
| `lua` | on | Lua scripting support (mlua + LuaJIT) |

```toml
# Disable Lua (pure Rust)
aberredengine = { path = "../aberredengine", default-features = false }
```

Disabling Lua removes: mlua dependency, LuaJIT compilation, all Lua-specific systems. Faster builds, smaller binary.

### Optimization tip

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```
