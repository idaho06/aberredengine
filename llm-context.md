# ABERRED ENGINE - LLM CONTEXT DATA
# Machine-readable context for AI assistants working on this codebase
# Last updated: 2025-12-29 (synced with codebase)

## QUICK REFERENCE

STACK: Rust + Bevy ECS 0.17 + Raylib 5.5 + MLua (LuaJIT)
GAME_TYPE: Arkanoid-style breakout clone (2D)
ENTRY: src/main.rs
LUA_ENTRY: assets/scripts/main.lua
WINDOW: 672x768 @ 120fps

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
│   ├── phase.rs               # Rust phase state machine
│   ├── luaphase.rs            # Lua-based phase state machine
│   ├── signals.rs             # Per-entity signals (scalars/ints/flags/strings)
│   ├── dynamictext.rs         # Text rendering component with cached size
│   ├── signalbinding.rs       # Bind text to world signals
│   ├── tween.rs               # TweenPosition, TweenRotation, TweenScale
│   ├── timer.rs               # Countdown timer
│   ├── luatimer.rs            # Lua callback timer
│   ├── stuckto.rs             # Attach entity to another
│   ├── menu.rs                # Interactive menu
│   ├── gridlayout.rs          # JSON grid spawning
│   ├── group.rs               # Entity grouping tag
│   ├── persistent.rs          # Survive scene transitions
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
│   ├── phase.rs               # Rust phase callbacks
│   ├── luaphase.rs            # Lua phase callbacks
│   ├── lua_commands.rs        # Process EntityCmd/CollisionEntityCmd/SpawnCmd
│   ├── luatimer.rs            # Lua timer processing
│   ├── time.rs                # WorldTime update, timer ticks
│   ├── signalbinding.rs       # Update bound text
│   ├── dynamictext_size.rs    # Cache DynamicText bounding box sizes
│   ├── tween.rs               # Tween animation systems (position/rotation/scale)
│   ├── stuckto.rs             # StuckTo entity following
│   ├── gridlayout.rs          # Grid entity spawning
│   ├── group.rs               # Group counting
│   ├── menu.rs                # Menu spawn/input
│   ├── audio.rs               # Audio thread bridge
│   └── gamestate.rs           # State transition check
├── resources/
│   ├── mod.rs                 # Re-exports
│   ├── worldtime.rs           # Delta time, time scale
│   ├── input.rs               # InputState cached keyboard (F10=fullscreen, F11=debug)
│   ├── fullscreen.rs          # FullScreen marker resource
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
│       ├── commands.rs        # EntityCmd, CollisionEntityCmd, SpawnCmd, etc.
│       ├── entity_builder.rs  # LuaEntityBuilder, LuaCollisionEntityBuilder fluent API
│       └── spawn_data.rs      # SpawnComponentData structures
└── events/
    ├── mod.rs                 # Re-exports
    ├── collision.rs           # CollisionEvent
    ├── gamestate.rs           # GameStateTransition
    ├── input.rs               # InputAction events
    ├── menu.rs                # MenuSelection
    ├── phase.rs               # PhaseTransition
    ├── timer.rs               # TimerEvent
    ├── luatimer.rs            # LuaTimerEvent
    ├── switchdebug.rs         # DebugToggle (F11)
    ├── switchfullscreen.rs    # FullScreen toggle event + observer (F10)
    └── audio.rs               # AudioCmd, AudioMessage

assets/scripts/
├── main.lua                   # Entry: on_setup, on_enter_play, on_switch_scene
├── setup.lua                  # Asset loading helpers
├── engine.lua                 # LSP autocomplete stubs
├── README.md                  # Lua API documentation
└── scenes/
    ├── menu.lua               # Menu scene
    └── level01.lua            # Gameplay scene
```

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
Timer { duration: f32, elapsed: f32, signal: String }
LuaTimer { duration: f32, elapsed: f32, callback: String }
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

## RESOURCE QUICK-REF

WorldTime { delta: f32, scale: f32 }
InputState { action_back, action_1, mode_debug, fullscreen_toggle, action_special, ...: BoolState }
BoolState { active: bool, just_pressed: bool, just_released: bool, key_binding: KeyboardKey }
TextureStore(FxHashMap<String, Texture2D>)
FontStore(FxHashMap<String, Font>) - NON_SEND
AnimationStore(FxHashMap<String, AnimationDef>)
TilemapStore(FxHashMap<String, TilemapData>)
GameState { Setup, Playing, Paused, Quitting }
NextGameState(Option<GameState>)
WorldSignals { scalars, integers, strings, flags, entities, group_counts }
SignalSnapshot { scalars, integers, strings, flags, entities, group_counts } - read-only cache for Lua
TrackedGroups(FxHashSet<String>)
Camera2D { target, offset, rotation, zoom }
ScreenSize { width: u32, height: u32 } - game's internal render resolution
WindowSize { w: i32, h: i32 } - actual window dimensions
RenderTarget { texture: RenderTexture2D, game_width, game_height, filter: RenderFilter } - NON_SEND
RenderFilter { Nearest, Bilinear }
DebugMode (marker)
FullScreen (marker) - presence indicates fullscreen mode
SystemsStore(FxHashMap<String, SystemFn>)
LuaRuntime { lua: Lua, ... } - NON_SEND
AudioBridge { sender, receiver }

## COMMAND ENUMS (lua_runtime/commands.rs)

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
    InsertTweenPosition { entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode }
    InsertTweenRotation { entity_id, from, to, duration, easing, loop_mode }
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
}

CollisionEntityCmd {
    SetPosition { entity_id, x, y }
    SetVelocity { entity_id, vx, vy }
    Despawn { entity_id }
    SignalSetInteger { entity_id, key, value }
    SignalSetFlag { entity_id, flag }
    SignalClearFlag { entity_id, flag }
    InsertTimer { entity_id, duration, signal }
    InsertStuckTo { entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy }
    FreezeEntity { entity_id }
    UnfreezeEntity { entity_id }
    AddForce { entity_id, name, x, y, enabled }
    SetForceEnabled { entity_id, name, enabled }
    SetSpeed { entity_id, speed }
}

SpawnCmd { position, screen_position, sprite, collider, velocity, friction, max_speed, forces, frozen, ... }

SignalCmd { SetScalar, SetInteger, SetString, SetFlag, ClearFlag }
AudioLuaCmd { PlayMusic { id, looped }, PlaySound { id }, StopAllMusic, StopAllSounds }
GroupCmd { TrackGroup { name }, UntrackGroup { name }, ClearTrackedGroups }
PhaseCmd { TransitionTo { entity_id, phase } }
CameraCmd { SetCamera2D { target_x, target_y, offset_x, offset_y, rotation, zoom } }
TilemapCmd { SpawnTiles { id } }
AssetCmd { LoadTexture { id, path }, LoadFont { id, path, size }, LoadMusic { id, path }, LoadSound { id, path }, LoadTilemap { id, path } }
AnimationCmd { RegisterAnimation { id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped } }

## LUA API STRUCTURE (runtime.rs)

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

-- World Signals
engine.set_scalar/integer/string/flag(key, value?)
engine.get_scalar/integer/string(key) -> value|nil
engine.has_flag(key) -> bool
engine.clear_flag(key)
engine.get_entity(key) -> entity_id|nil
engine.get_group_count(group) -> count|nil

-- Entity Commands (queued for deferred processing)
engine.release_stuckto(id)
engine.entity_signal_set_flag(id, flag)
engine.entity_signal_clear_flag(id, flag)
engine.entity_signal_set_scalar(id, key, value)
engine.entity_signal_set_string(id, key, value)
engine.entity_set_velocity(id, vx, vy)  -- NOTE: overwrites to collision queue
engine.entity_set_rotation(id, deg)
engine.entity_set_scale(id, sx, sy)
engine.entity_insert_stuckto(id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)
engine.entity_set_animation(id, key)
engine.entity_restart_animation(id)
engine.entity_insert_lua_timer(id, duration, callback)
engine.entity_remove_lua_timer(id)
engine.entity_insert_tween_position(id, from_x, from_y, to_x, to_y, duration, easing, loop_mode)
engine.entity_insert_tween_rotation(id, from, to, duration, easing, loop_mode)
engine.entity_insert_tween_scale(id, from_x, from_y, to_x, to_y, duration, easing, loop_mode)
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

-- Collision-Context Commands (processed immediately after collision callbacks)
-- NOTE: entity_set_position, entity_set_velocity, entity_despawn push to collision queue
engine.entity_set_position(id, x, y)  -- collision queue only
engine.entity_set_velocity(id, vx, vy)  -- collision queue (overwrites entity API)
engine.entity_despawn(id)  -- collision queue only
engine.entity_signal_set_integer(id, key, value)  -- collision queue
engine.entity_signal_set_flag(id, flag)  -- collision queue (overwrites entity API)
engine.entity_signal_clear_flag(id, flag)  -- collision queue (overwrites entity API)
engine.entity_insert_timer(id, duration, signal)  -- collision queue only
engine.entity_insert_stuckto(...)  -- collision queue (overwrites entity API)
engine.collision_play_sound(id)
engine.collision_set_integer(key, value)
engine.collision_set_flag(key)
engine.collision_clear_flag(key)
engine.collision_spawn() -> LuaCollisionEntityBuilder
engine.collision_phase_transition(id, phase)
engine.collision_set_camera(target_x, target_y, offset_x, offset_y, rotation, zoom)
engine.collision_entity_freeze(id)
engine.collision_entity_unfreeze(id)
engine.collision_entity_add_force(id, name, x, y, enabled)
engine.collision_entity_set_force_enabled(id, name, enabled)
engine.collision_entity_set_speed(id, speed)

-- Phase Control
engine.phase_transition(id, phase)

-- Input
engine.is_action_back_pressed() -> bool
engine.is_action_back_just_pressed() -> bool
engine.is_action_confirm_pressed() -> bool
engine.is_action_confirm_just_pressed() -> bool

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
:with_tween_rotation(from, to, dur)
:with_tween_rotation_easing(easing)
:with_tween_rotation_loop(mode)
:with_tween_scale(fx, fy, tx, ty, dur)
:with_tween_scale_easing(easing)
:with_tween_scale_loop(mode)
:with_timer(duration, signal)
:with_lua_timer(duration, callback)
:with_lua_collision_rule(group_a, group_b, callback)
:with_grid_layout(path, group, zindex)
:register_as(key)
:build()

-- Collision Entity Builder (engine.collision_spawn())
-- Subset of methods: with_group, with_position, with_sprite, with_sprite_offset,
-- with_sprite_flip, with_zindex, with_velocity, with_friction, with_max_speed,
-- with_accel, with_frozen, with_collider, with_collider_offset, with_rotation,
-- with_scale, with_signal_integer, with_signal_flag, with_signals, with_timer,
-- with_lua_timer, with_animation, :build()

## COLLISION CONTEXT (Lua callback ctx table)

ctx.a.id           -- u64 entity ID
ctx.a.group        -- string group name
ctx.a.pos          -- { x, y } or nil
ctx.a.vel          -- { x, y } or nil
ctx.a.speed_sq     -- f32 squared speed
ctx.a.rect         -- { x, y, w, h } or nil
ctx.a.signals      -- { flags={...}, integers={...}, scalars={...}, strings={...} } or nil

ctx.b.id/group/pos/vel/speed_sq/rect/signals  -- same structure

ctx.sides.a.top/bottom/left/right  -- bool collision sides
ctx.sides.b.top/bottom/left/right

## SYSTEM EXECUTION ORDER (main.rs schedule)

Systems with explicit ordering constraints (`.after()`):
- phase_update_system.after(phase_change_detector)
- stuck_to_entity_system.after(collision_detector)
- collision_detector.after(mouse_controller).after(movement)
- lua_phase_system.after(collision_detector)
- animation_controller.after(lua_phase_system)
- animation.after(animation_controller)
- dynamictext_size_system.after(update_world_signals_binding_system)
- render_system.after(collision_detector)

Approximate execution order:
1. phase_change_detector
2. phase_update_system
3. menu_spawn_system
4. gridlayout_spawn_system
5. update_input_state
6. check_pending_state
7. update_group_counts_system
8. audio systems (update_world_time, poll_audio_messages, forward_audio_cmds, ...)
9. input_simple_controller
10. input_acceleration_controller
11. mouse_controller
12. tween_mapposition_system, tween_rotation_system, tween_scale_system
13. movement
14. collision_detector
15. stuck_to_entity_system, lua_phase_system, render_system (all after collision)
16. animation_controller (after lua_phase)
17. animation (after animation_controller)
18. update_timers
19. update_lua_timers
20. update_world_signals_binding_system
21. dynamictext_size_system
22. run_<scene>_update (via game.rs)

## KEY PATTERNS

### Adding a new EntityCmd:
1. Add variant to EntityCmd enum (commands.rs)
2. Add Lua function in runtime.rs (register_entity_api)
3. Process in process_entity_commands (lua_commands.rs)
4. Add to engine.lua autocomplete stubs
5. Document in README.md

### Adding a CollisionEntityCmd:
1. Add variant to CollisionEntityCmd enum (commands.rs)
2. Add Lua function in runtime.rs (register_collision_api)
3. Process in process_collision_entity_commands (lua_commands.rs)
4. Add to engine.lua autocomplete stubs
5. Document in README.md

### Adding a new Component:
1. Create file in src/components/
2. Derive Component, add fields
3. Export from components/mod.rs
4. Add to SpawnComponentData if spawnable (spawn_data.rs)
5. Add builder method in entity_builder.rs
6. Process in process_spawn_command (lua_commands.rs)

### Adding a new System:
1. Create file in src/systems/
2. Define system function with queries
3. Export from systems/mod.rs
4. Add to schedule in main.rs (correct position)

### Adding a new Resource:
1. Create file in src/resources/
2. Derive Resource (or use non-send pattern)
3. Export from resources/mod.rs
4. Insert into world in main.rs

### Lua-Rust Command Flow:
Lua calls engine.* -> LuaAppData.commands.borrow_mut().push(Cmd)
-> Lua returns -> game.rs processes queued commands
-> Commands modify ECS world

### Collision Flow:
movement_system moves entities
-> collision_system detects AABB overlaps
-> dispatches CollisionEvent or calls Lua callback
-> Lua callback uses engine.collision_* functions
-> CollisionEntityCmd queued
-> process_collision_entity_commands runs immediately

## IMPORTANT FILES TO READ FIRST

For features touching:
- Physics: rigidbody.rs, movement.rs
- Lua API: runtime.rs, commands.rs, entity_builder.rs
- Collision: collision.rs (systems), boxcollider.rs, luacollision.rs (components)
- Rendering: render.rs, sprite.rs, rendertarget.rs, windowsize.rs
- Animation: animation.rs (component + controller), animationstore.rs
- State machines: luaphase.rs (component + system)
- Signals: signals.rs, worldsignals.rs
- Text: dynamictext.rs, dynamictext_size.rs (system), signalbinding.rs
- Input: inputcontrolled.rs (InputControlled, AccelerationControlled, MouseControlled)

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
2. Collision callbacks have separate command queue (CollisionEntityCmd)
3. SpawnCmd processed in lua_commands.rs, not inline
4. Entity IDs are u64 in Lua (Entity::to_bits)
5. Raylib Vector2 methods differ from other math libs
6. Signals component auto-created if using :with_signal_* builders
7. Phase callbacks receive entity_id as first arg
8. Timer callbacks receive entity_id as first arg
9. Animation controller evaluates rules in order, first match wins
10. Frozen entities skip movement but still render
11. engine.entity_set_position/velocity/despawn go to collision queue (not entity queue)
12. Some entity_* functions are overwritten by collision API registration order
13. DynamicText.size is cached by dynamictext_size_system (not calculated per-frame)
14. WindowSize vs ScreenSize: WindowSize is actual window, ScreenSize is game resolution
15. Mouse position is automatically corrected for letterboxing in mouse_controller
