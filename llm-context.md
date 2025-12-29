# ABERRED ENGINE - LLM CONTEXT DATA
# Machine-readable context for AI assistants working on this codebase
# Last updated: 2025-12-29

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
│   ├── position.rs            # MapPosition (world), ScreenPosition (UI)
│   ├── rigidbody.rs           # Velocity, friction, max_speed, named accel forces, frozen
│   ├── collision.rs           # BoxCollider, CollisionRule, LuaCollisionRule
│   ├── sprite.rs              # Sprite rendering (tex_key, offset, origin, flip)
│   ├── animation.rs           # Animation playback state
│   ├── animation_controller.rs # Rule-based animation switching
│   ├── phase.rs               # Rust phase state machine
│   ├── luaphase.rs            # Lua-based phase state machine
│   ├── signals.rs             # Per-entity signals (scalars/ints/flags/strings)
│   ├── dynamictext.rs         # Text rendering component
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
│   ├── inputcontrolled.rs     # Keyboard velocity control
│   ├── accelerationcontrolled.rs # Keyboard acceleration control
│   └── mousecontrolled.rs     # Mouse position following
├── systems/
│   ├── mod.rs                 # Re-exports all systems
│   ├── movement.rs            # Physics: accel→vel→pos, friction, max_speed
│   ├── collision.rs           # AABB detection, Lua callback dispatch
│   ├── render.rs              # Raylib drawing, camera, debug overlays
│   ├── input.rs               # Poll keyboard state
│   ├── inputsimplecontroller.rs    # Input→velocity
│   ├── inputaccelerationcontroller.rs # Input→acceleration
│   ├── mousecontroller.rs     # Mouse position tracking
│   ├── animation.rs           # Frame advancement
│   ├── animation_controller.rs # Rule evaluation
│   ├── phase.rs               # Rust phase callbacks
│   ├── luaphase.rs            # Lua phase callbacks
│   ├── lua_commands.rs        # Process EntityCmd/CollisionEntityCmd/SpawnCmd
│   ├── luatimer.rs            # Lua timer processing
│   ├── time.rs                # WorldTime update, timer ticks
│   ├── signalbinding.rs       # Update bound text
│   ├── gridlayout.rs          # Grid entity spawning
│   ├── group.rs               # Group counting
│   ├── menu.rs                # Menu spawn/input
│   ├── audio.rs               # Audio thread bridge
│   └── gamestate.rs           # State transition check
├── resources/
│   ├── mod.rs                 # Re-exports
│   ├── worldtime.rs           # Delta time, time scale
│   ├── input.rs               # InputState cached keyboard
│   ├── texturestore.rs        # FxHashMap<String, Texture2D>
│   ├── fontstore.rs           # FxHashMap<String, Font> (non-send)
│   ├── animationstore.rs      # Animation definitions
│   ├── tilemapstore.rs        # Tilemap layouts
│   ├── gamestate.rs           # GameState enum + NextGameState
│   ├── worldsignals.rs        # Global signal storage
│   ├── group.rs               # TrackedGroups set
│   ├── camera2d.rs            # Camera2D config
│   ├── screensize.rs          # Window dimensions
│   ├── debugmode.rs           # Debug render toggle
│   ├── systemsstore.rs        # Named system lookup
│   ├── audio.rs               # AudioBridge channels
│   └── lua_runtime/
│       ├── mod.rs             # Public exports
│       ├── runtime.rs         # LuaRuntime, engine table API registration
│       ├── commands.rs        # EntityCmd, CollisionEntityCmd, SpawnCmd, etc.
│       ├── entity_builder.rs  # LuaEntityBuilder fluent API
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
    ├── switchdebug.rs         # DebugToggle
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
DynamicText { content: String, font_key: String, font_size: f32, color: Color }
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

## RESOURCE QUICK-REF

WorldTime { delta: f32, scale: f32 }
InputState { action_back: bool, action_back_just: bool, action_confirm: bool, action_confirm_just: bool, ... }
TextureStore(FxHashMap<String, Texture2D>)
FontStore(FxHashMap<String, Font>) - NON_SEND
AnimationStore(FxHashMap<String, AnimationDef>)
TilemapStore(FxHashMap<String, TilemapData>)
GameState { Setup, Playing, Paused, Quitting }
NextGameState(Option<GameState>)
WorldSignals { scalars, integers, strings, flags, entities, groups }
TrackedGroups(FxHashSet<String>)
Camera2D { target, offset, rotation, zoom }
ScreenSize { width, height }
DebugMode (marker)
SystemsStore(FxHashMap<String, SystemFn>)
LuaRuntime { lua: Lua, ... } - NON_SEND
AudioBridge { sender, receiver }

## COMMAND ENUMS (lua_runtime/commands.rs)

EntityCmd {
    SetPosition { entity_id, x, y }
    SetVelocity { entity_id, vx, vy }
    SetRotation { entity_id, degrees }
    SetScale { entity_id, sx, sy }
    Despawn { entity_id }
    SignalSetFlag/ClearFlag/SetInteger/SetScalar/SetString { entity_id, key, value? }
    InsertTimer { entity_id, duration, signal }
    InsertLuaTimer { entity_id, duration, callback }
    RemoveLuaTimer { entity_id }
    InsertTweenPosition/Rotation/Scale { entity_id, from, to, duration, easing, loop_mode }
    RemoveTweenPosition/Rotation/Scale { entity_id }
    InsertStuckTo { entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy }
    ReleaseStuckTo { entity_id }
    SetAnimation { entity_id, animation_key }
    RestartAnimation { entity_id }
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
    SetPosition, SetVelocity, Despawn, SignalSetFlag/ClearFlag/SetInteger,
    InsertTimer, InsertLuaTimer, InsertStuckTo, ReleaseStuckTo,
    AddForce, SetForceEnabled, FreezeEntity, UnfreezeEntity, SetSpeed
}

SpawnCmd { position, screen_position, sprite, collider, velocity, friction, max_speed, forces, frozen, ... }

SignalCmd { SetScalar, SetInteger, SetString, SetFlag, ClearFlag }
AudioCmd { PlayMusic, PlaySound, StopAllMusic, StopAllSounds }
GroupCmd { TrackGroup, UntrackGroup, ClearTrackedGroups }
PhaseCmd { Transition { entity_id, phase } }
CameraCmd { SetCamera { target_x, target_y, offset_x, offset_y, rotation, zoom } }
TilemapCmd { SpawnTiles { tilemap_key } }
AssetCmd { LoadTexture, LoadFont, LoadMusic, LoadSound, LoadTilemap }
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

-- Entity Commands
engine.entity_set_position(id, x, y)
engine.entity_set_velocity(id, vx, vy)
engine.entity_set_rotation(id, deg)
engine.entity_set_scale(id, sx, sy)
engine.entity_despawn(id)
engine.entity_signal_set_flag/clear_flag/set_integer/set_scalar/set_string(id, key, value?)
engine.entity_insert_timer(id, duration, signal)
engine.entity_insert_lua_timer(id, duration, callback)
engine.entity_remove_lua_timer(id)
engine.entity_insert_tween_position/rotation/scale(id, from..., to..., duration, easing, loop_mode)
engine.entity_remove_tween_position/rotation/scale(id)
engine.entity_insert_stuckto(id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)
engine.release_stuckto(id)
engine.entity_set_animation(id, key)
engine.entity_restart_animation(id)
engine.entity_add_force(id, name, x, y, enabled)
engine.entity_remove_force(id, name)
engine.entity_set_force_enabled(id, name, enabled)
engine.entity_set_force_value(id, name, x, y)
engine.entity_set_friction(id, friction)
engine.entity_set_max_speed(id, max_speed|nil)
engine.entity_freeze(id)
engine.entity_unfreeze(id)
engine.entity_set_speed(id, speed)

-- Collision Commands (inside collision callbacks)
engine.collision_play_sound(id)
engine.collision_set_integer/set_flag/clear_flag(key, value?)
engine.collision_spawn() -> builder
engine.collision_phase_transition(id, phase)
engine.collision_entity_freeze/unfreeze(id)
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
engine.spawn_tiles(tilemap_key)

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
:with_frozen(frozen)
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

1. phase_detect_transitions
2. phase_update_current
3. spawn_menu_entities
4. grid_layout_system
5. poll_input
6. acceleration_controller_system
7. simple_controller_system
8. audio_system
9. mouse_controller_system
10. run_collision_system
11. lua_phase_system
12. animation_controller_system
13. animation_system
14. world_time_system
15. timer_system
16. lua_timer_system
17. signal_binding_system
18. update_group_count_system
19. run_<scene>_update (via game.rs)
20. movement_system
21. stuckto_system
22. render_system

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
- Collision: collision.rs (systems), collision.rs (components)
- Rendering: render.rs, sprite.rs
- Animation: animation.rs, animation_controller.rs, animationstore.rs
- State machines: luaphase.rs (component + system)
- Signals: signals.rs, worldsignals.rs

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

1. Non-send resources (FontStore, LuaRuntime) need NonSend/NonSendMut
2. Collision callbacks have separate command queue (CollisionEntityCmd)
3. SpawnCmd processed in lua_commands.rs, not inline
4. Entity IDs are u64 in Lua (Entity::to_bits)
5. Raylib Vector2 methods differ from other math libs
6. Signals component auto-created if using :with_signal_* builders
7. Phase callbacks receive entity_id as first arg
8. Timer callbacks receive entity_id as first arg
9. Animation controller evaluates rules in order, first match wins
10. Frozen entities skip movement but still render
