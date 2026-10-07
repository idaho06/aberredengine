//! Lua-based phase state machine systems.
//!
//! This module provides systems for processing [`LuaPhase`] components:
//!
//! - [`lua_phase_system`] – runs Lua callbacks for phase enter/update/exit
//!
//! Unlike the Rust-based [`phase`](aberred_core::systems::phase) system, this system delegates
//! all callback logic to Lua scripts via named function references.
//!
//! # System Flow
//!
//! Each frame, for each entity with a `LuaPhase` component:
//!
//! 1. On the first run, call the on_enter function for the initial phase
//! 2. If `next` is set (transition requested):
//!    - Swap phases, reset time
//!    - Call on_exit for the old phase (looked up by the old name; `current`
//!      is already the new phase)
//!    - Call on_enter for new phase
//! 3. Call on_update for current phase
//! 4. Increment `time_in_phase` by delta
//! 5. Process any phase transition commands from Lua
//!
//! # Callback Signatures (Lua side)
//!
//! ```lua
//! function my_enter_callback(ctx, input)      -- ctx.previous_phase available
//! function my_update_callback(ctx, input, dt) -- ctx.time_in_phase in ctx
//! function my_exit_callback(ctx)
//! ```
//!
//! # Performance
//!
//! Context tables are pooled and reused across callbacks to reduce Lua GC pressure.
//! See `EntityCtxTables` in runtime.rs.

use bevy_ecs::prelude::*;
use bevy_ecs::system::Local;
use mlua::prelude::*;

use crate::components::luaphase::{LuaPhase, PhaseCallbacks};
use crate::resources::lua_runtime::{LuaPhaseSnapshot, LuaRuntime, PhaseCmd};
use crate::systems::lua_commands::{
    ContextQueries, DrainScope, EffectCmdBufs, EntityCmdQueries, build_entity_context,
    drain_and_process_effect_commands, drain_and_process_phase_commands,
};
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::resources::animationstore::AnimationStore;
use aberred_core::resources::input::InputState;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::resources::worldsignals::WorldSignals;
use aberred_core::resources::worldtime::WorldTime;
use log::{error, warn};

fn build_phase_context(
    lua_runtime: &LuaRuntime,
    entity: Entity,
    lua_phase: &LuaPhase,
    previous_phase: Option<&str>,
    ctx_queries: &ContextQueries,
    cmd_queries: &EntityCmdQueries,
) -> LuaResult<LuaTable> {
    let lua_phase_snapshot = Some(LuaPhaseSnapshot::from(lua_phase));
    build_entity_context(
        lua_runtime,
        entity,
        ctx_queries,
        cmd_queries,
        lua_phase_snapshot,
        previous_phase,
    )
}

/// Process the return value from a phase callback.
/// Returns Some(phase_name) if a valid transition was requested (different from current phase).
fn process_callback_return(result: LuaValue, current_phase: &str, fn_name: &str) -> Option<String> {
    match result {
        LuaValue::String(s) => match s.to_str() {
            Ok(phase) => {
                if phase != current_phase {
                    Some(phase.to_string())
                } else {
                    None // Same phase, ignore
                }
            }
            Err(e) => {
                error!(target: "lua", "Error converting return value in {}(): {}", fn_name, e);
                None
            }
        },
        LuaValue::Nil => None,
        _ => {
            warn!(target: "lua", "Phase callback '{}' returned non-string, non-nil value", fn_name);
            None
        }
    }
}

/// Call phase enter callback: (ctx, input)
/// Returns Some(phase_name) if the callback returned a phase transition request.
fn call_phase_enter(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    ctx_table: &LuaTable,
    input_table: &LuaTable,
    current_phase: &str,
) -> Option<String> {
    let result = lua_runtime.call_named(fn_name, "Phase", |func| {
        func.call::<LuaValue>((ctx_table.clone(), input_table.clone()))
    })?;
    process_callback_return(result, current_phase, fn_name)
}

/// Call phase update callback: (ctx, input, dt)
/// Returns Some(phase_name) if the callback returned a phase transition request.
fn call_phase_update(
    lua_runtime: &LuaRuntime,
    fn_name: &str,
    ctx_table: &LuaTable,
    input_table: &LuaTable,
    dt: f32,
    current_phase: &str,
) -> Option<String> {
    let result = lua_runtime.call_named(fn_name, "Phase", |func| {
        func.call::<LuaValue>((ctx_table.clone(), input_table.clone(), dt))
    })?;
    process_callback_return(result, current_phase, fn_name)
}

/// Call phase exit callback: (ctx)
fn call_phase_exit(lua_runtime: &LuaRuntime, fn_name: &str, ctx_table: &LuaTable) {
    lua_runtime.call_named(fn_name, "Phase", |func| func.call::<()>(ctx_table.clone()));
}

/// Run one frame of phase lifecycle processing for every [`LuaPhase`] entity.
///
/// For each entity this function:
/// 1. Fires `on_enter` on its first run (core `Phase::begin`).
/// 2. Applies any already-queued `next` transition (core `Phase::apply_next`), then
///    fires `on_exit` for the old phase and `on_enter` for the new one.
/// 3. Runs the current phase's `on_update` callback.
/// 4. Adds `delta` to `time_in_phase`.
///
/// Any phase name returned by `on_enter`/`on_update` is collected into
/// `callback_transitions` for deferred application via [`apply_callback_transitions`].
/// Callbacks only queue commands, so they never touch a `LuaPhase` while it is
/// borrowed here.
fn run_phase_callbacks(
    phase_query: &mut Query<(Entity, &mut LuaPhase)>,
    delta: f32,
    callback_transitions: &mut Vec<(Entity, String)>,
    runner: &LuaPhaseRunner,
) {
    for (entity, mut lua_phase) in phase_query.iter_mut() {
        let mut queue = |next: Option<String>| {
            if let Some(next) = next {
                callback_transitions.push((entity, next));
            }
        };

        if lua_phase.phase.begin() {
            queue(runner.call_enter(entity, &lua_phase));
        }
        if let Some(old_phase) = lua_phase.phase.apply_next() {
            // exit runs after the swap: `current` is already the new phase, and
            // the exit callback is looked up by the old phase's name.
            if let Some(callbacks) = lua_phase.get_callbacks(&old_phase) {
                runner.call_exit(entity, &lua_phase, callbacks);
            }
            queue(runner.call_enter(entity, &lua_phase));
        }
        queue(runner.call_update(entity, &lua_phase, delta));
        lua_phase.phase.time_in_phase += delta;
    }
}

/// Store a callback-requested phase change in the wrapped core `Phase::next`.
///
/// Callback returns are not applied inline inside [`run_phase_callbacks`]; they are
/// queued first so the current entity-loop pass finishes before the transition is
/// picked up by the next phase-processing step.
pub(crate) fn queue_phase_transition(
    phase_query: &mut Query<(Entity, &mut LuaPhase)>,
    entity: Entity,
    next_phase: String,
) {
    if let Ok((_, mut lua_phase)) = phase_query.get_mut(entity) {
        lua_phase.phase.next = Some(next_phase);
    }
}

/// Drain callback-requested transitions after the entity loop completes.
///
/// Deferring this step avoids mutating phase state in the middle of
/// [`run_phase_callbacks`], which would otherwise make callback-triggered
/// transitions re-enter the lifecycle flow during the same pass.
pub(crate) fn apply_callback_transitions(
    phase_query: &mut Query<(Entity, &mut LuaPhase)>,
    callback_transitions: &mut Vec<(Entity, String)>,
) {
    for (entity, next_phase) in callback_transitions.drain(..) {
        queue_phase_transition(phase_query, entity, next_phase);
    }
}

struct LuaPhaseRunner<'a, 'w, 's> {
    lua_runtime: &'a LuaRuntime,
    input_table: &'a LuaTable,
    ctx_queries: &'a ContextQueries<'w, 's>,
    cmd_queries: &'a EntityCmdQueries<'w, 's>,
}

impl LuaPhaseRunner<'_, '_, '_> {
    /// Run the current phase's enter callback, if it has one.
    fn call_enter(&self, entity: Entity, lua_phase: &LuaPhase) -> Option<String> {
        let fn_name = lua_phase.current_callbacks()?.on_enter.as_deref()?;

        match build_phase_context(
            self.lua_runtime,
            entity,
            lua_phase,
            lua_phase.phase.previous.as_deref(),
            self.ctx_queries,
            self.cmd_queries,
        ) {
            Ok(ctx) => call_phase_enter(
                self.lua_runtime,
                fn_name,
                &ctx,
                self.input_table,
                &lua_phase.phase.current,
            ),
            Err(e) => {
                error!("Error building context: {}", e);
                None
            }
        }
    }

    /// Run the current phase's update callback, if it has one.
    fn call_update(&self, entity: Entity, lua_phase: &LuaPhase, delta: f32) -> Option<String> {
        let fn_name = lua_phase.current_callbacks()?.on_update.as_deref()?;

        match build_phase_context(
            self.lua_runtime,
            entity,
            lua_phase,
            None,
            self.ctx_queries,
            self.cmd_queries,
        ) {
            Ok(ctx) => call_phase_update(
                self.lua_runtime,
                fn_name,
                &ctx,
                self.input_table,
                delta,
                &lua_phase.phase.current,
            ),
            Err(e) => {
                error!("Error building context: {}", e);
                None
            }
        }
    }

    /// Run an exit callback from `callbacks` (the old phase's, after the swap).
    fn call_exit(&self, entity: Entity, lua_phase: &LuaPhase, callbacks: &PhaseCallbacks) {
        let Some(fn_name) = callbacks.on_exit.as_deref() else {
            return;
        };

        match build_phase_context(
            self.lua_runtime,
            entity,
            lua_phase,
            None,
            self.ctx_queries,
            self.cmd_queries,
        ) {
            Ok(ctx) => call_phase_exit(self.lua_runtime, fn_name, &ctx),
            Err(e) => error!("Error building context: {}", e),
        }
    }
}

/// Process Lua-based phase state machines.
///
/// This system:
/// 1. Updates signal cache for Lua to read
/// 2. Runs Lua phase callbacks (enter/update/exit) via named functions
/// 3. Processes commands queued by Lua (audio, signals, phases, spawns, entity ops)
/// 4. Handles phase transitions
#[allow(clippy::too_many_arguments)]
pub fn lua_phase_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut LuaPhase)>,
    // Bundled read-only queries for context building
    ctx_queries: ContextQueries,
    // Bundled mutable queries for command processing
    mut cmd_queries: EntityCmdQueries,
    // Resources
    time: Res<WorldTime>,
    input: Res<InputState>,
    mut world_signals: ResMut<WorldSignals>,
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    systems_store: Res<SystemsStore>,
    animation_store: Res<AnimationStore>,
    // Local resources to avoid per-frame allocation
    mut callback_transitions: Local<Vec<(Entity, String)>>,
    mut phase_buf: Local<Vec<PhaseCmd>>,
    mut effect_bufs: Local<EffectCmdBufs>,
) {
    aberred_core::tracy::tracy_span!("lua_phase");
    // Clear previous frame's transitions (reuses allocated capacity)
    callback_transitions.clear();

    if query.is_empty() {
        return;
    }

    // Update signal cache so Lua can read current values
    lua_runtime.sync_signals(&mut world_signals);

    let input_table = match lua_runtime.resolve_input_table(&input, time.frame_count) {
        Ok(table) => table,
        Err(e) => {
            error!("Error creating input table for phase system: {}", e);
            return;
        }
    };

    let delta = time.delta;
    let runner = LuaPhaseRunner {
        lua_runtime: &lua_runtime,
        input_table: &input_table,
        ctx_queries: &ctx_queries,
        cmd_queries: &cmd_queries,
    };

    run_phase_callbacks(&mut query, delta, &mut callback_transitions, &runner);

    // Phase and effect drains are kept separate here (not via
    // dispatch::drain_dispatch_commands) because apply_callback_transitions
    // must run between them — see the doc comment on
    // drain_and_process_effect_commands in lua_commands/mod.rs.
    drain_and_process_phase_commands(
        &lua_runtime,
        DrainScope::Regular,
        &mut phase_buf,
        &mut query,
    );

    // Apply return value transitions after phase drain — return values take
    // precedence over engine.phase_transition() calls in the same callback.
    apply_callback_transitions(&mut query, &mut callback_transitions);

    drain_and_process_effect_commands(
        &lua_runtime,
        DrainScope::Regular,
        &mut effect_bufs,
        &mut commands,
        &mut world_signals,
        &mut cmd_queries,
        &mut audio_cmd_writer,
        &systems_store,
        &animation_store,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::testing::approx_eq;
    use bevy_ecs::system::RunSystemOnce;
    use rustc_hash::FxHashMap;

    /// Minimal world for `lua_phase_system`: exactly the resources its
    /// params read, plus a fresh `LuaRuntime`.
    fn make_lua_phase_world(delta: f32) -> World {
        let mut world = World::new();
        world.insert_resource(WorldTime {
            delta,
            ..WorldTime::default()
        });
        world.insert_resource(InputState::default());
        world.insert_resource(WorldSignals::default());
        world.init_resource::<Messages<AudioCmd>>();
        world.insert_resource(SystemsStore::new());
        world.insert_resource(AnimationStore::default());
        world.insert_non_send(LuaRuntime::new().expect("Failed to init Lua runtime"));
        world
    }

    fn tick_lua_phases(world: &mut World) {
        world
            .run_system_once(lua_phase_system)
            .expect("lua_phase_system should run");
    }

    /// `on_exit` fires after the phase swap: the callback's ctx already
    /// reports the new phase and a reset `time_in_phase`.
    #[test]
    fn lua_phase_on_exit_sees_post_swap_phase_state() {
        let mut world = make_lua_phase_world(0.25);

        {
            let lua_runtime = world.non_send::<LuaRuntime>();
            lua_runtime
                .lua()
                .load(
                    r#"
                    function moving_exit(ctx)
                        engine.set_string("exit_phase_seen", ctx.phase)
                        engine.set_scalar("exit_time_in_phase_seen", ctx.time_in_phase)
                    end
                    "#,
                )
                .exec()
                .expect("Failed to load Lua phase callback");
        }

        let mut phases = FxHashMap::default();
        phases.insert("idle".into(), PhaseCallbacks::default());
        phases.insert(
            "moving".into(),
            PhaseCallbacks {
                on_enter: None,
                on_update: None,
                on_exit: Some("moving_exit".into()),
            },
        );
        phases.insert("attacking".into(), PhaseCallbacks::default());

        let entity = world.spawn((LuaPhase::new("moving", phases),)).id();
        world.get_mut::<LuaPhase>(entity).unwrap().phase.next = Some("attacking".into());

        tick_lua_phases(&mut world);

        let world_signals = world.resource::<WorldSignals>();
        assert_eq!(
            world_signals.get_string("exit_phase_seen"),
            Some("attacking")
        );
        assert!(approx_eq(
            world_signals
                .get_scalar("exit_time_in_phase_seen")
                .expect("exit time signal"),
            0.0
        ));
    }

    /// Lua callbacks that log each call as `kind:ctx.phase:fn name` into the
    /// global `calls`; `idle_up` returns the global `go_to` (nil by default).
    const RECORDING_CALLBACKS: &str = r#"
        calls = {}
        local function log(kind, ctx, name)
            table.insert(calls, kind .. ":" .. ctx.phase .. ":" .. name)
        end
        function idle_in(ctx, input) log("enter", ctx, "idle_in") end
        function idle_up(ctx, input, dt) log("update", ctx, "idle_up") return go_to end
        function idle_out(ctx) log("exit", ctx, "idle_out") end
        function run_in(ctx, input) log("enter", ctx, "run_in") end
        function run_up(ctx, input, dt) log("update", ctx, "run_up") end
    "#;

    /// World with [`RECORDING_CALLBACKS`] loaded and one `LuaPhase` entity in
    /// `idle` (with `idle`/`run` phases).
    fn make_recording_world(delta: f32) -> (World, Entity) {
        let mut world = make_lua_phase_world(delta);
        world
            .non_send::<LuaRuntime>()
            .lua()
            .load(RECORDING_CALLBACKS)
            .exec()
            .expect("Failed to load recording callbacks");

        let mut phases = FxHashMap::default();
        phases.insert(
            "idle".into(),
            PhaseCallbacks {
                on_enter: Some("idle_in".into()),
                on_update: Some("idle_up".into()),
                on_exit: Some("idle_out".into()),
            },
        );
        phases.insert(
            "run".into(),
            PhaseCallbacks {
                on_enter: Some("run_in".into()),
                on_update: Some("run_up".into()),
                on_exit: None,
            },
        );
        let entity = world.spawn(LuaPhase::new("idle", phases)).id();
        (world, entity)
    }

    /// Returns and clears the calls logged by [`RECORDING_CALLBACKS`].
    fn take_calls(world: &World) -> Vec<String> {
        world
            .non_send::<LuaRuntime>()
            .lua()
            .load("local c = calls; calls = {}; return c")
            .eval()
            .expect("calls should be a list of strings")
    }

    /// Lifecycle order: first-run enter, then update; a queued `next` swaps
    /// first, so exit sees the new `current` but runs the old phase's
    /// callback, then enter and update run for the new phase.
    #[test]
    fn lua_phase_lifecycle_order() {
        let (mut world, entity) = make_recording_world(0.25);

        tick_lua_phases(&mut world);
        assert_eq!(
            take_calls(&world),
            ["enter:idle:idle_in", "update:idle:idle_up"]
        );

        tick_lua_phases(&mut world);
        assert_eq!(
            take_calls(&world),
            ["update:idle:idle_up"],
            "enter fires only once"
        );

        world.get_mut::<LuaPhase>(entity).unwrap().phase.next = Some("run".into());
        tick_lua_phases(&mut world);
        assert_eq!(
            take_calls(&world),
            ["exit:run:idle_out", "enter:run:run_in", "update:run:run_up"]
        );

        let phase = world.get::<LuaPhase>(entity).unwrap();
        assert_eq!(phase.phase.previous.as_deref(), Some("idle"));
        assert!(phase.phase.next.is_none());
        assert!(approx_eq(phase.phase.time_in_phase, 0.25));
    }

    /// A transition returned by `on_update` is queued into `next` and applied
    /// on the following tick, not the current one.
    #[test]
    fn lua_phase_returned_transition_applies_next_tick() {
        let (mut world, entity) = make_recording_world(0.25);
        world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .set("go_to", "run")
            .expect("set go_to");

        tick_lua_phases(&mut world);
        take_calls(&world);
        let phase = world.get::<LuaPhase>(entity).unwrap();
        assert_eq!(phase.phase.current, "idle");
        assert_eq!(phase.phase.next.as_deref(), Some("run"));

        tick_lua_phases(&mut world);
        assert_eq!(
            take_calls(&world),
            ["exit:run:idle_out", "enter:run:run_in", "update:run:run_up"]
        );
    }
}
