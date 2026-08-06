# Aberred Engine - Lua Interface Architecture

This document describes the Lua scripting interface architecture and provides a guide for developers who want to add new Lua commands to interact with ECS components.

All code described here lives in the `aberred-lua` crate (`crates/aberred-lua/`), the workspace member dedicated to the optional Lua scripting layer (`lua` feature). `aberred-lua` depends on `aberred-core` for ECS components/resources/protocol types; it never depends on `aberred-render` or `aberred-audio`.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Module Structure](#module-structure)
3. [Command Flow: Lua to ECS](#command-flow-lua-to-ecs)
4. [Scene Lifecycle and Callback Dispatch](#scene-lifecycle-and-callback-dispatch)
5. [Command Types and Queues](#command-types-and-queues)
6. [Entity Builder Pattern](#entity-builder-pattern)
7. [Signal Keys Vocabulary](#signal-keys-vocabulary)
8. [Camera Follow System](#camera-follow-system)
9. [Input Rebinding](#input-rebinding)
10. [Parent-Child Hierarchy](#parent-child-hierarchy)
11. [Signal Snapshot System](#signal-snapshot-system)
12. [Context Table Pooling](#context-table-pooling)
13. [Meta Schema (`engine.__meta`)](#meta-schema-enginemeta)
14. [How to Add New Lua Commands](#how-to-add-new-lua-commands)
15. [Best Practices](#best-practices)

---

## Architecture Overview

The Aberred Engine uses a **deferred command pattern** for Lua-Rust integration. Lua scripts cannot directly modify ECS entities—instead, they queue commands that are processed by Rust systems after Lua callbacks return.

### High-Level Flow

```text
┌───────────────────────────────────────────────────────────────────────────────┐
│                             GAME LOOP                                         │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐         │
│   │   Lua Script    │───▶│  Command Queue  │───▶│  Rust Systems   │         │
│   │                 │     │  (LuaAppData)   │     │  (process_*)    │         │
│   └─────────────────┘     └─────────────────┘     └─────────────────┘         │
│          │                       │                      │                     │
│          │ engine.spawn()        │ SpawnCmd             │ Commands.spawn()    │
│          │ engine.set_flag()     │ SignalCmd            │ world_signals.set   │
│          │ engine.despawn()      │ EntityCmd            │ entity.despawn()    │
│          ▼                       ▼                      ▼                     │
│   ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐         │
│   │ Signal Snapshot │◀───│  WorldSignals   │◀───│   ECS World     │         │
│   │   (read-only)   │     │   (Resource)    │     │                 │         │
│   └─────────────────┘     └─────────────────┘     └─────────────────┘         │
│                                                                               │
└───────────────────────────────────────────────────────────────────────────────┘
```

### Why Deferred Commands?

1. **Thread Safety**: Lua is single-threaded; direct ECS access would require complex synchronization.
2. **Consistency**: Commands are processed at predictable points in the game loop.
3. **Error Handling**: Commands can be validated and errors reported cleanly.
4. **Performance**: Batch processing of commands is more efficient than immediate execution.

---

## Module Structure

The Lua runtime is organized in `crates/aberred-lua/src/resources/lua_runtime/`:

```text
crates/aberred-lua/src/resources/lua_runtime/
├── mod.rs              # Public exports
├── runtime.rs          # LuaRuntime struct, LuaAppData (queue fields generated via lua_queues!), pool types, GameConfigSnapshot, CameraSnapshot
├── entity_builder/      # LuaEntityBuilder fluent API, split by category; builder_method! macro (single source for runtime + stubs)
│   ├── mod.rs           # LuaEntityBuilder struct, builder_method!/DummyMethods, register_methods() dispatch, register_as/build
│   ├── transform.rs     # with_position, with_screen_position, with_rotation, with_scale, with_parent, with_stuckto*, with_camera_target, ...
│   ├── physics.rs       # with_velocity, with_accel, with_friction, with_max_speed, with_frozen, with_collider*
│   ├── sprite.rs        # with_sprite*, with_tint, with_shadow, with_animation*, with_zindex
│   ├── gui.rs            # with_gui_window/button/label/image/progress_bar/offset/theme_key and their per-state offset variants
│   ├── menu.rs           # with_menu and its with_menu_* configuration methods
│   ├── tween.rs          # with_tween_position/rotation/scale/screen_position and their *_easing/*_loop/*_backwards/*_on_finished variants
│   └── behavior.rs       # with_phase, with_lua_timer, with_lua_collision_rule, with_lua_setup, with_on_animation_end, with_signal*, with_group, with_persistent, with_grid_layout, with_tilemap, with_particle_emitter, with_mouse_controlled, with_text
├── engine_api/          # engine.* API registration, split by category
│   ├── mod.rs          # Re-exports, module declarations
│   ├── macros.rs       # register_cmd!, register_getter!, define_cmd_twins! (+ its define_*_cmd_twins!/define_entity_cmds! specializations), push_fn_meta()
│   ├── animation.rs    # register_animation_api()
│   ├── assets.rs       # register_asset_api()
│   ├── audio.rs        # register_audio_api()
│   ├── base.rs         # register_base_api() (logging)
│   ├── camera.rs       # register_camera_api(), register_camera_follow_api()
│   ├── entity.rs       # register_entity_api(), register_collision_api()
│   ├── gameconfig.rs   # register_gameconfig_api()
│   ├── input.rs        # register_input_api()
│   ├── phase_group.rs  # register_phase_api(), register_group_api()
│   ├── render.rs       # register_render_api() (post-process shaders + GUI theme configuration)
│   ├── signal.rs       # register_signal_api() (also change_scene/quit — see Signal Keys Vocabulary)
│   └── spawn.rs        # register_spawn_api()
├── queue_registry.rs   # lua_queues! macro: authoritative list of all 23 command queues
├── command_queues.rs   # drain_*_commands() methods (generated), clear_all_commands (generated), cache updates
├── stub_meta.rs        # Type/enum/callback metadata; builder meta delegated to entity_builder/mod.rs
├── commands.rs         # Command enums (EntityCmd, SignalCmd, CameraFollowCmd, InputCmd, etc.)
├── context.rs          # Entity context builder for Lua callbacks (pooled), snapshot types
├── input_snapshot.rs   # InputSnapshot, DigitalInputs, AnalogInputs for Lua callbacks
└── spawn_data.rs       # Data structures for spawn configuration (SpawnCmd, component data structs)
```

Signal key constants live in `aberred-core`, since Rust-side gameplay code that has no Lua dependency also reads/writes them:

```text
crates/aberred-core/src/resources/signal_keys.rs   # pub const SWITCH_SCENE, QUIT_GAME, SCENE, MOVING, SPEED_SQ, etc.
```

Command processing lives in a separate submodule:

```text
crates/aberred-lua/src/systems/lua_commands/
├── mod.rs              # Re-exports; EntityCmdQueries/ContextQueries SystemParams; EffectCmdBufs/DrainScope; drain_and_process_effect_commands/drain_and_process_phase_commands; build_tween/apply_tween_finished_callback helpers
├── context.rs          # build_entity_context: gathers ECS data → pooled Lua ctx table
├── dispatch.rs          # LuaDispatch SystemParam bundle + call_entity_callback/dispatch_and_drain/drain_dispatch_commands: shared entity-callback dispatch flow used by timer/on_animation_end/on_tween_finished observers
├── entity_cmd.rs        # process_entity_commands: runtime entity manipulation (physics, signals, tweens, shaders, hierarchy, GUI)
├── processors.rs        # small per-command-domain process_* functions (signal, camera, camera_follow, audio, phase, gameconfig, input, group, render, animation) and asset-command translation helpers
├── spawn_cmd.rs         # process_spawn_command, process_clone_command: entity creation via apply_components()
└── parse.rs             # Animation condition parsing helpers
```

`crates/aberred-lua/src/systems/` also holds the Bevy systems/observers that call into `lua_commands/` (`luaphase.rs`, `luatimer.rs`, `lua_setup_entity.rs`, `lua_animation_finished.rs`, `lua_tween_finished.rs`, `lua_collision.rs`, `lua_collision_rule_index.rs`, `lua_menu.rs`, `lua_mapspawn.rs`, `lua_gui_interactable_click.rs`) and `lua_plugin.rs` (scene setup/switch, `on_update_<scene>` dispatch) at the crate root.

### Key Components

#### `LuaRuntime` (runtime.rs)

The main struct managing the Lua interpreter. It:

- Initializes the Lua state with MLua
- Delegates API registration to `register_*_api()` methods — one call per category, all chained in `LuaRuntime::new()`
- Manages `LuaAppData` for command queuing
- Manages **context table pools** for collision, entity, and input callbacks (see [Context Table Pooling](#context-table-pooling))
- Provides `get_function()`/`get_function_cached()` to resolve global Lua functions by name, and `call_named()`/`call_resolved()` to invoke them with unified not-found/error logging

#### `engine_api/` directory

Contains all `engine` table API registration, split by category. Each category file defines one `register_*_api()` method on `LuaRuntime`. The shared macros are in `macros.rs`:

- `register_cmd!` — registers a Lua function that pushes a command to a queue, with metadata. Its argument grammar is `|$args:pat_param| $arg_ty:ty` (type *after* the closing pipe) rather than the more familiar `|$args: $arg_ty|` — `macro_rules!`'s follow-set restrictions on `pat_param` fragments make the familiar shape inexpressible here (see the macro's own doc comment in `macros.rs` for the full reasoning).
- `register_getter!` — registers an `engine.*` function plus its `__meta` entry for closures that do *not* push to a command queue: read-only lookups against `LuaAppData`'s caches, or plain computations with no `LuaAppData` access at all (`spawn()`/`clone()`'s builder constructors). Unlike `register_cmd!`, the caller supplies the whole closure with ordinary Rust closure syntax — no macro-imposed argument grammar.
- `define_cmd_twins!` — registers a declarative list of commands under one queue/category, with each function name prefixed by `$prefix` and each description suffixed by `$desc_suffix`; used to define a regular and collision-scoped variant from a single list. `define_signal_cmd_twins!`, `define_camera_cmd_twins!`, `define_audio_cmd_twins!`, `define_phase_cmd_twins!`, and `define_entity_cmds!` are specializations of it for their respective command categories.

And one helper function:

- `push_fn_meta()` — pushes function metadata to `engine.__meta.functions` (used for manually registered functions that don't go through `register_cmd!`/`register_getter!`)

#### `queue_registry.rs` — the authoritative queue list

Defines the `lua_queues!` macro, the **single authoritative source** for all 23 command queues. Expanding the macro with different modes generates:

- `lua_queues!{drain_methods}` — all 23 `drain_*_into()` methods (used in `command_queues.rs`)
- `lua_queues!{clear_body data}` — the body of `clear_all_commands`, which clears every queue whose row is tagged `clear` and leaves `preserve`-tagged queues untouched
- `lua_queues!{app_data_struct { ... }}` — `LuaAppData`'s full field list: one `RefCell<Vec<T>>` per queue row, spliced together with the caller-supplied non-queue cache fields (`runtime.rs`)

To add a new queue you need exactly **one** edit: add a `(field_name, CmdType, clear_policy)` row to the `@master` arm in `queue_registry.rs`. `clear_policy` is `clear` (the default — for queues whose commands may reference entities about to be despawned on scene switch) or `preserve` (for scene-agnostic queues whose only drain site runs after `switch_scene`, e.g. `map_commands`/`asset_commands`/`gui_theme_commands`). Drain methods, `clear_all_commands`'s body, and `LuaAppData`'s queue fields are all generated automatically from that one row.

#### `command_queues.rs`

Contains all `drain_*_commands_into()` methods (generated by `lua_queues!{drain_methods}`), `clear_all_commands` (body generated by `lua_queues!{clear_body data}`), and cache update functions (`update_signal_cache`, `update_bindings_cache`, etc.).

#### `LuaAppData` (runtime.rs)

Internal shared state accessible from Lua closures. Its 23 queue fields are generated by `crate::lua_queues!{app_data_struct { ... }}` from the row list in `queue_registry.rs`, in the same order — the struct definition itself doesn't list them; only the non-queue cache fields are spelled out:

```rust
crate::lua_queues! {app_data_struct {
    // Read-only caches — updated before each Lua callback
    pub(super) signal_snapshot: RefCell<Arc<SignalSnapshot>>,
    pub(super) tracked_groups: RefCell<FxHashSet<String>>,
    pub(super) gameconfig_snapshot: RefCell<GameConfigSnapshot>,
    pub(super) bindings_snapshot: RefCell<std::collections::HashMap<String, String>>,
    pub(super) camera_snapshot: RefCell<CameraSnapshot>,
    // Resolved Lua function handles, cached by global name; cleared on scene switch
    pub(super) function_cache: RefCell<FxHashMap<String, LuaFunction>>,
    // Frame number + last InputSnapshot written to the pooled input table
    pub(super) last_input: RefCell<Option<(u64, InputSnapshot)>>,
}}
```

The 23 generated queue fields, in `queue_registry.rs`'s row order: `asset_commands` (preserve), `spawn_commands`, `audio_commands`, `signal_commands`, `phase_commands`, `entity_commands`, `group_commands`, `camera_commands`, `animation_commands`, `render_commands`, `gui_theme_commands` (preserve, `RenderCmd`), `clone_commands`, `gameconfig_commands`, `camera_follow_commands`, `input_commands`, `map_commands` (preserve), then the 7 collision-scoped queues: `collision_entity_commands`, `collision_signal_commands`, `collision_audio_commands`, `collision_spawn_commands`, `collision_clone_commands`, `collision_phase_commands`, `collision_camera_commands`.

#### Command Enums (commands.rs)

Each command type is a Rust enum that encapsulates all data needed to perform an operation. See [Command Types and Queues](#command-types-and-queues) for the full list.

---

## Command Flow: Lua to ECS

### Step 1: Lua Calls Engine API

```lua
-- In a Lua script
engine.entity_set_velocity(ball_id, new_vx, new_vy)
engine.set_flag("switch_scene")
```

### Step 2: Command is Queued

Most Lua functions are registered via the `register_cmd!` macro (or a `define_*_cmd_twins!` specialization built on top of it), which generates the closure, pushes to the correct queue, and registers metadata in `engine.__meta` — all in one declaration:

```rust
// In engine_api/signal.rs — define_signal_cmd_twins! entry (typical pattern)
(
    "set_scalar",
    |(key, value)| (String, f32),
    SignalCmd::SetScalar { key, value },
    desc = "Set a world signal scalar value",
    params = [("key", "string"), ("value", "number")]
),
```

Entity commands are registered in bulk via `define_entity_cmds!` in `engine_api/entity.rs`. A single definition under `define_entity_cmds!` is invoked twice — once with `""` prefix for regular commands and once with `"collision_"` for collision commands:

```rust
// In engine_api/entity.rs
define_entity_cmds!(engine, self.lua, meta_fns, "", entity_commands);
define_entity_cmds!(engine, self.lua, meta_fns, "collision_", collision_entity_commands);
```

For functions with non-push logic (reads, builders, validation), registration uses `register_getter!`:

```rust
// In engine_api/signal.rs
register_getter!(engine, self.lua, meta_fns, "get_scalar",
    |lua, key: LuaString| {
        let key = key.to_str()?;
        Ok(lua
            .app_data_ref::<LuaAppData>()
            .and_then(|data| data.signal_snapshot.borrow().scalars.get(&*key).copied()))
    },
    desc = "Get a world signal scalar value", cat = "signal",
    params = [("key", "string")], returns = "number?");
```

A handful of functions with irregular shapes (`change_scene`, `quit`, `set_target_fps`, `set_render_size`, `set_background_color`) are still registered manually with `engine.set()` + a `push_fn_meta()` call, when neither macro's grammar fits cleanly.

### Step 3: Rust Drains Commands

Every Lua entity callback (phase, timer, `on_animation_end`, `on_tween_finished`) goes through the shared `LuaDispatch` flow in `systems/lua_commands/dispatch.rs`: `refresh_signal_cache` syncs the signal cache from `WorldSignals`, `call_entity_callback` builds the entity context (and input table, if the call shape wants one) and invokes the named Lua function, then `drain_dispatch_commands` drains and processes its queued commands. `lua_phase_system` (`systems/luaphase.rs`) does not use `LuaDispatch` — phase transitions must interleave `apply_callback_transitions` between the phase drain and the effect drain, and it builds one input table for many entities per invocation, so it calls `drain_and_process_phase_commands`/`drain_and_process_effect_commands` directly instead.

The actual draining happens in `drain_and_process_effect_commands()` (`systems/lua_commands/mod.rs`), which drains the 6 non-phase effect queues in canonical order — `signal → entity → spawn → clone → audio → camera` — from either the regular or collision-scoped queue set (`DrainScope::Regular`/`DrainScope::Collision`), into a caller-owned `EffectCmdBufs` (a `Local<EffectCmdBufs>` on the calling system, so its `Vec`s retain heap capacity across frames):

```rust
// In systems/lua_commands/mod.rs
pub(crate) fn drain_and_process_effect_commands(
    lua_runtime: &LuaRuntime,
    scope: DrainScope,
    bufs: &mut EffectCmdBufs,
    commands: &mut Commands,
    world_signals: &mut WorldSignals,
    cmd_queries: &mut EntityCmdQueries,
    audio: &mut MessageWriter<AudioCmd>,
    systems_store: &SystemsStore,
    animation_store: &AnimationStore,
) { ... }
```

`lua_plugin.rs`'s `update()`/`switch_scene()` call this same helper (via their own `CommonCmdBufs`/`EffectCmdBufs` locals) for the queues that aren't tied to a single dispatched entity callback (`on_update_<scene>`, `on_switch_scene`, etc.).

### Step 4: Commands are Processed

The processing functions in `systems/lua_commands/` apply changes to the ECS. `process_entity_commands` takes an `EntityCmdQueries` SystemParam bundle and dispatches to sub-functions:

```rust
// In lua_commands/entity_cmd.rs
pub fn process_entity_commands(
    commands: &mut Commands,
    entity_commands: impl IntoIterator<Item = EntityCmd>,
    world_signals: &mut WorldSignals,
    cmd_queries: &mut EntityCmdQueries,
    systems_store: &SystemsStore,
    anim_store: &AnimationStore,
) { ... }
```

Spawn and clone commands are processed via `process_spawn_command()` and `process_clone_command()` in `lua_commands/spawn_cmd.rs`. Both delegate to the shared `apply_components()` helper. The remaining command categories (signal, camera, camera follow, audio, phase, game config, input, group, render, animation, asset translation) each have a small dedicated `process_*`/`translate_*` function in `lua_commands/processors.rs`.

---

## Scene Lifecycle and Callback Dispatch

The previous section covers how Lua calls *into* Rust. This section covers the other direction: how and when the engine calls *into* Lua. Every invocation goes through `LuaRuntime::call_named`/`call_resolved` (which logs a warning if the named function is missing, or an error if it throws) or, for entity-scoped callbacks, through the shared `LuaDispatch` flow introduced in [Command Flow: Lua to ECS](#command-flow-lua-to-ecs).

### Scene Lifecycle

`lua_plugin.rs` drives four lifecycle points, each its own Bevy system:

1. **`setup()`** — runs once at startup. Calls `on_setup()` if defined, drains `asset_commands` (translating each into an `AudioCmd` or a `RenderAssetCmd`), drains `animation_commands` into a fresh `AnimationStore` resource, then transitions `GameState` to `Playing`.
2. **`enter_play()`** — runs once, immediately after `setup()`. Calls `on_enter_play()` if defined, drains `signal_commands`/`group_commands` (world signals aren't touched by `setup()` — Lua is expected to seed them here), updates the tracked-groups cache, then runs the `switch_scene` system hook to spawn the initial scene.
3. **`switch_scene()`** — runs whenever `WorldSignals`' `switch_scene` flag is taken (set by `engine.change_scene()`), both from `enter_play()`'s initial call and from `update()` mid-game. It: clears every `clear`-policy command queue and the cached-function-handle table (`clear_all_commands()`/`clear_function_cache()` — callbacks are re-injected per scene, so stale closures must not survive the switch), despawns every non-`Persistent` `CleanableEntity`, clears non-persistent `WorldSignals` entity registrations and group counts, refreshes the signal cache so `on_switch_scene` observes the post-clear state (not a stale pre-clear snapshot), calls `on_switch_scene(scene_name)`, then drains the same "common" queue set `update()` does (`drain_common_commands`, below).
4. **`update()`** — runs once per sim tick. Refreshes the signal/gameconfig/camera/bindings caches, resolves the pooled input table, calls `on_update_<scene>(input, dt)` (the callback name is cached as `"on_update_" .. scene` and only rebuilt when the scene string actually changes), drains the common queues, then checks the `quit_game`/`switch_scene` flags Lua may have just set. A same-tick `engine.change_scene()` call runs `switch_scene()` synchronously before `update()` returns — there is no one-tick lag between requesting a switch and it taking effect.

`update()` and `switch_scene()` share `drain_common_commands()`, which drains, in order: `animation_commands` first (so a same-batch `entity_set_animation`/`entity_restart_animation` can resolve a texture key registered earlier in the same batch), phase commands, the 6 regular effect queues (via `drain_and_process_effect_commands`), `render_commands` + `gui_theme_commands` (merged into one `GuiThemeStore` clone-and-write-back, only performed when at least one was non-empty — this avoids marking `GuiThemeStore` "changed" on frames with no theme edits — and re-validated afterward for every staged theme's missing `normal`/`fill` skin), `gameconfig_commands`, `camera_follow_commands`, `input_commands`, and `group_commands`.

`process_lua_asset_commands` is the always-live drain site for `engine.load_*` calls made *after* `setup()` — `on_update_<scene>`, `on_switch_scene`, and every phase/timer/collision callback all queue into the same `asset_commands` queue; this system (registered on the sim schedule) is what actually translates and forwards them once `setup()`'s own one-shot local buffer is no longer in the picture.

### Phase System

`lua_phase_system` runs once per sim tick over every `LuaPhase` entity (`LuaPhase` is the generic `Phase<C>` component specialized over `PhaseCallbacks` — named Lua function strings instead of Rust fn pointers). Per entity, in phase-lifecycle order:

1. If `needs_enter_callback` is set (freshly spawned, or a transition just completed), call `phase_on_enter` — `(ctx, input)`, with `ctx.previous_phase` populated.
2. If a transition to a new phase is pending, call the *old* phase's `phase_on_exit` — `(ctx)` only, no `input` — then swap `current`/`previous`, reset `time_in_phase` to 0, and call the *new* phase's `phase_on_enter`.
3. Call `phase_on_update` — `(ctx, input, dt)`.
4. `time_in_phase` accumulates by `dt` regardless of whether a callback ran.

`phase_on_enter`/`phase_on_update` may return a phase-name string to request a transition (a same-name or `nil` return is a no-op); `engine.phase_transition()` is the other way to request one, queued and drained separately. Return-value transitions are applied *after* the phase-command drain, so they take precedence within the same tick over an `engine.phase_transition()` call made in the same callback.

This system deliberately does not go through the shared `LuaDispatch`/`dispatch_and_drain` helper that timer/animation/tween callbacks use: `apply_callback_transitions` (the return-value transition step) must run strictly between the phase-command drain and the effect-command drain, and the system batches one input table across every phase entity per invocation rather than resolving it per entity — so it calls `drain_and_process_phase_commands`/`drain_and_process_effect_commands` directly instead.

### Timer System

`update_lua_timers` accumulates `dt` on every `LuaTimer` component (`Timer<C>` specialized over `LuaTimerCallback`, mirroring `LuaPhase`'s relationship to `Phase<C>`). When `elapsed >= duration`, it fires a `LuaTimerEvent` and resets by subtracting `duration` (not zeroing) — the timer never self-removes, so a "fire once" callback must call `engine.entity_remove_lua_timer()` on itself. `lua_timer_observer` reacts to `LuaTimerEvent` via `LuaDispatch::dispatch_and_drain`, calling the named function as `(ctx, input)`.

### Collision System

`lua_collision_observer` reacts to `CollisionEvent`, raised by the shared, Lua-agnostic `collision_detector` in `aberred-core`. For each event it: looks up the two entities' `Group` names in `CollisionRuleIndex` (a bucket index over every `LuaCollisionRule`-bearing entity's `(group_a, group_b)` pair, maintained by `lua_collision_rule_index.rs`) to find a matching rule; builds both sides' pooled collision context — `id`, `group`, `pos`, `vel`, `speed_sq`, `rect`, `signals` (each optional field sparse-populated the same way the entity ctx is, see [Context Table Pooling](#context-table-pooling)) — plus `ctx.sides.a`/`ctx.sides.b` string arrays (`"left"`/`"right"`/`"top"`/`"bottom"`); calls the rule's named callback as `(ctx)` only — collision callbacks receive **no `input` argument**, unlike every other callback kind in this section; then drains the phase queue and the 6 effect queues from the **collision-scoped** buffers (`DrainScope::Collision`) immediately, not batched with the rest of the tick — collision response (position/velocity corrections) must land before the next collision in the same tick is detected. The signal cache is only refreshed when `WorldSignals` is actually dirty, so a collision-heavy frame with no signal writes between collisions skips the snapshot clone entirely.

### Animation-Finished / Tween-Finished Callbacks

`lua_animation_finished_observer` reacts to `AnimationFinishedEvent` (raised once, when a non-looped `Animation` first reaches its last frame) for entities carrying `LuaOnAnimationEnd`. `lua_tween_finished_observer::<T>` — one monomorphized instance registered per tweened type (`MapPosition`, `Rotation`, `Scale`, `ScreenPosition`) — reacts to `TweenFinishedEvent<T>` for entities carrying the matching `LuaOnTweenFinished<T>`. Both call the named callback as `(ctx, input)` via `LuaDispatch::dispatch_and_drain`, and both are silently skipped for entities that don't carry the matching component.

### Entity Setup Callback

`lua_setup_entity_system` reacts to every entity that gains a `LuaSetup` component (`Added<LuaSetup>`) and calls its named function once with `(ctx)` only — no `input` argument (`CallShape::CtxOnly`). It calls `LuaDispatch::call_entity_callback` directly rather than `dispatch_and_drain`, since it refreshes the signal cache once and drains commands once across the whole `Added<LuaSetup>` batch for the tick, instead of once per entity. It runs before `animation_controller` in the sim schedule's `SimSet::PostCollision`, so a setup callback can set animation state the same tick the entity is spawned.

### Menu Selection and GUI Interactable Click Dispatch

`menu_selection_observer` (`lua_menu.rs`) and `gui_interactable_click_observer` (`lua_gui_interactable_click.rs`) both shadow an `aberred-core` Rust-only equivalent with a three-tier priority chain: a Lua callback name (`Menu.on_select_callback`, set via `:with_menu_callback()`; or `GuiInteractable.on_click_callback`, set via the `callback_name` argument to `with_gui_button`/`with_gui_image`) wins if present and resolvable; otherwise a Rust fn-pointer callback (`Menu.on_rust_callback` / `GuiInteractable.on_rust_callback`); otherwise, menu-only, `MenuActions`. Both build a small ad-hoc context table directly rather than going through the pooled `EntityCtxTables`/`CollisionCtxTables` (these fire far less often than phase/timer/collision callbacks, so pooling isn't worth the complexity) — the menu callback's table carries `menu_id`/`item_id`/`item_index`; the GUI interactable callback's carries just `entity_id` — and calls the named function with that single table as its only argument (no `input`).

### Stub and `.luarc.json` Generation

`stub_generator.rs` (`generate_stubs`/`write_stubs`) reads every `engine.__meta` table (functions, classes, types, enums, callbacks) and renders `assets/scripts/engine.lua` — an EmmyLua-annotated stub file consumed by Lua language servers for autocomplete and type-checking, with functions grouped by category via `CATEGORY_ORDER`. `luarc_generator.rs` (`generate_luarc`/`write_luarc`) renders `.luarc.json`, the LSP workspace config that points a language server at that stub file. Both run only via the facade binary's CLI flags (`cargo run -- --create-lua-stubs` / `--create-luarc`) — never at engine startup, and their output is never hand-edited (see [Best Practices](#best-practices)).

---

## Command Types and Queues

### Regular vs Collision Queues

The engine maintains **two sets** of command queues:

| Queue Type | When Processed | Use Case |
| ---------- | -------------- | -------- |
| Regular (`entity_commands`, etc.) | After phase/timer/update callbacks | Normal game logic |
| Collision (`collision_entity_commands`, etc.) | Immediately after each collision callback | Collision response |

This distinction matters because collision callbacks need immediate processing to ensure position corrections and velocity changes happen before the next collision is detected.

### Command Categories

| Category | Enum | Purpose |
| -------- | ---- | ------- |
| **Entity** | `EntityCmd` | Manipulate existing entities (velocity, position, signals, shaders, tweens, hierarchy, camera target, GUI widget state) |
| **Spawn** | `SpawnCmd` | Create new entities with components (boxed — `SpawnCmd` is ~2KB, so queues hold `Box<SpawnCmd>`) |
| **Clone** | `CloneCmd` | Clone an entity registered in WorldSignals and apply builder overrides |
| **Signal** | `SignalCmd` | Modify global WorldSignals |
| **Audio** | `AudioLuaCmd` | Play/stop music and sounds (with optional pitch) |
| **Phase** | `PhaseCmd` | Trigger state machine transitions |
| **Camera** | `CameraCmd` | Set 2D camera target/offset/rotation/zoom directly |
| **CameraFollow** | `CameraFollowCmd` | Configure the camera follow system (mode, speed, zoom_lerp_speed, bounds, deadzone) |
| **Asset** | `AssetCmd` | Load textures, fonts, music, sounds, maps (setup only) |
| **Group** | `GroupCmd` | Manage tracked entity groups |
| **Map** | `MapLuaCmd` | Load a map JSON file (spawns its assets and entities) |
| **Animation** | `AnimationCmd` | Register animation definitions |
| **Render** | `RenderCmd` | Configure post-process shaders and uniforms; also carries GUI theme configuration (`gui_theme_commands` queue uses this same enum) |
| **GameConfig** | `GameConfigCmd` | Runtime game settings (fullscreen, vsync, FPS, render size, background color, pixel-snap camera, render-target filter) |
| **Input** | `InputCmd` | Runtime input rebinding (rebind action, add binding) |

In addition to the regular queues, most write APIs have a collision-scoped variant (prefixed with `collision_` or `collision_entity_`) that queues into collision-specific buffers.

`render_commands` (clear policy) and `gui_theme_commands` (preserve policy) both carry `RenderCmd` values but are drained separately — `gui_theme_commands` survives `clear_all_commands` (called at the start of `switch_scene`) so `engine.set_gui_theme_*` calls queued from `on_setup()` aren't lost before their first drain.

---

## Current Lua API Index

This section is meant to stay in sync with the actual implementation.

- Source of truth for `engine.*`: `crates/aberred-lua/src/resources/lua_runtime/engine_api/` (each `register_*_api()` method)
- Source of truth for `engine.spawn()/engine.clone()` builder methods: `crates/aberred-lua/src/resources/lua_runtime/entity_builder/`

### `engine` Table Functions

#### Logging

- `log`, `log_info`, `log_warn`, `log_error`, `log_debug`

#### Assets

- `load_texture`, `load_font`, `load_music`, `load_sound`, `load_map`

`load_texture`'s `filter` parameter is one of `"nearest"` (default), `"bilinear"`, `"trilinear"`, `"anisotropic_4x"`, `"anisotropic_8x"`, `"anisotropic_16x"`. `load_map` loads a map JSON file and spawns all its assets and entities (replaces the older per-tile spawn call). Tilemap loading for a single entity goes through the entity builder's `:with_tilemap(path)` instead of an `engine.*` function.

#### Spawning / Cloning

- `spawn`, `clone`

#### Audio

- `play_music`, `play_sound`, `play_sound_pitched`
- `pause_music`, `resume_music`, `stop_music`, `stop_all_music`
- `stop_all_sounds`
- `set_music_volume`
- `unload_music`, `unload_all_music`, `unload_sound`, `unload_all_sounds`

`stop_*`/`stop_all_*` stop and drain active playback but leave the loaded asset data intact for reuse; `unload_*`/`unload_all_*` destroy the loaded data entirely. Use stop for between-level resets; use unload when freeing assets for good.

#### Navigation

- `change_scene`, `quit`

Both are registered in `engine_api/signal.rs`: `change_scene(scene_name)` sets the `scene` string signal and the `switch_scene` flag; `quit()` sets the `quit_game` flag. See [Signal Keys Vocabulary](#signal-keys-vocabulary).

#### Global Signals (read)

- `get_scalar`, `get_integer`, `get_string`, `has_flag`, `get_group_count`, `get_entity`
- `get_scalars`, `get_integers`, `get_strings`, `get_flags` — bulk snapshot reads (return every current key/value as a table)

#### Global Signals (write)

- `set_scalar`, `set_integer`, `set_string`, `set_flag`
- `clear_scalar`, `clear_integer`, `clear_string`, `clear_flag`
- `toggle_flag`
- `set_entity`, `remove_entity`

#### Groups

- `track_group`, `untrack_group`, `clear_tracked_groups`, `has_tracked_group`

#### Phase / Map / Animation

- `phase_transition`
- `register_animation`

#### Camera

- `set_camera`, `get_camera`, `get_camera_view_rect`

#### Camera Follow

- `camera_follow_enable`, `camera_follow_set_mode`, `camera_follow_set_deadzone`
- `camera_follow_set_easing`, `camera_follow_set_speed`, `camera_follow_set_spring`
- `camera_follow_set_offset`, `camera_follow_set_bounds`, `camera_follow_clear_bounds`
- `camera_follow_reset_velocity`, `camera_follow_set_zoom_speed`

#### Game Config

- `set_fullscreen`, `get_fullscreen`
- `set_vsync`, `get_vsync`
- `set_target_fps`, `get_target_fps`
- `set_render_size`, `get_render_size`
- `set_background_color`, `get_background_color`
- `set_pixel_snap_camera`, `get_pixel_snap_camera`
- `set_render_target_filter`

#### Input Rebinding

- `rebind_action`, `add_binding`, `get_binding`

#### Post-Process Shaders

- `load_shader`
- `post_process_shader`
- `post_process_set_float`, `post_process_set_int`, `post_process_set_vec2`, `post_process_set_vec4`
- `post_process_clear_uniform`, `post_process_clear_uniforms`

#### GUI Theming

- `set_gui_theme_panel` — the theme's `GuiWindow` nine-patch panel texture/region/borders
- `set_gui_theme_button` — one button-state nine-patch skin per call (`"normal"`/`"hover"`/`"pressed"`/`"disabled"`)
- `set_gui_theme_button_shadow` — drop shadow for one state of the theme's button skin
- `set_gui_theme_label` — the theme's `GuiLabel` nine-patch panel
- `set_gui_theme_progress_bar` — one part (`"track"`/`"fill"`) of the theme's progress-bar skin
- `set_gui_theme_font` — caption font/size/color, used by every `GuiButton`/`GuiLabel` caption referencing the theme
- `set_gui_theme_panel_shadow` — panel drop shadow shared by all nine-patch backgrounds (`GuiWindow`, `GuiButton`, `GuiLabel`, `GuiProgressBar`)
- `set_gui_theme_text_shadow` — caption text drop shadow, applied to spawned `DynamicText` caption children

All are registered in `engine_api/render.rs` and queue into `gui_theme_commands` (preserve policy — see [Command Types and Queues](#command-types-and-queues)).

#### Entity Commands

- `entity_set_position`, `entity_set_screen_position`, `entity_remove_screen_position`
- `entity_set_velocity`, `entity_set_speed`, `entity_set_rotation`, `entity_set_scale`
- `entity_add_force`, `entity_remove_force`, `entity_set_force_enabled`, `entity_set_force_value`
- `entity_set_friction`, `entity_set_max_speed`
- `entity_freeze`, `entity_unfreeze`
- `entity_set_animation`, `entity_restart_animation`, `entity_set_sprite_flip`
- `entity_insert_lua_timer`, `entity_remove_lua_timer`
- `entity_insert_ttl`
- `entity_insert_tween_position`, `entity_remove_tween_position`
- `entity_insert_tween_rotation`, `entity_remove_tween_rotation`
- `entity_insert_tween_scale`, `entity_remove_tween_scale`
- `entity_insert_tween_screen_position`, `entity_remove_tween_screen_position`
- `entity_insert_stuckto`, `release_stuckto`
- `entity_signal_set_scalar`, `entity_signal_set_integer`, `entity_signal_set_string`, `entity_signal_set_flag`
- `entity_signal_clear_scalar`, `entity_signal_clear_integer`, `entity_signal_clear_string`, `entity_signal_clear_flag`
- `entity_signal_toggle_flag`
- `entity_despawn`, `entity_menu_despawn`
- `entity_set_shader`, `entity_remove_shader`
- `entity_shader_set_float`, `entity_shader_set_int`, `entity_shader_set_vec2`, `entity_shader_set_vec4`
- `entity_shader_clear_uniform`, `entity_shader_clear_uniforms`
- `entity_set_tint`, `entity_remove_tint`
- `entity_set_shadow`, `entity_remove_shadow`
- `entity_set_parent`, `entity_remove_parent`
- `entity_set_camera_target`, `entity_remove_camera_target`
- `entity_set_gui_disabled` — enable/disable a `GuiButton`/`GuiImage` (cosmetic: `gui_hit_test_system` stops promoting it and skips its click callback)
- `entity_set_gui_progress`, `entity_set_gui_progress_max` — set/re-clamp a `GuiProgressBar`'s value

The four `entity_insert_tween_*` commands (`position`/`rotation`/`scale`/`screen_position`) all accept a trailing `on_finished` callback-name string (empty string = no callback); when non-empty, the handler inserts a `LuaOnTweenFinished<T>` component (`T` being the tweened `MapPosition`/`Rotation`/`Scale`/`ScreenPosition`) on the entity, fired once by the corresponding `lua_tween_finished_observer::<T>` when `TweenFinishedEvent<T>` triggers.

#### Collision Context Functions

- `collision_spawn`, `collision_clone`
- `collision_play_sound`, `collision_play_sound_pitched`
- `collision_phase_transition`
- `collision_set_camera`
- `collision_set_scalar`, `collision_set_integer`, `collision_set_string`, `collision_set_flag`
- `collision_toggle_flag`
- `collision_clear_scalar`, `collision_clear_integer`, `collision_clear_string`, `collision_clear_flag`
- `collision_set_entity`, `collision_remove_entity`

#### Collision Entity Commands

All `entity_*` commands have a `collision_entity_*` counterpart (auto-generated via `define_entity_cmds!`). These queue into collision-scoped buffers for immediate processing.

### `LuaEntityBuilder` Methods

The builder returned by `engine.spawn()`, `engine.clone(source_key)`, `engine.collision_spawn()`, and `engine.collision_clone(source_key)` supports these methods, organized by the `entity_builder/` submodule that defines them:

**Lifecycle** (`entity_builder/mod.rs`)

```text
build
register_as
```

**Transform** (`transform.rs`)

```text
with_position
with_screen_position
with_rotation
with_scale
with_parent
with_stuckto
with_stuckto_offset
with_stuckto_stored_velocity
with_camera_target
```

**Physics** (`physics.rs`)

```text
with_velocity
with_accel
with_friction
with_max_speed
with_frozen
with_collider
with_collider_offset
```

**Sprite / visuals** (`sprite.rs`)

```text
with_sprite
with_sprite_flip
with_sprite_offset
with_tint
with_shadow
with_zindex
with_animation
with_animation_controller
with_animation_rule
```

**GUI widgets** (`gui.rs`)

```text
with_gui_window
with_gui_offset
with_gui_theme_key
with_gui_button
with_gui_button_disabled
with_gui_label
with_gui_label_signal_binding
with_gui_label_signal_binding_format
with_gui_image
with_gui_image_hover_offset
with_gui_image_pressed_offset
with_gui_image_disabled_offset
with_gui_progress_bar
with_gui_progress_bar_vertical
with_gui_progress_bar_reversed
with_gui_progress_bar_signal_binding
```

**Menu** (`menu.rs`)

```text
with_menu
with_menu_action_quit
with_menu_action_set_scene
with_menu_action_show_submenu
with_menu_callback
with_menu_colors
with_menu_cursor
with_menu_dynamic_text
with_menu_selection_sound
with_menu_visible_count
```

**Tweens** (`tween.rs`)

```text
with_tween_position
with_tween_position_backwards
with_tween_position_easing
with_tween_position_loop
with_tween_position_on_finished
with_tween_rotation
with_tween_rotation_backwards
with_tween_rotation_easing
with_tween_rotation_loop
with_tween_rotation_on_finished
with_tween_scale
with_tween_scale_backwards
with_tween_scale_easing
with_tween_scale_loop
with_tween_scale_on_finished
with_tween_screen_position
with_tween_screen_position_backwards
with_tween_screen_position_easing
with_tween_screen_position_loop
with_tween_screen_position_on_finished
```

**Behavior / misc** (`behavior.rs`)

```text
with_phase
with_lua_timer
with_lua_collision_rule
with_lua_setup
with_on_animation_end
with_signal_binding
with_signal_binding_format
with_signal_flag
with_signal_integer
with_signal_scalar
with_signal_string
with_signals
with_group
with_persistent
with_grid_layout
with_tilemap
with_particle_emitter
with_mouse_controlled
with_text
```

Several methods validate a prerequisite call and raise a Lua runtime error if it's missing — e.g. `with_sprite_offset()` requires `with_sprite()` first, `with_gui_offset()` requires `with_parent()` first, `with_gui_theme_key()` requires one of the four `with_gui_*` widget methods first, `with_tween_position_easing()` requires `with_tween_position()` first. See `entity_builder/*.rs`'s inline `LuaError::runtime(...)` messages for the exact requirement text; `entity_builder/mod.rs`'s `#[cfg(test)] mod tests` block has one regression test per such guard.

---

## Entity Builder Pattern

The engine uses a fluent builder pattern for spawning entities from Lua:

```lua
engine.spawn()
    :with_group("player")
    :with_position(400, 700)
    :with_sprite("vaus", 48, 12, 24, 6)
    :with_velocity(0, 0)
    :with_collider(48, 12, 24, 6)
    :with_phase({
        initial = "idle",
        phases = {
            idle = {
                on_enter = "player_idle_on_enter",
                on_update = "player_idle_on_update"
            },
            running = {
                on_enter = "player_running_on_enter",
                on_update = "player_running_on_update",
                on_exit = "player_running_on_exit"
            }
        }
    })
    :register_as("player")
    :build()
```

Cloning is supported via the same builder pattern:

```lua
engine.clone("some_template_key")
    :with_position(100, 200)
    :with_velocity(0, -120)
    :build()
```

### GUI Widget Builders

GUI widgets are spawned the same way as any other entity, via dedicated `with_gui_*` methods. Each widget requires `:with_screen_position()` (or `:with_parent()` + `:with_gui_offset()` for a child widget positioned relative to its parent) and `:with_zindex()` to render:

```lua
engine.spawn()
    :with_screen_position(100, 40)
    :with_zindex(10)
    :with_gui_button(120, 32, "Start", "on_start_clicked")
    :with_gui_theme_key("hud")
    :build()
```

`with_gui_window`/`with_gui_button`/`with_gui_label`/`with_gui_image`/`with_gui_progress_bar` each spawn one widget component; `gui_button_spawn_system`/`gui_label_spawn_system`/`gui_image_spawn_system` react to `Added<...>` one frame later to attach the co-located `GuiInteractable` (buttons/images) and any caption `DynamicText` child (buttons/labels). An empty `label`/`text` on a button or label skips spawning the caption child entirely. `with_gui_theme_key(key)` overrides which `GuiThemeStore` entry the widget looks up (default `"default"`); theme lookup is flat — a child widget under a themed `GuiWindow` does not inherit the window's theme_key. See [GUI Theming](#gui-theming) above for the `engine.set_gui_theme_*` functions that populate `GuiThemeStore`.

### Menu Selection Callback

Menus can optionally invoke a Lua callback when an item is selected.

- Set the callback via `:with_menu_callback("callback_name")`.
- When a callback is set, `MenuActions` are ignored (the callback takes full control).

The callback receives three arguments:

```lua
-- entity_id = menu entity, item_id = string ID, item_index = 1-based index
function on_menu_select(entity_id, item_id, item_index)
    -- Use engine.* to queue commands
end
```

### How it Works

1. `engine.spawn()` / `engine.clone(source_key)` returns a `LuaEntityBuilder` UserData object
2. Each `:with_*()` method modifies the internal `SpawnCmd` and returns the same builder handle (in-place mutation, not a clone — chaining stays O(n), not O(n²))
3. `:build()` pushes a `SpawnCmd` (spawn mode) or a `CloneCmd` (clone mode) to the correct queue based on context (regular vs collision)
4. `:register_as(key)` stores the entity ID in WorldSignals after spawning

### Builder Metadata

`entity_builder/mod.rs` is the **single source of truth** for both runtime method registration and stub metadata. Every `with_*` method is declared with the `builder_method!` macro (defined in `entity_builder/mod.rs`, used by every submodule), which registers the method *and* records its description and parameter types in one place:

```rust
// In entity_builder/transform.rs register()
builder_method!(
    methods, meta,
    "with_group", "Set entity group",
    [("name", "string")],
    |_, this, name: String| {
        this.cmd.group = Some(name);
        Ok(())
    }
);
```

`register_as` and `build` are outside the `with_*` pattern and are appended manually in `collect_builder_meta()`. All methods are reflected into `engine.__meta.classes` via `register_builder_meta()` in `stub_meta.rs`.

For builder methods that accept complex table arguments, a `schema` field on the param points to a type name in `engine.__meta.types`. Schema mappings are configured in `stub_meta.rs::register_builder_meta()`:

```lua
-- Example: with_phase's "table" param has schema = "PhaseDefinition"
local p = engine.__meta.classes.EntityBuilder.methods.with_phase.params[1]
assert(p.schema == "PhaseDefinition")
assert(engine.__meta.types.PhaseDefinition)  -- full type definition
```

Current schema mappings:
- `with_phase` → `"PhaseDefinition"`
- `with_particle_emitter` → `"ParticleEmitterConfig"`
- `with_animation_rule` → `"AnimationRuleCondition"`
- `with_menu` → `"MenuItem[]"`

When adding a new builder method that accepts a table, add a `schema_refs` entry in `register_builder_meta()`.

### Spawn Processing

Spawn and clone commands are processed via `process_spawn_command()` and `process_clone_command()` in `lua_commands/spawn_cmd.rs`. Both delegate to the shared `apply_components()` helper, which applies all component data from `SpawnCmd` to the entity. This ensures spawn and clone have identical component support.

`apply_components()` is split into focused sub-functions:
- `apply_transform_components()` — position, screen position, rotation, scale, parent, stuckto, camera target
- `apply_physics_components()` — rigidbody, collider
- `apply_render_components()` — sprite, zindex, shader, tint, shadow
- `apply_animation_components()` — animation, animation controller, tweens
- `apply_signal_components()` — signals, signal bindings
- `apply_behavior_components()` — phase, lua timer, lua collision rule, lua setup, on_animation_end
- `apply_ui_components()` — text, menu, GUI widgets, grid layout, tilemap, mouse controlled
- `apply_particle_emitter()` — particle emitter setup with template resolution

---

## Signal Keys Vocabulary

Engine-internal signal keys (WorldSignals flags/scalars/strings used by the engine itself) are centralized in `crates/aberred-core/src/resources/signal_keys.rs` as `pub const` values:

```rust
pub const SWITCH_SCENE: &str = "switch_scene";  // flag: set by engine.change_scene() to request a scene change
pub const QUIT_GAME:    &str = "quit_game";      // flag: set by engine.quit() to request a clean shutdown
pub const SCENE:        &str = "scene";          // string: name of the currently active scene
pub const ANIMATION_ENDED: &str = "animation_ended"; // entity Signals flag: non-looped animation reached last frame
pub const MOVING:       &str = "moving";         // entity Signals flag: set by `movement` while velocity is non-zero
pub const SPEED_SQ:     &str = "speed_sq";       // entity Signals scalar: squared speed, published each frame by `movement`
pub const DEFAULT_SCENE:   &str = "menu";        // fallback scene name when SCENE is unset
pub const GROUP_COUNT_PREFIX: &str = "group_count:"; // integer key prefix
```

All callers import with `use aberred_core::resources::signal_keys as sk;` (or `crate::resources::signal_keys` from within `aberred-core` itself) and reference `sk::SWITCH_SCENE` etc. This gives a single rename point and compile-time typo detection. **Never write these as bare string literals in new code.**

---

## Camera Follow System

The camera follow system allows the camera to automatically track an entity marked with `CameraTarget`.

### Lua API

**Configuration** (called from `on_enter_play` or `on_switch_scene`):

```lua
engine.camera_follow_enable(true)
engine.camera_follow_set_mode("lerp")       -- "instant", "lerp", "smooth_damp"
engine.camera_follow_set_speed(5.0)
engine.camera_follow_set_easing("ease_out") -- "linear", "ease_out", "ease_in", "ease_in_out"
engine.camera_follow_set_offset(0, -20)
engine.camera_follow_set_bounds(0, 0, 2000, 1000) -- world-space bounds
```

**Deadzone mode:**

```lua
engine.camera_follow_set_deadzone(32, 24) -- sets mode to deadzone with half-dimensions
```

**Spring mode:**

```lua
engine.camera_follow_set_mode("smooth_damp")
engine.camera_follow_set_spring(80.0, 8.0) -- stiffness, damping
```

**Marking an entity as the camera target (with optional zoom):**

```lua
-- Via builder: priority=10, zoom in to 2x when this target wins
engine.spawn()
    :with_camera_target(10, 2.0)
    :build()

-- At runtime
engine.entity_set_camera_target(entity_id, 10)
engine.entity_set_camera_target(entity_id, nil, 2.0)  -- update zoom independently
engine.entity_remove_camera_target(entity_id)
```

**Zoom interpolation speed:**

```lua
engine.camera_follow_set_zoom_speed(5.0)  -- default; higher = faster zoom transition
```

The camera lerps `Camera2D.zoom` toward the winning target's `CameraTarget.zoom` every frame using `EaseOut`, at the rate set by `zoom_lerp_speed`. This is independent of the position follow mode.

### Reading Camera State

```lua
local cam = engine.get_camera()               -- CameraState: target_x/y, offset_x/y, rotation, zoom
local rect = engine.get_camera_view_rect()     -- CameraViewRect: visible world-space rect (x, y, w, h); assumes zero rotation
```

### Easing Strings

`EasingCurve` (`camera_follow_set_easing`) implements `FromStr`: `"linear"`, `"ease_out"`, `"ease_in"`, `"ease_in_out"`.

`Easing` (tween easing) implements `FromStr`: `"linear"`, `"quad_in"`, `"quad_out"`, `"quad_in_out"`, `"cubic_in"`, `"cubic_out"`, `"cubic_in_out"`.

`LoopMode` implements `FromStr`: `"once"`, `"loop"`, `"ping_pong"`.

---

## Input Rebinding

Lua can rebind input actions at runtime via the input API.

### Lua API

```lua
-- Replace all bindings for an action
engine.rebind_action("action_1", "z")

-- Add an extra binding (multi-bind)
engine.add_binding("action_1", "space")

-- Read current first binding (snapshot, visible next frame)
local key = engine.get_binding("action_1") -- "z" or nil
```

### Valid Action Names

`main_up`, `main_down`, `main_left`, `main_right`, `secondary_up`, `secondary_down`, `secondary_left`, `secondary_right`, `back`, `action_1`, `action_2`, `action_3`, `special`, `toggle_debug`, `toggle_fullscreen`

### Valid Key Strings

Single lowercase letters `a`-`z`, digits `0`-`9`, `space`, `enter`/`return`, `escape`/`esc`, arrow keys (`up`, `down`, `left`, `right`), modifiers (`lshift`/`rshift`/`lctrl`/`rctrl`/`lalt`/`ralt`), `f1`-`f12`, `mouse_left`, `mouse_right`, `mouse_middle`.

---

## Parent-Child Hierarchy

Entities can be organized into parent-child hierarchies for transform propagation.

### Lua API

**At spawn time:**

```lua
engine.spawn()
    :with_parent(parent_id)
    :with_position(10, 0) -- local offset from parent
    :build()
```

**At runtime:**

```lua
engine.entity_set_parent(child_id, parent_id)
engine.entity_remove_parent(child_id) -- snaps to current world position
```

### Notes

- `GlobalTransform2D` is computed automatically by `propagate_transforms` system.
- Use `ComputeInitialGlobalTransform` EntityCommand after setting `ChildOf` on a newly spawned entity to avoid a one-frame world-origin flash.
- `ChildOf` entities skip the `StuckTo` system (hierarchy takes precedence).
- Entity context exposes `ctx.world_pos`, `ctx.world_scale`, and `ctx.parent_id` in phase/timer callbacks.
- GUI widget children resolve their screen position from `GuiOffset` (set via `:with_gui_offset()`), not `ChildOf`-based transform propagation — see [GUI Widget Builders](#gui-widget-builders).

---

## Signal Snapshot System

Lua reads world state through a **cached snapshot**, not directly from ECS resources:

```rust
// Before calling Lua callbacks
lua_runtime.update_signal_cache(world_signals.snapshot());
lua_runtime.update_tracked_groups_cache(&tracked_groups);
```

```lua
-- In Lua
local score = engine.get_integer("score")  -- Reads from cache
```

### Why Snapshots?

1. **Immutable reads**: Lua can't accidentally corrupt game state
2. **Consistency**: All reads within a callback see the same state
3. **Performance**: `Arc<SignalSnapshot>` is cheap to clone

### Additional Snapshots

Beyond signal snapshots, the engine also caches:
- **GameConfig snapshot** — fullscreen, vsync, fps, render size, background color, pixel-snap camera (read via `get_fullscreen()`, `get_render_size()`, `get_pixel_snap_camera()`, etc.)
- **Bindings snapshot** — current input bindings (read via `get_binding()`)
- **Camera snapshot** — camera target, offset, rotation, zoom, and visible rect (read via `get_camera()`, `get_camera_view_rect()`)

These are updated before Lua callbacks run and ensure consistent reads.

### Input Data

Input is not read from the signal snapshot; it is passed to callbacks via a dedicated input table built from an `InputSnapshot` using pooled tables.

---

## Context Table Pooling

To minimize Lua table allocations in hot paths, the engine uses **table pooling** for callback context tables. Instead of creating new tables for each collision or entity callback, pre-allocated tables are stored in the Lua registry and reused.

### Why Pooling?

Without pooling, each callback would allocate many Lua tables:

- **Collision callbacks**: ~15-17 tables per collision (ctx, ctx.a, ctx.b, pos tables, vel tables, rect tables, signals, sides, etc.)
- **Entity callbacks** (phase/timer): ~10-14 tables per callback (ctx, pos, screen_pos, vel, scale, rect, sprite, animation, timer, signals)
- **Input tables**: digital/analog subtables reused across all callbacks each frame

In a game with frequent collisions or many entities with phase/timer components, this creates significant GC pressure.

### Pool Architecture

Three pool types are maintained:

- **CollisionCtxPool** — for collision callbacks (ctx.a, ctx.b, sides, subtables)
- **EntityCtxPool** — for phase/timer callbacks (ctx, pos, vel, scale, rect, etc.)
- **InputCtxPool** — for the input table passed to all callbacks (digital, analog subtables)

### How It Works

1. **Initialization**: Pools are created once in `LuaRuntime::new()` via `create_*_pool()` functions
2. **Retrieval**: Before each callback, `get_*_pool()` fetches tables from the registry
3. **Population**: Context builder functions populate the pooled tables with current entity data — optional fields are explicitly set to `nil` when absent (prevents stale data)
4. **Reuse**: The same tables are reused for the next callback

### What Gets Pooled vs Created Fresh

| Data Type | Pooled? | Reason |
| --------- | ------- | ------ |
| Fixed-structure tables (ctx, pos, vel, rect, etc.) | Yes | Same structure every time |
| Scalar/numeric values | N/A | Set directly on pooled tables |
| Signal inner maps (flags, integers, scalars, strings) | No | Variable keys per entity |
| Collision side arrays | Cleared & repopulated | Variable length |

### Entity Context Table (`ctx`) — Phase / Timer / Setup / Animation-Finished / Tween-Finished Callbacks

Every callback described in [Scene Lifecycle and Callback Dispatch](#scene-lifecycle-and-callback-dispatch) except collision receives a `ctx` table built by `build_entity_context_pooled` (`lua_runtime/context.rs`) via `EntityCtxTables`. `id` is always present; every other field is `nil` when the entity lacks the corresponding component:

| Field | Type | Source component |
| ----- | ---- | ----------------- |
| `id` | integer | always present |
| `group` | string | `Group` |
| `rotation` | number | `Rotation` |
| `world_rotation` | number | `GlobalTransform2D` |
| `parent_id` | integer | `ChildOf` |
| `pos` = `{x, y}` | table | `MapPosition` |
| `screen_pos` = `{x, y}` | table | `ScreenPosition` |
| `scale` = `{x, y}` | table | `Scale` |
| `world_pos` = `{x, y}` | table | `GlobalTransform2D` |
| `world_scale` = `{x, y}` | table | `GlobalTransform2D` |
| `vel` = `{x, y}`, `speed_sq`, `frozen` | table, number, boolean | `RigidBody` (set/cleared together) |
| `rect` = `{x, y, w, h}` | table | `BoxCollider` AABB |
| `sprite` = `{tex_key, flip_h, flip_v}` | table | `Sprite` |
| `animation` = `{key, frame_index, elapsed}` | table | `Animation` |
| `signals` = `{flags, integers, scalars, strings}` | table | `Signals` |
| `phase`, `time_in_phase` | string, number | `LuaPhase` (set/cleared together) |
| `previous_phase` | string | `LuaPhase`, only populated for `phase_on_enter` |
| `timer` = `{duration, elapsed, callback}` | table | `LuaTimer`, only populated inside a timer callback |

### Collision Context Table (`ctx`) — Collision Callbacks

Collision callbacks receive a differently-shaped `ctx` built by `populate_collision_entity` (`systems/lua_collision.rs`) via `CollisionCtxTables`: `ctx.a`/`ctx.b` (one per colliding entity, each with `id`, `speed_sq` always set and `group`/`pos = {x, y}`/`vel = {x, y}`/`rect = {x, y, w, h}`/`signals = {flags, integers, scalars, strings}` present only when the corresponding component exists), and `ctx.sides.a`/`ctx.sides.b` — arrays of `"left"`/`"right"`/`"top"`/`"bottom"` strings describing which AABB sides overlapped. There is no `ctx.timer`/`ctx.phase`/`ctx.sprite`/`ctx.animation` on either side, and no top-level `id`/`pos`/etc. — always go through `ctx.a`/`ctx.b`.

### Important: No Persistent References

**Lua scripts must NOT store references to context tables or their subtables for later use.** The tables are reused and values will be overwritten on the next callback.

```lua
-- BAD: Don't do this!
local saved_pos = ctx.pos  -- This reference will have wrong values later

-- GOOD: Copy the values you need
local saved_x = ctx.pos.x
local saved_y = ctx.pos.y
```

### Implementation Files

- `runtime.rs`: Pool structs (`CollisionCtxTables`, `EntityCtxTables`, `InputCtxTables`), `create_*_pool()`/`create_*_tables()`, `get_*_pool()` methods
- `lua_runtime/context.rs`: `build_entity_context_pooled()` — low-level Lua table writer
- `systems/lua_commands/context.rs`: `build_entity_context()` — ECS-facing adapter that gathers component data and calls `build_entity_context_pooled()`

---

## Meta Schema (`engine.__meta`)

The `engine.__meta` table provides a complete, introspectable API contract for the Lua interface. It is populated during `LuaRuntime::new()` and can be used for automated stub generation, documentation, and drift protection tests.

### Structure

```lua
engine.__meta = {
    functions  = { ... },  -- All engine.* function signatures
    classes    = { ... },  -- EntityBuilder / CollisionEntityBuilder method signatures
    types      = { ... },  -- Type shape definitions (table schemas)
    enums      = { ... },  -- Valid string literal value sets
    callbacks  = { ... },  -- Well-known callback signatures the engine invokes
}
```

### `__meta.types` — Type Shape Definitions

Each entry describes a Lua table shape with typed fields. Registered by `register_types_meta()` in `stub_meta.rs`.

```lua
engine.__meta.types["EntityContext"] = {
    description = "Entity state passed to phase/timer callbacks",
    fields = {
        { name = "id",    type = "integer",  optional = false, description = "Entity ID" },
        { name = "pos",   type = "Vec2",     optional = true },
        { name = "phase", type = "string",   optional = true },
        -- ...
    }
}
```

Current types: `Vec2`, `Rect`, `CameraState`, `CameraViewRect`, `SpriteInfo`, `AnimationInfo`, `TimerInfo`, `SignalSet`, `EntityContext`, `CollisionEntity`, `CollisionSides`, `CollisionContext`, `DigitalButtonState`, `DigitalInputs`, `AnalogInputs`, `InputSnapshot`, `PhaseCallbacks`, `PhaseDefinition`, `ParticleEmitterConfig`, `MenuItem`, `AnimationRuleCondition`.

### `__meta.enums` — String Literal Value Sets

Each entry lists the valid string values for a domain concept. Registered by `register_enums_meta()` in `stub_meta.rs`.

```lua
engine.__meta.enums["Easing"] = {
    description = "Tween easing function",
    values = { "linear", "quad_in", "quad_out", "quad_in_out",
               "cubic_in", "cubic_out", "cubic_in_out" }
}
```

Current enums: `Easing`, `LoopMode`, `BoxSide`, `ComparisonOp`, `ConditionType`, `EmitterShape`, `TtlSpec`, `TextureFilter`, `Category`.

### `__meta.callbacks` — Engine-Invoked Callback Signatures

Each entry documents a global Lua function the engine calls, including parameter types, return types, and context. Registered by `register_callbacks_meta()` in `stub_meta.rs`.

```lua
engine.__meta.callbacks["phase_on_enter"] = {
    description = "Called when entering a phase",
    params  = { { name = "ctx", type = "EntityContext" },
                { name = "input", type = "InputSnapshot" } },
    returns = { type = "string?" },
    context = "play",
    note    = "Return phase name to trigger transition"
}
```

Current callbacks: `on_setup`, `on_enter_play`, `on_switch_scene`, `on_update_<scene>`, `phase_on_enter`, `phase_on_update`, `phase_on_exit`, `timer_callback`, `collision_callback`, `menu_callback`.

### Drift Protection

Tests in `crates/aberredengine/tests/engine_tick_integration.rs` verify the meta schema stays in sync with the implementation:

- `meta_types_table_is_populated` — all type entries have `description` + `fields` with `name`/`type`/`optional`
- `meta_enums_table_is_populated` — hard-coded expected values for `Easing`, `LoopMode`, `BoxSide`, `Category`
- `meta_callbacks_table_is_populated` — all callback entries have `params` with correct shapes
- `meta_functions_complete` — comprehensive function list + collision/entity command parity check
- `meta_builder_methods_have_schema_refs` — schema references point to existing types

`crates/aberred-lua/tests/stub_generator_integration.rs` covers the companion concern — that `stub_generator.rs` produces `assets/scripts/engine.lua` output consistent with the registered `__meta` tables.

When adding new Rust types, easing functions, callback conventions, or API functions, update the corresponding `register_*_meta()` method in `stub_meta.rs`. If you don't, these tests will fail.

---

## How to Add New Lua Commands

This section provides step-by-step instructions for adding new Lua commands.

### Example: Adding `entity_set_health`

Let's add a command that sets a "health" scalar on an entity's Signals component.

#### Step 1: Add Command Variant

In `crates/aberred-lua/src/resources/lua_runtime/commands.rs`:

```rust
pub enum EntityCmd {
    // ... existing variants ...
    SetHealth { entity_id: u64, health: f32 },
}
```

#### Step 2: Register Lua Function

In `crates/aberred-lua/src/resources/lua_runtime/engine_api/entity.rs`, add the entry to the `define_entity_cmds!` macro body. This single entry auto-registers both the regular (`entity_set_health`) and collision (`collision_entity_set_health`) variants, along with metadata. Note the argument list uses the `|$args:pat_param| $arg_ty:ty` grammar described in [Module Structure](#module-structure) — a single non-tuple argument is written as `|entity_id| u64`, a multi-argument one as `|(entity_id, health)| (u64, f32)`:

```rust
// Inside define_entity_cmds! macro body in engine_api/entity.rs
("entity_set_health",
    |(entity_id, health)| (u64, f32),
    EntityCmd::SetHealth { entity_id, health },
    desc = "Set entity health signal",
    params = [("entity_id", "integer"), ("health", "number")]),
```

For non-entity commands (signals, audio, etc.), use `register_cmd!` (or the relevant `define_*_cmd_twins!`) directly in the appropriate `register_*_api()` method in the correct `engine_api/*.rs` file.

#### Step 3: Process the Command

In `crates/aberred-lua/src/systems/lua_commands/entity_cmd.rs`, add the match arm:

```rust
EntityCmd::SetHealth { entity_id, health } => {
    let entity = Entity::from_bits(entity_id);
    if let Ok(mut signals) = cmd_queries.signals.get_mut(entity) {
        signals.set_scalar("health", health);
    }
}
```

#### Step 4: Update Meta Schema (If Applicable)

If the new command introduces new string literal values, update `register_enums_meta()` in `stub_meta.rs`. If it introduces a new callback convention, update `register_callbacks_meta()`. If it accepts a complex table argument, add a type definition in `register_types_meta()`.

#### Step 5: Regenerate LSP Stubs

```
cargo run -- --create-lua-stubs
```

This regenerates `assets/scripts/engine.lua`. Never hand-edit that file.

> **Note**: Because entity commands are registered via `define_entity_cmds!`, the collision-prefixed variant (`collision_entity_set_health`) is automatically available. Metadata for `engine.__meta` is also generated automatically.

---

### Adding a Completely New Command Type

If you need a new category of commands (e.g., `HealthCmd`):

#### Step 1: Define the Enum

In `commands.rs`:

```rust
#[derive(Debug, Clone)]
pub enum HealthCmd {
    SetEntityHealth { entity_id: u64, health: f32 },
    HealEntity { entity_id: u64, amount: f32 },
}
```

#### Step 2: Add a Queue Row to queue_registry.rs

In `queue_registry.rs`, add one row to the `@master` list:

```rust
(health_commands, HealthCmd, clear),
```

The drain method (`drain_health_commands_into`), its inclusion in `clear_all_commands`, and the `health_commands` field on `LuaAppData` are all generated automatically from this one row — no separate edit to `runtime.rs` is needed (see [Module Structure](#module-structure)'s `queue_registry.rs` description).

#### Step 3: Add a Category Module and Register It

Create `crates/aberred-lua/src/resources/lua_runtime/engine_api/health.rs`:

```rust
use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_health_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        register_cmd!(engine, self.lua, meta_fns, "heal_entity", health_commands,
            |(entity_id, amount)| (u64, f32), HealthCmd::HealEntity { entity_id, amount },
            desc = "Heal an entity by amount", cat = "health",
            params = [("entity_id", "integer"), ("amount", "number")]);

        Ok(())
    }
}
```

Declare the module in `engine_api/mod.rs`:

```rust
mod health;
```

Call it in `LuaRuntime::new()` in `runtime.rs`:

```rust
runtime.register_health_api()?;
```

#### Step 4: Create Processing Function and Call from Game Loop

In `lua_commands/processors.rs` (or a new sub-file), add a `process_health_command()` function. Call it from wherever the new queue is drained (`lua_plugin.rs`, or a dedicated system, following the pattern of `drain_and_process_effect_commands` in `systems/lua_commands/mod.rs`):

```rust
let mut health_cmds = Vec::new();
lua_runtime.drain_health_commands_into(&mut health_cmds);
for cmd in health_cmds.drain(..) {
    process_health_command(cmd, &mut health_query);
}
```

---

### Adding Entity Builder Methods

To add spawning capabilities:

#### Step 1: Add Data Structure (if needed)

In `spawn_data.rs`.

#### Step 2: Add to SpawnCmd

In `spawn_data.rs`.

#### Step 3: Add Builder Method with builder_method! Macro

In the appropriate `entity_builder/*.rs` submodule (e.g. `behavior.rs` for a gameplay-state field), inside its `register()` function, add the method using `builder_method!`:

```rust
builder_method!(
    methods, meta,
    "with_health", "Set initial health",
    [("initial", "number"), ("max", "number")],
    |_, this, (initial, max): (f32, f32)| {
        this.cmd.health = Some(HealthData { initial_health: initial, max_health: max });
        Ok(())
    }
);
```

The `builder_method!` macro registers the runtime method **and** records the stub metadata in a single declaration. `entity_builder/mod.rs` is the single source of truth — no separate update to `stub_meta.rs` is needed for the method entry itself.

#### Step 4: Process During Spawn

In `lua_commands/spawn_cmd.rs`, inside `apply_components()`.

---

## Best Practices

### 1. Keep Commands Small and Focused

Each command should do one thing. If you need multiple operations, use multiple commands:

```rust
// Good: Separate concerns
EntityCmd::SetVelocity { entity_id, vx, vy },
EntityCmd::SetPosition { entity_id, x, y },

// Avoid: Combining unrelated operations
EntityCmd::SetPositionAndVelocity { ... }  // Too broad
```

### 2. Use Appropriate Queue

- **Regular queues**: For phase callbacks, timer callbacks, update callbacks
- **Collision queues**: For collision callbacks (immediate processing needed)

### 3. Handle Missing Entities Gracefully

```rust
// Good: Silent failure if entity doesn't exist
if let Ok(mut rb) = cmd_queries.rigid_bodies.get_mut(entity) {
    rb.velocity = Vec2::new(vx, vy);
}
```

### 4. Entity IDs are u64

Bevy's `Entity` type is not directly usable in Lua. Always convert:

```rust
// Rust to Lua: entity.to_bits()
// Lua to Rust: Entity::from_bits(entity_id)
```

### 5. Use Signal Key Constants

Always import and use `aberred_core::resources::signal_keys as sk` instead of bare string literals when reading or writing engine signal keys. This prevents silent typo bugs and keeps renames to a single file.

```rust
// Good
use aberred_core::resources::signal_keys as sk;
world_signals.take_flag(sk::SWITCH_SCENE);

// Bad — no compile-time check, silently wrong on typo
world_signals.take_flag("switch_scene");
```

### 6. Regenerate Stubs After API Changes

```
cargo run -- --create-lua-stubs
```

Never hand-edit `assets/scripts/engine.lua`; it is auto-generated.

### 7. Consider Collision Context

For entity commands, the `define_entity_cmds!` macro automatically registers both regular and collision variants from a single definition — no manual duplication needed.

For other command types, provide a separate `collision_*` registration using `register_cmd!`/`define_*_cmd_twins!` with the collision-scoped queue.

### 8. Registration Patterns Summary

| What | Where | How |
| ---- | ----- | --- |
| Entity commands (with auto collision variants) | `engine_api/entity.rs` | `define_entity_cmds!` entry |
| Simple push-to-queue functions | Appropriate `engine_api/*.rs` | `register_cmd!` macro (or a `define_*_cmd_twins!` specialization) |
| Read-only / no-queue functions | Appropriate `engine_api/*.rs` | `register_getter!` macro |
| Functions with irregular shapes (custom defaults, non-macro-friendly grammar) | Appropriate `engine_api/*.rs` | Manual `engine.set()` + `push_fn_meta()` |
| Builder `with_*` methods | `entity_builder/*.rs` `register()` | `builder_method!` macro |
| Type/enum/callback metadata | `stub_meta.rs` | `register_types_meta()` / `register_enums_meta()` / `register_callbacks_meta()` |
| New queue | `queue_registry.rs` | One `@master` row (drain/clear/`LuaAppData` field all generated) |

---

## Summary

The Lua interface follows these principles:

1. **Deferred Execution**: Commands are queued, not executed immediately
2. **Type Safety**: Rust enums ensure valid command structures
3. **Separation of Concerns**: Commands are defined, registered, and processed in different modules
4. **Read-Write Split**: Lua reads from cached snapshots, writes via command queues
5. **Context Awareness**: Collision callbacks have separate queues for immediate processing
6. **Single Source of Truth**: `queue_registry.rs` owns the queue list; `entity_builder/mod.rs` owns builder method definitions and their stub metadata

To add new commands:

1. Add variant to appropriate command enum in `commands.rs`
2. Register Lua function in the appropriate `engine_api/*.rs` file (use `register_cmd!`/`define_*_cmd_twins!` for push-to-queue, `register_getter!` for read-only, or add to `define_entity_cmds!` for entity commands)
3. **If adding a new queue type**: add one row to `queue_registry.rs`'s `@master` list — drain, clear, and the `LuaAppData` field are all generated
4. **If adding a new API category**: create `engine_api/category.rs`, declare `mod category` in `engine_api/mod.rs`, call `register_category_api()` in `LuaRuntime::new()`
5. Process command in `lua_commands/` (`entity_cmd.rs`, `spawn_cmd.rs`, `processors.rs`, or `mod.rs`)
6. Call drain from the game loop (`lua_plugin.rs`, `dispatch.rs`'s `LuaDispatch` flow, or the appropriate system)
7. Optionally add a builder method with `builder_method!` in the relevant `entity_builder/*.rs` submodule — stub metadata is included automatically
8. Update `register_types_meta()` / `register_enums_meta()` / `register_callbacks_meta()` in `stub_meta.rs` if new types/enums/callbacks are introduced
9. Run `cargo run -- --create-lua-stubs` to regenerate `assets/scripts/engine.lua`
