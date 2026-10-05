# Aberred Engine — Rust Game Developer Guide

This guide explains how to build a 2D game in pure Rust using the Aberred Engine as a library dependency, without Lua scripting.

## 1. What the Engine Provides

Aberred Engine is a 2D game engine built on **Bevy ECS 0.19** and **sola-raylib 6.3**. It handles the main loop, windowing, rendering, and a full ECS system schedule. You supply game-specific logic via hook functions.

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
- **Timers** — repeating or one-shot countdown timers that trigger a `TimerFired` event
- **Phase state machines** — per-entity state machines over named phases, with `PhaseEntered`/`PhaseExited` events
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
aberredengine = { path = "../aberredengine/crates/aberredengine", default-features = false }
log = "0.4"          # log::info!/warn!/error! from your own code
env_logger = "0.11"  # prints the engine's and your log output
```

For a git dependency:

```toml
[dependencies]
aberredengine = { git = "https://github.com/idaho06/aberredengine.git", default-features = false }
```

Cargo finds the `aberredengine` package by name inside the repository's workspace. Add `branch = "..."`, `tag = "..."` or `rev = "..."` to pin a specific revision.

The `path` form must point at the facade crate (`crates/aberredengine`), not the repository root: the root `Cargo.toml` is a virtual workspace manifest with no `[package]`.

Setting `default-features = false` disables the `lua` feature flag. This removes all mlua/LuaJIT dependencies, the Lua runtime, and all Lua-specific components and systems. Your binary will have zero Lua overhead.

The facade re-exports `bevy_ecs`, `glam`, `imgui` and `raylib`, but not `rustc-hash` or `log`: add those yourself when your code names them. The engine logs through `log` and installs no logger, so nothing is printed until your `main` installs one before building the engine:

```rust
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
```

### Imports

Start every game module with the prelude:

```rust,ignore
use aberredengine::prelude::*;
```

It brings in the `bevy_ecs` prelude (`Commands`, `Query`, `Res`, `On`, …) and the `bevy_ecs` crate itself, so `#[derive(Component)]`, `#[derive(Resource)]` and `#[derive(Event)]` work; the math types (`Vec2`, `Color`, `Rect`); the common components, resources, events and commands; the scene events (`SceneEntered`, `SceneExited`) and the `in_scene` run condition; and `EngineBuilder`, `SimSet` and `EngineError`. Less common items keep their full path under `aberredengine::core::...`, and the examples below import those explicitly. The render-thread types a GUI callback receives (`TextureStore`, `FontStore`, `GuiCtx`, `GuiCallback`) are in the prelude and in `aberredengine::render`; the rest of the render thread is out of reach of game code.

The `bevy_ecs` prelude has its own `Result` (`Result<T = (), E = BevyError>`) and a lifecycle event named `Add`, so in a module that glob-imports the prelude they shadow `std::result::Result` and `std::ops::Add`. `Result<T, E>` with an explicit error type is still the standard `Result`; write `std::ops::Add` in full when implementing it.

### Recommended directory layout

```
my_game/
├── Cargo.toml
├── config.ini                 # Engine configuration (optional; missing keys use defaults)
├── src/
│   ├── main.rs                # EngineBuilder entry point
│   └── scenes/
│       ├── mod.rs             # Re-exports scene modules
│       ├── menu.rs            # Menu scene observers and systems
│       └── level01.rs         # Gameplay scene observers and systems
└── assets/
    ├── textures/              # PNG images
    ├── fonts/                 # TTF fonts
    ├── audio/                 # WAV/OGG sounds and music
    └── shaders/               # GLSL fragment shaders (.fs)
```

### config.ini

The engine reads `config.ini` at startup for window and rendering settings. The file is optional: if it is missing, the engine logs a warning and starts with all defaults, and individual missing keys fall back to safe defaults too. A file that exists but cannot be read or parsed is a startup error. Out-of-range numbers are clamped with a warning; values that fail to parse (e.g. `hz = abc`, `vsync = no`) silently keep their defaults, with no warning.

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
pixel_snap_camera = true       ; Snap camera target/view-rect to integer pixels (avoids atlas bleeding)
render_target_filter = nearest ; Sampling filter for the render target: nearest|bilinear|trilinear|anisotropic_4x|anisotropic_8x|anisotropic_16x

[window]
width = 1280                   ; Window width in pixels
height = 720                   ; Window height in pixels
target_fps = 120               ; Target frames per second
vsync = true                   ; Enable vertical sync
fullscreen = false             ; Start in fullscreen mode

[simulation]
hz = 240                       ; Logic thread's sim tick rate (see Threading Model, Section 3)
; snapshot_skip = 1             ; Optional; PRESENT runs every N+1th sim tick; defaults to round(hz/target_fps)-1

[audio]
hz = 100                       ; Audio thread's tick rate

[input]
gamepad_deadzone = 0.15        ; Analog-stick deadzone radius for InputBinding::GamepadAxis, clamped to 0.0-1.0
```

---

## 3. EngineBuilder

The engine owns the main loop. You configure it through `EngineBuilder` and supply game logic via hook functions. There are two approaches depending on whether your game has multiple scenes.

### Threading Model: What Your Code Can Access

The engine runs **three separate ECS worlds on three threads**: render (the main thread — owns the raylib window and GPU resources), logic/sim (a spawned thread — this is where **all your game code runs**), and audio (a spawned thread, talked to via message queues you already use for sound/music). Understanding this is required to use the rest of this guide correctly.

- **Every hook and callback you write — `on_setup`, scene observers and systems (`.on_scene_enter()`/`.on_scene_exit()`/`.add_scene_system()`), systems added via `.add_system()`/`.configure_schedule()`, and observers (`.add_observer()`, `.observe()`, e.g. on `TimerFired` or `Collided`) — runs on the LOGIC thread**, once per **sim tick**. Sim ticks happen at a configurable rate (`[simulation] hz` in `config.ini`, default 240) **decoupled from your render frame rate** — a sim tick is not the same thing as a rendered frame. `dt` is always the fixed constant `1.0 / hz` (scaled by `WorldTime.time_scale`), never a measured wall-clock value — a render stall does not spike `dt` or replay missed ticks, it simply dilates game time until the stall clears, and sim ticks keep running throughout.
- **Scene GUI and world-draw callbacks (`.add_scene_gui()`/`.add_scene_world_draw()`) are the one exception** — they run on the RENDER thread, inside the render pass itself. This is why they take a context struct (`GuiCtx`/`WorldDrawCtx`) of read-only snapshots instead of live, mutable `WorldSignals` — see below.
- **A direct consequence**: logic-thread code (everything in the first bullet) can **never** take `RaylibAccess`, `NonSend<FontStore>`, `NonSend<ShaderStore>`, or `Res<TextureStore>` as a system parameter — those resources only exist in the render world. A logic-side system that requests one panics the first time it runs: Bevy checks a system's resources when the system runs, not when the schedule is built. There is no escape hatch to register a custom system on the render thread. This is why asset loading (Section 4) goes through a message queue instead of calling `rl.load_texture(...)` directly inside `setup()`.

### Approach A — SceneManager (recommended for multi-scene games)

Register named scenes, then attach behavior to them: `.on_scene_enter()`/`.on_scene_exit()` observers run when a scene becomes active or is left, and `.add_scene_system()` systems run once per sim tick while it is active (see [Scene-scoped systems and observers](#scene-scoped-systems-and-observers)). A scene's optional GUI and world-space draw callbacks are registered with `.add_scene_gui()`/`.add_scene_world_draw()`. Every scene switch despawns the non-persistent entities.

```rust
use aberredengine::prelude::*;

mod scenes;

fn main() -> Result<(), EngineError> {
    EngineBuilder::new()
        .config("config.ini")
        .title("My Game")
        .on_setup(scenes::load_assets)
        .add_scene("menu")
        .add_scene("level01")
        .on_scene_enter("menu", scenes::menu::enter)
        .add_scene_system("menu", scenes::menu::update)
        .on_scene_enter("level01", scenes::level01::enter)
        .add_scene_system("level01", scenes::level01::update)
        .on_scene_exit("level01", scenes::level01::exit)
        .initial_scene("menu")
        .try_run()
}

```

Scene observer and system signatures — ordinary Bevy observers and systems, with any parameters:

```rust
use aberredengine::prelude::*;

// Observes SceneEntered: runs each time the scene becomes active, after the old scene is torn down (logic thread)
fn enter(_: On<SceneEntered>, mut commands: Commands) { /* spawn entities, set signals */ }

// Scene system: runs once per sim tick while the scene is active (logic thread)
fn update(time: Res<WorldTime>, input: Res<InputState>) { /* per-tick logic */ }

// Observes SceneExited: runs when the scene is left, before its entities are despawned (logic thread)
fn exit(_: On<SceneExited>, mut signals: ResMut<WorldSignals>) { /* save state */ }

// Called every render frame to draw ImGui widgets — Rust-only, optional, RENDER thread
fn my_gui(ctx: &mut GuiCtx) { /* draw with ctx.ui, queue signal writes, read typed state */ }

// Called every render frame inside begin_mode2D in world space — Rust-only, optional, RENDER thread
fn my_world_draw(ctx: &mut WorldDrawCtx) { /* draw with ctx.draw, read camera/screen/app state */ }
```

To trigger a scene transition from a system or observer, call `WorldSignals::request_scene(name)`. It sets the target scene name (`sk::SCENE`) and the `sk::SWITCH_SCENE` flag; the engine's `scene_switch_poll` system (registered automatically for every Rust game) picks up the flag each sim tick and triggers the transition.

```rust
fn update(mut signals: ResMut<WorldSignals>) {
    if some_condition() {
        signals.request_scene("level01");
    }
}
```

> **Tip:** Scene transitions can be triggered from any logic-thread system or observer by setting the `"switch_scene"` flag on `WorldSignals` — the engine polls this automatically. For menu-driven transitions, `MenuAction::SetScene` handles the switch internally. See [Section 6.2](#62-triggering-scene-transitions) for details.

### ImGui GUI callback (Rust-only)

A scene's GUI callback, registered with `.add_scene_gui(scene, callback)`, draws an ImGui overlay every frame while that scene is active — useful for editors, debug tools, and dev GUIs. It runs whether or not F11 debug mode is active, inside the same ImGui frame as the debug panels.

**This callback runs on the render thread** (see [Threading Model](#threading-model-what-your-code-can-access) above) — unlike every other hook in this guide, which runs on the logic thread. That's why it can't reach a live `&mut WorldSignals`: the render thread never holds one. Instead it takes a `&mut GuiCtx`, whose fields are a read-only snapshot and a write-queue:

- `ui: &imgui::Ui` for drawing widgets
- `signals: &SignalSnapshot` — a read-only, frame-stale-by-one-tick copy of `WorldSignals`. Read it with the same getters as `WorldSignals` (`ctx.signals.get_scalar("key")`, `ctx.signals.has_flag("key")`, etc.), which both types implement through the `SignalsRead` trait in the prelude.
- `intents: &mut SignalIntents` — queue writes here (`ctx.intents.set_flag("key")`, `ctx.intents.set_scalar("key", 1.0)`, etc. — the method names mirror `WorldSignals`' setters). Queued writes are applied to the live `WorldSignals` on the logic thread at the start of its next sim tick — not immediately.
- `textures: &TextureStore` for texture previews
- `fonts: &FontStore` for font access (e.g. measuring text)
- `app_state: &AppState` for richer Rust-only typed snapshots/view-models produced by systems and observers

`AppState` is inserted automatically by the engine and stores one value per Rust type. Use newtypes when you need two values of the same underlying type.

```rust
use aberredengine::prelude::*;

#[derive(Clone)]
struct EditorPanelState {
    active_tool: String,
}

fn editor_gui(ctx: &mut GuiCtx) {
    let GuiCtx { ui, intents, app_state, .. } = ctx;

    // Read typed state written by ECS systems
    let tool = app_state
        .get::<EditorPanelState>()
        .map(|s| s.active_tool.as_str())
        .unwrap_or("select");

    ui.text(format!("Active tool: {tool}"));

    if let Some(_mb) = ui.begin_main_menu_bar()
        && let Some(_file) = ui.begin_menu("File")
        && ui.menu_item("Save")
    {
        intents.set_flag("gui:action:file:save"); // consumed by editor_update next sim tick
    }
}

fn editor_update(mut app_state: ResMut<AppState>, mut signals: ResMut<WorldSignals>) {
    // Every insert bumps AppState's generation, so the snapshot clones it again.
    // A real game inserts only when the value changes (see the AppState API section).
    app_state.insert(EditorPanelState {
        active_tool: "place".to_string(),
    });

    if signals.take_flag("gui:action:file:save") {
        // handle save
    }
}
```

Register both for the scene:

```rust
.add_scene_system("editor", editor_update)
.add_scene_gui("editor", editor_gui)
```

> **Convention:** prefix all GUI signal keys with `"gui:"` to avoid collisions with game signals. Use `"gui:action:<verb>"` for flags set by the GUI and consumed by a scene system, and `"gui:state:<name>"` for values set by a scene system and read by the GUI.

### World-space draw callback (Rust-only)

A scene's world-draw callback, registered with `.add_scene_world_draw(scene, callback)`, draws world-space overlays every frame while that scene is active, inside the render pass's `begin_mode2D` block. Use it for things like debug paths, editor gizmos, selection boxes, or navigation links that should follow the active camera transform.

**Like the GUI callback, this runs on the render thread** (see [Threading Model](#threading-model-what-your-code-can-access)) — its signals are a read-only `&SignalSnapshot`, not a live `&WorldSignals`.

The callback takes a `&mut WorldDrawCtx` with these fields:

- `draw: &mut dyn WorldDraw` with a minimal object-safe drawing API
- `camera: &Camera2D` for the active render camera
- `screen: &ScreenSize` for the internal game resolution
- `app_state: &AppState` for typed Rust-only snapshots
- `signals: &SignalSnapshot` for read-only signal access (the `SignalsRead` getters — `ctx.signals.has_flag("key")`; there's no write side here, a world-draw callback only draws)

Use the fields through `ctx`, or unpack them with `let WorldDrawCtx { draw, camera, .. } = ctx;`. Either `ctx.draw` or the unpacked `draw` passes straight to your own helpers that take `&mut dyn WorldDraw`, as below.

```rust
use aberredengine::prelude::*;

fn editor_world_draw(ctx: &mut WorldDrawCtx) {
    // `..` is required: WorldDrawCtx is #[non_exhaustive]
    let WorldDrawCtx { draw, .. } = ctx;
    draw_origin_cross(draw);
}

fn draw_origin_cross(draw: &mut dyn WorldDraw) {
    draw.draw_line_v(
        Vec2::new(-32.0, 0.0),
        Vec2::new(32.0, 0.0),
        Color::GREEN,
    );
    draw.draw_line(-16, -16, 16, 16, Color::YELLOW);
}
```

Register both render callbacks for the scene:

```rust
.add_scene_gui("editor", editor_gui)
.add_scene_world_draw("editor", editor_world_draw)
```

### Single-scene games

A game that registers no scene runs in one implicit scene named `"main"`. Load assets in `.on_setup()`, spawn the world when `"main"` is entered, and add per-tick systems with `.add_system()`. Every part is optional:

```rust
use aberredengine::prelude::*;

fn main() -> Result<(), EngineError> {
    EngineBuilder::new()
        .config("config.ini")
        .title("My Game")
        .on_setup(my_setup)
        .on_scene_enter("main", my_enter)
        .add_system(my_update)
        .try_run()
}
```

Add scenes with `.add_scene()` later and the same observers and systems move to their scenes.

### Startup error handling

Prefer `EngineBuilder::try_run()` in Rust applications. It returns `Result<(), aberredengine::EngineError>` for startup failures such as invalid builder configuration, an unreadable or unparsable `config.ini`, render-target creation failures, and Lua runtime creation failures.

`EngineBuilder::run()` is still available as a convenience wrapper around `.try_run()`, but on startup failure it logs the error, prints it to stderr, and exits the process with status 1 instead of returning it to your `main` function.

Each hook and system is a standard Bevy ECS system — it receives queries and resources as parameters. For example:

```rust
use aberredengine::prelude::*;

fn my_update(signals: ResMut<WorldSignals>, input: Res<InputState>) {
    if input.action(InputAction::Action1).just_pressed {
        // ...
    }
}
```

Bound actions are looked up by `InputAction` (`input.action(..)`, or
`input.action_mut(..)` in tests). The raw `mouse_left_button`, the mouse
coordinates, `scroll_y` and the gamepad axes are plain fields.

### Game lifecycle

```
Setup ──→ Playing ──→ Quitting
              ↑   ↓
          scene switches
```

1. **Setup** — The engine calls the `setup` hook once, on the logic thread. Load assets here (textures, fonts, sounds, shaders, animations) — see [Section 4](#4-loading-assets) for how texture/font/shader loading works. Omit `.on_setup()` if you have nothing to load. With `.loading_scene(name)`, that scene is entered right after the hook and stays active until Setup ends. Once the hook has run, the engine moves to `Playing` on its own, as soon as every load queued so far has been answered (see [Waiting for loads](#waiting-for-loads)); a failed load counts as answered. A hook that requests `Playing` itself waits the same way. A hook that requests another state through `ResMut<NextGameState>` (e.g. `GameStates::Quitting`) gets it at once.
2. **Playing** — The engine transitions to playing, enters the initial scene (triggering `SceneEntered`), then runs your `.add_system()` systems (and the active scene's `.add_scene_system()` systems) once per sim tick.
3. **Scene switches** — `WorldSignals::request_scene(name)` (which sets the `"switch_scene"` flag) runs the exit→enter sequence on the next sim tick; a `MenuAction::SetScene` menu item switches right away.
4. **Quitting** — When the game state becomes `GameStates::Quitting` (via `NextGameState::set(GameStates::Quitting)`, `WorldSignals::request_quit()`, or a `MenuAction::QuitGame` menu item) or the window is closed, the engine shuts down. Observers and systems take `ResMut<WorldSignals>` or `ResMut<NextGameState>` to request it.

### Builder method reference

| Method | Description |
|--------|-------------|
| `.config(path)` | Path to `config.ini` (default: `"config.ini"`) |
| `.config_str(content)` | Load INI config from an embedded `&'static str` instead of a file. Takes precedence over `.config(path)`. Useful for tests or games that ship with bundled defaults. |
| `.title(name)` | Window title (overrides config) |
| `.on_setup(system)` | Asset loading hook (called once in the `Setup` state, on the logic thread). Optional; the engine moves to `Playing` after it runs. |
| `.add_scene(name, descriptor)` | Register a named scene (SceneManager path) |
| `.add_scene_gui(scene, callback)` | Draw ImGui widgets every render frame while `scene` is active (render thread). See [ImGui GUI callback](#imgui-gui-callback-rust-only). |
| `.add_scene_world_draw(scene, callback)` | Draw world-space overlays every render frame while `scene` is active (render thread). See [World-space draw callback](#world-space-draw-callback-rust-only). |
| `.initial_scene(name)` | Which scene starts first (required with `.add_scene()`) |
| `.loading_scene(name)` | A registered scene shown while Setup waits for assets. See [Loading screen](#loading-screen). |
| `.track_group(name)` | Count group `name`'s entities for the whole game (published as `"group_count:<name>"`, kept across scene switches). Can be called multiple times. See [Group tracking across scenes](#64-group-tracking-across-scenes). |
| `.add_system(system)` | Add a per-sim-tick system, run only while `Playing` (`run_if(state_is_playing)`, ordered alongside script-update systems). Can be called multiple times. A sim tick is not the same as a render frame; see [Threading Model](#threading-model-what-your-code-can-access). |
| `.configure_schedule(closure)` | Add systems to the same schedule `.add_system()` targets, with full ordering control — no auto-constraints applied. Use this for custom ordering relative to the engine's own systems (via `SimSet` or `.after()`/`.before()`). |
| `.add_system_if(system, condition)` | `.add_system()` plus a run condition, e.g. `in_scene("level01")`. The condition is a separate argument because a `.run_if(..)`-configured system can't be passed to the builder. |
| `.add_scene_system(scene, system)` | A per-sim-tick system that runs whenever `scene` is active (like `.add_system_if(system, in_scene(scene))`, and also during `Setup` if `scene` is active then). |
| `.add_observer(observer_fn)` | Register a persistent observer for a custom or engine event. |
| `.on_scene_enter(scene, observer_fn)` / `.on_scene_exit(scene, observer_fn)` | Observe `SceneEntered`/`SceneExited` for one scene only. See [Scene-scoped systems and observers](#scene-scoped-systems-and-observers). |
| `.with_lua(path)` | **Lua builds only** (`lua` feature). Run a Lua game from the `main.lua` at `path`. Listed here because the conflict rules below refer to it; a pure-Rust game never calls it. |
| `.deterministic(seed)` | Seed `SimRng` from `seed` instead of entropy, pinning simulation randomness to a known value. Mutually exclusive with `.with_lua()`. See [Determinism and replay](#determinism-and-replay) below. |
| `.record_replay(path, game_version)` | Record this session's input stream to `path`. Requires `.deterministic(seed)` first; mutually exclusive with `.play_replay()`. |
| `.play_replay(path)` | Replay a previously recorded file from `path` instead of live input. The seed comes from the file's header — do not also call `.deterministic()`. Mutually exclusive with `.record_replay()`. |
| `.try_run()` | Start the engine and return `Result<(), aberredengine::EngineError>` on startup failure. Recommended for Rust `main`. |
| `.run()` | Convenience wrapper around `.try_run()` that logs the error, prints it to stderr, and exits the process with status 1 on startup failure. |

**Conflict rules:** `.add_scene()` cannot be combined with `.with_lua()` — a Lua game drives scenes from `main.lua`'s scene registry instead. `.add_scene()` requires `.initial_scene(...)`, and that name must match a scene actually registered via `.add_scene()` — a missing or misspelled `.initial_scene(...)` is a startup error, and so is calling `.initial_scene(...)` with no `.add_scene()` calls at all. Every scene named by `.add_scene_system()`, `.on_scene_enter()`/`.on_scene_exit()` or `.add_scene_gui()`/`.add_scene_world_draw()` must be registered too (the implicit `"main"` counts). `.with_lua()` also conflicts with an explicit `.on_setup()` call, in either order — it installs its own setup hook, and mixing in yours is ambiguous. `.add_system()` combines with `.with_lua()` freely. With `.try_run()`, all of these are returned as startup errors instead of panicking; `.run()` prints the error to stderr and exits with a nonzero status instead of failing silently.

### Custom systems and observers

These builder methods let you register multiple independent ECS systems and event observers alongside the existing hooks. **All of them run on the logic thread**, on the same schedule as every other per-tick system in this guide — see [Threading Model](#threading-model-what-your-code-can-access). None of them can take `RaylibAccess`/`NonSend<FontStore>`/`NonSend<ShaderStore>`/`Res<TextureStore>`; such a system panics the first time it runs, whichever of these methods registered it.

#### `.add_system(system)` — multiple per-sim-tick systems

Registers a Bevy ECS system that runs once per sim tick while `Playing` (`run_if(state_is_playing)`), ordered alongside the engine's own script-update systems. Can be called multiple times.

```rust
EngineBuilder::new()
    .config("config.ini")
    .on_setup(load_assets)
    .add_system(tilemap_load_system)   // checks a signal each tick, then queues a load
    .add_system(tilemap_save_system)   // independent second system
    .add_scene("editor")
    .initial_scene("editor")
    .try_run()
    .expect("engine startup failed");
```

The system signature is a standard Bevy ECS system. Since it runs on the logic thread, it cannot touch GL resources directly — queue a `RenderAssetCmd` instead (see [Section 4](#4-loading-assets)):

```rust
use aberredengine::prelude::*;

fn tilemap_load_system(
    mut world_signals: ResMut<WorldSignals>,
    mut asset_cmds: MessageWriter<RenderAssetCmd>,
) {
    let Some(path) = world_signals.remove_string("pending_load_path") else {
        return; // nothing to do this tick
    };
    asset_cmds.write(RenderAssetCmd::Texture {
        key: path.clone(),
        path,
        filter: TextureFilter::Nearest,
    });
    // spawn tile entities, etc. — the texture itself loads asynchronously
}
```

> **When to use `.add_system()` vs `.add_scene_system()`:** both take ordinary systems with any parameters. Use `.add_scene_system(scene, ..)` for logic that belongs to one scene, and `.add_system()` for logic that runs whichever scene is active.

#### `.configure_schedule(closure)` — full ordering control

For systems that need custom ordering relative to engine systems, or that must run outside the `Playing` state, pass a closure receiving `&mut Schedule`. This targets the exact same schedule as `.add_system()` — all logic-thread, once per sim tick:

```rust
use aberredengine::core::systems::movement::movement;
use aberredengine::core::systems::camera_follow::camera_follow_system;
use aberredengine::prelude::*;

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

Engine system functions are `pub` and importable from `aberredengine::core::systems::*`. Use them directly as `.after()` / `.before()` arguments — but only systems that live on this same logic-thread schedule; render-thread systems like `render_system` are never reachable from here, ordering relative to them is not expressible. No automatic `run_if` or `after` constraints are applied — you control everything.

#### `.add_observer(observer_fn)` — persistent event observers

Registers a Bevy ECS observer that fires when a specific event is triggered. The observer survives scene transitions (spawned with the `Persistent` component) — it is always active, not tied to a specific scene.

**Define a custom event:**

```rust
use aberredengine::prelude::*;

#[derive(Event)]
struct TilemapLoaded {
    pub path: String,
}
```

`use aberredengine::prelude::*;` also brings the `bevy_ecs` crate into scope, which the derive macro needs: it expands to `bevy_ecs::...` paths. Without the prelude, add `use aberredengine::bevy_ecs;`.

**Define the observer function** — first parameter must be `On<E>`:

```rust
use aberredengine::prelude::*;

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

**Trigger the event from any system or observer:**

```rust
// From a Bevy ECS system:
fn my_system(mut commands: Commands) {
    commands.trigger(TilemapLoaded { path: "maps/level01.json".into() });
}

// From a scene observer:
fn my_enter(_: On<SceneEntered>, mut commands: Commands) {
    commands.trigger(TilemapLoaded { path: "maps/intro.json".into() });
}
```

> You can also observe engine-defined events: `CollisionEvent`, `TimerFired`, `InputEvent`, `GameStateChangedEvent`, `WindowResizedEvent`, etc. Note that not every engine type that crosses a system boundary is an `Event` — `AudioCmd` and `RenderAssetCmd`, for example, are Bevy `Message`s (`MessageWriter`/`MessageReader`, queue-based), a different mechanism from `Commands::trigger`/`.add_observer()`.

#### Scene-scoped systems and observers

With `.add_scene()`, every scene has a persistent scene entity. A Rust game that registers no scene (and no `.initial_scene()`) runs in an implicit scene named `"main"` (`signal_keys::MAIN_SCENE`), so these methods work with `"main"` too. Each scene switch triggers `SceneExited` for the old scene, then `SceneEntered` for the new one, both targeted at that entity:

- `SceneExited { scene, name, next }` fires before the old scene is torn down: its entities are still alive.
- `SceneEntered { scene, name, previous }` fires after the teardown, so entities its observers spawn belong to the new scene. It also fires for the initial scene, with `previous: None`.

`.on_scene_enter(scene, ..)`/`.on_scene_exit(scene, ..)` attach an observer to one scene's entity; `.add_observer()` sees every scene and reads `name` from the event. `.add_scene_system(scene, ..)` and `.add_system_if(.., in_scene(..))` run a system only while a scene is active. Every scene these methods name must be registered with `.add_scene()`, or startup fails with `EngineError::SceneNotRegistered`.

```rust
use aberredengine::prelude::*;

// Runs once per sim tick, only while "level01" is active.
fn level_tick(mut signals: ResMut<WorldSignals>) {
    signals.set_flag("level01_ticking");
}

// Runs while either level is active.
fn hud_update(time: Res<WorldTime>) {
    let _elapsed = time.elapsed;
}

// Fires each time "level01" becomes active, after the previous scene is torn down.
fn spawn_level(_: On<SceneEntered>, mut commands: Commands) {
    commands.spawn((Group::new("player"), MapPosition::new(100.0, 200.0)));
}

// Fires each time "level01" is left, while its entities are still alive.
fn save_score(_: On<SceneExited>, players: Query<&Signals, With<Group>>) {
    let _count = players.iter().count();
}

// A global observer fires for every scene; the event carries the names.
fn log_scene(ev: On<SceneEntered>) {
    log::info!("entered {} from {:?}", ev.name, ev.previous);
}

fn register(builder: EngineBuilder) -> EngineBuilder {
    builder
        .add_scene_system("level01", level_tick)
        .add_system_if(hud_update, in_scene("level01").or_else(in_scene("level02")))
        .on_scene_enter("level01", spawn_level)
        .on_scene_exit("level01", save_score)
        .add_observer(log_scene)
}
```

Scene systems run in `SimSet::ScriptUpdate`, before movement and collision, so they see the state the previous tick left.

#### Scene-scoped (transient) observers

Observers registered with `.add_observer()` are always active. For observers that should only fire within a specific scene, spawn them from the scene's `SceneEntered` observer **without** the `Persistent` component:

```rust
use aberredengine::prelude::*;

fn editor_enter(_: On<SceneEntered>, mut commands: Commands) {
    // This observer lives only until the next scene switch.
    // The scene switch despawns every non-Persistent entity, this observer included.
    commands.spawn(Observer::new(on_tile_selected));
}

fn on_tile_selected(trigger: On<TileSelectedEvent>, /* params */) {
    // only fires while the editor scene is active
}
```

This is the standard pattern for scene-scoped behaviour in the engine — no special API needed.

### Determinism and replay

By default, simulation randomness is seeded from entropy and every run of your game plays out differently. Call `.deterministic(seed)` to pin `SimRng` (the one RNG resource systems and observers should draw from for anything that must reproduce — using `rand::thread_rng()` or any other RNG source instead breaks determinism even with `.deterministic()` set) to a known seed. Combined with the engine's fixed-`dt` sim tick and single-threaded schedule executor, this makes a run fully reproducible from its seed and input stream alone.

```rust
EngineBuilder::new()
    .config("config.ini")
    .deterministic(42)
    // …
    .try_run()
    .expect("engine startup failed");
```

**Drawing random numbers.** `SimRng` (`aberredengine::core::resources::sim_rng::SimRng`) is pre-inserted in every game, deterministic or not. Its field is a `fastrand::Rng`: call its methods (`f32()`, `usize(range)`, `i32(range)`, `bool()`, `shuffle(..)`, …) through `ResMut<SimRng>` in a system or observer. Calling methods needs no `fastrand` dependency; naming the `fastrand::Rng` type does.

```rust
use aberredengine::prelude::*;

// In a system
fn pick_spawn_point(mut rng: ResMut<SimRng>) {
    let x = rng.0.f32() * 640.0; // 0.0..640.0
    let lane = rng.0.usize(0..4); // 0, 1, 2 or 3
    let flip = rng.0.bool();
}
```

The engine's particle emitters draw from the same `SimRng`. A run reproduces only if the same draws happen in the same order, so draw from it only in sim-side code (systems and observers), and keep any other RNG out of gameplay. Without `.deterministic()`, the engine logs the seed it picked at startup (`Non-deterministic mode: SimRng entropy-seeded with …`), so a surprising session's seed is in the log.

Once deterministic mode is on, you can record or replay the input stream that drove a session:

```rust
// Record this session's input to a file:
EngineBuilder::new()
    .deterministic(42)
    .record_replay("session01.replay", env!("CARGO_PKG_VERSION"))
    // …
    .try_run()
    .expect("engine startup failed");

// Later, replay it deterministically instead of reading live input:
EngineBuilder::new()
    .play_replay("session01.replay")
    // seed comes from the file's header — do not also call .deterministic()
    // …
    .try_run()
    .expect("engine startup failed");
```

`.record_replay()` requires `.deterministic(seed)` to already be set, and is mutually exclusive with `.play_replay()`. `.play_replay()` reads its seed from the replay file's header, so it must **not** be combined with `.deterministic()`. Both `.deterministic()` and its replay variants are mutually exclusive with `.with_lua()` — Lua scripting sits outside the deterministic envelope. Violating any of these rules is reported as a startup error from `.try_run()`/`.run()`, not a panic.

This is useful for automated testing (replay a fixed input script, assert on the resulting state), bug reports (a player-submitted replay reproduces the exact conditions that triggered a bug), and any gameplay mode that depends on reproducible simulation.

**The envelope starts at `Playing`.** Setup lasts as long as its asset loads take, which varies from run to run. So the reproducible part of a session starts at the first `Playing` tick:

- `WorldTime` doesn't advance during Setup (in every mode): `elapsed` and `frame_count` are `0` on the first `Playing` tick, and `delta` stays `0` until then.
- A replay records and plays back from the first `Playing` tick; Setup runs live, loads included, in both.
- In deterministic mode, input that arrives during Setup is held back. Key and mouse samples are dropped, so F10/F11 do nothing while loading. GUI intents and the latest screen size are applied on the first `Playing` tick.
- Engine systems that don't use `delta` still run during Setup: collision rules, `phase_system` (phase events), group counts. Spawn gameplay entities in the initial scene's `SceneEntered` observer, not in the setup hook, so a longer Setup can't change them.

**Assets load during Setup.** A load finishes at a wall-clock time, so a deterministic game can't change its loaded assets while `Playing`:

- `AssetLoader::load_*` for a key that is already loaded returns `Ok` and does nothing. For a key that isn't, it returns `Err(AssetError::AssetChangeDuringDeterministicPlay { .. })` and queues nothing.
- A command written through a raw `MessageWriter<RenderAssetCmd>`/`MessageWriter<AudioCmd>` (or `assets.render()`/`assets.audio()`) has no result to return. A new load, or a removal, rename or unload of a loaded asset, is dropped and logged as an error, and panics in debug builds. Commands that change nothing (reloading a loaded key, removing one that isn't loaded) are dropped silently.
- Texture filter changes and audio playback work as usual.
- Engine features that load on the fly follow the same rule. Load each tilemap's atlas in Setup with `assets.load_tilemap(dir)` (same `dir` as its `TileMap`), spawn maps whose assets were loaded in Setup, and keep menus on dynamic text (the default; `with_dynamic_text(false)` rasterizes labels into new textures).

---

## 4. Loading Assets

The setup hook is a standard Bevy ECS system running on the **logic thread** (see [Threading Model](#threading-model-what-your-code-can-access)). That means it **cannot** take `RaylibAccess`, `NonSendMut<FontStore>`, `NonSendMut<ShaderStore>`, or `ResMut<TextureStore>` — those resources exist only in the render world, on the render thread. Requesting any of them from `setup()` (or any other logic-side system) panics the first time that system runs.

Instead, asset loading is **queued** from the logic thread and **performed** elsewhere: textures, fonts and shaders on the render thread, sounds and music on the audio thread. Take the `AssetLoader` system param and call its `load_*` methods. Each load is asynchronous relative to the tick that requested it.

```rust
use aberredengine::prelude::*;

fn setup(mut assets: AssetLoader) -> Result {
    assets.load_texture("player", "assets/textures/player.png")?;
    assets.load_texture_with("background", "assets/textures/bg.png", TextureFilter::Bilinear)?;
    assets.load_font("arcade", "assets/fonts/arcade.ttf", 32)?;
    assets.load_shader("glow", None, Some("assets/shaders/glow.fs"))?;
    assets.load_sound("jump", "assets/audio/jump.wav")?;
    assets.load_music("bgm", "assets/audio/music.ogg")?;
    Ok(())
}
```

Each `load_*` returns `Result<(), AssetError>`. It only fails in a `.deterministic()` game that is already `Playing` (see [Determinism and replay](#determinism-and-replay)). Hooks and `.add_system()` systems may return bevy's `Result`, so `?` works there. An error from an `.add_system()` system goes to bevy's error handler; an error from a hook is logged as a warning.

`AssetLoader` also answers whether a load has landed: `is_texture_loaded(key)`, `texture_size(key)` and `is_font_loaded(key)` return `false`/`None` until the render thread replies, usually a tick or two after the load was queued. Render assets and audio assets have separate key spaces, so a texture and a sound can share a key.

`AssetLoader` holds the `MessageWriter<RenderAssetCmd>` and the `MessageWriter<AudioCmd>`. A system that takes it must not also take either writer: Bevy rejects that system at startup with a conflicting-access panic. Write any other command through its passthroughs, `assets.audio()` and `assets.render()`, instead:

```rust
use aberredengine::prelude::*;

fn play_jump(mut assets: AssetLoader, input: Res<InputState>) {
    if input.action(InputAction::Action1).just_pressed {
        assets.audio().write(AudioCmd::PlayFx { id: "jump".into() });
    }
}
```

#### Waiting for loads

Every queued load stays in the `PendingAssets` resource until the render or audio thread answers it, whether it succeeded or failed. `PendingAssets::is_empty()` says every load has been answered; `len()` counts the loads still in flight and `contains(AssetKind::Texture, "player")` checks one. Each answer also triggers one global event, after the reply's data (texture size, font metrics) is stored:

```rust
use aberredengine::prelude::*;

fn on_asset_loaded(ev: On<AssetLoaded>) {
    log::info!("{:?} '{}' is ready", ev.kind, ev.key);
}

fn on_asset_failed(ev: On<AssetLoadFailed>, mut signals: ResMut<WorldSignals>) {
    // The engine has already logged the error.
    if ev.kind == AssetKind::Texture && ev.key == "player" {
        signals.request_quit();
    }
}

EngineBuilder::new()
    .add_observer(on_asset_loaded)
    .add_observer(on_asset_failed)
    // …
```

Setup waits for `PendingAssets` to empty (see [Game lifecycle](#game-lifecycle)). Loads count no matter how they were queued: through `AssetLoader`, a raw `MessageWriter<RenderAssetCmd>`/`MessageWriter<AudioCmd>`, a map spawn or a tilemap. Loading the same key twice counts twice and is answered twice.

#### Loading screen

`.loading_scene(name)` shows a registered scene while Setup waits. The engine enters it right after the setup hook, runs its `.add_scene_system()` systems every sim tick while loads are pending, and on `Playing` leaves it for the initial scene:

```rust
use aberredengine::prelude::*;

#[derive(Component)]
struct LoadingText;

fn load_assets(mut assets: AssetLoader) -> Result {
    assets.load_font("ui", "assets/fonts/ui.ttf", 16)?;
    assets.load_texture("player", "assets/textures/player.png")?;
    Ok(())
}

fn show_loading(_: On<SceneEntered>, mut commands: Commands) {
    commands.spawn((
        LoadingText,
        ScreenPosition::new(10.0, 10.0),
        DynamicText::new("Loading…", "ui", 16.0, Color::WHITE),
    ));
}

fn update_loading(
    pending: Res<PendingAssets>,
    mut texts: Query<&mut DynamicText, With<LoadingText>>,
) {
    if pending.is_changed() {
        for mut text in &mut texts {
            text.set_text(format!("Loading… ({} left)", pending.len()));
        }
    }
}

fn main() -> Result<(), EngineError> {
    EngineBuilder::new()
        .on_setup(load_assets)
        .add_scene("loading")
        .add_scene("level01")
        .initial_scene("level01")
        .loading_scene("loading")
        .on_scene_enter("loading", show_loading)
        .add_scene_system("loading", update_loading)
        .try_run()
}
```

- `WorldTime` doesn't advance during Setup, so a loading scene shows progress, not time-based animation (tweens, sprite animations).
- Its text appears once its font has loaded; until then it simply doesn't draw.
- Leaving it tears it down along with everything spawned during Setup, the setup hook's spawns included. Mark what must survive `Persistent`, or spawn it in the initial scene.
- During Setup the loading scene is the only scene: any other switch (`request_scene`, a menu's `SetScene`) is ignored with a warning, with or without a loading scene, and the engine always moves on to the initial scene. A quit requested during Setup takes effect on the first `Playing` tick.
- In a deterministic game, spawn only from the loading scene's `SceneEntered` observer, never from its per-tick systems: the number of Setup ticks depends on load times, and entity ids must not.
- The loading scene must differ from the initial scene.

The subsections below show the underlying `RenderAssetCmd`/`AudioCmd` variants; write any that `AssetLoader` has no helper for through `assets.render()`/`assets.audio()`.

### What's pre-inserted vs. what you must create

| Resource | Where it lives | Accessible from your logic-side code? |
|----------|-----------------|----------------------------------------|
| `FontStore` / `ShaderStore` / `TextureStore` / `RenderTarget` | Render world only | **No** — queue loads with `AssetLoader` instead |
| `FontMetricsStore` / `TextureDimsStore` | Logic world, pre-inserted | Yes — `Res<FontMetricsStore>` / `Res<TextureDimsStore>`; populated asynchronously after a load completes (see below) |
| `AnimationStore` | Logic world, pre-inserted | Yes — `ResMut<AnimationStore>` |
| `Camera2DRes` | Logic world, pre-inserted (pre-set to center offset) | Yes — `ResMut<Camera2DRes>` |

### Textures

Queue a load with `RenderAssetCmd::Texture`, passing the desired `TextureFilter` (use `TextureFilter::Nearest` for pixel art, `Bilinear`/`Trilinear`/`Anisotropic*` for smoothly scaled/rotated sprites):

```rust
use aberredengine::prelude::*;

asset_cmds.write(RenderAssetCmd::Texture {
    key: "player".to_string(),
    path: "assets/textures/player.png".to_string(),
    filter: TextureFilter::Nearest,
});
asset_cmds.write(RenderAssetCmd::Texture {
    key: "background".to_string(),
    path: "assets/textures/background.png".to_string(),
    filter: TextureFilter::Nearest,
});
```

Keys (`key`) are arbitrary strings you'll reference later in `Sprite` components — you can spawn a `Sprite` referencing `"player"` in the very same tick that queued the load; it just won't have anything to draw until the render thread's GL upload lands (typically the next tick or two).

**The load-then-use gap:** assets loaded in the setup hook are ready when Setup ends (show a [loading screen](#loading-screen) meanwhile), so this matters only for loads queued later. If your own logic-side code needs the texture's *dimensions* before the render thread has replied (e.g. to size a collider off a spritesheet), you can't read it back synchronously: the texture store lives on the render thread. Query `TextureDimsStore` instead, and tolerate `None` until the reply arrives:

```rust
use aberredengine::prelude::*;
use aberredengine::core::resources::texturedims::TextureDimsStore;

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
        Sprite::new("player", width as f32, height as f32).centered(),
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
    key: "intro_logo".to_string(),
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
    key: "arcade".to_string(),
    path: "assets/fonts/arcade.ttf".to_string(),
    size: 32,
    skip_if_loaded: false, // true = don't reload if "arcade" is already loaded
});
```

To measure text (e.g. to size UI elements) without touching the render-world `FontStore`, read `FontMetricsStore` — populated asynchronously the same way `TextureDimsStore` is, so tolerate a missing key the first tick or two after queuing the load:

```rust
use aberredengine::core::resources::fontmetrics::FontMetricsStore;

fn measure_label(fonts: Res<FontMetricsStore>) {
    if let Some(metrics) = fonts.0.get("arcade") {
        let size = metrics.measure_text("Hello!", 32.0, 1.0);
        // ... use size.x / size.y ...
    }
}
```

To remove a previously-loaded font (dropping its GPU handle and `FontStore` metadata), use `RenderAssetCmd::RemoveFont { key }`. The render thread also reports `LogicMsg::FontRemoved { key }`, which prunes the logic-side `FontMetricsStore` entry automatically.

To rename an already-loaded font's key in place (no reload, no glyph-atlas regeneration), use `RenderAssetCmd::RenameFont { old_key, new_key }`. It reports `LogicMsg::FontRenamed { old_key, new_key }`, which moves the `FontMetricsStore` entry to the new key automatically. No-ops with a warning if `old_key` isn't loaded.

To bake a string into a texture once (static labels that never change), use `RenderAssetCmd::RasterizeText`. The render thread draws `text` with the already-loaded font `font_key` into a new `Nearest`-filtered texture stored under `key`, then replies `LogicMsg::TextureLoaded`, so `TextureDimsStore` gets the label's pixel size:

```rust
use aberredengine::prelude::*;

asset_cmds.write(RenderAssetCmd::RasterizeText {
    key: "title_label".to_string(),
    font_key: "arcade".to_string(),
    text: "PRESS START".to_string(),
    font_size: 32.0,
    spacing: 1.0,
    color: Color::new(255, 255, 255, 255),
});
```

Queue it only after the font has loaded (wait for its `FontMetricsStore` entry): if `font_key` is missing on the render thread, the command logs an error and is dropped. Draw the result with a `Sprite` whose `tex_key` is `key`, and free it with `RenderAssetCmd::RemoveTexture` when done. The engine's menus use this command for their static labels.

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

#### Audio replies

The audio thread answers with `AudioMessage`s (`aberredengine::core::protocol::audio::AudioMessage`): `FxLoaded`/`FxLoadFailed { id, error }`, `MusicLoaded`/`MusicLoadFailed { id, error }`, `MusicPlayStarted`, `MusicStopped`, `MusicFinished` (non-looping music reached its end), `MusicVolumeChanged`, and the unload acknowledgements. There is no reply when a sound effect finishes. Read them with a `MessageReader<AudioMessage>`:

```rust
use aberredengine::prelude::*;
use aberredengine::core::protocol::audio::AudioMessage;

fn on_audio_replies(mut replies: MessageReader<AudioMessage>, mut audio: MessageWriter<AudioCmd>) {
    for reply in replies.read() {
        match reply {
            AudioMessage::MusicLoaded { id } if id == "bgm" => {
                audio.write(AudioCmd::PlayMusic { id: id.clone(), looped: true });
            }
            AudioMessage::FxLoadFailed { id, error }
            | AudioMessage::MusicLoadFailed { id, error } => {
                log::error!("audio '{id}' failed to load: {error}");
            }
            AudioMessage::MusicFinished { id } => log::info!("music '{id}' ended"),
            _ => {}
        }
    }
}

EngineBuilder::new()
    .configure_schedule(|schedule| {
        schedule.add_systems(on_audio_replies.in_set(SimSet::ScriptUpdate));
    })
    // …
```

- A reader in `SimSet::ScriptUpdate` or later sees a reply only during the sim tick it arrives in; after that it's dropped. Register the reader with `.configure_schedule()` as above, so it runs in every game state. An `.add_system()` reader runs only while `Playing` and misses the replies to loads queued during `Setup`.
- Replies arrive whenever the audio thread gets to them, not on a fixed tick, and replays don't record them. In a `.deterministic()` game, use them for logging and presentation only; don't let gameplay state depend on them.

### Shaders

Queue a load with `RenderAssetCmd::Shader`. `vs_path`/`fs_path` mirror raylib's `load_shader(vertex, fragment)` — pass `None` for a path to use raylib's default for that stage:

```rust
asset_cmds.write(RenderAssetCmd::Shader {
    key: "glow".to_string(),
    vs_path: None, // default vertex shader
    fs_path: Some("assets/shaders/glow.fs".to_string()),
});
```

There's no synchronous "did it load, is it valid" result: if the file is missing or fails validation, the render thread logs an error, doesn't register the shader and triggers `AssetLoadFailed` (see [Waiting for loads](#waiting-for-loads)). Reference the shader by its `key` from an `EntityShader` component (see [Per-entity shaders](#per-entity-shaders)); after a failed load, entities using that key draw without it.

To load a shader from in-memory source strings instead of file paths (e.g. shaders embedded via `include_str!`), use `RenderAssetCmd::ShaderFromMemory`:

```rust
asset_cmds.write(RenderAssetCmd::ShaderFromMemory {
    key: "glitch".to_string(),
    vs_src: None, // default vertex shader
    fs_src: Some(include_str!("../assets/shaders/glitch.fs").to_string()),
});
```

Same `None`-per-stage convention as `RenderAssetCmd::Shader`, and the same "no synchronous success/failure signal" caveat applies — a failed compile just logs an error and the shader key stays unregistered.

#### Per-entity shaders

Attach an `EntityShader` naming a loaded shader key to draw one entity through that shader. It applies to world-space sprites and `DynamicText` (entities with `MapPosition`); screen-space entities ignore it. If the key isn't loaded (yet, or because the load failed), the entity draws without the shader and the render thread logs a warning each frame.

```rust
use aberredengine::prelude::*;

fn spawn_glowing(mut commands: Commands) {
    let mut shader = EntityShader::new("glow");
    shader.set_uniform("uIntensity", UniformValue::Float(0.8));
    commands.spawn((
        // MapPosition, Sprite, ZIndex, … as in Section 5
        shader,
    ));
}

fn pulse_glow(mut shaders: Query<&mut EntityShader>, time: Res<WorldTime>) {
    for mut shader in &mut shaders {
        let intensity = 0.5 + 0.5 * time.elapsed.sin();
        shader.set_uniform("uIntensity", UniformValue::Float(intensity));
    }
}
```

`UniformValue` has `Float`, `Int`, `Vec2 { x, y }` and `Vec4 { x, y, z, w }`. Uniforms the shader doesn't declare are skipped. Before each draw, the engine also sets these uniforms when the shader declares them:

| Uniform | Type | Value |
|---------|------|-------|
| `uTime` | `float` | `WorldTime.elapsed` |
| `uDeltaTime` | `float` | `WorldTime.delta` |
| `uFrame` | `int` | `WorldTime.frame_count` |
| `uResolution` | `vec2` | Game (render) resolution |
| `uWindowResolution` | `vec2` | OS window size |
| `uLetterbox` | `vec4` | The entity's destination rectangle `(x, y, w, h)` |
| `uEntityId` | `int` | The entity's id bits (lower 32) |
| `uEntityPos` | `vec2` | World position |
| `uSpriteSize` | `vec2` | Sprite size, or the text's bounding box |
| `uRotation` | `float` | Rotation in degrees (sprites with `Rotation` only) |
| `uScale` | `vec2` | Scale (sprites with `Scale` only) |
| `uVelocity` | `vec2` | Velocity (only with `RigidBody`) |

Your own uniforms are set last, so one with a reserved name overrides the engine's value.

#### Post-process shaders

The `PostProcessShader` resource (pre-inserted, logic-owned) applies a chain of loaded shaders to the whole frame when the render target is drawn to the window. Each shader reads the previous pass's output as its texture; the last pass draws into the letterboxed window area. An empty chain turns post-processing off.

```rust
use aberredengine::prelude::*;

fn enable_crt(mut post: ResMut<PostProcessShader>) {
    post.set_shader_chain(Some(vec!["crt".to_string(), "vignette".to_string()]));
    post.set_uniform("uCurvature", UniformValue::Float(0.1));
}

fn disable_post_processing(mut post: ResMut<PostProcessShader>) {
    post.set_shader_chain(None);
}
```

- Every shader in the chain gets the same uniforms: the standard ones (`uTime`, `uDeltaTime`, `uFrame`, `uResolution`, `uWindowResolution`, `uLetterbox`, as in the table above; `uLetterbox` is the letterboxed window rectangle on the last pass and the full game-resolution rectangle on earlier passes), then yours from `set_uniform`. `set_uniform` returns `true` for a reserved name; don't use those. `clear_uniform(name)` and `clear_uniforms()` remove yours.
- A key that isn't loaded, or failed to load, is skipped with a warning; the rest of the chain still runs.
- The resource survives scene switches: reset the chain yourself when a scene shouldn't keep it.

### Animations

Animations are pure data — no raylib calls needed. `AnimationStore` is pre-inserted by the engine. Request it as `ResMut<AnimationStore>` and populate it with `AnimationResource` entries:

```rust
use aberredengine::prelude::*;

anim_store.insert("player_idle", AnimationResource::new("player", 32.0, 4, 8.0));

anim_store.insert(
    "player_run",
    AnimationResource::new("player", 32.0, 6, 12.0)
        .with_position(Vec2::new(0.0, 64.0)), // second row of the spritesheet
);
```

`AnimationResource::new(tex_key, frame_width, frame_count, fps)` loops, starts at the texture's top-left and steps `frame_width` pixels per frame on one row; `tex_key` must match a loaded texture key. `.with_position(v)` moves frame 0, `.with_vertical_displacement(row_height)` wraps frames that run past the texture's right edge onto the next row, and `.with_looped(false)` plays once (see [Animation Finished Event](#75-animation-finished-event)).

### Tilemaps

Tilemaps use the **Tilesetter 2.1.0** export format: a directory containing a `.png` tileset texture and a `.txt` JSON data file, both named after the directory.

```
assets/tilemaps/level01/
├── level01.png    # tileset texture (atlas)
└── level01.txt    # JSON: { tile_size, map_width, map_height, layers: [{ name, positions: [{ x, y, id }] }] }
```

Spawn a tilemap by attaching the `TileMap` component to any entity. `tilemap_spawn_system` reacts to `Added<TileMap>`, loads the PNG + JSON from disk, and spawns all tile entities as `ChildOf` children of the root entity. The entire tilemap then moves, scales, and rotates as one unit:

```rust
use aberredengine::prelude::*;

// Minimal — tiles appear at world origin (default MapPosition inserted automatically)
commands.spawn(TileMap::new("assets/tilemaps/level01"));

// Positioned and scaled
commands.spawn((
    TileMap::new("assets/tilemaps/level01"),
    MapPosition::new(100.0, 200.0),
    Scale::new(2.0, 2.0),
));
```

A `.deterministic()` game preloads the atlas in Setup with `assets.load_tilemap("assets/tilemaps/level01")` (see [Determinism and replay](#determinism-and-replay)).

Move the whole tilemap at runtime by updating the root entity's `MapPosition`:

```rust
commands.entity(tilemap_root).insert(MapPosition::new(new_x, new_y));
```

> **Lua builds only:** a Lua game spawns the same tilemap through the entity builder. A pure-Rust game ignores this snippet.

```lua
engine.spawn()
    :with_tilemap("./assets/tilemaps/level01")
    :with_position(0, 0)  -- optional, defaults to (0, 0)
    :build()
```

The atlas texture is stored in `TextureStore` under `"tilemap:"` plus the directory path (`"tilemap:assets/tilemaps/level01"` above; `tilemap_texture_key(path)` in `aberredengine::core::systems::tilemap` computes it), and loaded once per key, so two `TileMap` entities pointing to the same directory share one GPU texture. Directories with the same name in different places get separate atlases, and the prefix keeps the key apart from your own texture keys. Tile entities are in `Group("tiles")` and get `ZIndex` values automatically based on layer order (first layer most negative, last layer least negative).

Tile entities carry only `Group`, `Sprite`, `MapPosition`, `ZIndex` and `ChildOf` — **no `BoxCollider`**. Collision detection needs a `BoxCollider` on both entities, so a `CollisionRule` on `"tiles"` never fires until your game adds colliders. One way is a system that gives every newly spawned tile a sprite-sized collider:

```rust
use aberredengine::prelude::*;
use aberredengine::core::systems::tilemap::TILES_GROUP;

type NewUncollidedTiles = (Added<Group>, Without<BoxCollider>);

fn add_tile_colliders(
    mut commands: Commands,
    tiles: Query<(Entity, &Group, &Sprite), NewUncollidedTiles>,
) {
    for (entity, group, sprite) in &tiles {
        if group.0 == TILES_GROUP {
            commands
                .entity(entity)
                .insert(BoxCollider::new(sprite.width, sprite.height));
        }
    }
}

EngineBuilder::new()
    .add_system(add_tile_colliders)
    // …
```

> **Warning:** every tile that gets a collider joins `collision_detector`'s all-pairs test (see [Collision Rules](#73-collision-rules)). A 100×50 map has 5,000 tiles: about 12.5 million pair tests every sim tick. Collide only the tiles that must be solid. Filter by layer `ZIndex` or by the tile's `Sprite.offset` in the atlas, or use a few large colliders on plain entities instead of one per tile.

> **Note:** `load_tilemap_data` and `spawn_tiles` remain available as low-level utilities for advanced use cases where manual control of the load/spawn cycle is needed.

### Camera

`Camera2DRes` is pre-inserted by the engine with `Camera2D::screen_centered(&screen)`: `target` at the origin and `offset` at half the render resolution (center-screen). If you need a different initial position, request `ResMut<Camera2DRes>` and overwrite it — use `ScreenSize` (a logic-side resource) rather than a live raylib handle for the resolution, since `RaylibAccess` isn't available here:

```rust
use aberredengine::prelude::*;

fn setup_camera(mut camera: ResMut<Camera2DRes>, screen: Res<ScreenSize>) {
    // Look at (512, 256) from the screen center, zoomed in 2×
    camera.0 = Camera2D::screen_centered(&screen).with_zoom(2.0);
    camera.0.target = Vec2::new(512.0, 256.0);
}
```

`offset` is the screen point the camera looks through. `target` is the world position it looks at. `Camera2D::new(target, offset)` and `Camera2D::screen_centered` start at zoom 1 with no rotation; chain `.with_zoom(z)` or `.with_rotation(degrees)` to change them.

#### Following an entity

To make the camera track an entity, give it a `CameraTarget` and enable the pre-inserted `CameraFollowConfig` resource (it starts disabled):

```rust
use aberredengine::prelude::*;

fn follow_player(mut commands: Commands, mut follow: ResMut<CameraFollowConfig>) {
    commands.spawn((
        MapPosition::new(100.0, 200.0),
        // … sprite, physics, etc. …
        CameraTarget::new(1).with_zoom(2.0),
    ));

    follow.enabled = true;
    follow.mode = FollowMode::Deadzone { half_w: 32.0, half_h: 24.0 };
    follow.lerp_speed = 6.0;
    follow.bounds = Some(Rect::new(0.0, 0.0, 2048.0, 1024.0)); // the level's extent
}
```

- Every sim tick, `camera_follow_system` picks the `CameraTarget` with the highest `priority` (ties go to the lower `Entity`), and moves `Camera2DRes.target` toward its world position plus `follow.offset`. With no `CameraTarget` entity, the camera stays where it is.
- `mode`: `Instant` snaps; `Lerp` eases by `easing` (`EasingCurve::EaseOut` by default) at `lerp_speed`; `SmoothDamp` is a spring tuned by `spring_stiffness`/`spring_damping` (call `reset_velocity()` when you switch targets or modes); `Deadzone { half_w, half_h }` holds still until the target leaves that box around the camera, then catches up at `lerp_speed`.
- While following, the camera's zoom eases toward the winning target's `zoom` at `zoom_lerp_speed`, so setting `Camera2DRes.zoom` yourself has no lasting effect; change the target's `zoom` instead.
- `bounds` (a world-space `Rect`, `x`/`y` top-left) clamps the camera so the view stays inside it. The clamp assumes the default centered `offset`.

### Complete setup example

```rust
fn setup(mut assets: AssetLoader, mut anim_store: ResMut<AnimationStore>) -> Result {
    // Textures — queued, loaded asynchronously on the render thread
    assets.load_texture("player", "assets/textures/player.png")?;

    // Fonts — mipmap generation is handled internally by the render thread
    assets.load_font("arcade", "assets/fonts/arcade.ttf", 32)?;

    // Audio — loaded asynchronously on the audio thread
    assets.load_sound("jump", "assets/audio/jump.wav")?;
    assets.load_music("bgm", "assets/audio/music.ogg")?;

    // Shaders
    assets.load_shader("glow", None, Some("assets/shaders/glow.fs"))?;

    // Animations (AnimationStore is pre-inserted, logic-owned — just populate it)
    anim_store.insert("player_idle", AnimationResource::new("player", 32.0, 4, 8.0));
    Ok(())
}
```

Setup waits for these loads, so they're ready by the initial scene's `SceneEntered`. Assets loaded later, during `Playing`, follow the load-then-use gap described in the Textures/Fonts subsections.

---

## 5. Spawning Entities

Entities are spawned with `commands.spawn((component_tuple))` — the standard Bevy ECS pattern. You build entities by composing components as a tuple.

### Example 1: Sprite entity

A minimal visible entity needs a position, a sprite, a draw order, and optionally a group:

```rust
use aberredengine::prelude::*;

commands.spawn((
    MapPosition::new(100.0, 200.0),
    Sprite::new("player", 32.0, 32.0).centered(),
    ZIndex(1.0),
    Group::new("player"),
));
```

### Example 2: Physics entity

Add `RigidBody`, `BoxCollider`, and `AccelerationControlled` for a player character with momentum-based movement:

```rust
use aberredengine::prelude::*;

commands.spawn((
    MapPosition::new(100.0, 200.0),
    Sprite::new("player", 32.0, 32.0).centered(),
    ZIndex(1.0),
    Group::new("player"),
    RigidBody::with_physics(5.0, Some(300.0)),  // friction=5.0, max_speed=300
    BoxCollider::new(28.0, 30.0)
        .with_origin(Vec2::new(16.0, 16.0))
        .with_offset(Vec2::new(2.0, 2.0)),
    AccelerationControlled::symmetric(800.0),    // 800 units/s² in all directions
));
```

### Example 3: UI text with signal binding

Screen-space text that auto-updates from `WorldSignals`:

```rust
use aberredengine::prelude::*;

commands.spawn((
    ScreenPosition::new(10.0, 10.0),
    DynamicText::new("0", "arcade", 16.0, Color::WHITE),
    SignalBinding::new("score").with_format("Score: {}"),
    ZIndex(100.0),
));
```

When `WorldSignals` has a value for key `"score"`, the text automatically updates to `"Score: 42"` (or whatever the value is).

### Component constructor quick reference

`Sprite`, `Camera2D`, `AnimationResource` and the GUI theme types (`GuiTheme`, `GuiNinePatch`, `GuiButtonSkin`, `GuiProgressBarSkin`) are `#[non_exhaustive]`: build them with their constructors (or `Default` plus field assignment for `GuiTheme`), not struct literals, so the engine can add fields without breaking your game. Their fields stay public to read and write.

| Component | Constructor |
|-----------|-------------|
| `MapPosition` | `MapPosition::new(x, y)` |
| `ScreenPosition` | `ScreenPosition::new(x, y)` |
| `Sprite` | `Sprite::new("key", w, h)` (origin top-left) then `.centered()` or `.with_origin(v)`, `.with_offset(v)` (spritesheet frame), `.with_flip(h, v)` |
| `RigidBody` | `RigidBody::new()` or `RigidBody::with_physics(friction, max_speed)` |
| `BoxCollider` | `BoxCollider::new(w, h).with_origin(v).with_offset(v)` |
| `Animation` | `Animation::new("anim_key")` |
| `AnimationController` | `AnimationController::new("fallback_key").with_rule(condition, "key")`, `condition` a `Condition` (see below) |
| `Group` | `Group::new("name")` |
| `ZIndex` | `ZIndex(f32)` |
| `Rotation` | `Rotation::new(degrees)` |
| `Scale` | `Scale::new(sx, sy)` |
| `Tint` | `Tint::new(r, g, b, a)` — values are `u8` (0–255) |
| `Persistent` | `Persistent` — tag, survives scene transitions |
| `Ttl` | `Ttl::new(seconds)` — auto-despawn after duration |
| `DynamicText` | `DynamicText::new(text, font_key, size, color)` |
| `SignalBinding` | `SignalBinding::new("key").with_format("Score: {}")` |
| `Signals` | `Signals::default()` — per-entity signal bag |
| `InputControlled` | `InputControlled::symmetric(speed)` (fields `up_velocity`, `down_velocity`, `left_velocity`, `right_velocity` for per-direction speeds) |
| `AccelerationControlled` | `AccelerationControlled::symmetric(accel)` |
| `MouseControlled` | `MouseControlled { follow_x: true, follow_y: true }` |
| `Timer` | `Timer::new(duration_secs)` (repeating) or `Timer::once(duration_secs)` — triggers `TimerFired` on its entity; see §7.1 |
| `Phase` | `Phase::new("initial_phase")` — set `next` to transition; triggers `PhaseEntered`/`PhaseExited`; see §7.2 |
| `CollisionRule` | `CollisionRule::new("group_a", "group_b")` — observe `Collided` on it; see §7.3 |
| `Tween<MapPosition>` | `Tween::position(from: Vec2, to: Vec2, duration)` |
| `Tween<Rotation>` | `Tween::rotation(from_degrees, to_degrees, duration)` |
| `Tween<Scale>` | `Tween::scale(from: Vec2, to: Vec2, duration)` |
| `Tween<ScreenPosition>` | `Tween::screen_position(from: Vec2, to: Vec2, duration)` |
| `GuiWindow` | `GuiWindow::new(w, h)` — `theme_key` defaults to `"default"`; override with `.with_theme_key("my_theme")` |
| `GuiButton` | `GuiButton::new(width, height, "Caption")` — add `.with_theme_key(key)` to use a non-default theme |
| `GuiLabel` | `GuiLabel::new(width, height, "Text")` — add `.with_signal_binding(key)` / `.with_signal_binding_format("fmt {}") ` to bind text to `WorldSignals` |
| `GuiImage` | `GuiImage::new(width, height, "tex_key", offset_x, offset_y)` — add `.with_offset_hover(x, y)` / `.with_offset_pressed(x, y)` / `.with_offset_disabled(x, y)` for per-state atlas offsets |
| `GuiProgressBar` | `GuiProgressBar::new(w, h, value, max)` — add `.with_direction(ProgressBarDirection)`, `.with_signal_binding(key)`, `.with_theme_key(key)`; requires `ScreenPosition` + `ZIndex` |
| `Shadow` | `Shadow::new(dx, dy, r, g, b, a)` or `Shadow::default_color(dx, dy)` — pre-pass shadow for `Sprite` and `DynamicText` entities; see §7.7 |
| `GuiInteractable` | `GuiInteractable::new(width, height)` — hit-test/click state; observe `GuiClicked` for clicks; see §7.7 |
| `GuiOffset` | `GuiOffset(Vec2::new(x, y))` — position relative to a `ChildOf` parent |

**Animation controller rules.** `with_rule` takes a `Condition` (`aberredengine::core::components::animation::{Condition, CmpOp}`) evaluated against the entity's **own** `Signals` component, not `WorldSignals`. An entity without `Signals` is skipped. Rules run in order every sim tick and the first match sets the animation; when none matches, the fallback key plays. `Condition` variants: `ScalarCmp`, `ScalarRange`, `IntegerCmp`, `IntegerRange`, `HasFlag`, `LacksFlag`, and the combinators `All`, `Any`, `Not`.

```rust
use aberredengine::prelude::*;
use aberredengine::core::components::animation::{CmpOp, Condition};

commands.spawn((
    Animation::new("player_idle"),
    AnimationController::new("player_idle")
        .with_rule(Condition::HasFlag { key: "dead".into() }, "player_dead")
        .with_rule(
            Condition::ScalarCmp { key: "speed".into(), op: CmpOp::Gt, value: 1.0 },
            "player_run",
        ),
    Signals::default(), // write "dead" / "speed" here from your game logic
));
```

### Tween components in Rust

Tweens are represented by a single generic component: `Tween<T>`.

Use the target component type as `T`:

- `Tween<MapPosition>` for position animation
- `Tween<Rotation>` for rotation animation
- `Tween<Scale>` for scale animation
- `Tween<ScreenPosition>` for screen-space (UI) position animation

`EngineBuilder` registers the built-in tween systems for these four component types automatically, so in normal game code you only need to spawn the tween component itself.

`Tween::position`, `Tween::rotation`, `Tween::scale` and `Tween::screen_position` take raw values (`Vec2`, or degrees for rotation); `Tween::new(from, to, duration)` takes the component values themselves.

**Position tween example:**

```rust
use aberredengine::prelude::*;

commands.spawn((
    MapPosition::new(0.0, 0.0),
    Tween::position(Vec2::ZERO, Vec2::new(200.0, 120.0), 1.5)
        .with_easing(Easing::CubicOut)
        .with_loop_mode(LoopMode::PingPong),
));
```

**Rotation tween example:**

```rust
use aberredengine::prelude::*;

commands.spawn((
    Rotation::new(0.0),
    Tween::rotation(0.0, 360.0, 2.0),
));
```

**Scale tween example:**

```rust
use aberredengine::prelude::*;

commands.spawn((
    Scale::new(1.0, 1.0),
    Tween::scale(Vec2::ONE, Vec2::new(1.5, 0.75), 0.75)
        .with_backwards(),
));
```

**Screen-position tween example (UI):**

```rust
use aberredengine::prelude::*;

commands.spawn((
    ScreenPosition::new(-200.0, 50.0),
    Tween::screen_position(Vec2::new(-200.0, 50.0), Vec2::new(20.0, 50.0), 0.4),
));
```

> **Important:** The generic parameter must match the component you want the engine to animate. For example, use `Tween<MapPosition>` with `MapPosition`, not `Tween<Vec2>`. The tween systems query concrete ECS component types, not raw value types.

### Spawning context: observers and systems

In **scene observers**, take `Commands` as a parameter:

```rust
fn enter(_: On<SceneEntered>, mut commands: Commands) {
    commands.spawn(( /* ... */ ));
}
```

In **systems** (the setup hook, `.add_system()`, `.add_scene_system()`), likewise:

```rust
fn my_update(mut commands: Commands) {
    commands.spawn(( /* ... */ ));
}
```

Both are standard Bevy `Commands` — the API is identical.

### Your own components and resources

Game state that the engine's components don't cover goes in your own Bevy types. Derive them as usual; the prelude puts the `bevy_ecs` crate in scope for the derive macros.

```rust
use aberredengine::prelude::*;

#[derive(Component)]
struct Health(i32);

#[derive(Resource, Default)]
struct Score(u32);

fn setup(mut commands: Commands) {
    commands.insert_resource(Score::default());
}

fn remove_dead(mut commands: Commands, mut score: ResMut<Score>, query: Query<(Entity, &Health)>) {
    for (entity, health) in &query {
        if health.0 <= 0 {
            commands.entity(entity).despawn();
            score.0 += 100;
        }
    }
}

EngineBuilder::new()
    .on_setup(setup)
    .add_system(remove_dead)
    // …
```

- `EngineBuilder` has no `insert_resource`. Insert your resources from the setup hook (as above) or any other system with `commands.insert_resource(...)`. A system that takes `Res<Score>` panics the first time it runs while the resource is missing; take `Option<Res<Score>>` if it may run earlier.
- Resources survive scene switches. Entities carrying your components are despawned on a scene switch like any other entity, unless they also have `Persistent`.
- GUI and world-draw callbacks run on the render thread and can't read your resources. Copy what they need into `AppState` (see [AppState API](#appstate-api)).

---

## 6. Scene Management Deep Dive

Section 3 introduced `SceneManager` at the API level. This section covers internals and practical patterns.

### 6.1 What happens during a scene switch

When the `scene_switch_system` runs, it performs these steps in order:

1. **Read and check the target scene** — reads `WorldSignals["scene"]` for the target scene name (defaults to `"menu"` if unset). If no scene is registered under that name, the system logs "No scene registered" and returns: nothing below runs, and the current scene keeps running untouched
2. **Trigger `SceneExited`** — if a scene was active, its exit observers run while its entities, `WorldSignals` entity registrations and group counts are all still in place
3. **Despawn non-persistent entities** — every entity *without* the `Persistent` component is despawned
4. **Clear entity registrations** — non-persistent entity refs stored in `WorldSignals` are removed
5. **Reset group tracking** — `TrackedGroups` drops every group except the `.track_group()` ones, and `WorldSignals` group counts are wiped (they are published again on the next tick)
6. **Write `previous_scene`** — the old active scene name is stored in `WorldSignals["previous_scene"]`
7. **Set active scene** — updates `SceneManager.active_scene` to the new scene name
8. **Trigger `SceneEntered`** — the new scene's enter observers run, typically spawning entities and setting up initial state; what they spawn belongs to the new scene

Steps 2–6 tear down the scene being left, so they run only when a scene is active. Entering the first scene skips them: entities spawned before it (during Setup) survive into it.

The new scene's `.add_scene_system()` systems start running on the next sim tick.

> **Note:** a mistyped scene name only logs an error ("No scene registered for '…'", followed by the list of registered scenes), so it is easy to miss. Prefer constants over repeated string literals.

### 6.2 Triggering scene transitions

Scene transitions work by running the `scene_switch_system` as a one-shot system via `commands.run_system()`. The system is registered in `SystemsStore` under the key `"switch_scene"` when you use `EngineBuilder::add_scene()`.

**Approaches:**

**1. Menu-driven (recommended):** Use `MenuAction::SetScene("level01")` — the menu selection observer calls `commands.run_system()` internally, so the switch lands in the same tick.

**2. Flag-based from systems and observers:** Call `WorldSignals::request_scene(name)`, which sets the target scene name and the `sk::SWITCH_SCENE` flag. The engine's `scene_switch_poll` system (registered automatically for every Rust game) picks up the flag each sim tick and triggers the transition:

```rust
fn update(mut signals: ResMut<WorldSignals>) {
    if player_reached_exit() {
        signals.request_scene("level02");
    }
}
```

### 6.3 Persistent entities

The `Persistent` tag component (`aberred-core/src/components/persistent.rs`) marks entities that survive scene switches. During a transition, `scene_switch_system` despawns everything *without* `Persistent`.

Typical uses:

- **Score UI** — a `DynamicText` + `SignalBinding` that displays the score across all scenes
- **Collision rules** — `CollisionRule` entities are regular entities and will be despawned on scene switch unless they have `Persistent`
- **Global state entities** — entities carrying `Signals` or custom components that hold cross-scene state

```rust
use aberredengine::prelude::*;

commands.spawn((
    ScreenPosition::new(10.0, 10.0),
    DynamicText::new("0", "arcade", 16.0, Color::WHITE),
    SignalBinding::new("score").with_format("Score: {}"),
    ZIndex(100.0),
    Persistent,  // survives scene switches
));
```

A per-entity observer added with `.observe(...)` on a `Persistent` entity survives scene switches too: the engine keeps the observers of persistent entities. Observers of scene entities go away with the entity they watch. A global observer (`EngineBuilder::add_observer` or a spawned `Observer`) is an entity of its own, so it survives only if it carries `Persistent` itself.

> **Custom cleanup:** to despawn "everything a scene switch would" from your own system (e.g. a reset), use `aberredengine::core::components::persistent::SceneCleanup`, the system parameter the engine's own scene-switch cleanup uses. A bare `Query<Entity, Without<Persistent>>` also matches the internal entities Bevy 0.19 backs each `Resource` with, and the observer entities of persistent entities, and would despawn both:
>
> ```rust
> use aberredengine::core::components::persistent::SceneCleanup;
>
> fn my_cleanup(scene_cleanup: SceneCleanup, mut commands: Commands) {
>     scene_cleanup.despawn_all(&mut commands);
> }
> ```

### 6.4 Group tracking across scenes

`TrackedGroups` (`aberred-core/src/resources/group.rs`) is a resource holding a set of group names to count. The engine's `update_group_counts_system` publishes entity counts for each tracked group to `WorldSignals` every sim tick.

Register the groups your game counts on the builder. `.track_group()` keeps them tracked for the whole game, across every scene switch:

```rust
EngineBuilder::new()
    .track_group("enemies")
    .track_group("bricks")
    // …
```

Read the counts with `Res<WorldSignals>`: `signals.get_group_count("enemies")`.

Key behaviors:

- The engine publishes `"group_count:enemies"` to `WorldSignals` each sim tick
- Group names are limited to `MAX_GROUP_NAME_LEN` (52) bytes; a longer `.track_group()` name is a startup error (`EngineError::GroupNameTooLong`)
- **Per-scene groups** — a system with `ResMut<TrackedGroups>` can call `add_group("bullets")` at runtime. A scene switch drops these and keeps only the `.track_group()` groups
- Bind a `SignalBinding::new("group_count:enemies")` to auto-display the count in UI text

### 6.5 Per-sim-tick scene updates

A system registered with `.add_scene_system(scene, system)` runs once per sim tick while `scene` is active — not once per rendered frame; see [Threading Model](#threading-model-what-your-code-can-access) for why those differ. It takes whatever system parameters it needs:

```rust
fn update(time: Res<WorldTime>, input: Res<InputState>) {
    let dt = time.delta; // the fixed sim period, 1.0 / hz, scaled by time_scale
    // input = current action state (just_pressed, active, just_released)
}
```

`WorldTime.delta` — always the fixed constant `1.0 / hz` (`[simulation] hz` in `config.ini`), scaled only by `WorldTime.time_scale`. It is never a measured, render-frame-dependent value — a render stall dilates game time instead of spiking `dt`. Use it for frame-rate-independent logic (e.g., `speed * dt`) exactly as you would a measured delta; the fixed-constant behavior only matters if you're reasoning about stalls or determinism.

---

## 7. Gameplay Systems

The engine provides several gameplay systems: **timers**, **phase state machines**, **collision rules**, **menus**, animation/tween-finished events, and GUI widgets. Timers, phases, collision rules and menus are data **components** that trigger **events**, which you handle with ordinary observers and systems.

Gameplay code is ordinary Bevy systems and observers that take `Commands`, queries and resources such as `WorldSignals` or `SimRng` as parameters (see [Section 8](#8-engine-resources-quick-reference)). It runs on the logic thread and has **no direct texture access** — if a system needs texture data, load it via `RenderAssetCmd` and read back dimensions from `TextureDimsStore` (see [Section 4](#4-loading-assets)).

### 7.1 Timers

**Source:** `aberred-core/src/components/timer.rs`, `aberred-core/src/systems/timer.rs`, `aberred-core/src/events/timer.rs`

`Timer` is a countdown component. When `elapsed >= duration`, it triggers a `TimerFired` event on its entity, at most once per sim tick. `Timer::new` repeats, resetting by subtracting `duration` (not zeroing) for timing accuracy.

**Creating a timer:**

```rust
use aberredengine::prelude::*;

// Spawn an entity with a 2-second repeating timer and a per-entity observer
commands
    .spawn((MapPosition::new(0.0, 0.0), Timer::new(2.0)))
    .observe(on_timer_fired);

fn on_timer_fired(ev: On<TimerFired>, mut signals: ResMut<WorldSignals>) {
    // This fires every 2 seconds; ev.entity is the timer's entity
    signals.set_string("timer_count", "fired!".to_string());
}
```

An observer is a system, so it takes any system params. To handle many timers in one place, register one global observer with `EngineBuilder::add_observer` and filter by a marker component. Each per-entity observer is itself an entity, so prefer the global form for thousands of timers. Every global observer runs on every `TimerFired`, so keep one per kind of timer rather than many:

```rust
#[derive(Component)]
struct Spawner;

fn on_spawner_timer(
    ev: On<TimerFired>,
    spawners: Query<&MapPosition, With<Spawner>>,
    mut commands: Commands,
) {
    let Ok(pos) = spawners.get(ev.entity) else { return };
    commands.spawn(MapPosition::new(pos.pos.x, pos.pos.y));
}
```

**One-shot timers:** `Timer::once(duration)` (`TimerMode::Once`) fires once, then removes its `Timer` component. The entity stays, so a one-shot fits a temporary state on a long-lived entity. Observers still see the `Timer` while handling the event, and a new `Timer` an observer inserts is kept, so one-shots can chain:

```rust
#[derive(Component)]
struct Invulnerable;

// Three seconds of invulnerability for the player
fn grant_invulnerability(commands: &mut Commands, player: Entity) {
    commands.entity(player).insert((Invulnerable, Timer::once(3.0)));
}

// Registered once with `EngineBuilder::add_observer`
fn end_invulnerability(
    ev: On<TimerFired>,
    invulnerable: Query<(), With<Invulnerable>>,
    mut commands: Commands,
) {
    if invulnerable.contains(ev.entity) {
        commands.entity(ev.entity).remove::<Invulnerable>();
    }
}
```

### 7.2 Phase State Machines

**Source:** `aberred-core/src/components/phase.rs`, `aberred-core/src/systems/phase.rs`, `aberred-core/src/events/phase.rs`

`Phase` is a per-entity state machine over string-labeled phases. It holds data only. Per-phase behavior is an ordinary system that matches on `phase.current`, and a transition is a write to `phase.next`:

```rust
use aberredengine::prelude::*;

#[derive(Component)]
struct Player;

fn spawn_player(mut commands: Commands) {
    commands.spawn((
        Player,
        MapPosition::new(100.0, 200.0),
        RigidBody::default(),
        // ... sprite, collider, etc ...
        Phase::new("idle"),
    ));
}

// Registered with `EngineBuilder::add_system`; runs once per sim tick
fn player_phases(
    mut players: Query<(&mut Phase, &mut RigidBody), With<Player>>,
    input: Res<InputState>,
) {
    for (mut phase, mut rb) in &mut players {
        let next = match phase.current.as_str() {
            "idle" if input.action(InputAction::Action1).just_pressed => {
                rb.velocity.y = -400.0;
                "jumping"
            }
            "jumping" if rb.velocity.y > 0.0 => "falling",
            "falling" if rb.velocity.y == 0.0 => "idle",
            _ => continue,
        };
        phase.next = Some(next.into());
    }
}
```

The engine's `phase_system` applies transitions and triggers two events on the phase's entity:

- `PhaseEntered { entity, name, previous }` fires once for the initial phase (`previous` is `None`) at the start of the first sim tick after the entity spawns, and again after every transition.
- `PhaseExited { entity, name, next }` fires for the old phase right before the new phase's `PhaseEntered`.

Their observers run after the swap, so `phase.current` is already the new phase in both events of a transition; read the phase from `ev.name`. One-shot effects such as a sound on entering a phase belong in an observer, not in the per-tick system:

```rust
// Registered once with `EngineBuilder::add_observer`
fn on_player_phase_entered(
    ev: On<PhaseEntered>,
    players: Query<(), With<Player>>,
    mut audio: MessageWriter<AudioCmd>,
) {
    if players.contains(ev.entity) && &*ev.name == "jumping" {
        audio.write(AudioCmd::PlayFx { id: "jump".into() });
    }
}
```

As with timers (§7.1), observe one entity with `.observe(handler)` or many with one global observer filtered by a marker component.

**Transitions:** `phase_system` runs at the start of every sim tick, in `SimSet::Phases`, before `.add_system()` systems and in every game state. It applies a `next` set at any point in the previous tick: it sets `previous` to the old phase, `current` to the new one, resets `time_in_phase` to 0, and triggers `PhaseExited` then `PhaseEntered`. A transition requested in tick N therefore applies in tick N+1, and chained transitions (a `PhaseEntered` observer setting `next`) advance one per sim tick. Setting `next` to the current phase re-enters it: both events fire with the same name and `time_in_phase` resets. Phase names are never validated: `phase.next = Some("jumpin".into())` switches to a phase no system handles, and the entity silently does nothing.

**Phase fields:**

| Field | Type | Description |
|-------|------|-------------|
| `current` | `String` | Current phase label |
| `previous` | `Option<String>` | Phase before the last transition |
| `next` | `Option<String>` | Set to request a transition; cleared when applied |
| `time_in_phase` | `f32` | Seconds since entering the current phase; grows by `dt` every sim tick |

### 7.3 Collision Rules

**Source:** `aberred-core/src/components/collision.rs`, `aberred-core/src/systems/collision_rule.rs`, `aberred-core/src/systems/collision_detector.rs`

`CollisionRule` names two entity groups. Rules are spawned as their own entities. When an entity of one group overlaps an entity of the other, the engine triggers a `Collided` event on the rule entity. React with an observer on the rule (`.observe(handler)`), or with a global observer (`.add_observer(handler)`) that sees every rule's collisions.

**The event:** `Collided { rule, a, b, sides_a, sides_b }`

- `rule` — the matched rule entity (the event target)
- `a`, `b` — the two colliding entities, ordered to match `group_a` and `group_b`
- `sides_a`, `sides_b` — `BoxSides = SmallVec<[BoxSide; 4]>`, which sides of each collider touch the other
- `BoxSide` variants: `Left`, `Right`, `Top`, `Bottom`

**Detection pipeline:**

1. `collision_detector` system iterates all entity pairs with `MapPosition` + `BoxCollider`
2. Uses AABB overlap via `BoxCollider::as_rectangle()` + `Rect::overlaps()`
3. On overlap, triggers a `CollisionEvent { a, b }` (observe it directly for "any overlap" logic)
4. `collision_rule_observer` receives the event, looks up `Group` names, finds a matching `CollisionRule`, computes collision sides, and triggers `Collided` on the rule entity

**Bidirectional matching:** A rule for `("ball", "brick")` matches regardless of which entity is `ball` vs `brick`. `Collided.a` is always the `group_a` entity and `Collided.b` the `group_b` entity.

**Timing and cost:**

- `Collided` fires **every sim tick** while the two boxes overlap (240 times a second at the default `[simulation] hz`). There are no enter/exit events. To react once, remember the pair yourself (a flag, a `Signals` entry, or despawning one side, as the example below does).
- Only **one rule** fires per overlapping pair. If several rules cover the same two groups, the one on the lowest `Entity` wins and the others never fire for that pair.
- `collision_detector` tests **every pair** of entities with `MapPosition` + `BoxCollider` (O(n²)), whether or not any rule names their groups. Keep the number of colliders small; give a `BoxCollider` only to entities that need one.

**Creating a collision rule:**

```rust
use aberredengine::prelude::*;

commands
    .spawn((
        CollisionRule::new("ball", "brick"),
        Persistent, // survive scene switches
    ))
    .observe(ball_brick_collision);
```

> **Note:** `CollisionRule` entities are regular entities — they get despawned on scene switch unless marked `Persistent`. An observer added with `.observe()` lives on its own entity and is despawned with the rule.

**Example observer — ball/brick collision with side-based reflection:**

```rust
use aberredengine::prelude::*;

fn ball_brick_collision(
    hit: On<Collided>,
    mut commands: Commands,
    mut rigid_bodies: Query<&mut RigidBody>,
    mut audio: MessageWriter<AudioCmd>,
) {
    // Despawn the brick
    commands.entity(hit.b).despawn();
    audio.write(AudioCmd::PlayFx { id: "break".into() });

    // Reflect ball velocity based on collision side
    if let Ok(mut rb) = rigid_bodies.get_mut(hit.a) {
        for side in hit.sides_a.iter() {
            match side {
                BoxSide::Top | BoxSide::Bottom => rb.velocity.y = -rb.velocity.y,
                BoxSide::Left | BoxSide::Right => rb.velocity.x = -rb.velocity.x,
            }
        }
    }
}
```

### 7.4 Menus

**Source:** `aberred-core/src/components/menu.rs`, `aberred-core/src/systems/menu.rs`

`Menu` is a component that creates an interactive, navigable menu. Spawn it on an entity and the engine handles rendering, input, scrolling, and selection dispatch.

**Constructor:**

```rust
use aberredengine::prelude::*;

let menu = Menu::new(
    &[("start", "Start Game"), ("options", "Options"), ("quit", "Quit")],
    Vec2::new(100.0, 80.0), // origin position
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
| `.with_visible_count(n)` | Limit visible items (enables scrolling) |
| `.with_cursor(entity)` | Attach a cursor entity to the selection |

**Two selection handling approaches:**

**1. MenuActions (declarative):** Attach a `MenuActions` component alongside the `Menu`. Each item ID maps to an action:

```rust
let actions = MenuActions::new()
    .with("start", MenuAction::SetScene("level01".to_string()))
    .with("options", MenuAction::SetScene("options_menu".to_string()))
    .with("quit", MenuAction::QuitGame);

commands.spawn((menu, actions));
```

`MenuAction` variants:

| Variant | Effect |
|---------|--------|
| `SetScene(String)` | Switches scene in the same tick (calls `commands.run_system()` internally) |
| `QuitGame` | Transitions to quitting state |
| `Noop` | Does nothing |

**2. Observe `MenuSelected`:** For custom logic, observe `MenuSelected` on the menu entity. It carries the menu `entity`, the selected `item_id` and its `index` in `Menu::items`:

```rust
use aberredengine::prelude::*;

fn on_menu_select(ev: On<MenuSelected>, mut signals: ResMut<WorldSignals>) {
    match ev.item_id.as_str() {
        "start" => signals.request_scene("level01"),
        "quit" => signals.request_quit(),
        _ => {}
    }
}

commands.spawn(menu).observe(on_menu_select);
```

A global observer registered with `EngineBuilder::add_observer` sees every menu's selections; filter by `ev.entity` or a marker component.

**Dispatch order:** When an item is confirmed, the engine triggers `MenuSelected`, and then:

1. **Lua callback** (`on_select_callback`, only in games that run Lua) — when set, `MenuActions` is skipped.
2. **MenuActions** (declarative).

`MenuSelected` observers always run, alongside either. Don't combine `MenuActions` with an observer for the same item, or both act on it.

**Navigation:** Up/down arrows move selection. `action_1` or `action_2` confirms. With `.with_visible_count(n)`, the menu shows at most `n` items at a time with bounded navigation and auto-scrolling.

**Complete menu example:**

```rust
use aberredengine::prelude::*;

fn enter(_: On<SceneEntered>, mut commands: Commands) {
    let menu = Menu::new(
        &[("play", "Play"), ("quit", "Quit")],
        Vec2::new(200.0, 150.0),
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

    commands.spawn((menu, actions));
}
```

### 7.5 Animation Finished Event

**Source:** `aberred-core/src/events/animation.rs`, `aberred-core/src/systems/animation.rs` (fires the event)

`AnimationFinished` is triggered **once** by the animation system on the frame a non-looped animation first reaches its final frame. Looped animations never trigger it. It is not re-triggered on subsequent frames even though the entity stays on the last frame.

**Event struct:**

```rust,ignore
pub struct AnimationFinished {
    pub entity: Entity,
}
```

**Observing from Rust** — register a persistent observer with `EngineBuilder::add_observer`:

```rust
use aberredengine::prelude::*;

fn on_anim_done(
    trigger: On<AnimationFinished>,
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

**Lua consumers (Lua builds only):** attach `LuaOnAnimationEnd::new("fn_name")` to the entity (or use `:with_on_animation_end("fn_name")` in the Lua spawn builder). The Lua callback signature is `fn(ctx, input)` — the same as timer and phase callbacks.

### 7.6 Tween Finished Event

**Source:** `aberred-core/src/events/tween.rs`, `aberred-core/src/systems/tween.rs`

`TweenFinished<T>` is triggered **once** by the tween system for a given `Tween<T>` the frame it stops playing — either a `LoopMode::Once` tween reaching its end, or a zero-duration tween snapping immediately. `LoopMode::Loop` and `LoopMode::PingPong` tweens never trigger it, since they never stop playing on their own.

**Event struct (generic over the tweened component type):**

```rust,ignore
pub struct TweenFinished<T: TweenValue> {
    pub entity: Entity,
}
```

**Observing from Rust** — register a persistent observer per tweened type with `EngineBuilder::add_observer`. The engine already runs one monomorphized tween system per `T` (`MapPosition`, `Rotation`, `Scale`, `ScreenPosition`), so register one observer per `T` you care about:

```rust
use aberredengine::prelude::*;

fn on_move_tween_done(
    trigger: On<TweenFinished<MapPosition>>,
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

**Lua consumers (Lua builds only):** attach `LuaOnTweenFinished<T>::new("fn_name")` to the entity (or use the matching
`:with_tween_{position,rotation,scale,screen_position}_on_finished("fn_name")` builder method). The Lua
callback signature is `fn(ctx, input)` — the same as the animation-finished and timer/phase callbacks.

### 7.7 GUI Widgets

**Source:** `aberred-core/src/components/{guiwindow,guibutton,guilabel,guiimage,guiinteractable,guioffset}.rs`, `aberred-core/src/resources/guitheme.rs`, `aberred-core/src/systems/{gui_spawn,gui_layout,gui_hit_test}.rs`, `aberred-core/src/events/gui_interactable.rs`

The engine provides a themed, nine-patch-skinned in-game GUI widget system — panels, buttons, labels, and
clickable images. It is plain ECS components and systems, fully usable from pure Rust; all of its systems
(`gui_button_spawn_system`, `gui_label_spawn_system`, `gui_image_spawn_system`, `gui_layout_system`,
`gui_hit_test_system`) are registered automatically by `EngineBuilder`
regardless of the `lua` feature — there's nothing extra to wire up.

This is distinct from the ImGui GUI callback covered in Section 3 — ImGui is for editor/debug
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
| `GuiInteractable` | `GuiInteractable::new(w, h)` | Shared hit-test/click runtime state (`Normal`/`Hovered`/`Pressed`/`Disabled`). A click on it triggers `GuiClicked` on the entity. |
| `GuiOffset` | `GuiOffset(Vec2::new(x, y))` | A child widget's position relative to its `ChildOf` parent. `gui_layout_system` resolves it into the child's `ScreenPosition` every sim tick; `ChildOf` is used for lifecycle (cascade despawn) only, not positioning. |

> **Note:** `GuiButton`/`GuiImage`'s spawn systems use `insert_if_new` for the `GuiInteractable` they
> add, so they never overwrite one you pre-spawned in the same bundle (for example, to give the widget a
> different hit area). The spawn system copies the widget's `callback_name` (if non-empty) into
> `GuiInteractable.on_click_callback`, the Lua function a Lua game calls on click.

**Theming:** Themes are stored in `GuiThemeStore` — a `FxHashMap<Arc<str>, GuiTheme>` pre-inserted by the engine. Each widget carries a `theme_key: Arc<str>` (default `"default"`) that is resolved against `GuiThemeStore` at render time. Set up themes in your setup system before spawning any widgets:

```rust
use aberredengine::prelude::*;
use std::sync::Arc;

fn setup_gui_theme(mut theme_store: ResMut<GuiThemeStore>) {
    let theme = theme_store.themes.entry(Arc::from("default")).or_default();
    theme.panel = GuiNinePatch::new("gui_panel", Rect::new(0.0, 0.0, 64.0, 64.0), 6);
    // hover/pressed/disabled fall back to normal unless set with .with_hover(...) etc.
    theme.button = Some(GuiButtonSkin::new(GuiNinePatch::new(
        "gui_button",
        Rect::new(0.0, 0.0, 32.0, 32.0),
        4,
    )));
    theme.font = "main_font".into();
    theme.font_size = 16.0;
    theme.text_color = Color::WHITE;

    // Optional: drop shadows. panel_shadow applies to all nine-patch backgrounds;
    // text_shadow is inserted as a Shadow component on spawned caption DynamicText children.
    // theme.panel_shadow = Some(Shadow::default_color(2.0, 2.0));
    // theme.text_shadow = Some(Shadow::default_color(1.0, 1.0));
}
```

`GuiNinePatch::new(tex_key, source, border)` maps onto raylib's `NPatchInfo`: the `source` region of the texture, with a
`border`-pixel border on every side (`.with_borders(left, top, right, bottom)` for per-side widths). `tex_key` must already be loaded into `TextureStore` (see Section 4). `theme.font` defaults to an empty key — if it's
still unset when a non-empty caption is about to spawn, the engine logs an `error!`; the caption entity still
spawns, it just renders no visible glyphs.

**`GuiTheme` fields of note:**
- `panel: GuiNinePatch` — background patch for `GuiWindow`, `GuiLabel`, `GuiProgressBar`.
- `button: Option<GuiButtonSkin>` — four nine-patches (`normal`/`hover`/`pressed`/`disabled`); unset states fall back to `normal`. Also has four optional per-state shadows (`shadow`, `hover_shadow`, `pressed_shadow`, `disabled_shadow`); unset states fall back to `shadow` (normal), which itself falls back to `panel_shadow`. Build it with `GuiButtonSkin::new(normal)` plus `.with_hover(patch)`, `.with_pressed(patch)`, `.with_disabled(patch)`, `.with_shadow(shadow)` and `.with_hover_shadow`/`.with_pressed_shadow`/`.with_disabled_shadow`.
- `label: Option<GuiNinePatch>` — separate background for `GuiLabel` (falls back to `panel` if unset).
- `progress_bar: Option<GuiProgressBarSkin>` — `track: Option<GuiNinePatch>` (full-size background, optional) and `fill: GuiNinePatch` (scaled to `value/max`). Build it with `GuiProgressBarSkin::new(fill)`, plus `.with_track(track)` for a background.
- `panel_shadow: Option<Shadow>` — drop shadow drawn behind all nine-patch backgrounds.
- `text_shadow: Option<Shadow>` — `Shadow` component inserted on spawned caption `DynamicText` children.

**Multi-theme UIs:** insert additional entries and use `.with_theme_key("my_theme")` on any widget:

```rust
let hud_theme = theme_store.themes.entry(Arc::from("hud")).or_default();
hud_theme.panel = GuiNinePatch::new("hud_panel", Rect::new(0.0, 0.0, 48.0, 48.0), 4);
hud_theme.font = "hud_font".into();

// Spawn a widget using the "hud" theme
commands.spawn((
    GuiWindow::new(200.0, 40.0).with_theme_key("hud"),
    ScreenPosition::new(10.0, 10.0),
    ZIndex(5.0),
));
```

A missing/unregistered `theme_key` skips the themed background (caption/sprite still renders) and logs a warning once per widget via `GuiThemeWarnCache`.

**Complete example — a panel with one clickable button:**

```rust
use aberredengine::prelude::*;

fn on_start_clicked(_ev: On<GuiClicked>, mut signals: ResMut<WorldSignals>) {
    signals.set_flag("start_pressed");
}

fn spawn_menu_panel(mut commands: Commands) {
    let panel = commands
        .spawn((
            GuiWindow::new(200.0, 100.0),
            ScreenPosition::new(50.0, 50.0),
            ZIndex(10.0),
        ))
        .id();

    commands
        .spawn((
            GuiButton::new(120.0, 32.0, "Start"),
            ChildOf(panel),
            GuiOffset(Vec2::new(40.0, 34.0)),
            ZIndex(10.0),
        ))
        .observe(on_start_clicked);
}
```

The button entity needs no `ScreenPosition` set directly — `gui_layout_system` supplies it each sim tick from
the parent's `ScreenPosition` plus `GuiOffset`.

**Click events:** a press then release inside any `GuiInteractable`-carrying widget (`GuiButton` or
`GuiImage`) triggers `GuiClicked { entity }` (`aberred-core/src/events/gui_interactable.rs`) on that widget.
Observe it per widget with `.observe(handler)`, as above, or once with `EngineBuilder::add_observer` for
cross-cutting logic such as a UI click sound. A disabled widget never triggers it.

### 7.8 Particle Emitters

**Source:** `aberred-core/src/components/particleemitter.rs`, `aberred-core/src/systems/particleemitter.rs`

A `ParticleEmitter` on an entity with `MapPosition` spawns particles by cloning **template** entities. Each emission picks a random template, clones every component it has, then sets the clone's `MapPosition` (inside the emitter's `shape`, plus `offset`), `Rotation` (the emission angle), `RigidBody` velocity (angle × a speed from `speed_range`; the template's own `RigidBody` friction, max speed and forces carry over), an optional `Ttl`, and `EmittedParticle(emitter)`.

```rust
use aberredengine::prelude::*;

fn spawn_smoke(mut commands: Commands) {
    // The template has no MapPosition, so it is never drawn, moved or collided
    let smoke = commands
        .spawn((
            Sprite::new("smoke", 8.0, 8.0).centered(),
            ZIndex(5.0),
            RigidBody::with_physics(2.0, None), // every particle keeps this friction
        ))
        .id();

    commands.spawn((
        MapPosition::new(100.0, 100.0),
        ParticleEmitter {
            templates: vec![smoke],
            shape: EmitterShape::Rect { width: 16.0, height: 4.0 },
            particles_per_emission: 3,
            emissions_per_second: 10.0,
            emissions_remaining: 100,
            initial_emissions_remaining: 100,
            arc_degrees: (-30.0, 30.0), // 0° is up, angles grow clockwise
            speed_range: (50.0, 100.0),
            ttl: TtlSpec::Range { min: 0.5, max: 1.0 },
            ..Default::default()
        },
    ));
}
```

- Leave `MapPosition` off a template: without it the template itself is never drawn, moved or collided, while its clones get a position. Give it a `ZIndex`, or the particles aren't drawn.
- The emitter runs every sim tick in `SimSet::Movement`. It emits `emissions_per_second` times a second, `particles_per_emission` particles each time, until `emissions_remaining` reaches 0; set `emissions_remaining` again to restart it. `TtlSpec::None` keeps particles until you despawn them.
- Random choices come from `SimRng`, so a `.deterministic(seed)` game emits the same particles every run.
- Templates are regular entities: a scene switch despawns them (and the emitter) unless they're `Persistent`. An emitter skips templates that no longer exist.
- `EmittedParticle(Entity)` names the emitter that spawned a particle, e.g. to despawn one emitter's particles.

### 7.9 Attaching Entities (StuckTo)

**Source:** `aberred-core/src/components/stuckto.rs`, `aberred-core/src/systems/stuckto.rs`

`StuckTo` makes an entity's `MapPosition` follow another entity's, plus an offset, on both axes (`StuckTo::new`) or one (`follow_x_only`, `follow_y_only`). The classic use is a ball resting on a paddle until launch:

```rust
use aberredengine::prelude::*;

fn stick_ball_to_paddle(commands: &mut Commands, ball: Entity, paddle: Entity) {
    commands.entity(ball).insert(
        StuckTo::follow_x_only(paddle)
            .with_offset(Vec2::new(0.0, -12.0))
            .with_stored_velocity(Vec2::new(150.0, -300.0)),
    );
}

fn launch_ball(
    mut commands: Commands,
    input: Res<InputState>,
    mut stuck: Query<(Entity, &StuckTo, &mut RigidBody)>,
) {
    if !input.action(InputAction::Action1).just_pressed {
        return;
    }
    for (ball, stuck_to, mut rb) in &mut stuck {
        // Removing StuckTo doesn't apply stored_velocity; do it here
        if let Some(velocity) = stuck_to.stored_velocity {
            rb.velocity = velocity;
        }
        commands.entity(ball).remove::<StuckTo>();
    }
}
```

- `stuck_to_entity_system` runs every sim tick in `SimSet::Collision`, after movement and collision detection, and overwrites only the followed axes. Keep a stuck entity's velocity at zero, or it drifts along any axis it doesn't follow.
- `stored_velocity` is only data: removing `StuckTo` from Rust doesn't apply it. Apply it yourself when you release the entity, as `launch_ball` does. (Lua's `engine.release_stuckto` does it for you.)
- The follower copies the target's `MapPosition` field, which is the target's local position if the target is a `ChildOf` child. A target that is itself `StuckTo` isn't followed (no chains), and a follower with `ChildOf` is skipped. For permanent parent-relative placement, use `ChildOf` instead.
- If the target is despawned, the follower stays where it is and keeps its `StuckTo` until you remove it.

---

## 8. Engine Resources Quick Reference

All resources are accessed as Bevy ECS system parameters. Use `Res<T>` / `ResMut<T>` for Send resources, `NonSend<T>` / `NonSendMut<T>` for main-thread-only resources.

**Read this table by thread, not just by Send/NonSend** — see [Threading Model](#threading-model-what-your-code-can-access). The tables below are split into logic-thread resources (everything your `setup` hook, systems and observers can request) and render-thread-only resources (things only `process_render_asset_cmds`/`render_system` touch — a logic-side system that requests one panics the first time it runs, whether through `Res` or `NonSend`).

### Logic-thread resources (Send) — what your game code can use

| Resource | Access | Purpose |
|----------|--------|---------|
| `WorldTime` | `Res` | `elapsed`, `delta` (the fixed sim period, `1.0 / hz`, scaled by `time_scale` — never a measured value), `time_scale`, `frame_count` |
| `WorldSignals` | `ResMut` | Global cross-system communication (scalars, integers, strings, flags, entities) |
| `AppState` | `ResMut` | Rust-only typed state store keyed by Rust type; useful for GUI/editor snapshots and view-models |
| `TrackedGroups` | `ResMut` | Group names to count — engine publishes counts to `WorldSignals` each sim tick |
| `ScreenSize` | `Res` | Internal render resolution (`w`, `h`) — inserted independently on both the logic and render threads, always in sync |
| `WindowSize` | `Res` | OS window dimensions (`w`, `h`), has `calculate_letterbox()` and `window_to_game_pos()` |
| `GameConfig` | `ResMut` | Loaded from `config.ini` — all render/window/simulation/audio settings |
| `GameConfigDefaults` | `Res` | Read-only snapshot (`.0: GameConfig`) of `GameConfig` as loaded at startup, before any runtime mutation — use to restore a field to its loaded default (e.g. window title after a map override) without needing your own capture-resource |
| `InputState` | `Res` | Input state — bound actions via `input.action(InputAction::X)` and the raw `mouse_left_button` are `BoolState { active, just_pressed, just_released }`; analog fields (`scroll_y`, `mouse_x/y`, `mouse_world_x/y`) are `f32`; pad 0 is `gamepad_connected` / `gamepad_axes` |
| `InputBindings` | `ResMut` | Runtime key/mouse binding map (`InputAction` → `Vec<InputBinding>`). Modify to rebind actions at runtime — takes effect the very next sim tick. |
| `GameState` | `Res` | Current state: `None → Setup → Playing → Quitting` |
| `NextGameState` | `ResMut` | Request state transitions, e.g. `.set(GameStates::Quitting)` |
| `PostProcessShader` | `ResMut` | Shader chain + uniforms (reserved: `uTime`, `uDeltaTime`, `uResolution`, `uFrame`, `uWindowResolution`, `uLetterbox`) |
| `CameraFollowConfig` | `ResMut` | Camera-follow behavior (mode, easing, zoom speed, bounds, offsets); see [Following an entity](#following-an-entity) |
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

These exist only in the render world. `RaylibAccess`/`NonSend<FontStore>`/`NonSend<ShaderStore>`/`Res<TextureStore>` as a parameter on any `setup` hook, observer or `.add_system()`/`.configure_schedule()` system panics the first time that system runs — there is no way around this from a custom Rust system. Listed here for completeness (e.g. if you're reading engine source), not because you can request them:

| Resource | Access (render-side only) | Purpose |
|----------|---------------------------|---------|
| `RaylibHandle` / `RaylibThread` | via `RaylibAccess` SystemParam | Raylib context |
| `FontStore` | `NonSendMut` | Loaded fonts by key (GPU-bound) |
| `ShaderStore` | `NonSendMut` | Loaded shaders with cached uniform locations |
| `TextureStore` | `Res` / `ResMut` | Loaded textures by key (GPU-bound) |
| `RenderTarget` | `NonSendMut` | Internal framebuffer |

### Render-thread-only resources used by GUI and world-draw callbacks

Since those two callbacks are the one exception that runs render-side (see [Threading Model](#threading-model-what-your-code-can-access) and [Section 3](#imgui-gui-callback-rust-only)), they're handed these as fields of their `GuiCtx`/`WorldDrawCtx` instead of fetched as `Res<T>`/`ResMut<T>` — and instead of the live `WorldSignals`:

| Resource | Purpose |
|----------|---------|
| `SignalSnapshot` | Read-only, one-tick-stale copy of `WorldSignals`. Read it through `SignalsRead` (in the prelude), the getters it shares with `WorldSignals`: `get_scalar`, `get_integer`, `get_string`, `has_flag`, `get_entity`, `get_group_count`. |
| `SignalIntents` | Queue of pending `WorldSignals` writes. Setter methods mirror `WorldSignals`' own: `.set_flag(key)`, `.set_scalar(key, v)`, `.set_integer(key, v)`, `.set_string(key, v)`, `.remove_flag(key)`. Applied to the live `WorldSignals` at the start of the logic thread's next sim tick. |
| `TextureStore` | Read-only access to loaded textures by key, e.g. for previews. |
| `FontStore` | Read-only access to loaded fonts by key, e.g. for text measurement. |
| `AppState` | The same read-only, generation-gated snapshot copy described in the AppState API section below — write typed Rust state from a logic-thread system/observer instead. |

### Developer-inserted resources

| Resource | Access | Purpose |
|----------|--------|---------|
| `DebugMode` | marker resource | Presence enables debug overlays |

Fullscreen is not a resource your game can insert: the engine's `FullScreen` marker lives only in the render world. To start fullscreen, set `fullscreen = true` under `[window]` in `config.ini` (see [Section 9](#9-the-configini-file)). At runtime, F10 toggles it by default (the `ToggleFullscreen` action).

### WorldSignals API

`WorldSignals` is the most-used resource. It provides typed key-value storage for cross-system communication.

**Scalars (`f32`):**

| Method | Signature |
|--------|-----------|
| `set_scalar` | `(&mut self, key: impl Into<String>, value: f32)` |
| `get_scalar` | `(&self, key: &str) -> Option<f32>` |
| `remove_scalar` | `(&mut self, key: &str) -> Option<f32>` |

**Integers (`i32`):**

| Method | Signature |
|--------|-----------|
| `set_integer` | `(&mut self, key: impl Into<String>, value: i32)` |
| `get_integer` | `(&self, key: &str) -> Option<i32>` |
| `remove_integer` | `(&mut self, key: &str) -> Option<i32>` |

**Strings:**

| Method | Signature |
|--------|-----------|
| `set_string` | `(&mut self, key: impl Into<String>, value: impl Into<String>)` |
| `get_string` | `(&self, key: &str) -> Option<&str>` |
| `remove_string` | `(&mut self, key: &str) -> Option<String>` |

**Flags (presence-based booleans):**

| Method | Signature |
|--------|-----------|
| `set_flag` | `(&mut self, key: impl Into<String>)` |
| `has_flag` | `(&self, key: &str) -> bool` |
| `remove_flag` | `(&mut self, key: &str) -> bool` — clears the flag; returns whether it was present. |
| `take_flag` | `(&mut self, key: &str) -> bool` — the same operation as `remove_flag`, named for consuming a one-shot flag: `if signals.take_flag("gui:action:save") { … }`. Preferred in `on_update` to consume a GUI action flag. |

**Entities:**

| Method | Signature |
|--------|-----------|
| `set_entity` | `(&mut self, key: impl Into<String>, entity: Entity)` |
| `get_entity` | `(&self, key: &str) -> Option<Entity>` |
| `remove_entity` | `(&mut self, key: &str) -> Option<Entity>` |
| `remove_entity_registrations_for` | `(&mut self, entity: Entity)` — removes every entity-keyed registration pointing at `entity`, regardless of key. You rarely need it: at the end of every sim tick the engine drops registrations whose entity has been despawned, however it was despawned (`commands.entity(e).despawn()`, `Ttl`, a scene switch), so a registration never resolves to a dead entity past the tick that despawned it. Call it yourself only when a later system in the *same* tick must already see the key gone. |
| `clear_non_persistent_entities` | `(&mut self, persistent_entities: &FxHashSet<Entity>)` — drops every registered entity not present in `persistent_entities`. Called automatically by `scene_switch_system` on every scene transition; call it yourself only if you build custom scene-transition logic outside `.add_scene()`. |

**Group counts** (stored as integers with `"group_count:"` prefix):

| Method | Signature |
|--------|-----------|
| `set_group_count` | `(&mut self, group_name: &str, count: i32)` |
| `get_group_count` | `(&self, group_name: &str) -> Option<i32>` |
| `clear_group_counts` | `(&mut self)` |

`WorldSignals` intentionally stays limited to those primitive/value-like channels. For richer Rust-only typed data, use `AppState` instead.

The engine's own reserved signal keys (`"scene"`, `"switch_scene"`, `"quit_game"`, and others) are exposed as constants in `aberredengine::core::resources::signal_keys` ; the prelude imports the module as `sk`. Prefer `sk::SCENE`/`sk::SWITCH_SCENE`/etc. over hand-typing the string literals: a typo in a bare string key fails silently, with no compiler error.

### AppState API

`AppState` is a Rust-only typed store keyed by `TypeId`. The engine inserts it automatically at startup. It stores one value per Rust type, so `insert::<T>` replaces any previous `T`.

| Method | Signature |
|--------|-----------|
| `insert` | `(&mut self, value: T) where T: Any + Send + Sync + Clone` |
| `get::<T>` | `(&self) -> Option<&T>` |
| `get_mut::<T>` | `(&mut self) -> Option<&mut T>` |
| `remove::<T>` | `(&mut self) -> Option<T>` |
| `contains::<T>` | `(&self) -> bool` |

Use `AppState` for richer GUI/editor snapshots and view-models that do not belong in the Lua-visible signal bus. If you need two values of the same underlying type, wrap them in newtypes.

**How render-side callbacks see it:** the live `AppState` exists only in the logic world. Render-thread callbacks (GUI and world-draw callbacks) receive a cloned, read-only copy carried in the snapshot. A `generation` counter decides when that copy is refreshed:

- `insert` and `get_mut` always bump `generation` (`get_mut` even if you don't end up writing); `remove` bumps it only when a value was removed; `get` never does.
- The snapshot clones `AppState` again only when `generation` has changed since the last publish. Inserting every tick (as the §3 editor example does for brevity) therefore forces a full clone on every snapshot — insert or `get_mut` only when the value actually changes.
- Nothing a render-side callback does to its copy reaches the logic world. Send changes back through `SignalIntents` instead.
- For state that both sides must share mutably (e.g. an editor cache), store an `Arc<Mutex<T>>` (or `Arc<RwLock<T>>`): cloning it copies the pointer, so both sides see the same data. A bare `Mutex<T>` is not `Clone` and cannot be inserted.

```rust
use aberredengine::prelude::*;

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
fn inspector_gui(ctx: &mut GuiCtx) {
    if let Some(snapshot) = ctx.app_state.get::<InspectorSnapshot>() {
        ctx.ui.text(format!("Selected: {}", snapshot.selected_name));
    }
}
```

### InputState key bindings

Each bound action is a `BoolState { active, just_pressed, just_released }`, read with `input.action(InputAction::X)`. Hardware assignments live in `InputBindings`, not in `BoolState`. Mouse and scroll values are plain `f32`; pad 0 has its own fields (below).

**Actions (`InputAction` → `BoolState`):**

| `InputAction` | Default binding | Description |
|---------------|-----------------|-------------|
| `MainDirectionUp` | W | WASD up |
| `MainDirectionDown` | S | WASD down |
| `MainDirectionLeft` | A | WASD left |
| `MainDirectionRight` | D | WASD right |
| `SecondaryDirectionUp` | Up arrow | Alternative up |
| `SecondaryDirectionDown` | Down arrow | Alternative down |
| `SecondaryDirectionLeft` | Left arrow | Alternative left |
| `SecondaryDirectionRight` | Right arrow | Alternative right |
| `Action1` | Space, mouse left | Primary action |
| `Action2` | Enter, mouse right | Secondary action |
| `Action3` | Mouse middle | Tertiary action (no keyboard default) |
| `Back` | Escape | Back/cancel |
| `Special` | F12 | Special action |
| `ToggleDebug` | F11 | Debug toggle |
| `ToggleFullscreen` | F10 | Fullscreen toggle |

**Raw digital field:** `mouse_left_button` (`BoolState`) is the literal left mouse button, not routed through `InputBindings`; GUI hit-testing reads it.

**Analog fields (`f32`):**

| Field | Description |
|-------|-------------|
| `scroll_y` | Mouse wheel delta this sim tick. Positive = up, negative = down. |
| `mouse_x` | Cursor X in game/render-target space (letterbox-corrected, 0..render_width). |
| `mouse_y` | Cursor Y in game/render-target space (letterbox-corrected, 0..render_height). |
| `mouse_world_x` | Cursor X in world-space (after camera transform, matches `MapPosition`). |
| `mouse_world_y` | Cursor Y in world-space (after camera transform, matches `MapPosition`). |

**Gamepad fields (pad 0):**

| Field | Type | Description |
|-------|------|-------------|
| `gamepad_connected` | `bool` | Whether pad 0 is connected this tick. |
| `gamepad_axes` | `[f32; 6]` | Raw axes `[LX, LY, RX, RY, LT, RT]`, newest sample wins; not deadzoned (the deadzone applies only to axis→button bindings). |

### InputBindings resource

`InputBindings` (`aberred-core/src/resources/input_bindings.rs`) maps logical `InputAction` variants to a `Vec<InputBinding>`, supporting multiple hardware bindings per action (e.g. both W and the Up arrow can trigger `MainDirectionUp`).

Each `InputBinding` is one of:

| Variant | Values |
|---------|--------|
| `InputBinding::Keyboard(Key)` | `Key::KEY_*` constants (`Key::KEY_SPACE`, `Key::KEY_A`, `Key::KEY_UP`, …) |
| `InputBinding::MouseButton(MouseButton)` | `MouseButton::MOUSE_BUTTON_LEFT`/`_RIGHT`/`_MIDDLE`, … |
| `InputBinding::GamepadButton { pad, button }` | `pad` is `0..4`; `GamepadButton::GAMEPAD_BUTTON_*` |
| `InputBinding::GamepadAxis { pad, axis, direction }` | `GamepadAxis::GAMEPAD_AXIS_*` and `AxisDirection::Positive`/`Negative`; the action is active while the axis is past the engine's fixed threshold in that direction |

`InputBindings::default()` binds every action to its keyboard/mouse default (see the table above) **and** additively binds the equivalent pad-0 gamepad button/axis/d-pad input to the same action, so a game gets working gamepad input with zero configuration.

Change bindings from any logic-side system through `ResMut<InputBindings>`. `rebind` replaces all of an action's bindings (its gamepad defaults included); `add_binding` appends one and keeps the rest. A change takes effect on the next sim tick.

```rust
use aberredengine::prelude::*;
use aberredengine::core::resources::input_bindings::{
    AxisDirection, GamepadAxis, InputBinding, InputBindings, Key,
};

fn setup_controls(mut bindings: ResMut<InputBindings>) {
    // J becomes the only Action1 binding
    bindings.rebind(InputAction::Action1, InputBinding::Keyboard(Key::KEY_J));

    // Backspace also means Back; Escape still works
    bindings.add_binding(InputAction::Back, InputBinding::Keyboard(Key::KEY_BACKSPACE));

    // Pad 1's left stick also drives MainDirectionRight
    bindings.add_binding(
        InputAction::MainDirectionRight,
        InputBinding::GamepadAxis {
            pad: 1,
            axis: GamepadAxis::GAMEPAD_AXIS_LEFT_X,
            direction: AxisDirection::Positive,
        },
    );

    let current: &[InputBinding] = bindings.get_bindings(InputAction::Action1);
}
```

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
| `snapshot_skip` | `u32` | `round(hz / effective_fps) - 1`, `effective_fps` = `[window] target_fps` (default `120`, so the default skip is `1`), or `60` when `target_fps = 0` | Number of sim ticks the PRESENT schedule skips between publishes of render state to the render thread — `0` publishes every tick, `N` publishes every `N+1`th tick (effective rate `hz / (N+1)`). Re-resolved from `hz`/`target_fps` whenever this key isn't set explicitly. Clamped to `[0, 1000]`. |

**`[audio]` section:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `hz` | `f64` | `100` | Audio thread's tick rate. Read once at startup; same clamp range as `[simulation] hz`. |

**`[input]` section:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `gamepad_deadzone` | `f32` | `0.15` | Analog-stick deadzone radius used when resolving an `InputBinding::GamepadAxis` binding to a digital press/release edge. Clamped to `[0.0, 1.0]`, out-of-range values warn and clamp rather than error. Raw `InputState`/`InputSnapshot` gamepad axis values are never deadzoned regardless of this setting — the deadzone only affects binding resolution. |

### Parsing behavior

- **`config_str()`** — alternative to a file: pass INI content as a `&'static str`. The file path is ignored when this is set.
- **Missing file** -> startup error from `EngineBuilder::try_run()`
- **Unreadable or malformed INI** -> startup error from `EngineBuilder::try_run()`
- **Missing key** -> default for that key
- **Out-of-range value** -> numeric fields (`simulation.hz`, `audio.hz`, `simulation.snapshot_skip`, `input.gamepad_deadzone`) warn and clamp to the nearest valid bound; `render_target_filter` warns and falls back to its default (`nearest`)
- **Unparseable value** -> a present key whose value does not parse as its type (e.g. `hz = abc`, `vsync = no`) **silently** keeps its default — no warning is logged
- **Booleans** — only `true`/`false`, case-insensitive (`TRUE`, `False`). `yes`/`no`, `on`/`off` and `1`/`0` do not parse, so the key keeps its default
- **`background_color`** — comma-separated `R,G,B` integers (e.g., `80,80,80`)

If you want the engine defaults, you can leave `config.ini` out entirely. To override some settings, create the file with only those keys.

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

> **Note:** `save_to_file()` reads the existing file first and replaces only the keys `GameConfig` owns. Other sections and keys, including your game's own, are kept. Comments are not: the file is rewritten without them. `[simulation] snapshot_skip` is written only when the file already sets it, so its default keeps following `hz` and `target_fps`; a runtime change to `snapshot_skip` is saved only if the file already sets it. It writes to the `.config(path)` path (default `config.ini`), even when the config came from `.config_str()`.

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
RUST_LOG=debug cargo run       # More log output (needs the logger from Section 2)
```

Working directory matters — `config.ini` and `assets/` are loaded relative to where you run the binary.

### Feature flags

| Flag | Default | Effect |
|------|---------|--------|
| `lua` | on | Lua scripting support (mlua + LuaJIT) |
| `tracy` | off | Profiling via the Tracy client. Never enable in a normal build — build with `--profile release-tracy` instead when profiling. |
| `test-support` | off | Enables the headless `TestWorld`/`TestWorldBuilder` harness (a real logic-thread `World` with no window/GL/audio thread), for your own game's tests; see [Testing your game](#testing-your-game). Never in default. |

```toml
# Disable Lua (pure Rust)
aberredengine = { path = "../aberredengine/crates/aberredengine", default-features = false }
```

Disabling Lua removes: mlua dependency, LuaJIT compilation, all Lua-specific systems. Faster builds, smaller binary.

### Testing your game

The `test-support` feature adds `aberredengine::test_support::TestWorld`: the engine's real logic-thread world and sim schedule, with no window, GL or audio thread. Enable it for tests only, as a dev-dependency alongside your normal dependency:

```toml
[dev-dependencies]
aberredengine = { path = "../aberredengine/crates/aberredengine", default-features = false, features = ["test-support"] }
```

Repeat the exact source of your `[dependencies]` entry (the same `path`, or the same `git` plus `branch`/`tag`/`rev`): Cargo rejects a crate whose dependency and dev-dependency point to different sources.

`TestWorld::builder()` takes the same registration calls as `EngineBuilder` (`on_setup`, `add_system`, `add_system_if`, `add_scene_system`, `on_scene_enter`/`on_scene_exit`, `configure_schedule`, `add_observer`, `add_scene`/`initial_scene`, `deterministic`, plus `config(GameConfig)`). Like a real game, it moves from `Setup` to `Playing` on its own. `tick(n, dt)` runs `n` sim ticks with a fixed `dt`, and `tw.world` is a plain Bevy `World` to spawn into and inspect:

```rust
#[cfg(test)]
mod tests {
    use super::{Health, remove_dead};
    use aberredengine::test_support::TestWorld;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn dead_entities_are_removed() {
        let mut tw = TestWorld::builder()
            .add_system(remove_dead)
            .build()
            .expect("test world");
        tw.tick_to_play(DT, 10);

        let enemy = tw.world.spawn(Health(0)).id();
        tw.tick(1, DT);

        assert!(tw.world.get_entity(enemy).is_err());
    }
}
```

- No real assets load: `deliver_texture_dims(key, w, h)` and `deliver_font_metrics(key, metrics)` fake the render thread's load replies, and `sent_to_render` receives the `RenderMsg`s (asset commands, quit) the game would have sent.
- `audio_cmds` receives the `AudioCmd`s your code writes; send fake replies through `audio_msgs_tx`.
- `send_input(sample)` feeds one raw input sample: start from `RawDeviceSnapshot::default()` (`aberredengine::core::protocol::raw_input`) and press keys with `sample.set_key(Key::KEY_SPACE.as_u32())`. `present()` returns the `DrawableSnapshot` the render thread would draw.

### Optimization tip

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```
