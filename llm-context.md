# ABERRED ENGINE - LLM CONTEXT DATA

# Machine-readable context for AI assistants working on this codebase

# Last updated: 2026-03-12 (Latest changes: Timer generic refactor, timer_core.rs shared loop, LuaTimerCallback newtype)

## QUICK REFERENCE

STACK: Rust (edition 2024) + Bevy ECS 0.18 + Raylib 5.5.1 + MLua 0.11 (LuaJIT, optional) + configparser 3 + fastrand 2.3 + clap (CLI) + serde/serde_json + log/env_logger + arrayvec + smallvec + rustc-hash + crossbeam-channel + imgui 0.12
GAME_TYPE: Multi-game 2D showcase engine with optional Lua scripting
ENTRY: src/main.rs
LIB_ENTRY: src/lib.rs (reusable library crate)
LUA_ENTRY: assets/scripts/main.lua
CONFIG: config.ini (INI format, loaded at startup)
WINDOW: Configurable via config.ini (default 1280x720 @ 120fps)

FEATURE FLAGS:
  lua = ["dep:mlua"]   -- Lua scripting support (LuaJIT via mlua). In default features.
  default = ["lua"]    -- lua enabled by default; downstream Rust-only games opt out with default-features = false

LUA API REFERENCE: assets/scripts/engine.lua (generated EmmyLua stubs — regenerate: `cargo run -- --create-lua-stubs`)
LUA API DOCS:      assets/scripts/README.md
COMMAND ENUMS:     src/resources/lua_runtime/commands.rs + spawn_data.rs (EntityCmd, SpawnCmd, CloneCmd, SignalCmd, etc.)

## STATUS (2026-03-11)

- Multi-game showcase: main.lua uses a scene registry + callback injection system to run multiple independent game examples from a shared menu.
- Registered scenes: menu, asteroids_level01, arkanoid_level01, birthday_intro, birthday_card, kraken_intro, sidescroller_level01, bunnymark_menu, bunnymark_map_loop, bunnymark_screen_loop, bunnymark_map_phase, bunnymark_screen_phase.
- Callback injection: each scene module exports `_callbacks` table with local functions; main.lua injects/removes them from `_G` on scene switch to prevent naming conflicts between scenes.
- Assets loaded per scene group in setup.lua: common (fonts, cursor, shaders), asteroids (ship, space tiles, asteroids, explosions, laser, sounds), arkanoid (bricks, vaus, ball, background, title, music, tilemaps), birthday (hearts, gems, photo, fonts, music), kraken (mouth, tentacle), bunnymark (raybunny texture).
- Shaders loaded: invert, wave, bloom, outline, crt (from crt2.fs), blink, fade.
- Parent-child entity hierarchy: ChildOf relationship + GlobalTransform2D component for recursive transform propagation (position, rotation, scale). `propagate_transforms` runs after movement/tweens, before collision. `cleanup_orphaned_global_transforms` runs after `propagate_transforms`, before `collision_detector` — removes stale `GlobalTransform2D` from entities that no longer have children (prevents `resolve_world_pos` from returning a frozen world position instead of live `MapPosition`). `ComputeInitialGlobalTransform` EntityCommand (in `propagate_transforms.rs`) is queued after giving an entity its `ChildOf` component to compute correct initial world-space transform on first frame, avoiding the one-frame world-origin flash.
- Menu scrolling: visible_count + scroll_offset with "..." indicator entities for overflow.
- ParticleEmitter, TTL, Entity cloning, EntityShader, Tint, Multi-pass post-processing, Runtime game configuration API, Lua stubs generator, Luarc generator, Library crate - all functional.
- Animation row-wrapping: `AnimationResource` uses `horizontal_displacement` + `vertical_displacement` (renamed from `displacement`). When `vertical_displacement > 0`, the animation system looks up the texture width from `TextureStore` and wraps frames to subsequent rows when `x + frame_width > texture_width`. First (possibly partial) row starts at `position.x`; subsequent rows start at x=0. Frame offset logic extracted into `compute_frame_offset()` pure function in `systems/animation.rs`.
- `SetAnimation` EntityCmd and `animation_controller` both sync `Sprite.tex_key` to match the animation's texture when the animation key changes. `animation_controller` queries `AnimationStore` and `Sprite` directly to apply the sync.
- Camera follow system: `CameraTarget { priority: u8 }` component marks entities as follow candidates. `CameraFollowConfig` resource (inserted disabled by default) controls follow behaviour: `FollowMode` (Instant/Lerp/SmoothDamp/Deadzone), `EasingCurve` (Linear/EaseOut/EaseIn/EaseInOut), lerp speed, spring params, offset, optional world-space bounds clamping. `camera_follow_system` runs after `propagate_transforms`, before `render_system`. Lua API: `engine.camera_follow_enable/set_mode/set_deadzone/set_easing/set_speed/set_spring/set_offset/set_bounds/clear_bounds/reset_velocity` + `engine.entity_set_camera_target/entity_remove_camera_target`. Spawn builder: `:with_camera_target(priority?)`.
- Collision: shared collision helpers in `systems/collision.rs` (`resolve_world_pos`, `resolve_collider_rect`, `compute_sides`, `resolve_groups`). `match_groups` free function in `components/collision.rs`. `CollisionCallback` uses `GameCtx` — collision callbacks have full GameCtx access.
- Sidescroller example: `scenes/sidescroller/level01.lua` registered as `sidescroller_level01`. Demonstrates sprite sheet animations with row-wrapping (char_red_1.png, char_red_2.png), `AnimationController` rules, and LuaPhase state machine with idle/running/walking/falling/attack phases. Uses `engine.entity_set_sprite_flip` for directional flip and `utils.has_flag()` helper from `lib/utils.lua`.
- imgui debug overlay: `imgui = "0.12"` dependency; raylib built with `imgui` feature. When F11 debug mode is active, an imgui window renders with checkboxes to toggle individual world-space overlays. `DebugOverlayConfig` resource (`resources/debugoverlayconfig.rs`) controls: `show_collider_boxes`, `show_position_crosshairs`, `show_entity_signals`, `show_text_bounds`, `show_sprite_bounds` (all default true). Inserted by `EngineBuilder` alongside `DebugMode`.
- `InputSnapshot` / `DigitalInputs`: exposes raw WASD (`main_up/down/left/right`), raw arrow keys (`secondary_up/down/left/right`), function keys (`debug` F11, `fullscreen` F10), and `action_3` in addition to combined directional fields (`up/down/left/right`) and action buttons. All fields exposed to Lua callbacks via `input.digital.*`.
- `lua` Cargo feature flag: all Lua-specific code gated behind `#[cfg(feature = "lua")]`. Feature is in `default = ["lua"]`. `cargo build --no-default-features` compiles a pure-Rust engine.
- `Timer<C>` generic component: shared timer storage used by both the Rust and Lua timer paths. Default `Timer<TimerCallback>` stores a Rust fn-pointer; `LuaTimer` (= `Timer<LuaTimerCallback>`) stores a Lua callback name string. Rust callback: `TimerCallback = fn(Entity, &mut GameCtx, &InputState)`. Event-based: `update_timers`/`update_lua_timers` → `TimerEvent`/`LuaTimerEvent` → `timer_observer`/`lua_timer_observer`. Shared update loop in `systems/timer_core.rs` (`TimerRunner<C>` trait + `run_timer_update`, pub(crate) only).
- `Phase<C>` generic component: shared state machine storage used by both the Rust and Lua phase paths. Default form `Phase<PhaseCallbackFns>` stores Rust fn-pointers; `LuaPhase` (= `Phase<PhaseCallbacks>`) stores Lua callback name strings. Rust callbacks: `PhaseEnterFn(Entity, &mut GameCtx, &InputState) -> Option<String>`, `PhaseUpdateFn(Entity, &mut GameCtx, &InputState, f32) -> Option<String>`, `PhaseExitFn(Entity, &mut GameCtx)`. Transitions via callback return value or external `phase.next` mutation. `phase_system` runs after collision_detector, always compiled. Shared phase loop logic lives in `systems/phase_core.rs` (`PhaseRunner` trait + `run_phase_callbacks` / `apply_callback_transitions` free functions, pub(crate) only).
- `EngineBuilder` pattern: `src/engine_app.rs` extracts all engine bootstrapping into a configurable builder with discrete methods: `validate_builder`, `load_config`, `setup_window`, `setup_world`, `register_systems`, `spawn_observers`, `build_schedule`, `main_loop`. `run()` orchestrates them in sequence. Developer supplies game hooks via `.on_setup()`, `.on_enter_play()`, `.on_update()`, `.on_switch_scene()`. Lua games use `.with_lua("path")`. Rust multi-scene games use `.add_scene(name, descriptor)` + `.initial_scene(name)`. `main.rs` is a thin CLI + builder call (~100 lines).
- Rust `CollisionRule` component: fn-pointer collision callback. Event-based dispatch: `collision_detector` → `CollisionEvent` → `rust_collision_observer` → callback. Callback signature: `fn(Entity, Entity, &BoxSides, &BoxSides, &mut GameCtx)`. Always compiled.
- Rust `MenuRustCallback`: fn-pointer menu selection callback. `Menu` has optional `on_rust_callback: Option<MenuRustCallback>` field. Callback signature: `fn(Entity, &str, usize, &mut GameCtx)`. Priority: Lua callback → Rust callback → `MenuActions`.
- `SceneManager` pattern: optional higher-level alternative to raw `.on_switch_scene()`. `SceneDescriptor` has fn-pointer fields: `on_enter: SceneEnterFn`, `on_update: Option<SceneUpdateFn>`, `on_exit: Option<SceneExitFn>`. Engine systems: `scene_switch_system` (despawn non-Persistent → on_exit → on_enter), `scene_update_system` (per-frame on_update with dt), `scene_enter_play` (seeds initial scene), `scene_switch_poll` (polls `"switch_scene"` flag in `WorldSignals` each frame and runs `scene_switch_system` when set). Always compiled.
- `GameCtx` SystemParam (`src/systems/game_ctx.rs`): Commands + 6 mutable queries + 8 read-only queries + world_signals + audio + world_time + texture_store. Re-exported from `systems::GameCtx`.
- `InputBindings` resource (`resources/input_bindings.rs`): maps `InputAction` → `Vec<InputBinding>` (`InputBinding::Keyboard(KeyboardKey)` or `InputBinding::MouseButton(MouseButton)`). `update_input_state` reads `Res<InputBindings>` via `poll_action!` macro + `any_binding_down/pressed/released` helpers. 16 InputAction variants (including `Action3`, `ToggleDebug` F11, `ToggleFullscreen` F10). Lua API: `engine.rebind_action(action, key)`, `engine.add_binding(action, key)`, `engine.get_binding(action) -> string?`. `action_from_str`/`action_to_str` in `runtime.rs`. `key_from_str`/`key_to_str`, `binding_from_str`/`binding_to_str` in `input_bindings.rs`.
- Mouse input: Default bindings: Action1 = Space + mouse_left, Action2 = Enter + mouse_right, Action3 = mouse_middle. Scroll wheel in `InputState.scroll_y`. Mouse position: `mouse_x`/`mouse_y` (game-space, letterbox-corrected), `mouse_world_x`/`mouse_world_y` (world-space). Computed in `update_input_state`.
- Bunnymark example scenes: 5 variants — `bunnymark_menu`, `bunnymark_map_loop`, `bunnymark_screen_loop`, `bunnymark_map_phase`, `bunnymark_screen_phase`. Shared helpers in `scenes/bunnymark/common.lua`.
- `EntitySnapshot` struct in `context.rs`: `build_entity_context_pooled` takes a single `EntitySnapshot` struct. `ContextQueries` SystemParam groups read-only queries (Group, Rotation, Scale, BoxCollider, LuaTimer, GlobalTransform2D, ChildOf).
- `SetScreenPosition` EntityCmd + `entity_set_screen_position` Lua API: sets entity's ScreenPosition (distinct from `SetPosition`/`entity_set_position` for MapPosition).
- `WorldSignals::clear_non_persistent_entities(persistent_entities: &FxHashSet<Entity>)`: removes entity registrations whose entity is not in the persistent set. Called during scene transitions (both Lua and SceneManager paths) to mirror entity despawn logic.
- Lua runtime (`src/resources/lua_runtime/`): `runtime.rs` houses core types (`LuaRuntime`, `LuaAppData`, `GameConfigSnapshot`, pool types). API registration methods live in `engine_api.rs` (`register_base_api`, `register_asset_api`, `register_spawn_api`, `register_audio_api`, `register_signal_api`, `register_phase_api`, `register_entity_api`, `register_group_api`, `register_tilemap_api`, `register_camera_api`, `register_camera_follow_api`, `register_collision_api`, `register_animation_api`, `register_render_api`, `register_gameconfig_api`, `register_input_api`) plus free functions `push_fn_meta`, `register_cmd`, `register_entity_cmds`, `define_entity_cmds`. Command queue drain methods live in `command_queues.rs`. Stub/meta type definitions (`BuilderMethodParam`, `BuilderMethodDef`, `TypeFieldDef`, `LuaTypeDef`) and `push_type_field` live in `stub_meta.rs`.

## FILE TREE (ESSENTIAL)

```
src/
├── main.rs                    # App entry, CLI (clap), delegates to EngineBuilder
├── lib.rs                     # Library crate entry (components, engine_app, events, lua_plugin, resources, systems, stub_generator, luarc_generator)
├── engine_app.rs              # EngineBuilder: builder pattern for engine bootstrapping (world, window, resources, schedule, main loop)
├── lua_plugin.rs              # [feature=lua] Lua plugin: GameState logic, scene switching, Lua callbacks + ScriptingContext/GameSceneState/EntityProcessing SystemParams
├── stub_generator.rs          # [feature=lua] Lua EmmyLua stub generator (reads __meta, emits engine.lua)
├── luarc_generator.rs         # [feature=lua] .luarc.json generator (Lua Language Server config from __meta)
├── components/
│   ├── mod.rs                 # Re-exports all components
│   ├── mapposition.rs         # MapPosition (world-space position)
│   ├── screenposition.rs      # ScreenPosition (UI/screen-space position)
│   ├── rigidbody.rs           # Velocity, friction, max_speed, named accel forces, frozen
│   ├── boxcollider.rs         # BoxCollider (AABB collision shape)
│   ├── cameratarget.rs        # CameraTarget { priority: u8 } — marks entity as camera follow candidate
│   ├── collision.rs           # CollisionRule + CollisionCallback (Rust fn-pointer), BoxSide, get_colliding_sides, match_groups
│   ├── luacollision.rs        # [feature=lua] LuaCollisionRule for Lua callbacks
│   ├── sprite.rs              # Sprite rendering (tex_key, offset, origin, flip)
│   ├── animation.rs           # Animation playback state + AnimationController
│   ├── luaphase.rs            # [feature=lua] LuaPhase type alias = Phase<PhaseCallbacks> (Lua callback names)
│   ├── phase.rs               # Generic Phase<C> component; default Phase = Phase<PhaseCallbackFns> (Rust fn-ptrs)
│   ├── signals.rs             # Per-entity signals (scalars/ints/flags/strings)
│   ├── dynamictext.rs         # Text rendering component with cached size
│   ├── entityshader.rs        # Per-entity shader for custom rendering effects
│   ├── signalbinding.rs       # Bind text to world signals
│   ├── tween.rs               # TweenPosition, TweenRotation, TweenScale
│   ├── luatimer.rs            # [feature=lua] LuaTimer type alias = Timer<LuaTimerCallback>; LuaTimerCallback { name: String }
│   ├── timer.rs               # Generic Timer<C = TimerCallback> component; TimerCallback fn-pointer type alias
│   ├── stuckto.rs             # Attach entity to another
│   ├── tint.rs                # Color tint for rendering (sprites/text)
│   ├── menu.rs                # Interactive menu (with scroll support); MenuRustCallback type alias
│   ├── gridlayout.rs          # JSON grid spawning
│   ├── group.rs               # Entity grouping tag
│   ├── persistent.rs          # Survive scene transitions
│   ├── particleemitter.rs     # Particle emitter (templates, shape, arc, speed, TTL)
│   ├── globaltransform2d.rs   # World-space transform for hierarchy children (ChildOf)
│   ├── rotation.rs            # Rotation in degrees
│   ├── ttl.rs                 # Time-to-live for automatic entity despawn
│   ├── scale.rs               # 2D scale
│   ├── zindex.rs              # Render order
│   └── inputcontrolled.rs     # InputControlled, AccelerationControlled, MouseControlled
├── systems/
│   ├── mod.rs                 # Re-exports all systems + RaylibAccess SystemParam + GameCtx (pub use game_ctx::GameCtx)
│   ├── game_ctx.rs            # GameCtx SystemParam (unified ECS access for all Rust callbacks)
│   ├── movement.rs            # Physics: accel→vel→pos, friction, max_speed
│   ├── camera_follow.rs       # camera_follow_system: tracks CameraTarget entities via CameraFollowConfig
│   ├── collision.rs           # Shared collision helpers: resolve_world_pos, resolve_collider_rect, compute_sides, resolve_groups
│   ├── collision_detector.rs  # AABB detection (pure Rust, shared by Lua and Rust game paths)
│   ├── lua_collision.rs       # [feature=lua] Lua collision observer + callback dispatch (LuaCollisionObserverParams, lua_collision_observer)
│   ├── rust_collision.rs      # Rust collision observer (rust_collision_observer, always compiled)
│   ├── scene_dispatch.rs      # SceneManager scene dispatch: SceneDescriptor, scene_switch_system, scene_update_system, scene_enter_play, scene_switch_poll
│   ├── render.rs              # Raylib drawing, camera, debug overlays (imgui), letterboxing + RenderResources/RenderQueries SystemParams
│   ├── input.rs               # Poll keyboard via InputBindings, poll_action! macro, emit InputEvent
│   ├── inputsimplecontroller.rs    # Input→velocity
│   ├── inputaccelerationcontroller.rs # Input→acceleration
│   ├── mousecontroller.rs     # Mouse position tracking (with letterbox correction)
│   ├── animation.rs           # Frame advancement + rule evaluation (AnimationController) + compute_frame_offset (row-wrapping)
│   ├── luaphase.rs            # [feature=lua] Lua phase callbacks (LuaPhaseRunner impl + lua_phase_system)
│   ├── lua_commands/          # [feature=lua] Process EntityCmd/CollisionEntityCmd/SpawnCmd/InputCmd
│   │   ├── mod.rs             # Re-exports + EntityCmdQueries/ContextQueries SystemParam bundles + shared imports
│   │   ├── entity_cmd.rs      # process_entity_commands: runtime entity manipulation
│   │   ├── spawn_cmd.rs       # process_spawn_command, process_clone_command: entity creation
│   │   └── parse.rs           # parse_cmp_op, convert_animation_condition: animation condition helpers
│   ├── luatimer.rs            # [feature=lua] Lua timer: LuaTimerRunner + update_lua_timers + lua_timer_observer
│   ├── timer.rs               # Rust timer: RustTimerRunner + update_timers + timer_observer
│   ├── timer_core.rs          # Shared timer loop: TimerRunner<C> trait + run_timer_update (pub(crate))
│   ├── phase.rs               # Rust phase state machine (RustPhaseRunner impl + phase_system)
│   ├── phase_core.rs          # Shared phase loop: PhaseRunner<C> trait, run_phase_callbacks, apply_callback_transitions (pub(crate))
│   ├── time.rs                # WorldTime update
│   ├── signalbinding.rs       # Update bound text
│   ├── dynamictext_size.rs    # Cache DynamicText bounding box sizes
│   ├── tween.rs               # Tween animation systems (position/rotation/scale)
│   ├── stuckto.rs             # StuckTo entity following (skips entities with ChildOf)
│   ├── propagate_transforms.rs # Recursive GlobalTransform2D computation from ChildOf hierarchy; cleanup_orphaned_global_transforms; ComputeInitialGlobalTransform EntityCommand
│   ├── gridlayout.rs          # Grid entity spawning
│   ├── group.rs               # Group counting
│   ├── menu.rs                # Menu spawn/input (menu_selection_observer has dual #[cfg] implementations: with/without LuaRuntime param; shared dispatch_menu_action() helper)
│   ├── particleemitter.rs     # Particle emission system (clones templates)
│   ├── ttl.rs                 # TTL countdown and entity despawn
│   ├── audio.rs               # Audio thread bridge
│   ├── gameconfig.rs          # Apply GameConfig changes (render size, window, vsync, fps, background_color)
│   └── gamestate.rs           # State transition check + quit_game + clean_all_entities
├── resources/
│   ├── mod.rs                 # Re-exports
│   ├── worldtime.rs           # Delta time, time scale
│   ├── input.rs               # InputState + BoolState (no key_binding; both derive Default)
│   ├── input_bindings.rs      # InputBindings resource (InputAction→Vec<InputBinding>), InputBinding enum, key_from_str/key_to_str
│   ├── fullscreen.rs          # FullScreen marker resource
│   ├── gameconfig.rs          # GameConfig (render/window size, fps, vsync, fullscreen, background_color, window_title)
│   ├── texturestore.rs        # FxHashMap<String, Texture2D> + load_texture_from_text() utility
│   ├── fontstore.rs           # FxHashMap<String, Font> (non-send)
│   ├── animationstore.rs      # Animation definitions
│   ├── tilemapstore.rs        # Tilemap layouts
│   ├── gamestate.rs           # GameState enum + NextGameState
│   ├── worldsignals.rs        # Global signal storage + SignalSnapshot
│   ├── group.rs               # TrackedGroups set
│   ├── camera2d.rs            # Camera2D config
│   ├── camerafollowconfig.rs  # CameraFollowConfig resource + FollowMode + EasingCurve
│   ├── screensize.rs          # Game's internal render resolution
│   ├── windowsize.rs          # Actual window dimensions (for letterboxing)
│   ├── rendertarget.rs        # RenderTarget for fixed-resolution rendering
│   ├── debugmode.rs           # Debug render toggle
│   ├── debugoverlayconfig.rs  # DebugOverlayConfig: per-overlay imgui toggles (colliders, positions, signals, text bounds, sprite bounds)
│   ├── systemsstore.rs        # Named system lookup
│   ├── scenemanager.rs        # SceneManager: named scene registry for Rust games (SceneDescriptor, active_scene, initial_scene)
│   ├── audio.rs               # AudioBridge channels
│   ├── shaderstore.rs         # ShaderStore (loaded shaders with cached uniform locations)
│   ├── postprocessshader.rs   # PostProcessShader resource (active shader chain + user uniforms)
│   ├── uniformvalue.rs        # UniformValue enum (Float/Vec2/Vec3/Vec4/Int/Bool) used by shaders
│   └── lua_runtime/           # [feature=lua]
│       ├── mod.rs             # Public exports
│       ├── runtime.rs         # LuaRuntime, LuaAppData, GameConfigSnapshot, pool types (CollisionCtxPool, EntityCtxPool, etc.), action_to_str/action_from_str
│       ├── engine_api.rs      # All register_*_api methods + free functions: push_fn_meta, register_cmd, register_entity_cmds, define_entity_cmds
│       ├── command_queues.rs  # All drain_*_commands methods, clear_all_commands, update_signal_cache, update_bindings_cache, etc.
│       ├── stub_meta.rs       # Stub metadata types (BuilderMethodParam, BuilderMethodDef, TypeFieldDef, LuaTypeDef), push_type_field, stub/meta registration methods
│       ├── commands.rs        # EntityCmd, SpawnCmd, SignalCmd, InputCmd, etc.
│       ├── context.rs         # Entity context builder for Lua callbacks (phase/timer); EntitySnapshot struct
│       ├── input_snapshot.rs  # InputSnapshot for Lua callbacks
│       ├── entity_builder.rs  # LuaEntityBuilder fluent API (unified spawn/clone, regular/collision)
│       └── spawn_data.rs      # SpawnComponentData structures
└── events/
    ├── mod.rs                 # Re-exports
    ├── collision.rs           # CollisionEvent
    ├── gamestate.rs           # GameStateTransition
    ├── input.rs               # InputEvent + InputAction enum (16 variants: directions, actions incl. Action3, special, ToggleDebug, ToggleFullscreen)
    ├── menu.rs                # MenuSelection
    ├── luatimer.rs            # [feature=lua] LuaTimerEvent
    ├── timer.rs               # TimerEvent (Rust fn-pointer timer)
    ├── switchdebug.rs         # DebugToggle (F11)
    ├── switchfullscreen.rs    # FullScreen toggle event + observer (F10)
    └── audio.rs               # AudioCmd, AudioMessage
tests/
├── bevy_ecs_integration.rs    # ECS integration tests (pure Rust)
├── camera_follow_integration.rs # Camera follow system integration tests (pure Rust)
├── engine_tick_integration.rs # Engine tick + meta drift protection tests (mixed; Lua tests gated by #[cfg(feature = "lua")]; includes Rust Timer integration tests)
├── stub_generator_integration.rs # Stub generator tests (#![cfg(feature = "lua")])
├── hierarchy_integration.rs   # Parent-child hierarchy tests (mixed; Lua tests gated by #[cfg(feature = "lua")])
├── menu_callback_integration.rs  # Rust menu callback integration tests (pure Rust)
├── scene_manager_integration.rs  # SceneManager integration tests (pure Rust)
└── input_bindings_integration.rs # InputBindings + rebinding pipeline integration tests (17 tests)
assets/
├── scripts/
│   ├── main.lua               # Entry: scene registry, callback injection, on_setup, on_enter_play, on_switch_scene
│   ├── setup.lua              # Asset loading (common + per-scene-group sections)
│   ├── engine.lua             # Auto-generated LSP stubs (EmmyLua annotations, via --create-lua-stubs)
│   ├── .luarc.json            # Auto-generated Lua Language Server config (via --create-luarc)
│   ├── README.md              # Lua API documentation
│   ├── lib/
│   │   ├── math.lua           # Math helpers (lerp, inv_lerp, remap, lerp2)
│   │   └── utils.lua          # Debug utilities (dump_value)
│   └── scenes/
│       ├── menu.lua           # Shared menu scene
│       ├── asteroids/level01.lua
│       ├── arkanoid/level01.lua
│       ├── birthday/intro.lua + card.lua
│       ├── kraken/intro.lua
│       ├── sidescroller/level01.lua  # Sidescroller demo (sprite sheet animations, camera follow)
│       └── bunnymark/               # Bunnymark benchmark (5 variants)
│           ├── common.lua           # Shared helpers (spawn, bounce, constants)
│           ├── menu.lua             # Sub-menu for variant selection
│           ├── map_loop.lua         # MapPosition + on_update loop
│           ├── screen_loop.lua      # ScreenPosition + on_update loop
│           ├── map_phase.lua        # MapPosition + Phase state machine
│           └── screen_phase.lua     # ScreenPosition + Phase state machine
├── textures/                  # Organized by game: asteroids/, arkanoid/, birthday/, kraken/, bunnymark/ + shared (cursor.png, black.png)
├── shaders/                   # OpenGL 3.3 fragment shaders: invert.fs, wave.fs, bloom.fs, outline.fs, crt.fs, crt2.fs, blink.fs, fade.fs
├── audio/                     # Organized by game: asteroids/, arkanoid/, birthday/ + shared (option.wav)
├── fonts/                     # Arcade_Cabinet.ttf, Formal_Future.ttf, birthday/Endless_Love.ttf
├── tilemaps/arkanoid/         # Tilemap data (level01.txt + level01.png)
└── levels/arkanoid/           # Level JSON data (level01.json)

config.ini                     # Game configuration (INI format)
docs/
└── lua-interface-architecture.md
```

## CONFIG.INI FORMAT

```ini
[render]
width = 640                    ; Internal render width
height = 360                   ; Internal render height
background_color = 0,2,4       ; Background clear color (R,G,B 0-255)

[window]
target_fps = 120               ; Target frames per second
vsync = true
fullscreen = false
title = My Game
```

`GameConfig::load_from_file()` runs before world setup. `apply_gameconfig_changes` (run_if state_is_playing) reacts to `is_added()`/`is_changed()` to resize render target, sync fullscreen, and apply vsync/fps.

## COMPONENT QUICK-REF

```
MapPosition { pos: Vector2 }
ScreenPosition { pos: Vector2 }
RigidBody { velocity: Vector2, friction: Option<f32>, max_speed: Option<f32>, forces: FxHashMap<String, AccelerationForce>, frozen: bool }
AccelerationForce { value: Vector2, enabled: bool }
BoxCollider { offset: Vector2, origin: Vector2, size: Vector2 }
CameraTarget { priority: u8 }                         -- marks entity as camera follow candidate; highest priority wins
LuaCollisionRule { group_a, group_b, callback: String } -- [feature=lua]
CollisionRule { group_a, group_b, callback: CollisionCallback } -- fn(Entity, Entity, &BoxSides, &BoxSides, &mut GameCtx)
Sprite { tex_key: Arc<str>, width: f32, height: f32, offset: Vector2, origin: Vector2, flip_h: bool, flip_v: bool }
Animation { animation_key: String, frame_index: usize, elapsed: f32 }
AnimationController { fallback_key: String, rules: Vec<AnimationRule> }
Signals { scalars: FxHashMap, integers: FxHashMap, flags: FxHashSet, strings: FxHashMap }
Phase<C = PhaseCallbackFns> { current: String, previous: Option<String>, next: Option<String>, time_in_phase: f32, needs_enter_callback: bool, phases: FxHashMap<String, C> }
  PhaseCallbackFns { on_enter: Option<PhaseEnterFn>, on_update: Option<PhaseUpdateFn>, on_exit: Option<PhaseExitFn> }
LuaPhase = Phase<PhaseCallbacks>   -- type alias; PhaseCallbacks { on_enter: Option<String>, on_update: Option<String>, on_exit: Option<String> }
DynamicText { text: Arc<str>, font: Arc<str>, font_size: f32, color: Color, size: Vector2 }
SignalBinding { key: String, format: Option<String>, binding_type: BindingType }
TweenPosition/TweenRotation/TweenScale { from, to, duration, elapsed, easing, loop_mode }
Timer<C = TimerCallback> { duration: f32, elapsed: f32, callback: C }
  TimerCallback = fn(Entity, &mut GameCtx, &InputState)
LuaTimer = Timer<LuaTimerCallback>   -- type alias; LuaTimerCallback { name: String }
Ttl { remaining: f32 }
StuckTo { target: Entity, follow_x: bool, follow_y: bool, offset: Vector2, stored_velocity: Vector2 }
Menu { items: Vec<MenuItem>, selected_index, font, font_size, item_spacing, normal_color, selected_color, cursor_entity, selection_change_sound, origin, use_screen_space, on_select_callback, on_rust_callback: Option<MenuRustCallback>, visible_count: Option<usize>, scroll_offset, top_indicator_entity, bottom_indicator_entity }
GridLayout { path: String, group: String, z_index: f32 }
Group { name: String }
Persistent (marker)
Rotation { angle: f32 }
Scale { x: f32, y: f32 }
ZIndex(f32)
InputControlled { up_velocity, down_velocity, left_velocity, right_velocity: Vector2 }
AccelerationControlled { up_acceleration, down_acceleration, left_acceleration, right_acceleration: Vector2 }
MouseControlled { follow_x: bool, follow_y: bool }
ParticleEmitter { templates: Vec<Entity>, shape: EmitterShape, offset, particles_per_emission, emissions_per_second, emissions_remaining, arc_degrees, speed_range, ttl: TtlSpec, time_since_emit }
EntityShader { shader_key: Arc<str>, uniforms: FxHashMap<Arc<str>, UniformValue> }
Tint { color: Color }                    -- sprites: replaces Color::WHITE; text: multiplies with text.color
GlobalTransform2D { position: Vector2, rotation_degrees: f32, scale: Vector2 }  -- world-space, computed by propagate_transforms
```

## RESOURCE QUICK-REF

```
GameConfig { render_width, render_height, window_width, window_height, target_fps, vsync, fullscreen, background_color: Color, window_title: String, config_path }
WorldTime { elapsed: f32, delta: f32, time_scale: f32, frame_count: u64 }
InputState { maindirection_up/down/left/right, secondarydirection_up/down/left/right, action_back, action_1, action_2, action_3, mode_debug, fullscreen_toggle, action_special: BoolState; scroll_y, mouse_x, mouse_y, mouse_world_x, mouse_world_y: f32 }  -- derives Default
BoolState { active: bool, just_pressed: bool, just_released: bool }  -- derives Default; key_binding removed (now in InputBindings)
InputBindings { map: HashMap<InputAction, Vec<InputBinding>> }  -- runtime-configurable key bindings; Default has all 15 actions mapped
InputBinding::Keyboard(KeyboardKey) | InputBinding::MouseButton(MouseButton)  -- extensible enum
InputSnapshot { digital: DigitalInputs, analog: AnalogInputs }  -- frozen snapshot for Lua callbacks
DigitalInputs { up, down, left, right, action_1, action_2, action_3, back, special: DigitalButtonState (combined/action); main_up/down/left/right (raw WASD); secondary_up/down/left/right (raw arrows); debug (F11), fullscreen (F10): DigitalButtonState }
AnalogInputs { scroll_y: f32, mouse_x: f32, mouse_y: f32, mouse_world_x: f32, mouse_world_y: f32 }  -- scroll_y: mouse wheel delta (positive=up); mouse_x/y: game-space cursor (letterbox-corrected); mouse_world_x/y: world-space cursor (after camera)
DigitalButtonState { pressed: bool, just_pressed: bool, just_released: bool }
TextureStore(FxHashMap<String, Texture2D>)
FontStore(FxHashMap<String, Font>)              -- NON_SEND
AnimationStore { animations: FxHashMap<String, AnimationResource> }
TilemapStore(FxHashMap<String, TilemapData>)
GameState { None, Setup, Playing, Quitting }
NextGameState(Option<GameState>)
WorldSignals { scalars, integers, strings, flags, entities }  -- group_counts derived in SignalSnapshot from "group_count:" prefixed integers
SignalSnapshot { scalars, integers, strings, flags, entities, group_counts }  -- read-only Arc cache for Lua
TrackedGroups(FxHashSet<String>)
Camera2D { target, offset, rotation, zoom }
CameraFollowConfig { enabled: bool, mode: FollowMode, easing: EasingCurve, lerp_speed: f32, spring_stiffness, spring_damping, offset: Vector2, bounds: Option<Rectangle>, velocity: Vector2 }  -- inserted disabled by default
FollowMode { Instant, Lerp, SmoothDamp, Deadzone { half_w, half_h } }
EasingCurve { Linear, EaseOut (default), EaseIn, EaseInOut }
ScreenSize { w: i32, h: i32 }           -- game's internal render resolution
WindowSize { w: i32, h: i32 }           -- actual window dimensions; has window_to_game_pos() for letterbox-corrected mouse coords
RenderTarget { texture: RenderTexture2D, game_width, game_height, filter: RenderFilter }  -- NON_SEND
DebugMode (marker)
DebugOverlayConfig { show_collider_boxes, show_position_crosshairs, show_entity_signals, show_text_bounds, show_sprite_bounds: bool }  -- all default true; toggled via imgui checkboxes in debug mode
FullScreen (marker)
SystemsStore { map: FxHashMap<String, SystemId>, entity_map: FxHashMap<String, SystemId<In<Entity>>> }
SceneManager { scenes: FxHashMap<String, SceneDescriptor>, active_scene: Option<String>, initial_scene: Option<String> }  -- scene_switch_poll polls "switch_scene" flag in WorldSignals to trigger scene_switch_system
LuaRuntime { lua: Lua, collision_ctx_pool, entity_ctx_pool }  -- NON_SEND [feature=lua]
AudioBridge { sender, receiver }
ShaderStore { shaders: FxHashMap<String, ShaderEntry> }  -- NON_SEND
PostProcessShader { keys: Vec<Arc<str>>, uniforms: FxHashMap<Arc<str>, UniformValue> }
UniformValue { Float(f32), Vec2([f32;2]), Vec3([f32;3]), Vec4([f32;4]), Int(i32), Bool(bool) }  -- resources/uniformvalue.rs
```

## SYSTEMPARAM BUNDLES

```
RaylibAccess<'w> { rl: NonSendMut<RaylibHandle>, th: NonSend<RaylibThread> }                                          -- systems/mod.rs
ScriptingContext<'w> { lua_runtime: NonSend<LuaRuntime>, audio_cmd_writer: MessageWriter<AudioCmd> }                  -- lua_plugin.rs [feature=lua]
GameSceneState<'w> { world_signals: ResMut<WorldSignals>, post_process: ResMut<PostProcessShader>, config: ResMut<GameConfig>, systems_store: Res<SystemsStore> } -- lua_plugin.rs [feature=lua]
EntityProcessing<'w, 's> { cmd_queries: EntityCmdQueries, luaphase: Query<(Entity, &mut LuaPhase)> }                  -- lua_plugin.rs [feature=lua]
EntityCmdQueries<'w, 's> { stuckto, signals, animation, rigid_bodies, positions, screen_positions, sprites, shaders, global_transforms } -- systems/lua_commands/mod.rs [feature=lua]
ContextQueries<'w, 's> { groups, rotations, scales, box_colliders, lua_timers, global_transforms, child_of }         -- systems/lua_commands/mod.rs [feature=lua] (read-only queries for entity context building)
RenderResources<'w> { camera, screensize, window_size, textures, world_time, post_process, config, maybe_debug, fonts } -- systems/render.rs
RenderQueries<'w, 's> { map_sprites, colliders, positions, map_texts, rigidbodies, screen_texts, screen_sprites }     -- systems/render.rs
GameCtx<'w, 's> { commands, positions, rigid_bodies, signals, animations, shaders, groups, screen_positions, box_colliders, global_transforms, stuckto, rotations, scales, sprites, world_signals, audio, world_time, texture_store } -- systems/game_ctx.rs
```

## LUA CALLBACK SIGNATURES

Input table structure (passed as argument to all Lua callbacks — not polled via engine functions):

```lua
input.digital.up/down/left/right          -- { pressed, just_pressed, just_released } — combined WASD+arrows
input.digital.action_1/action_2/action_3/back/special
input.digital.main_up/down/left/right     -- raw WASD only
input.digital.secondary_up/down/left/right -- raw arrow keys only
input.digital.debug                       -- F11
input.digital.fullscreen                  -- F10
input.analog.scroll_y                     -- mouse wheel delta this frame (f32, positive=up, negative=down)
input.analog.mouse_x / mouse_y           -- cursor in game-space (0..render_width/height, letterbox-corrected)
input.analog.mouse_world_x / mouse_world_y -- cursor in world-space (after camera transform, matches MapPosition)
```

Callback signatures:

```lua
on_setup()                                -- asset loading only (engine.load_* calls)
on_enter_play()                           -- first frame; spawn initial entities
on_switch_scene(scene_name)
on_update_<scenename>(input, dt)
-- Phase (LuaPhase):
phase_on_enter(ctx, input)  -> phase_name?   -- return string to transition immediately
phase_on_update(ctx, input, dt) -> phase_name?
phase_on_exit(ctx)
-- Timer (LuaTimer):
timer_callback(ctx, input)
-- Collision (LuaCollisionRule):
collision_callback(ctx)                   -- ctx.a, ctx.b, ctx.sides
-- Menu:
menu_callback(entity_id, item_id, item_index)
```

## ENTITY CONTEXT (Lua phase/timer callback ctx table)

```
ctx.id             -- u64 entity ID (always present)
ctx.group          -- string group name or nil
ctx.pos            -- { x, y } or nil (MapPosition)
ctx.screen_pos     -- { x, y } or nil (ScreenPosition)
ctx.vel            -- { x, y } or nil (RigidBody velocity)
ctx.speed_sq       -- f32 squared speed or nil
ctx.frozen         -- bool or nil (RigidBody frozen state)
ctx.rotation       -- f32 degrees or nil (Rotation)
ctx.scale          -- { x, y } or nil (Scale)
ctx.rect           -- { x, y, w, h } or nil (BoxCollider AABB)
ctx.sprite         -- { tex_key, flip_h, flip_v } or nil
ctx.animation      -- { key, frame_index, elapsed } or nil
ctx.signals        -- { flags=[...], integers={...}, scalars={...}, strings={...} } or nil
ctx.phase          -- string current phase name or nil (LuaPhase)
ctx.time_in_phase  -- f32 or nil
ctx.previous_phase -- string or nil (only in on_enter callbacks)
ctx.timer          -- { duration, elapsed, callback } or nil (LuaTimer)
ctx.world_pos      -- { x, y } or nil (GlobalTransform2D world position, hierarchy entities only)
ctx.world_rotation -- f32 degrees or nil
ctx.world_scale    -- { x, y } or nil
ctx.parent_id      -- u64 parent entity ID or nil (ChildOf relationship)
```

IMPORTANT: ctx is POOLED and REUSED between callbacks. Do NOT store references to ctx or its subtables. (Implementation: EntityCtxPool/EntityCtxTables in runtime.rs, build_entity_context_pooled in context.rs)

## COLLISION CONTEXT (Lua collision callback ctx table)

```
ctx.a.id / ctx.b.id        -- u64 entity ID
ctx.a.group / ctx.b.group  -- string group name
ctx.a.pos / ctx.b.pos      -- { x, y } or nil
ctx.a.vel / ctx.b.vel      -- { x, y } or nil
ctx.a.speed_sq             -- f32 squared speed
ctx.a.rect / ctx.b.rect    -- { x, y, w, h } or nil
ctx.a.signals / ctx.b.signals -- { flags, integers, scalars, strings } or nil
ctx.sides.a                -- array of strings: {"left", "top", ...} (collision sides for entity A)
ctx.sides.b                -- array of strings: {"right", ...}
```

IMPORTANT: Collision ctx is POOLED and REUSED. Do NOT store references to it. (Implementation: CollisionCtxPool/CollisionCtxTables in runtime.rs)

## SYSTEM EXECUTION ORDER

Observers (registered as persistent entities in engine_app.rs):

```
lua_collision_observer  (on CollisionEvent)    [feature=lua, with_lua() only]
rust_collision_observer (on CollisionEvent)    -- always compiled
switch_debug_observer, switch_fullscreen_observer
menu_controller_observer (on InputEvent)
menu_selection_observer  (on MenuSelectionEvent)
lua_timer_observer (on LuaTimerEvent)          [feature=lua, with_lua() only]
timer_observer     (on TimerEvent)
```

Ordering constraints (see engine_app.rs for authoritative schedule):

```
particle_emitter_system.before(movement)
propagate_transforms.after(movement, tween_*).before(collision_detector)
cleanup_orphaned_global_transforms.after(propagate_transforms).before(collision_detector)
camera_follow_system.after(propagate_transforms).before(render_system)
collision_detector.after(mouse_controller, movement)
stuck_to_entity_system.after(collision_detector)          -- skips entities with ChildOf
phase_system.after(collision_detector)                    -- always compiled
lua_phase_system.after(collision_detector)                [feature=lua]
animation_controller.after(phase_system, lua_phase_system)
animation.after(animation_controller)
ttl_system.after(movement)
dynamictext_size_system.after(update_world_signals_binding_system)
update_lua_timers                                         [feature=lua, with_lua() only]
lua_plugin::update.after(check_pending_state, lua_phase_system)  [with_lua() only]
scene_update_system.run_if(state_is_playing).after(check_pending_state)   [use_scene_manager only]
scene_switch_poll.run_if(state_is_playing).after(scene_update_system)     [use_scene_manager only]
render_system.after(collision_detector)
```

## KEY PATTERNS

### Adding a new EntityCmd

1. Add variant to `EntityCmd` enum (`commands.rs`)
2. Call `define_entity_cmds` function in `engine_api.rs` (one entry; auto-registers for both regular and `collision_` prefixed queues)
3. Process in `process_entity_commands` (`lua_commands.rs`)
4. If new types/enums/callbacks introduced, update stub/meta registration methods in `stub_meta.rs`
5. Regenerate stubs: `cargo run -- --create-lua-stubs` and `cargo run -- --create-luarc`
6. Document in `assets/scripts/README.md`

### Registering Entity-Input Systems (SystemsStore)

For systems that need to be called with a specific entity (like menu_despawn):

1. Define system with `In(target): In<Entity>` parameter
2. Register with `world.register_system(system_fn)` -> `SystemId<In<Entity>>`
3. Store in `systems_store.insert_entity_system("name", system_id)`
4. Call via `commands.run_system_with(*system_id, entity)` in EntityCmd handler

### Adding a new Component

1. Create file in `src/components/`
2. Derive `Component`, add fields
3. Export from `components/mod.rs`
4. Add to `SpawnComponentData` if spawnable (`spawn_data.rs`)
5. Add builder method in `entity_builder.rs` + entry in `register_builder_meta()` in `runtime.rs`
6. Process in `apply_components` (`lua_commands.rs`) — shared by both spawn and clone
7. If builder accepts complex table arg, add type in `register_types_meta()` and schema ref in `register_builder_meta()`

### Adding a new System

1. Create file in `src/systems/`
2. Define system function with queries
3. Export from `systems/mod.rs`
4. Add to schedule in `engine_app.rs` at the correct position

### Adding a new Resource

1. Create file in `src/resources/`
2. Derive `Resource` (or use non-send pattern)
3. Export from `resources/mod.rs`
4. Insert into world in `engine_app.rs`

### Adding a new Scene (Lua)

1. Create scene module in `assets/scripts/scenes/<name>/<scene>.lua`
2. Module returns table with `spawn()` function and `_callbacks` table
3. All engine-callable functions (phase/collision/timer/update callbacks) must be in `_callbacks`
4. Add entry to `scene_registry` in `main.lua`: `scene_name = "scenes.<name>.<scene>"`
5. Load scene assets in a new section in `setup.lua`

### Lua-Rust Command Flow

```
Lua calls engine.*
  -> LuaAppData.commands.borrow_mut().push(Cmd)
  -> Lua returns
  -> lua_plugin.rs processes queued commands
  -> Commands modify ECS world
```

### Collision Flow

```
movement_system
  -> collision_detector: detects AABB overlaps, emits CollisionEvent
  -> rust_collision_observer: matches CollisionRule by group names
       -> get_colliding_sides() -> calls Rust callback(ent_a, ent_b, &sides_a, &sides_b, &mut GameCtx)
  -> lua_collision_observer [feature=lua]: matches LuaCollisionRule by group names
       -> pooled ctx tables -> Lua callback
       -> engine.entity_* commands work in all contexts
       -> engine.collision_* commands use separate queues, drain immediately after callback
```

### Entity Commands Architecture

- `engine.entity_*` commands are **unified across all contexts** (phase, timer, collision, update)
- `define_entity_cmds!` macro registers every entity command to both regular (`""`) and collision (`"collision_"`) prefixed queues from one definition — full parity guaranteed
- `engine.collision_*` commands (spawn, audio, signals, camera, phase) use a separate queue that drains immediately after each collision callback
- `GameConfigCmd` mutates `GameConfig` resource; `apply_gameconfig_changes` handles vsync, fullscreen, fps, render size via Bevy change detection
- `runtime.rs` uses `register_cmd!` macro for all push-to-queue registrations

### Parent-Child Hierarchy

- Uses Bevy's `ChildOf` relationship + custom `GlobalTransform2D` component
- `:with_parent(parent_id)` on spawn or `engine.entity_set_parent(id, parent_id)` at runtime
- `propagate_transforms` recursively computes world-space position/rotation/scale
- `RemoveParent` snaps entity to current world position before detaching
- `StuckTo` system skips entities with `ChildOf` (hierarchy takes precedence)
- Render system uses `GlobalTransform2D` when present for world-space drawing
- Entity context exposes `world_pos`, `world_rotation`, `world_scale`, `parent_id` to Lua

## IMPORTANT FILES TO READ FIRST

```
Physics:          rigidbody.rs, movement.rs
Lua API:          runtime.rs, engine_api.rs, command_queues.rs, stub_meta.rs, commands.rs, entity_builder.rs, context.rs, input_snapshot.rs  [feature=lua]
Lua commands:     lua_commands/mod.rs (SystemParams), lua_commands/entity_cmd.rs (entity ops), lua_commands/spawn_cmd.rs (spawn/clone), lua_commands/parse.rs (anim conditions)  [feature=lua]
Collision:        collision_detector.rs, collision.rs (shared helpers), rust_collision.rs, lua_collision.rs (systems)
                  collision.rs, boxcollider.rs, luacollision.rs (components)
Camera follow:    cameratarget.rs (component), camerafollowconfig.rs (resource), camera_follow.rs (system)
Rendering:        render.rs, sprite.rs, rendertarget.rs, shaderstore.rs, postprocessshader.rs, entityshader.rs
Debug overlay:    debugoverlayconfig.rs (resource), render.rs (imgui integration)
Animation:        animation.rs (component + controller), animationstore.rs
State machines:   luaphase.rs (component + system) [feature=lua], phase.rs (Rust fn-pointer equivalent)
Timers:           luatimer.rs (LuaTimer alias + LuaTimerCallback) [feature=lua], timer.rs (Timer<C> generic), timer_core.rs (shared loop)
Signals:          signals.rs, worldsignals.rs
Text:             dynamictext.rs, dynamictext_size.rs (system), signalbinding.rs
Input:            inputcontrolled.rs, input.rs (InputState), input_bindings.rs (InputBindings), input_snapshot.rs, systems/input.rs (poll_action!)
Particles:        particleemitter.rs (component + system), spawn_data.rs (ParticleEmitterData)
Hierarchy:        globaltransform2d.rs (component), propagate_transforms.rs (system), lua_commands.rs (SetParent/RemoveParent)
Menus:            menu.rs (component + system), entity_builder.rs (with_menu_* methods)
Scene mgmt:       main.lua (scene registry + callback injection), lua_plugin.rs (switch_scene)
Engine bootstrap: engine_app.rs (EngineBuilder, system schedule)
```

## RAYLIB NOTES

- Uses `raylib::math::Vector2` (not Bevy's)
- `length_sqr()` not `length_squared()`
- `normalized()` returns new vector
- `Camera2D { target, offset, rotation, zoom }`
- `Texture2D`, `Font` are non-Send
- `Color::new(r, g, b, a)` with u8 values

## BEVY ECS NOTES

- `Entity::to_bits()` / `Entity::from_bits()` for u64 conversion
- `Query<(Entity, &Component, &mut Component)>`
- `Res<Resource>`, `ResMut<Resource>`
- `NonSend<Resource>`, `NonSendMut<Resource>` for non-thread-safe types
- `Commands` for deferred entity operations
- `world.get_resource::<T>()`, `world.get_resource_mut::<T>()`
- `world.entity(entity).get::<T>()`
- `ChildOf(Entity)` for parent-child relationships; `Children` component auto-managed by Bevy

## MLUA NOTES

- `lua.create_function(|lua, args| { ... })`
- `lua.app_data_ref::<T>()` for accessing shared data
- `LuaError::runtime("message")` for errors
- `table.set("key", value)`, `table.get::<Type>("key")`
- `Function::call::<ReturnType>(args)`
- Most Lua API registrations use `register_cmd` function in `engine_api.rs`: `register_cmd(engine, lua, "name", queue, |args| ArgType, Cmd::Variant { args })`
- Entity commands use `define_entity_cmds` function in `engine_api.rs` (single definition → regular + collision contexts)

## COMMON GOTCHAS

1. Non-send resources (FontStore, LuaRuntime, RenderTarget, ShaderStore) need `NonSend`/`NonSendMut`. LuaRuntime only exists with `feature=lua`.
2. All `engine.entity_*` commands work in all Lua contexts — no collision-only restrictions.
3. `engine.collision_*` commands use separate queues that drain immediately after the callback.
4. Use `engine.collision_spawn()` in collision callbacks for proper timing (not `engine.spawn()`).
5. `SpawnCmd` processed by `apply_components()` in `lua_commands.rs` (shared by spawn and clone), defined in `spawn_data.rs`.
6. Entity IDs are u64 in Lua (`Entity::to_bits`).
7. Raylib Vector2 methods differ from other math libs (see RAYLIB NOTES).
8. `Signals` component is auto-created if using `:with_signal_*` builders.
9. Scene update callbacks: `on_update_scenename(input, dt)`.
10. Phase callbacks: `on_enter(ctx, input)`, `on_update(ctx, input, dt)`, `on_exit(ctx)`. `on_enter`/`on_update` can return a phase name string to trigger a transition (takes precedence over `engine.phase_transition`).
11. Timer callbacks receive `(ctx, input)` — ctx contains entity state including timer info.
12. Animation controller evaluates rules in order; first match wins.
13. Frozen entities skip movement but still render.
14. `DynamicText.size` is cached by `dynamictext_size_system` (not calculated per-frame).
15. `WindowSize` vs `ScreenSize`: WindowSize is actual window, ScreenSize is game resolution.
16. Mouse position is automatically corrected for letterboxing in `mouse_controller`.
17. `engine.collision_spawn()` / `engine.collision_clone()` have IDENTICAL capabilities to `engine.spawn()` / `engine.clone()`.
18. Rust edition 2024 is used (newer than typical projects).
19. Entity context (ctx) in phase/timer callbacks is built by `context.rs` — includes all component data.
20. `InputSnapshot` combines WASD+arrows into unified directional inputs; also exposes raw WASD, raw arrows, and F10/F11 function keys separately.
21. Entity cloning (`engine.clone` / `engine.collision_clone`) requires the source to be registered via `:register_as()`.
22. Clone overrides always win; Animation always resets to frame 0; `:register_as()` stores the NEW cloned entity.
23. `ParticleEmitter` templates must be registered via `:register_as()` before the emitter is spawned.
24. `ParticleEmitter` uses Bevy's `clone_and_spawn()` internally; templates need `MapPosition` to emit.
25. Particle emitter runs before movement so particles move on their spawn frame.
26. TTL countdown respects `WorldTime::time_scale`; `ttl_system` runs after movement.
27. `Menu on_select_callback` takes precedence over `MenuActions` — when callback is set, actions are ignored.
28. Lua global helpers: `Lerp`, `Lerp2`, `InvLerp`, `Remap` (from `lib/math.lua`), `Dump_value` (from `lib/utils.lua`).
29. Post-process shaders: load in `on_setup`, activate in `on_switch_scene`/`on_update` via `engine.post_process_shader({"id1", "id2"})` for multi-pass chaining.
30. Reserved shader uniforms set automatically each frame: `uTime`, `uDeltaTime`, `uResolution`, `uFrame`, `uWindowResolution`, `uLetterbox`.
31. `ShaderStore` is NON_SEND (contains raylib `Shader`); `PostProcessShader` is a regular Resource.
32. `EntityShader` uses the same `ShaderStore`; uniforms are per-entity and set before drawing each entity.
33. Entity shader commands (`engine.entity_shader_*`) have full parity with collision context (`engine.collision_entity_shader_*`).
34. `Tint`: for sprites replaces `Color::WHITE` (color multiply); for text multiplies with `DynamicText.color`. RGBA values 0-255.
35. Input system emits `InputEvent` for ALL actions with pressed/released — systems can observe these instead of polling `InputState`.
36. The `"crt"` shader is loaded from `crt2.fs` (not `crt.fs`) in `setup.lua`.
37. Scene callbacks must be declared in the module's `_callbacks` table to be injected into `_G` on scene switch — prevents cross-scene naming conflicts.
38. `ChildOf` entities skip `StuckTo` system (hierarchy takes precedence over stuck-to following).
39. `WorldSignals.group_counts` is NOT a field — group counts are derived in `SignalSnapshot` from integers with `"group_count:"` prefix.
40. Registered systems (`SystemsStore`) are entities that need `Persistent` component to survive scene transitions.
41. `lua` feature flag gates all Lua-specific code. Add `#[cfg(feature = "lua")]` to any new code depending on mlua/LuaRuntime. Bevy system params cannot be conditionally compiled inline — use two separate function definitions under `#[cfg]`/`#[cfg(not)]` with a shared helper.
42. `CameraFollowConfig` is inserted disabled by default; enable it and set `mode`/`lerp_speed` etc. from scene-enter code. `CameraTarget { priority }` picks the follow target; ties broken by Entity id.
43. `CollisionCallback` takes `&mut GameCtx` (not a separate `CollisionCtx` — that type is removed). Rust collision callbacks have full GameCtx access.
44. Both `SetAnimation` EntityCmd and `animation_controller` sync `Sprite.tex_key` to the animation's texture when the animation key changes. `animation_controller` queries `AnimationStore` and a separate `Query<&mut Sprite>` for this.
45. imgui is integrated into debug mode (F11): `DebugOverlayConfig` resource controls individual overlay visibility; toggled via an imgui window at runtime. raylib must be built with `imgui` feature (enabled in `Cargo.toml` for linux/windows targets).
46. `InputSnapshot`/`DigitalInputs` now expose raw WASD (`main_*`), raw arrows (`secondary_*`), function keys (`debug`, `fullscreen`), and `action_3` in addition to combined directional fields. `AnalogInputs` has `scroll_y: f32` (mouse wheel delta), `mouse_x`/`mouse_y` (game-space, letterbox-corrected), `mouse_world_x`/`mouse_world_y` (world-space, after camera). Existing combined fields (`up/down/left/right`) are unchanged — backward compatible.
47. `InputBindings` resource decouples hardware inputs from `InputState`. `BoolState` no longer has `key_binding`. `update_input_state` reads `Res<InputBindings>` and handles both `InputBinding::Keyboard` and `InputBinding::MouseButton`. Helper functions renamed `any_binding_down/pressed/released`.
48. Lua input rebinding: `engine.rebind_action(action, key)` replaces all bindings; `engine.add_binding(action, key)` appends (multi-bind); `engine.get_binding(action)` reads snapshot (visible next frame). Action names: `main_up/down/left/right`, `secondary_up/down/left/right`, `back`, `action_1`, `action_2`, `action_3`, `special`, `toggle_debug`, `toggle_fullscreen`. Binding strings: single lowercase letters `a`-`z`, digits `0`-`9`, `space`, `enter`/`return`, `escape`/`esc`, arrows, modifiers (`lshift`/`rshift`/`lctrl`/`rctrl`/`lalt`/`ralt`), `f1`-`f12`, `mouse_left`, `mouse_right`, `mouse_middle`.
49. `InputCmd` (Rebind/AddBinding) processed by `process_input_command` in `lua_commands.rs` using `binding_from_str` (handles both keyboard and mouse); drained in `lua_plugin.rs` update/switch_scene after `drain_camera_follow_commands`.
50. `EntitySnapshot` struct in `context.rs` replaces 20+ individual parameters to `build_entity_context_pooled`. Callers build the snapshot struct and pass `&EntitySnapshot`. `ContextQueries` SystemParam in `lua_commands.rs` groups the read-only queries needed.
51. `entity_set_screen_position(entity_id, x, y)` sets `ScreenPosition` (screen-space). Distinct from `entity_set_position` which sets `MapPosition` (world-space).
52. Mouse position is computed in `update_input_state` (not just `mouse_controller`): game-space via `WindowSize::window_to_game_pos()`, world-space via `rl.get_screen_to_world2D()`. Available every frame in `InputState` and `InputSnapshot`.
53. Lua runtime is split across 4 files in `lua_runtime/`: `runtime.rs` (core types), `engine_api.rs` (all `register_*_api` methods + `register_cmd`/`define_entity_cmds`/`push_fn_meta` free functions), `command_queues.rs` (all `drain_*_commands` methods), `stub_meta.rs` (stub type definitions + meta registration). `LuaAppData` fields are `pub(super)` to allow cross-module access.
54. `cleanup_orphaned_global_transforms` runs after `propagate_transforms` and before `collision_detector`. It removes `GlobalTransform2D` from entities that have no `Children` and no `ChildOf`. Without it, a root entity that loses its last child retains a stale frozen world transform that `resolve_world_pos` returns instead of live `MapPosition`.
55. Use `ComputeInitialGlobalTransform` EntityCommand (queue via `entity_commands.queue(ComputeInitialGlobalTransform)`) after setting `ChildOf` on a newly spawned entity to avoid the one-frame world-origin flash. If the parent lacks a `GlobalTransform2D`, the command synthesizes one from the parent's local transform.
56. `WorldSignals::clear_non_persistent_entities(persistent_entities)` must be called during scene transitions to clean up entity registrations for despawned entities. Both `lua_plugin.rs` and `scene_dispatch.rs` call this.
57. `scene_switch_poll` is added to the schedule when using `SceneManager` (`.add_scene()`). It polls the `"switch_scene"` flag in `WorldSignals` each frame and runs `scene_switch_system` when set. This is distinct from `scene_switch_system` itself (which is registered as a persistent `SystemsStore` entry but not in the per-frame schedule directly).
58. `utils.lua` (`assets/scripts/lib/utils.lua`) exports `M.has_flag(flags, name)` (checks array-like flags table) and `M.dump_value(value, max_depth, ...)` (pretty-print for debugging). Load with `local utils = require("lib.utils")`.
59. `EngineBuilder.run()` calls these methods in order: `validate_builder` → `load_config` → `setup_window` → `setup_world` → `register_systems` → `spawn_observers` → `build_schedule` → `main_loop`. Each is a private method; `run()` consumes the builder.
60. `lua_commands` is now a **submodule** (`systems/lua_commands/`), not a single file. `EntityCmdQueries` and `ContextQueries` SystemParams live in `mod.rs`; command processing is split across `entity_cmd.rs` (runtime entity ops), `spawn_cmd.rs` (spawn/clone), and `parse.rs` (animation condition helpers). Public API: `process_entity_commands`, `process_spawn_command`, `process_clone_command`.
61. `Easing` and `LoopMode` (in `components/tween.rs`) implement `std::str::FromStr`. Parse strings directly: `"linear".parse::<Easing>()`, `"ping_pong".parse::<LoopMode>()`. Unknown strings default to `Easing::Linear` / `LoopMode::Once` respectively (infallible).
62. `LuaPhase` is now a type alias `Phase<PhaseCallbacks>` (not a separate struct). `Phase<C>` is generic; both Rust and Lua phases share the same component with different callback payload types (`PhaseCallbackFns` for Rust, `PhaseCallbacks` for Lua).
63. Phase `on_exit` fires AFTER the phase swap: when `on_exit` is called, `phase.current` is already the new phase. Exit callbacks are looked up by the old phase name, but the component reflects the new state.
64. `process_entity_commands` takes `&mut EntityCmdQueries` (not individual query parameters). `LuaCollisionObserverParams` embeds `EntityCmdQueries` as `entity_cmds` field instead of separate query fields.
65. `phase_core.rs` (`systems/phase_core.rs`) is `pub(crate)` — not public API. `PhaseRunner<C>` trait + `run_phase_callbacks` / `apply_callback_transitions` / `queue_phase_transition` are internal shared helpers used by both `phase_system` and `lua_phase_system`.
