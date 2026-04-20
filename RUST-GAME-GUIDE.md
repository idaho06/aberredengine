# Aberred Engine — Rust Game Developer Guide

This guide explains how to build a 2D game in pure Rust using the Aberred Engine as a library dependency, without Lua scripting.

## 1. What the Engine Provides

Aberred Engine is a 2D game engine built on **Bevy ECS 0.18** and **Raylib 5.5.1**. It handles the main loop, windowing, rendering, and a full ECS system schedule. You supply game-specific logic via hook functions.

Built-in systems:

- **Rendering** — sprites, text, per-entity shaders, post-process shader chains, camera, letterboxing
- **Physics** — velocity, friction, max speed, named acceleration forces, freeze/unfreeze
- **Collision** — AABB detection with group-based rules and callback dispatch
- **Audio** — music and sound playback via a background thread bridge
- **Input** — keyboard polling with `just_pressed`/`just_released` tracking
- **Menus** — scrollable interactive menus with selection callbacks
- **Animation** — frame-based sprite animation with controller rules
- **Particles** — emitter system with templates, shapes, arcs, speed ranges
- **Scene management** — named scenes with enter/update/exit callbacks, auto-despawn
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
├── config.ini                 # Engine configuration (optional, defaults apply)
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

The engine reads `config.ini` at startup for window and rendering settings. If the file is missing or a value is absent, safe defaults are used.

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
```

---

## 3. EngineBuilder

The engine owns the main loop. You configure it through `EngineBuilder` and supply game logic via hook functions. There are two approaches depending on whether your game has multiple scenes.

### Approach A — SceneManager (recommended for multi-scene games)

Register named scenes with enter/update/exit callbacks. The engine handles despawning non-persistent entities on scene transitions and dispatching to the correct scene's callbacks.

```rust
use aberredengine::engine_app::EngineBuilder;
use aberredengine::systems::scene_dispatch::SceneDescriptor;

mod scenes;

fn main() {
    EngineBuilder::new()
        .config("config.ini")
        .title("My Game")
        .on_setup(scenes::load_assets)
        .add_scene("menu", SceneDescriptor {
            on_enter:     scenes::menu::enter,
            on_update:    Some(scenes::menu::update),
            on_exit:      None,
            gui_callback: None,
        })
        .add_scene("level01", SceneDescriptor {
            on_enter:     scenes::level01::enter,
            on_update:    Some(scenes::level01::update),
            on_exit:      Some(scenes::level01::exit),
            gui_callback: None,
        })
        .initial_scene("menu")
        .run();
}

```

Scene callback signatures:

```rust
use aberredengine::systems::GameCtx;
use aberredengine::resources::input::InputState;
use aberredengine::systems::scene_dispatch::GuiCallback; // only needed if using gui_callback

// Called once when the scene becomes active
fn enter(ctx: &mut GameCtx) { /* spawn entities, set signals */ }

// Called every frame while the scene is active
fn update(ctx: &mut GameCtx, dt: f32, input: &InputState) { /* per-frame logic */ }

// Called once when leaving the scene (before entities are despawned)
fn exit(ctx: &mut GameCtx) { /* cleanup */ }

// Called every frame to draw ImGui widgets — Rust-only, optional
// Signature must match: fn(&aberredengine::imgui::Ui, &mut WorldSignals)
fn my_gui(ui: &aberredengine::imgui::Ui, signals: &mut WorldSignals) { /* draw widgets, write signals */ }
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

The callback receives a `&aberredengine::imgui::Ui` handle for drawing widgets and a `&mut WorldSignals` for bidirectional communication with the scene's `on_update`:

```rust
use aberredengine::imgui;
use aberredengine::resources::worldsignals::WorldSignals;

fn editor_gui(ui: &imgui::Ui, signals: &mut WorldSignals) {
    // Read state written by on_update
    let tool = signals.get_string("gui:state:active_tool")
        .map(|s| s.as_str())
        .unwrap_or("select");

    if let Some(_mb) = ui.begin_main_menu_bar() {
        if let Some(_file) = ui.begin_menu("File") {
            if ui.menu_item("Save") {
                signals.set_flag("gui:action:file:save"); // consumed by on_update next frame
            }
        }
    }
}

fn editor_update(ctx: &mut GameCtx, _dt: f32, _input: &InputState) {
    ctx.world_signals.set_string("gui:state:active_tool", "place");

    if ctx.world_signals.flags.remove("gui:action:file:save") {
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
})
```

> **Convention:** prefix all GUI signal keys with `"gui:"` to avoid collisions with game signals. Use `"gui:action:<verb>"` for flags set by the GUI and consumed by `on_update`, and `"gui:state:<name>"` for values set by `on_update` and read by the GUI.

### Approach B — Raw hooks (single-scene or full manual control)

For single-scene games or when you need full control over scene transitions, use the four hook methods directly:

```rust
use aberredengine::engine_app::EngineBuilder;

fn main() {
    EngineBuilder::new()
        .config("config.ini")
        .title("My Game")
        .on_setup(my_setup)
        .on_enter_play(my_enter_play)
        .on_update(my_update)
        .on_switch_scene(my_switch_scene)
        .run();
}
```

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

1. **Setup** — The engine calls the `setup` hook once. Load assets here (textures, fonts, sounds, shaders, animations).
2. **Playing** — The engine transitions to playing, calls `enter_play` (or the initial scene's `on_enter`), then runs `update` (or `on_update`) every frame.
3. **Scene switches** — When `WorldSignals` has the `"switch_scene"` flag set, the engine calls the `switch_scene` hook (or the SceneManager's exit→enter sequence).
4. **Quitting** — When `WorldSignals` has the `"quit_game"` flag set or the window is closed, the engine shuts down.

### Builder method reference

| Method | Description |
|--------|-------------|
| `.config(path)` | Path to `config.ini` (default: `"config.ini"`) |
| `.title(name)` | Window title (overrides config) |
| `.on_setup(system)` | Asset loading hook (called during `Setup` state) |
| `.on_enter_play(system)` | Called once when transitioning to `Playing` |
| `.on_update(system)` | Runs every frame while `Playing` (after `check_pending_state`) — single system only |
| `.on_switch_scene(system)` | Called when a scene transition is requested |
| `.add_scene(name, descriptor)` | Register a named scene (SceneManager path) |
| `.initial_scene(name)` | Which scene starts first (required with `.add_scene()`) |
| `.add_system(system)` | Add a per-frame system. Same auto-constraints as `.on_update()`. Can be called multiple times. |
| `.configure_schedule(closure)` | Add systems with full ordering control — no auto-constraints applied. |
| `.add_observer(observer_fn)` | Register a persistent observer for a custom or engine event. |

**Conflict rules:** `.add_scene()` cannot be combined with `.on_switch_scene()` or `.on_enter_play()` — the SceneManager owns those hooks. Use `.on_setup()` for asset loading in both approaches.

### Custom systems and observers

The three new builder methods let you register multiple independent ECS systems and event observers alongside the existing hooks.

#### `.add_system(system)` — multiple per-frame systems

Registers a Bevy ECS system that runs every frame while `Playing`. Same automatic constraints as `.on_update()`: `run_if(state_is_playing).after(check_pending_state)`. Can be called multiple times.

```rust
EngineBuilder::new()
    .config("config.ini")
    .on_setup(load_assets)
    .add_system(tilemap_load_system)   // runs every frame — checks a signal, then loads
    .add_system(tilemap_save_system)   // independent second system
    .add_scene("editor", /* … */)
    .initial_scene("editor")
    .run();
```

The system signature is a standard Bevy ECS system:

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::systems::RaylibAccess;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::resources::texturestore::TextureStore;

fn tilemap_load_system(
    world_signals: ResMut<WorldSignals>,
    mut tex_store: ResMut<TextureStore>,
    mut raylib: RaylibAccess,
    mut commands: Commands,
) {
    let Some(path) = world_signals.get_string("pending_load_path").cloned() else {
        return; // nothing to do this frame
    };
    // load texture, spawn tile entities, etc.
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    // ...
}
```

> **When to use `.add_system()` vs scene callbacks:** Scene callbacks (`on_enter`, `on_update`) receive `&mut GameCtx` and cannot access `RaylibAccess`. If a per-frame operation needs `RaylibAccess` (e.g., loading textures on demand), register it as a system with `.add_system()` instead. Use a `WorldSignals` flag to communicate the trigger from the scene callback.

#### `.configure_schedule(closure)` — full ordering control

For systems that need custom ordering relative to engine systems, or that must run outside the `Playing` state, pass a closure receiving `&mut Schedule`:

```rust
use aberredengine::systems::movement::movement;
use aberredengine::systems::render::render_system;

EngineBuilder::new()
    .configure_schedule(|schedule| {
        schedule.add_systems(
            undo_system
                .run_if(state_is_playing)
                .after(movement)
                .before(render_system),
        );
    })
    // …
    .run();
```

Engine system functions are `pub` and importable from `aberredengine::systems::*`. Use them directly as `.after()` / `.before()` arguments. No automatic `run_if` or `after` constraints are applied — you control everything.

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
    .run();
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

> You can also observe engine-defined events: `CollisionEvent`, `TimerEvent`, `InputEvent`, `GameStateChangedEvent`, `AudioCmd`, etc.

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

The setup hook is a standard Bevy ECS system. It receives ECS resources as parameters — you request exactly what you need:

```rust
use aberredengine::systems::RaylibAccess;
use aberredengine::resources::texturestore::TextureStore;
use aberredengine::resources::fontstore::FontStore;
use aberredengine::resources::shaderstore::ShaderStore;
use aberredengine::resources::animationstore::{AnimationStore, AnimationResource};
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::raylib::prelude::*;
use aberredengine::events::audio::AudioCmd;
use std::sync::Arc;

fn setup(
    mut next_state: ResMut<NextGameState>,
    mut tex_store: ResMut<TextureStore>,
    mut anim_store: ResMut<AnimationStore>,
    mut raylib: RaylibAccess,
    mut fonts: NonSendMut<FontStore>,
    mut shaders: NonSendMut<ShaderStore>,
    mut audio: MessageWriter<AudioCmd>,
) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    // ... load assets here ...
}
```

`RaylibAccess` is a `SystemParam` that bundles `NonSendMut<RaylibHandle>` and `NonSend<RaylibThread>`. You destructure it into `(rl, th)` to call Raylib loading functions.

### What's pre-inserted vs. what you must create

| Resource | Pre-inserted by engine? | Send? |
|----------|------------------------|-------|
| `FontStore` | Yes | No (`NonSendMut`) |
| `ShaderStore` | Yes | No (`NonSendMut`) |
| `TextureStore` | Yes — request via `ResMut<TextureStore>` | Yes (`Res`/`ResMut`) |
| `AnimationStore` | Yes — request via `ResMut<AnimationStore>` | Yes |
| `Camera2DRes` | Yes — pre-set to center offset; override via `ResMut<Camera2DRes>` | Yes |

### Textures

`TextureStore` is pre-inserted by the engine. Request it as `ResMut<TextureStore>` and call `.insert()` to populate it:

```rust
let tex = rl.load_texture(th, "assets/textures/player.png")
    .expect("Failed to load player texture");
tex_store.insert("player", tex);

let bg = rl.load_texture(th, "assets/textures/background.png")
    .expect("Failed to load background texture");
tex_store.insert("background", bg);
```

Keys are arbitrary strings you'll reference later in `Sprite` components.

### Fonts

Fonts require `NonSendMut<FontStore>` (already pre-inserted). After loading, you **must** generate mipmaps and set anisotropic filtering to avoid blurry text at non-native sizes.

```rust
use aberredengine::raylib::ffi::{self, TextureFilter::TEXTURE_FILTER_ANISOTROPIC_8X};

/// Load a font with mipmaps and anisotropic filtering.
fn load_font_with_mipmaps(rl: &mut RaylibHandle, th: &RaylibThread, path: &str, size: i32) -> Font {
    let mut font = rl
        .load_font_ex(th, path, size, None)
        .unwrap_or_else(|_| panic!("Failed to load font '{}'", path));
    unsafe {
        ffi::GenTextureMipmaps(&mut font.texture);
        ffi::SetTextureFilter(font.texture, TEXTURE_FILTER_ANISOTROPIC_8X as i32);
    }
    font
}
```

Then in setup:

```rust
let font = load_font_with_mipmaps(rl, th, "assets/fonts/arcade.ttf", 32);
fonts.add("arcade", font);
```

> **Warning:** The mipmap/filter step uses `unsafe` FFI calls. Without it, fonts render blurry when scaled. This is a Raylib-specific requirement — copy the helper function above into your project.

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

Shaders require `NonSendMut<ShaderStore>` (pre-inserted). Load GLSL fragment shaders:

```rust
let shader = rl.load_shader(th, None, Some("assets/shaders/glow.fs"))
    .expect("Failed to load glow shader");

if shader.is_shader_valid() {
    shaders.add("glow", shader);
} else {
    panic!("Shader 'glow' failed validation");
}
```

The first argument to `load_shader` is the vertex shader (`None` uses the default). The second is the fragment shader path.

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

**`TilemapStore` is not pre-inserted** for Rust-only games — you must insert it yourself. Request `ResMut<TilemapStore>` and `ResMut<TextureStore>` in your setup hook alongside `RaylibAccess`:

```rust
use aberredengine::systems::tilemap::load_tilemap;
use aberredengine::resources::tilemapstore::TilemapStore;
use aberredengine::resources::texturestore::TextureStore;
use aberredengine::systems::RaylibAccess;
use aberredengine::bevy_ecs::prelude::*;

fn setup(
    mut commands: Commands,
    mut tex_store: ResMut<TextureStore>,
    mut tilemap_store: ResMut<TilemapStore>,
    mut raylib: RaylibAccess,
    // ... other params
) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);

    // Load tileset texture + JSON data
    let (tex, map) = load_tilemap(rl, th, "assets/tilemaps/level01");
    tex_store.insert("level01", tex);
    tilemap_store.insert("level01", map);
}
```

`load_tilemap` derives both file paths from the last path segment: `<path>/<stem>.png` and `<path>/<stem>.txt`. It panics on IO or parse failure (setup-time, not hot path).

You also need to **insert `TilemapStore` as a resource** before setup runs. Do this in your main:

```rust
EngineBuilder::new()
    .config("config.ini")
    .add_system(|world: &mut World| {
        world.insert_resource(TilemapStore::new());
    })
    .on_setup(setup)
    // ...
```

Or insert it directly before calling `.run()` — the cleanest option is using `.configure_schedule` to insert it as a startup resource, but in practice inserting it in `on_setup` via `Commands::insert_resource` also works:

```rust
fn setup(mut commands: Commands, mut raylib: RaylibAccess, /* ... */) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    let (tex, map) = load_tilemap(rl, th, "assets/tilemaps/level01");

    let mut tex_store = TextureStore::new();
    tex_store.insert("level01", tex);

    let mut tilemap_store = TilemapStore::new();
    tilemap_store.insert("level01", map);

    commands.insert_resource(tex_store);
    commands.insert_resource(tilemap_store);
}
```

**Spawning tiles** is done from a scene's `on_enter` (or `on_enter_play`) callback using `spawn_tiles`. This is a two-phase operation:

1. **Template phase** — one entity per atlas cell, with `Group("tiles-templates")` and `Sprite`. Templates carry no `MapPosition` so they are invisible. They stay alive in the world for potential future re-use.
2. **Instance phase** — one entity per tile placement in the map layers, cloned from the matching template with `Group("tiles")`, `MapPosition`, and `ZIndex` added.

```rust
use aberredengine::systems::tilemap::spawn_tiles;
use aberredengine::resources::tilemapstore::TilemapStore;
use aberredengine::resources::texturestore::TextureStore;

fn enter(ctx: &mut GameCtx) {
    // Access stores through the world (or request them in a raw system)
    // In a scene callback, use a Bevy system to pass TilemapStore and TextureStore:
}
```

Because `GameCtx` does not expose `TilemapStore` or `TextureStore`, tile spawning in a scene callback requires a small helper system registered with `.add_system()`:

```rust
use aberredengine::bevy_ecs::prelude::*;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::resources::tilemapstore::TilemapStore;
use aberredengine::resources::texturestore::TextureStore;
use aberredengine::systems::tilemap::spawn_tiles;

fn spawn_tilemap_system(
    signals: Res<WorldSignals>,
    tilemaps: Res<TilemapStore>,
    textures: Res<TextureStore>,
    mut commands: Commands,
) {
    if !signals.has_flag("spawn_tilemap") {
        return;
    }
    if let (Some(tilemap), Some(tex)) = (tilemaps.get("level01"), textures.get("level01")) {
        spawn_tiles(&mut commands, "level01", tex.width, tex.height, tilemap);
    }
}
```

Trigger it from `on_enter`:

```rust
fn enter(ctx: &mut GameCtx) {
    ctx.world_signals.set_flag("spawn_tilemap");
    // ... spawn other entities
}
```

Register both in your builder:

```rust
EngineBuilder::new()
    .on_setup(setup)
    .add_system(spawn_tilemap_system)  // consumes the flag and spawns tiles
    .add_scene("level01", SceneDescriptor { on_enter: enter, .. })
    .initial_scene("level01")
    .run();
```

**Z-layering:** `spawn_tiles` assigns `ZIndex` values automatically based on layer order. The first layer gets the most negative Z (furthest back), and the last layer gets the least negative. Tile instances are in `Group("tiles")`; use `Group("tiles")` for group-based collision rules if needed.

> **Note:** `load_tilemap` and `spawn_tiles` are always compiled regardless of the `lua` feature flag, making them available to Rust-only downstream crates.

### Camera

`Camera2DRes` is pre-inserted by the engine with `target` at the origin and `offset` at half the render resolution (center-screen). If you need a different initial position, request `ResMut<Camera2DRes>` and overwrite it:

```rust
camera.0 = Camera2D {
    target: Vector2 { x: 0.0, y: 0.0 },
    offset: Vector2 {
        x: rl.get_screen_width() as f32 * 0.5,
        y: rl.get_screen_height() as f32 * 0.5,
    },
    rotation: 0.0,
    zoom: 1.0,
};
```

`offset` is the screen point the camera looks through. `target` is the world position it looks at.

### Complete setup example

```rust
fn setup(
    mut next_state: ResMut<NextGameState>,
    mut tex_store: ResMut<TextureStore>,
    mut anim_store: ResMut<AnimationStore>,
    mut raylib: RaylibAccess,
    mut fonts: NonSendMut<FontStore>,
    mut shaders: NonSendMut<ShaderStore>,
    mut audio: MessageWriter<AudioCmd>,
) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);

    // Textures (TextureStore is pre-inserted — just populate it)
    let player_tex = rl.load_texture(th, "assets/textures/player.png").unwrap();
    tex_store.insert("player", player_tex);

    // Fonts
    let font = load_font_with_mipmaps(rl, th, "assets/fonts/arcade.ttf", 32);
    fonts.add("arcade", font);

    // Audio
    audio.write(AudioCmd::LoadFx { id: "jump".into(), path: "assets/audio/jump.wav".into() });
    audio.write(AudioCmd::LoadMusic { id: "bgm".into(), path: "assets/audio/music.ogg".into() });

    // Shaders
    if let Ok(shader) = rl.load_shader(th, None, Some("assets/shaders/glow.fs")) {
        if shader.is_shader_valid() {
            shaders.add("glow", shader);
        }
    }

    // Animations (AnimationStore is pre-inserted — just populate it)
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
| `Timer` | `Timer::new(duration_secs, callback as TimerCallback)` — **must cast** |
| `Phase` | `Phase::new("initial_phase", phases)` where `phases: FxHashMap<String, PhaseCallbackFns>` |
| `CollisionRule` | `CollisionRule::new("group_a", "group_b", callback as CollisionCallback)` — **must cast** |
| `Tween<MapPosition>` | `Tween::new(MapPosition::from_vec(from), MapPosition::from_vec(to), duration)` |
| `Tween<Rotation>` | `Tween::new(Rotation { degrees: from }, Rotation { degrees: to }, duration)` |
| `Tween<Scale>` | `Tween::new(Scale::new(from_x, from_y), Scale::new(to_x, to_y), duration)` |

### Tween components in Rust

Tweens are now represented by a single generic component: `Tween<T>`.

Use the target component type as `T`:

- `Tween<MapPosition>` for position animation
- `Tween<Rotation>` for rotation animation
- `Tween<Scale>` for scale animation

This replaces the older concrete Rust types such as `TweenPosition`, `TweenRotation`, and `TweenScale`.

`EngineBuilder` already registers the built-in tween systems for these three component types, so in normal game code you only need to spawn the tween component itself.

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

When the `scene_switch_system` runs (`src/systems/scene_dispatch.rs:157-214`), it performs these steps in order:

1. **Stop all music** — sends `AudioCmd::StopAllMusic` to the audio thread
2. **Despawn non-persistent entities** — every entity *without* the `Persistent` component is despawned
3. **Clear group tracking** — `TrackedGroups::clear()` and `WorldSignals` group counts are wiped
4. **Read target scene** — reads `WorldSignals["scene"]` for the target scene name (defaults to `"menu"` if unset)
5. **Call `on_exit` on previous scene** — if there was an active scene with an `on_exit` callback, it fires
6. **Set active scene** — updates `SceneManager.active_scene` to the new scene name
7. **Call `on_enter` on new scene** — fires the new scene's `on_enter` callback, which typically spawns entities and sets up initial state

### 6.2 Triggering scene transitions

Scene transitions work by running the `scene_switch_system` as a one-shot system via `commands.run_system()`. The system is registered in `SystemsStore` under the key `"switch_scene"` when you use `EngineBuilder::add_scene()`.

**Approaches:**

**1. Menu-driven (recommended):** Use `MenuAction::SetScene("level01")` — the menu system calls `commands.run_system()` internally via `dispatch_menu_action`.

**2. Flag-based from scene callbacks:** Set the target scene name and the `"switch_scene"` flag on `WorldSignals`. The engine's `scene_switch_poll` system (registered automatically by `EngineBuilder::add_scene()`) picks up the flag each frame and triggers the transition:

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

### 6.4 Group tracking across scenes

`TrackedGroups` (`src/resources/group.rs`) is a resource holding a set of group names to count. The engine's `update_group_counts_system` publishes entity counts for each tracked group to `WorldSignals` every frame.

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

### 6.5 Per-frame scene updates

The `scene_update_system` (`src/systems/scene_dispatch.rs:220-236`) runs every frame while a scene is active. It looks up the active scene in `SceneManager`, and if it has an `on_update` callback, calls it:

```rust
fn update(ctx: &mut GameCtx, dt: f32, input: &InputState) {
    // dt = world_time.delta (unscaled frame time in seconds)
    // input = current keyboard state (just_pressed, active, just_released)
    // Use ctx to read/write ECS state every frame
}
```

The `dt` parameter is `WorldTime.delta` — the unscaled time since the last frame, in seconds. Use it for frame-rate-independent logic (e.g., `speed * dt`).

---

## 7. Gameplay Systems

The engine provides four major gameplay systems: **timers**, **phase state machines**, **collision rules**, and **menus**. Each follows the same pattern: a **component** attached to an entity, a **callback type** (Rust function pointer), and a **context SystemParam** providing full ECS access.

All callback types — timers, phases, collisions, menus, and scene callbacks — receive `&mut GameCtx` (`src/systems/game_ctx.rs`), which provides: commands, positions, rigid_bodies, signals, animations, shaders, groups, screen_positions, box_colliders, global_transforms, stuckto, rotations, scales, sprites, world_signals, audio, world_time, and texture_store. Callbacks have full ECS access.

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
use aberredengine::components::timer::{Timer, TimerCallback};

// Spawn an entity with a 2-second repeating timer
ctx.commands.spawn((
    MapPosition::new(0.0, 0.0),
    Timer::new(2.0, on_timer_fire as TimerCallback),
));

fn on_timer_fire(entity: Entity, ctx: &mut GameCtx, _input: &InputState) {
    // This fires every 2 seconds
    ctx.world_signals.set_string("timer_count", "fired!".to_string());
}
```

> **Warning:** The `as TimerCallback` cast is required. `Timer<C>` is generic — without the cast, Rust infers `C` from the specific function item type rather than the `TimerCallback` alias. The `update_timers` system queries `Query<(Entity, &mut Timer)>` (which expands to `Timer<TimerCallback>`), so it will **never find the entity** and the timer will silently never fire.

**One-shot pattern:** Timers always repeat. To make a one-shot timer, despawn the entity in the callback. Remember to use the `as TimerCallback` cast when spawning:

```rust
ctx.commands.spawn((
    MapPosition::new(0.0, 0.0),
    Timer::new(5.0, one_shot_callback as TimerCallback),
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

**Callback signatures:**

```rust
use aberredengine::systems::GameCtx;

// Called when entering a phase. Return Some("phase") to immediately chain-transition.
type PhaseEnterFn = fn(Entity, &mut GameCtx, &InputState) -> Option<String>;

// Called every frame in a phase. Return Some("phase") to transition.
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
use aberredengine::components::collision::{CollisionRule, CollisionCallback, BoxSide};
use aberredengine::components::persistent::Persistent;

ctx.commands.spawn((
    CollisionRule::new("ball", "brick", ball_brick_collision as CollisionCallback),
    Persistent, // survive scene switches
));
```

> **Warning:** The `as CollisionCallback` cast is required. `CollisionRule<C>` is generic — without the cast, Rust infers `C` from the specific function item type rather than the `CollisionCallback` alias. The `rust_collision_observer` queries `Query<&CollisionRule>` (which expands to `CollisionRule<CollisionCallback>`), so it will **never find the entity** and the callback will silently never fire.

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

---

## 8. Engine Resources Quick Reference

All resources are accessed as Bevy ECS system parameters. Use `Res<T>` / `ResMut<T>` for Send resources, `NonSend<T>` / `NonSendMut<T>` for main-thread-only resources. Scene callbacks access most of these through `GameCtx` fields.

### Engine-inserted resources (Send)

| Resource | Access | Purpose |
|----------|--------|---------|
| `WorldTime` | `Res` | `elapsed`, `delta`, `time_scale`, `frame_count` |
| `WorldSignals` | `ResMut` | Global cross-system communication (scalars, integers, strings, flags, entities) |
| `TrackedGroups` | `ResMut` | Group names to count — engine publishes counts to `WorldSignals` each frame |
| `ScreenSize` | `Res` | Internal render resolution (`w`, `h`) |
| `WindowSize` | `Res` | OS window dimensions (`w`, `h`), has `calculate_letterbox()` and `window_to_game_pos()` |
| `GameConfig` | `ResMut` | Loaded from `config.ini` — all render/window settings |
| `InputState` | `Res` | Input state — digital fields are `BoolState { active, just_pressed, just_released }`; analog fields (`scroll_y`, `mouse_x/y`, `mouse_world_x/y`) are `f32` |
| `InputBindings` | `ResMut` | Runtime key/mouse binding map (`InputAction` → `Vec<InputBinding>`). Modify to rebind actions at runtime. |
| `GameState` | `Res` | Current state: `None → Setup → Playing → Quitting` |
| `NextGameState` | `ResMut` | Request state transitions with `.set(GameStates::Playing)` |
| `PostProcessShader` | `ResMut` | Shader chain + uniforms (reserved: `uTime`, `uDeltaTime`, `uResolution`, `uFrame`, `uWindowResolution`, `uLetterbox`) |
| `SystemsStore` | `Res` | Named system registry for `commands.run_system()` |
| `SceneManager` | `Res` | Scene registry (only present with `.add_scene()`) |

### Engine-inserted resources (NonSend)

| Resource | Access | Purpose |
|----------|--------|---------|
| `RaylibHandle` / `RaylibThread` | via `RaylibAccess` SystemParam | Raylib context — use `(&mut *raylib.rl, &*raylib.th)` |
| `FontStore` | `NonSendMut` | Loaded fonts by key |
| `ShaderStore` | `NonSendMut` | Loaded shaders with cached uniform locations |
| `RenderTarget` | `NonSendMut` | Internal framebuffer (not typically accessed by game code) |

### Developer-inserted resources

| Resource | Access | Purpose |
|----------|--------|---------|
| `TextureStore` | `Res` / `ResMut` | Loaded textures by key |
| `AnimationStore` | `Res` | Animation definitions |
| `Camera2DRes` | `ResMut` | 2D camera (target, offset, zoom, rotation) |
| `TilemapStore` | `Res` | Loaded tilemap data — **not pre-inserted**; insert manually in setup via `commands.insert_resource(TilemapStore::new())` |
| `DebugMode` | marker resource | Presence enables debug overlays |
| `FullScreen` | marker resource | Presence enables fullscreen |

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

**Typed payloads** (Rust-only, `Arc<dyn Any + Send + Sync>`; not exposed to Lua):

| Method | Signature |
|--------|-----------|
| `set_payload` | `(&mut self, key: impl Into<String>, value: T) where T: Any + Send + Sync + 'static` |
| `get_payload` | `(&self, key: &str) -> Option<&T>` |
| `remove_payload` | `(&mut self, key: &str) -> bool` |
| `take_payload` | `(&mut self, key: &str) -> Option<Arc<T>>` |

Payloads carry rich Rust types that don't map to the scalar/integer/flag/entity primitives — for example, a completed pathfinding result (`Vec<Vector2>`), a multi-field dialogue state struct, or a boss-phase attack schedule. They are stored as type-erased `Arc<dyn Any>` and recovered via `downcast_ref`/`downcast`. `get_payload::<T>` returns `None` for both absent keys and type mismatches.

> **Important:** Payloads have no automatic cleanup on scene transitions (unlike `entities`, which are cleaned by `clear_non_persistent_entities`). The system or callback that writes a payload is responsible for removing it — typically via `take_payload` in the consuming system. Forgetting this leaks the payload into the next scene.

```rust
// Pathfinding system writes a result payload
fn pathfinding_system(mut signals: ResMut<WorldSignals>) {
    let path = vec![Vector2::new(100.0, 200.0), Vector2::new(300.0, 200.0)];
    signals.set_payload("nav:player_path", path);
}

// AI steering system consumes it
fn steering_system(mut signals: ResMut<WorldSignals>) {
    if let Some(path) = signals.take_payload::<Vec<Vector2>>("nav:player_path") {
        // Arc<Vec<Vector2>> — use path, it's now removed from signals
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

**`[window]` section:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `width` | `u32` | `1280` | Window width |
| `height` | `u32` | `720` | Window height |
| `target_fps` | `u32` | `120` | Target FPS |
| `vsync` | `bool` | `true` | Vertical sync |
| `fullscreen` | `bool` | `false` | Start fullscreen |
| `title` | `string` | `"Aberred Engine"` | Window title |

### Parsing behavior

- **Missing file** → safe defaults used, no error
- **Missing key** → default for that key
- **Invalid value** → silently ignored, default used
- **Booleans** — case-insensitive: `true`/`false`, `yes`/`no`, `on`/`off`
- **`background_color`** — comma-separated `R,G,B` integers (e.g., `80,80,80`)

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
