# ABERRED ENGINE - LLM CONTEXT DATA

# Machine-readable context for AI assistants working on this codebase

# Last updated: 2026-01-19 (synced with codebase)

## QUICK REFERENCE

STACK: Rust (edition 2024) + Bevy ECS 0.18 + Raylib 5.5.1 + MLua 0.11 (LuaJIT) + configparser 3 + fastrand 2.3
GAME_TYPE: Asteroids-style arcade clone (2D)
ENTRY: src/main.rs
LUA_ENTRY: assets/scripts/main.lua
CONFIG: config.ini (INI format, loaded at startup)
WINDOW: Configurable via config.ini (default 1280x720 @ 120fps)

## STATUS (2026-01-19)

- Playable loop: menu ("DRIFTERS") -> level01 asteroids prototype (ship with idle/propulsion Lua phases, random drifting asteroids, tiled space background, ship fires lasers); legacy Arkanoid/paddle/brick/ball logic is currently commented out.
- Assets loaded: fonts (arcade, future), textures (cursor, ship_sheet, space01-04, asteroids-big01-03), sound (option.wav); music/tilemap/brick assets are not loaded.
- Lua on_enter_play seeds signals (score, high_score, lives, level) and sets scene="menu"; scene switches via WorldSignals flag "switch_scene".
- NEW: ParticleEmitter component and system (WIP) - emits particles by cloning template entities with configurable shape, arc, speed, TTL. Uses fastrand for RNG.

## FILE TREE (ESSENTIAL)

```
src/
├── main.rs                    # App entry, ECS world setup, main loop, system schedule
├── game.rs                    # GameState logic, scene switching, Lua callbacks
├── components/
│   ├── mod.rs                 # Re-exports all components
│   ├── mapposition.rs         # MapPosition (world-space position)
│   ├── screenposition.rs      # ScreenPosition (UI/screen-space position)
│   ├── rigidbody.rs           # Velocity, friction, max_speed, named accel forces, frozen
│   ├── boxcollider.rs         # BoxCollider (AABB collision shape)
│   ├── collision.rs           # CollisionRule, collision observer context
│   ├── luacollision.rs        # LuaCollisionRule for Lua callbacks
│   ├── sprite.rs              # Sprite rendering (tex_key, offset, origin, flip)
│   ├── animation.rs           # Animation playback state + AnimationController
│   ├── luaphase.rs            # Lua-based phase state machine
│   ├── signals.rs             # Per-entity signals (scalars/ints/flags/strings)
│   ├── dynamictext.rs         # Text rendering component with cached size
│   ├── signalbinding.rs       # Bind text to world signals
│   ├── tween.rs               # TweenPosition, TweenRotation, TweenScale
│   ├── luatimer.rs            # Lua callback timer
│   ├── stuckto.rs             # Attach entity to another
│   ├── menu.rs                # Interactive menu
│   ├── gridlayout.rs          # JSON grid spawning
│   ├── group.rs               # Entity grouping tag
│   ├── persistent.rs          # Survive scene transitions
│   ├── particleemitter.rs     # Particle emitter (templates, shape, arc, speed, TTL)
│   ├── rotation.rs            # Rotation in degrees
│   ├── scale.rs               # 2D scale
│   ├── zindex.rs              # Render order
│   └── inputcontrolled.rs     # InputControlled, AccelerationControlled, MouseControlled
├── systems/
│   ├── mod.rs                 # Re-exports all systems
│   ├── movement.rs            # Physics: accel→vel→pos, friction, max_speed
│   ├── collision.rs           # AABB detection, Lua callback dispatch
│   ├── render.rs              # Raylib drawing, camera, debug overlays, letterboxing
│   ├── input.rs               # Poll keyboard state
│   ├── inputsimplecontroller.rs    # Input→velocity
│   ├── inputaccelerationcontroller.rs # Input→acceleration
│   ├── mousecontroller.rs     # Mouse position tracking (with letterbox correction)
│   ├── animation.rs           # Frame advancement + rule evaluation (AnimationController)
│   ├── luaphase.rs            # Lua phase callbacks
│   ├── lua_commands.rs        # Process EntityCmd/CollisionEntityCmd/SpawnCmd
│   ├── luatimer.rs            # Lua timer processing
│   ├── time.rs                # WorldTime update
│   ├── signalbinding.rs       # Update bound text
│   ├── dynamictext_size.rs    # Cache DynamicText bounding box sizes
│   ├── tween.rs               # Tween animation systems (position/rotation/scale)
│   ├── stuckto.rs             # StuckTo entity following
│   ├── gridlayout.rs          # Grid entity spawning
│   ├── group.rs               # Group counting
│   ├── menu.rs                # Menu spawn/input
│   ├── particleemitter.rs     # Particle emission system (clones templates)
│   ├── audio.rs               # Audio thread bridge
│   ├── gameconfig.rs          # Apply GameConfig changes (render size, window, vsync, fps)
│   └── gamestate.rs           # State transition check
├── resources/
│   ├── mod.rs                 # Re-exports
│   ├── worldtime.rs           # Delta time, time scale
│   ├── input.rs               # InputState cached keyboard (F10=fullscreen, F11=debug)
│   ├── fullscreen.rs          # FullScreen marker resource
│   ├── gameconfig.rs          # GameConfig (render/window size, fps, vsync, fullscreen)
│   ├── texturestore.rs        # FxHashMap<String, Texture2D>
│   ├── fontstore.rs           # FxHashMap<String, Font> (non-send)
│   ├── animationstore.rs      # Animation definitions
│   ├── tilemapstore.rs        # Tilemap layouts
│   ├── gamestate.rs           # GameState enum + NextGameState
│   ├── worldsignals.rs        # Global signal storage + SignalSnapshot
│   ├── group.rs               # TrackedGroups set
│   ├── camera2d.rs            # Camera2D config
│   ├── screensize.rs          # Game's internal render resolution
│   ├── windowsize.rs          # Actual window dimensions (for letterboxing)
│   ├── rendertarget.rs        # RenderTarget for fixed-resolution rendering
│   ├── debugmode.rs           # Debug render toggle
│   ├── systemsstore.rs        # Named system lookup
│   ├── audio.rs               # AudioBridge channels
│   └── lua_runtime/
│       ├── mod.rs             # Public exports
│       ├── runtime.rs         # LuaRuntime, engine table API registration
│       ├── commands.rs        # EntityCmd, SpawnCmd, SignalCmd, etc.
│       ├── context.rs         # Entity context builder for Lua callbacks (phase/timer)
│       ├── input_snapshot.rs  # InputSnapshot for Lua callbacks
│       ├── entity_builder.rs  # LuaEntityBuilder fluent API (unified spawn/clone, regular/collision)
│       └── spawn_data.rs      # SpawnComponentData structures
└── events/
    ├── mod.rs                 # Re-exports
    ├── collision.rs           # CollisionEvent
    ├── gamestate.rs           # GameStateTransition
    ├── input.rs               # InputAction events
    ├── menu.rs                # MenuSelection
    ├── luatimer.rs            # LuaTimerEvent
    ├── switchdebug.rs         # DebugToggle (F11)
    ├── switchfullscreen.rs    # FullScreen toggle event + observer (F10)
    └── audio.rs               # AudioCmd, AudioMessage
assets/
├── scripts/
│   ├── main.lua               # Entry: on_setup, on_enter_play, on_switch_scene, on_update_*
│   ├── setup.lua              # Asset loading helpers (require in on_setup)
│   ├── engine.lua             # LSP autocomplete stubs (45k+ lines)
│   ├── .luarc.json            # Lua Language Server configuration for LuaJIT
│   ├── README.md              # Lua API documentation (78k+ lines)
│   └── scenes/
│       ├── menu.lua           # Menu scene (DRIFTERS title, Start Game -> level01, back=quit)
│       └── level01.lua        # Asteroids scene (ship phases, background tiles, drifting asteroids)
├── textures/                  # Space/asteroid art set
│   ├── cursor.png
│   ├── asteroids_ship.png
│   ├── space01.png
│   ├── space02.png
│   ├── space03.png
│   ├── space04.png
│   ├── asteroids-big01.png
│   ├── asteroids-big02.png
│   └── asteroids-big03.png
├── audio/
│   └── option.wav             # Menu selection sound
└── fonts/
    ├── Arcade_Cabinet.ttf
    └── Formal_Future.ttf

config.ini                     # Game configuration (INI format)
docs/
└── particle-emitter-plan.md   # Implementation plan for ParticleEmitter
```

## CONFIG.INI FORMAT

```ini
[render]
width = 640                    ; Internal render width
height = 360                   ; Internal render height

[window]
width = 1280                   ; Window width in pixels
height = 720                   ; Window height in pixels
target_fps = 120               ; Target frames per second
vsync = true                   ; Enable vertical sync
fullscreen = false             ; Start in fullscreen mode
```

GameConfig::load_from_file() is invoked in main.rs before world setup; `apply_gameconfig_changes`
(run_if state_is_playing) reacts to `is_added()`/`is_changed()` to resize the render target and
ScreenSize, sync fullscreen via SwitchFullScreenEvent, and apply vsync/target_fps (window resizing
code is currently commented out).

## COMPONENT QUICK-REF

MapPosition { pos: Vector2 }
ScreenPosition { pos: Vector2 }
RigidBody { velocity: Vector2, friction: Option<f32>, max_speed: Option<f32>, forces: FxHashMap<String, AccelForce>, frozen: bool }
AccelForce { acceleration: Vector2, enabled: bool }
BoxCollider { offset: Vector2, origin: Vector2, size: Vector2 }
Sprite { tex_key: String, offset: Vector2, origin: Vector2, flip_h: bool, flip_v: bool }
Animation { animation_key: String, frame_index: usize, elapsed: f32 }
AnimationController { fallback_key: String, rules: Vec<AnimationRule> }
Signals { scalars: FxHashMap, integers: FxHashMap, flags: FxHashSet, strings: FxHashMap }
LuaPhase { definition: LuaPhaseDefinition, current_phase: String, time_in_phase: f32, pending_transition: Option<String> }
DynamicText { text: Arc<str>, font: Arc<str>, font_size: f32, color: Color, size: Vector2 }
SignalBinding { key: String, format: Option<String>, binding_type: BindingType }
TweenPosition/TweenRotation/TweenScale { from, to, duration, elapsed, easing, loop_mode }
LuaTimer { duration: f32, elapsed: f32, callback: String }
Ttl { remaining: f32 }
StuckTo { target: Entity, follow_x: bool, follow_y: bool, offset: Vector2, stored_velocity: Vector2 }
Menu { items: Vec<MenuItem>, selected: usize, actions: FxHashMap<String, MenuAction>, ... }
GridLayout { path: String, group: String, zindex: i32 }
Group { name: String }
Persistent (marker)
Rotation { angle: f32 }
Scale { x: f32, y: f32 }
ZIndex { z: i32 }
InputControlled { up_velocity, down_velocity, left_velocity, right_velocity: Vector2 }
AccelerationControlled { up_acceleration, down_acceleration, left_acceleration, right_acceleration: Vector2 }
MouseControlled { follow_x: bool, follow_y: bool }
ParticleEmitter { templates: Vec<Entity>, shape: EmitterShape, offset: Vector2, particles_per_emission: u32, emissions_per_second: f32, emissions_remaining: u32, arc_degrees: (f32, f32), speed_range: (f32, f32), ttl: TtlSpec, time_since_emit: f32 }
EmitterShape { Point | Rect { width, height } }
TtlSpec { None | Fixed(f32) | Range { min, max } }

## RESOURCE QUICK-REF

GameConfig { render_width, render_height, window_width, window_height, target_fps, vsync, fullscreen, config_path }
WorldTime { delta: f32, scale: f32 }
InputState { maindirection_up/down/left/right, secondarydirection_up/down/left/right, action_back, action_1, action_2, mode_debug, fullscreen_toggle, action_special: BoolState }
BoolState { active: bool, just_pressed: bool, just_released: bool, key_binding: KeyboardKey }
InputSnapshot { digital: DigitalInputs, analog: AnalogInputs } - frozen snapshot for Lua callbacks
DigitalInputs { up, down, left, right, action_1, action_2, back, special: DigitalButtonState }
DigitalButtonState { pressed: bool, just_pressed: bool, just_released: bool }
TextureStore(FxHashMap<String, Texture2D>)
FontStore(FxHashMap<String, Font>) - NON_SEND
AnimationStore(FxHashMap<String, AnimationDef>)
TilemapStore(FxHashMap<String, TilemapData>)
GameState { None, Setup, Playing, Quitting }
NextGameState(Option<GameState>)
WorldSignals { scalars, integers, strings, flags, entities, group_counts }
SignalSnapshot { scalars, integers, strings, flags, entities, group_counts } - read-only cache for Lua
TrackedGroups(FxHashSet<String>)
Camera2D { target, offset, rotation, zoom }
ScreenSize { w: i32, h: i32 } - game's internal render resolution
WindowSize { w: i32, h: i32 } - actual window dimensions
RenderTarget { texture: RenderTexture2D, game_width, game_height, filter: RenderFilter } - NON_SEND
RenderFilter { Nearest, Bilinear }
DebugMode (marker)
FullScreen (marker) - presence indicates fullscreen mode
SystemsStore { map: FxHashMap<String, SystemId>, entity_map: FxHashMap<String, SystemId<In<Entity>>> }
LuaRuntime { lua: Lua, ... } - NON_SEND
AudioBridge { sender, receiver }

## COMMAND ENUMS (lua_runtime/commands.rs + spawn_data.rs)

EntityCmd {
    ReleaseStuckTo { entity_id }
    SignalSetFlag { entity_id, flag }
    SignalClearFlag { entity_id, flag }
    SetVelocity { entity_id, vx, vy }
    InsertStuckTo { entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy }
    RestartAnimation { entity_id }
    SetAnimation { entity_id, animation_key }
    InsertLuaTimer { entity_id, duration, callback }
    RemoveLuaTimer { entity_id }
    InsertTtl { entity_id, seconds }
    InsertTweenPosition { entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards }
    InsertTweenRotation { entity_id, from, to, duration, easing, loop_mode, backwards }
    InsertTweenScale { entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode }
    RemoveTweenPosition/Rotation/Scale { entity_id }
    SetRotation { entity_id, degrees }
    SetScale { entity_id, sx, sy }
    SignalSetScalar { entity_id, key, value }
    SignalSetString { entity_id, key, value }
    AddForce { entity_id, name, x, y, enabled }
    RemoveForce { entity_id, name }
    SetForceEnabled { entity_id, name, enabled }
    SetForceValue { entity_id, name, x, y }
    SetFriction { entity_id, friction }
    SetMaxSpeed { entity_id, max_speed: Option<f32> }
    FreezeEntity { entity_id }
    UnfreezeEntity { entity_id }
    SetSpeed { entity_id, speed }
    SetPosition { entity_id, x, y }
    Despawn { entity_id }
    MenuDespawn { entity_id }  -- despawn menu + items + cursor + textures via SystemsStore
    SignalSetInteger { entity_id, key, value }
}

SpawnCmd (spawn_data.rs) {
    group, position, screen_position, sprite, text, zindex, rigidbody, collider,
    mouse_controlled, rotation, scale, persistent, signal_scalars, signal_integers,
    signal_flags, signal_strings, phase_data, has_signals, stuckto, lua_timer, ttl,
    signal_binding, grid_layout, tween_position, tween_rotation, tween_scale, menu,
    register_as, lua_collision_rule, animation, animation_controller, particle_emitter
}

ParticleEmitterData (spawn_data.rs) {
    template_keys: Vec<String>, shape: ParticleEmitterShapeData, offset_x, offset_y,
    particles_per_emission, emissions_per_second, emissions_remaining,
    arc_min_deg, arc_max_deg, speed_min, speed_max, ttl: ParticleTtlData
}
ParticleEmitterShapeData { Point | Rect { width, height } }
ParticleTtlData { None | Fixed(f32) | Range { min, max } }

CloneCmd (commands.rs) {
    source_key: String,     -- WorldSignals key to look up source entity
    overrides: SpawnCmd     -- Component overrides (builder values win over template)
}

SignalCmd { SetScalar, SetInteger, SetString, SetFlag, ClearFlag, ClearScalar, ClearInteger, ClearString, SetEntity, RemoveEntity }
AudioLuaCmd { PlayMusic { id, looped }, PlaySound { id }, StopAllMusic, StopAllSounds }
GroupCmd { TrackGroup { name }, UntrackGroup { name }, ClearTrackedGroups }
PhaseCmd { TransitionTo { entity_id, phase } }
CameraCmd { SetCamera2D { target_x, target_y, offset_x, offset_y, rotation, zoom } }
TilemapCmd { SpawnTiles { id } }
AssetCmd { LoadTexture { id, path }, LoadFont { id, path, size }, LoadMusic { id, path }, LoadSound { id, path }, LoadTilemap { id, path } }
AnimationCmd { RegisterAnimation { id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped } }

## LUA API STRUCTURE (runtime.rs)

-- Logging
engine.log(msg), engine.log_info(msg), engine.log_warn(msg), engine.log_error(msg)

-- Assets (on_setup only)
engine.load_texture(id, path)
engine.load_font(id, path, size)
engine.load_music(id, path)
engine.load_sound(id, path)
engine.load_tilemap(id, path)
engine.register_animation(id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped)

-- Audio
engine.play_music(id, looped)
engine.play_sound(id)
engine.stop_all_music()
engine.stop_all_sounds()

-- World Signals (Reading)
engine.get_scalar(key) -> value|nil
engine.get_integer(key) -> value|nil
engine.get_string(key) -> value|nil
engine.has_flag(key) -> bool
engine.get_entity(key) -> entity_id|nil
engine.get_group_count(group) -> count|nil

-- World Signals (Writing)
engine.set_scalar(key, value)
engine.set_integer(key, value)
engine.set_string(key, value)
engine.set_flag(key)
engine.clear_flag(key)
engine.clear_scalar(key)
engine.clear_integer(key)
engine.clear_string(key)
engine.set_entity(key, entity_id)
engine.remove_entity(key)

-- Entity Commands (all contexts - phase/timer/collision/update callbacks)
-- All entity commands work in all Lua callback contexts
engine.entity_set_velocity(id, vx, vy)
engine.entity_set_rotation(id, deg)
engine.entity_set_scale(id, sx, sy)
engine.entity_signal_set_flag(id, flag)
engine.entity_signal_clear_flag(id, flag)
engine.entity_signal_set_scalar(id, key, value)
engine.entity_signal_set_string(id, key, value)
engine.entity_insert_stuckto(id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)
engine.release_stuckto(id)
engine.entity_set_animation(id, key)
engine.entity_restart_animation(id)
engine.entity_insert_lua_timer(id, duration, callback)
engine.entity_remove_lua_timer(id)
engine.entity_insert_ttl(id, seconds)
engine.entity_insert_tween_position(id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)
engine.entity_insert_tween_rotation(id, from, to, duration, easing, loop_mode, backwards)
engine.entity_insert_tween_scale(id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)
engine.entity_remove_tween_position/rotation/scale(id)
engine.entity_add_force(id, name, x, y, enabled)
engine.entity_remove_force(id, name)
engine.entity_set_force_enabled(id, name, enabled)
engine.entity_set_force_value(id, name, x, y)
engine.entity_set_friction(id, friction)
engine.entity_set_max_speed(id, max_speed|nil)
engine.entity_freeze(id)
engine.entity_unfreeze(id)
engine.entity_set_speed(id, speed)
engine.entity_set_position(id, x, y)
engine.entity_despawn(id)
engine.entity_menu_despawn(id)  -- despawn menu + items + cursor + associated textures
engine.entity_signal_set_integer(id, key, value)

-- Entity Cloning (all contexts)
engine.clone(source_key) -> EntityBuilder  -- Clone entity by WorldSignals key, apply overrides

-- Collision-Specific Commands (collision callbacks only - for proper timing)
-- These use separate queues that drain immediately after collision callback returns
engine.collision_spawn() -> EntityBuilder (same capabilities as engine.spawn())
engine.collision_clone(source_key) -> EntityBuilder  -- Clone entity in collision context
engine.collision_play_sound(id)
engine.collision_set_integer(key, value)
engine.collision_set_scalar(key, value)
engine.collision_set_string(key, value)
engine.collision_set_flag(key)
engine.collision_clear_flag(key)
engine.collision_clear_scalar(key)
engine.collision_clear_integer(key)
engine.collision_clear_string(key)
engine.collision_phase_transition(id, phase)
engine.collision_set_camera(target_x, target_y, offset_x, offset_y, rotation, zoom)

-- Collision Entity Commands (full parity with engine.entity_* commands)
engine.collision_entity_set_position(id, x, y)
engine.collision_entity_set_velocity(id, vx, vy)
engine.collision_entity_despawn(id)
engine.collision_entity_signal_set_integer(id, key, value)
engine.collision_entity_signal_set_flag(id, flag)
engine.collision_entity_signal_clear_flag(id, flag)
engine.collision_entity_signal_set_scalar(id, key, value)
engine.collision_entity_signal_set_string(id, key, value)
engine.collision_entity_insert_stuckto(id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)
engine.collision_release_stuckto(id)
engine.collision_entity_insert_lua_timer(id, duration, callback)
engine.collision_entity_remove_lua_timer(id)
engine.collision_entity_insert_ttl(id, seconds)
engine.collision_entity_restart_animation(id)
engine.collision_entity_set_animation(id, animation_key)
engine.collision_entity_insert_tween_position(id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)
engine.collision_entity_insert_tween_rotation(id, from, to, duration, easing, loop_mode, backwards)
engine.collision_entity_insert_tween_scale(id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)
engine.collision_entity_remove_tween_position/rotation/scale(id)
engine.collision_entity_set_rotation(id, degrees)
engine.collision_entity_set_scale(id, sx, sy)
engine.collision_entity_add_force(id, name, x, y, enabled)
engine.collision_entity_remove_force(id, name)
engine.collision_entity_set_force_enabled(id, name, enabled)
engine.collision_entity_set_force_value(id, name, x, y)
engine.collision_entity_set_friction(id, friction)
engine.collision_entity_set_max_speed(id, max_speed|nil)
engine.collision_entity_freeze(id)
engine.collision_entity_unfreeze(id)
engine.collision_entity_set_speed(id, speed)

-- Phase Control
engine.phase_transition(id, phase)
-- OR return phase name from on_enter/on_update callbacks (return value takes precedence)

-- Input (passed as argument to callbacks, not queried via functions)
-- Input table structure:
-- input.digital.up/down/left/right/action_1/action_2/back/special = { pressed, just_pressed, just_released }
-- input.analog = {} (reserved for future gamepad support)

-- Groups
engine.track_group(name)
engine.untrack_group(name)
engine.clear_tracked_groups()
engine.has_tracked_group(name) -> bool

-- Camera
engine.set_camera(target_x, target_y, offset_x, offset_y, rotation, zoom)

-- Tilemap
engine.spawn_tiles(id)

-- Entity Builder (engine.spawn())
:with_group(name)
:with_position(x, y)
:with_screen_position(x, y)
:with_sprite(tex_key, w, h, origin_x, origin_y)
:with_sprite_offset(ox, oy)
:with_sprite_flip(flip_h, flip_v)
:with_zindex(z)
:with_velocity(vx, vy)
:with_friction(friction)
:with_max_speed(max_speed)
:with_accel(name, x, y, enabled)
:with_frozen()
:with_collider(w, h, origin_x, origin_y)
:with_collider_offset(ox, oy)
:with_rotation(deg)
:with_scale(sx, sy)
:with_mouse_controlled(follow_x, follow_y)
:with_persistent()
:with_signals()
:with_signal_scalar/integer/flag/string(key, value?)
:with_text(content, font, size, r, g, b, a)
:with_signal_binding(key)
:with_signal_binding_format(format)
:with_menu(items, ox, oy, font, size, spacing, screen_space)
:with_menu_colors(nr, ng, nb, na, sr, sg, sb, sa)
:with_menu_dynamic_text(dynamic)
:with_menu_cursor(key)
:with_menu_selection_sound(sound_key)
:with_menu_action_set_scene/show_submenu/quit(item_id, ...)
:with_animation(key)
:with_animation_controller(fallback_key)
:with_animation_rule(condition_table, set_key)
:with_phase(table)
:with_stuckto(target_id, follow_x, follow_y)
:with_stuckto_offset(ox, oy)
:with_stuckto_stored_velocity(vx, vy)
:with_tween_position(fx, fy, tx, ty, dur)
:with_tween_position_easing(easing)
:with_tween_position_loop(mode)
:with_tween_position_backwards()
:with_tween_rotation(from, to, dur)
:with_tween_rotation_easing(easing)
:with_tween_rotation_loop(mode)
:with_tween_rotation_backwards()
:with_tween_scale(fx, fy, tx, ty, dur)
:with_tween_scale_easing(easing)
:with_tween_scale_loop(mode)
:with_tween_scale_backwards()
:with_lua_timer(duration, callback)
:with_ttl(seconds)
:with_lua_collision_rule(group_a, group_b, callback)
:with_grid_layout(path, group, zindex)
:with_particle_emitter(table)  -- { templates, shape, offset, particles_per_emission, emissions_per_second, emissions_remaining, arc, speed, ttl }
:register_as(key)
:build()

-- Clone Builder (engine.clone(source_key) / engine.collision_clone(source_key))
-- Clones existing entity, applies overrides. Animation reset to frame 0.
-- Source looked up by WorldSignals key. :register_as() stores NEW cloned entity.

-- Collision Entity Builder (engine.collision_spawn() / engine.collision_clone())
-- Has IDENTICAL capabilities as EntityBuilder - all methods available

## ENTITY CONTEXT (Lua callback ctx table for phase/timer callbacks)

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

IMPORTANT: The entity context table is POOLED and REUSED between callbacks for performance.
Do NOT store references to ctx or its subtables for later use - values will be overwritten.
(Implementation: EntityCtxPool/EntityCtxTables in runtime.rs, build_entity_context_pooled in context.rs)

## COLLISION CONTEXT (Lua callback ctx table)

ctx.a.id           -- u64 entity ID
ctx.a.group        -- string group name
ctx.a.pos          -- { x, y } or nil
ctx.a.vel          -- { x, y } or nil
ctx.a.speed_sq     -- f32 squared speed
ctx.a.rect         -- { x, y, w, h } or nil
ctx.a.signals      -- { flags={...}, integers={...}, scalars={...}, strings={...} } or nil

ctx.b.id/group/pos/vel/speed_sq/rect/signals  -- same structure

ctx.sides.a        -- array of strings: {"left", "top", ...} (collision sides for entity A)
ctx.sides.b        -- array of strings: {"right", ...} (collision sides for entity B)

IMPORTANT: The collision context table is POOLED and REUSED between collisions for performance.
Do NOT store references to ctx or its subtables for later use - values will be overwritten.
(Implementation: CollisionCtxPool/CollisionCtxTables in runtime.rs)

## SYSTEM EXECUTION ORDER (main.rs schedule)

Systems with explicit ordering constraints (`.after()` / `.before()`):

- apply_gameconfig_changes (run_if state_is_playing)
- particle_emitter_system.before(movement)
- stuck_to_entity_system.after(collision_detector)
- collision_detector.after(mouse_controller).after(movement)
- lua_phase_system.after(collision_detector)
- animation_controller.after(lua_phase_system)
- animation.after(animation_controller)
- dynamictext_size_system.after(update_world_signals_binding_system)
- render_system.after(collision_detector)

Approximate execution order:

1. apply_gameconfig_changes (run_if state_is_playing)
2. menu_spawn_system
3. gridlayout_spawn_system
4. update_input_state
5. check_pending_state
6. update_group_counts_system
7. audio systems chain (update_bevy_audio_cmds → forward_audio_cmds → poll_audio_messages → update_bevy_audio_messages)
8. input_simple_controller
9. input_acceleration_controller
10. mouse_controller
11. tween_mapposition_system, tween_rotation_system, tween_scale_system
12. particle_emitter_system (before movement - particles move on spawn frame)
13. movement
14. collision_detector
15. stuck_to_entity_system (after collision)
16. lua_phase_system (after collision)
17. animation_controller (after lua_phase)
18. animation (after animation_controller)
19. update_lua_timers
20. update_world_signals_binding_system
21. dynamictext_size_system
22. run_<scene>_update (game::update, run_if state_is_playing, after check_pending_state)
23. render_system (after collision_detector)

## KEY PATTERNS

### Adding a new EntityCmd

1. Add variant to EntityCmd enum (commands.rs)
2. Add Lua function in runtime.rs (register_entity_api)
3. Process in process_entity_commands (lua_commands.rs)
4. Add to engine.lua autocomplete stubs
5. Document in README.md

### Registering Entity-Input Systems (SystemsStore)

For systems that need to be called with a specific entity (like menu_despawn):

1. Define system with `In(target): In<Entity>` parameter
2. Register with `world.register_system(system_fn)` -> `SystemId<In<Entity>>`
3. Store in `systems_store.insert_entity_system("name", system_id)`
4. Call via `commands.run_system_with(*system_id, entity)` in EntityCmd handler

### Adding a new Component

1. Create file in src/components/
2. Derive Component, add fields
3. Export from components/mod.rs
4. Add to SpawnComponentData if spawnable (spawn_data.rs)
5. Add builder method in entity_builder.rs
6. Process in process_spawn_command (lua_commands.rs)

### Adding a new System

1. Create file in src/systems/
2. Define system function with queries
3. Export from systems/mod.rs
4. Add to schedule in main.rs (correct position)

### Adding a new Resource

1. Create file in src/resources/
2. Derive Resource (or use non-send pattern)
3. Export from resources/mod.rs
4. Insert into world in main.rs

### Lua-Rust Command Flow

Lua calls engine.* -> LuaAppData.commands.borrow_mut().push(Cmd)
-> Lua returns -> game.rs processes queued commands
-> Commands modify ECS world

### Collision Flow

movement_system moves entities
-> collision_system detects AABB overlaps
-> dispatches CollisionEvent or calls Lua callback
-> Lua callback uses engine.entity_*commands (work in all contexts)
-> Collision-specific commands (engine.collision_*) use separate queues
-> Collision commands drain immediately after callback

### Entity Commands Architecture

- Entity commands (engine.entity_*) are UNIFIED across all contexts
- They work in phase callbacks, timer callbacks, collision callbacks, and update callbacks
- Collision has FULL entity command parity: engine.collision_entity_*mirrors all engine.entity_* functions
- Collision-specific commands (engine.collision_*) also include spawning, audio, signals, camera
- Both regular and collision entity commands use the same EntityCmd enum internally
- Collision commands drain immediately after each collision callback (separate queue)

## IMPORTANT FILES TO READ FIRST

For features touching:

- Physics: rigidbody.rs, movement.rs
- Lua API: runtime.rs, commands.rs, entity_builder.rs, context.rs, input_snapshot.rs
- Collision: collision.rs (systems), boxcollider.rs, luacollision.rs (components)
- Rendering: render.rs, sprite.rs, rendertarget.rs, windowsize.rs
- Animation: animation.rs (component + controller), animationstore.rs
- State machines: luaphase.rs (component + system)
- Signals: signals.rs, worldsignals.rs
- Text: dynamictext.rs, dynamictext_size.rs (system), signalbinding.rs
- Input: inputcontrolled.rs (InputControlled, AccelerationControlled, MouseControlled), input.rs (InputState), input_snapshot.rs
- Particles: particleemitter.rs (component + system), spawn_data.rs (ParticleEmitterData)

## RAYLIB NOTES

- Uses raylib::math::Vector2 (not Bevy's)
- length_sqr() not length_squared()
- normalized() returns new vector
- Camera2D { target, offset, rotation, zoom }
- Texture2D, Font are non-Send
- Color::new(r, g, b, a) with u8 values

## BEVY ECS NOTES

- Entity::to_bits() / Entity::from_bits() for u64 conversion
- Query<(Entity, &Component, &mut Component)>
- Res<Resource>, ResMut<Resource>
- NonSend<Resource>, NonSendMut<Resource> for non-thread-safe types
- Commands for deferred entity operations
- world.get_resource::<T>(), world.get_resource_mut::<T>()
- world.entity(entity).get::<T>()

## MLUA NOTES

- lua.create_function(|lua, args| { ... })
- lua.app_data_ref::<T>() for accessing shared data
- LuaError::runtime("message") for errors
- table.set("key", value), table.get::<Type>("key")
- Function::call::<ReturnType>(args)

## COMMON GOTCHAS

1. Non-send resources (FontStore, LuaRuntime, RenderTarget) need NonSend/NonSendMut
2. All entity commands (engine.entity_*) work in all contexts (no collision-only restrictions)
3. Collision-specific commands (engine.collision_*) use separate queues that drain immediately
4. Use engine.collision_spawn() in collision callbacks for proper timing (not engine.spawn())
5. SpawnCmd processed in lua_commands.rs, defined in spawn_data.rs
6. Entity IDs are u64 in Lua (Entity::to_bits)
7. Raylib Vector2 methods differ from other math libs
8. Signals component auto-created if using :with_signal_* builders
9. Scene update callbacks: on_update_scenename(input, dt)
10. Phase callbacks: on_enter(ctx, input), on_update(ctx, input, dt), on_exit(ctx) - ctx contains entity state. on_enter/on_update can return phase name string to trigger transition (takes precedence over engine.phase_transition)
11. Timer callbacks receive (ctx, input) as args - ctx contains entity state including timer info
12. Animation controller evaluates rules in order, first match wins
13. Frozen entities skip movement but still render
14. DynamicText.size is cached by dynamictext_size_system (not calculated per-frame)
15. WindowSize vs ScreenSize: WindowSize is actual window, ScreenSize is game resolution
16. Mouse position is automatically corrected for letterboxing in mouse_controller
17. Collision entity builder (engine.collision_spawn()) has IDENTICAL capabilities to engine.spawn()
18. Rust edition 2024 is used (newer edition than typical projects)
19. Entity context (ctx) in phase/timer callbacks is built by context.rs - includes all component data
20. InputSnapshot (input_snapshot.rs) combines WASD+arrows into unified directional inputs
21. Entity cloning (engine.clone/collision_clone) requires source registered via :register_as()
22. Clone overrides always win; Animation always resets to frame 0; :register_as() stores NEW entity
23. ParticleEmitter templates must be registered via :register_as() before emitter is spawned
24. ParticleEmitter uses Bevy's clone_and_spawn() internally; templates need MapPosition to emit
25. Particle emitter runs before movement so particles move on their spawn frame
