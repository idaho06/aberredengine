use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SingleThreadedExecutor;

use super::builder::EngineBuilder;
use super::registrar::UpdateRegistrar;
use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::error::EngineError;
use crate::resources::drawable_snapshot::build_drawable_snapshot;
use crate::systems::animation::animation;
use crate::systems::animation::animation_controller;
use crate::systems::audio_bridge::{
    forward_audio_cmds, land_audio_stats, poll_audio_messages, update_bevy_audio_cmds,
    update_bevy_audio_messages,
};
use crate::systems::camera_follow::camera_follow_system;
use crate::systems::collision_detector::collision_detector;
use crate::systems::dynamictext_size::dynamictext_size_system;
use crate::systems::gamestate::{check_pending_state, state_is_playing};
use crate::systems::gridlayout::gridlayout_spawn_system;
use crate::systems::group::update_group_counts_system;
use crate::systems::gui_hit_test::gui_hit_test_system;
use crate::systems::gui_image_state_sync::gui_image_state_sync_system;
use crate::systems::gui_layout::gui_layout_system;
use crate::systems::gui_progressbar_signal_update::gui_progressbar_signal_update_system;
use crate::systems::gui_spawn::{
    gui_button_spawn_system, gui_image_spawn_system, gui_label_spawn_system,
};
use crate::systems::inputaccelerationcontroller::input_acceleration_controller;
use crate::systems::inputsimplecontroller::input_simple_controller;
use crate::systems::logic_bridge::{forward_render_asset_cmds, send_drawable_snapshot};
use crate::systems::menu::menu_spawn_system;
use crate::systems::mousecontroller::mouse_controller;
use crate::systems::movement::movement;
use crate::systems::particleemitter::particle_emitter_system;
use crate::systems::phase::phase_system;
use crate::systems::propagate_transforms::{
    cleanup_orphaned_global_transforms, propagate_transforms,
};
use crate::systems::render::render_system;
use crate::systems::render_assets::update_bevy_render_asset_cmds;
use crate::systems::scene_dispatch::{scene_switch_poll, scene_update_system};
use crate::systems::signal_intents::apply_signal_intents;
use crate::systems::signalbinding::update_world_signals_binding_system;
use crate::systems::stuckto::stuck_to_entity_system;
use crate::systems::tilemap::tilemap_spawn_system;
use crate::systems::timer::update_timers;
use crate::systems::ttl::ttl_system;
use crate::systems::tween::tween_system;
use crate::systems::window::detect_window_resize;

#[cfg(feature = "lua")]
use crate::systems::lua_setup_entity::lua_setup_entity_system;
#[cfg(feature = "lua")]
use crate::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use crate::systems::luatimer::update_lua_timers;
#[cfg(feature = "lua")]
use crate::systems::mapspawn::process_lua_map_commands;

/// System sets partitioning the logic thread's `sim` schedule pipeline.
/// [`EngineBuilder`]'s
/// `build_logic_schedules` declares one `configure_sets((...).chain())` over
/// this list as the single source of ordering truth between groups; edges
/// *within* a group that are still load-bearing remain explicit `.after()`
/// calls (see that function's doc comment for the convention).
///
/// Exported so [`configure_schedule`](EngineBuilder::configure_schedule)
/// closures can position custom systems relative to engine groups, e.g.
/// `.in_set(SimSet::Movement)` or `.before(SimSet::Collision)`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimSet {
    /// Drain `SignalIntents` queued by the render thread's `GuiCallback` into
    /// `WorldSignals`, before anything this tick reads them.
    ApplyIntents,
    /// One-shot spawns reacting to `Added<T>` (menu/gridlayout/tilemap) and
    /// game-state bookkeeping (`check_pending_state`).
    Spawn,
    /// Audio command/message pump (`update_bevy_audio_cmds` ->
    /// `forward_audio_cmds` -> `poll_audio_messages` ->
    /// `update_bevy_audio_messages`, kept as an explicit `.chain()`).
    AudioPump,
    /// User `on_update`/`add_system` hooks. Lua's
    /// `on_update_<scene>` is dispatched separately, from
    /// `lua_plugin::update` in `SimSet::Bookkeeping`, so it runs after
    /// camera-follow/collision rather than before.
    ScriptUpdate,
    /// Input-driven force/velocity controllers.
    Controllers,
    /// Particle emission, movement integration, TTL, and position/rotation/
    /// scale tweens.
    Movement,
    /// World-space transform propagation and camera follow.
    Transforms,
    /// Collision detection and its direct reactions (`stuck_to`, `phase`).
    Collision,
    /// GUI layout, hit-test, and per-state visual sync.
    Gui,
    /// Group counts, Lua phase callbacks, and animation controller
    /// resolution -- all downstream of this tick's collision results.
    PostCollision,
    /// Lua command-queue draining (map/asset commands, entity setup) and
    /// scene lifecycle polling.
    Drain,
    /// Tail-of-tick housekeeping: signal bindings, text sizing, input
    /// binding change notification, and (for Lua games) `lua_plugin::update`.
    Bookkeeping,
}

impl EngineBuilder {
    /// Registers the non-Lua `PostCollision` pair
    /// (`update_group_counts_system` + `animation_controller`, with no
    /// intra-pair ordering edge, unlike the Lua branch's
    /// `update_group_counts_system.before(lua_phase_system)`/
    /// `animation_controller.after(lua_phase_system)`). Shared by both the
    /// `#[cfg(feature = "lua")] if has_lua {} else {}` else-arm and the
    /// standalone `#[cfg(not(feature = "lua"))]` block in
    /// `build_logic_schedules` -- neither system is Lua-specific, so this
    /// helper compiles unconditionally in both feature configs.
    fn add_non_lua_post_collision(sim: &mut Schedule) {
        sim.add_systems(update_group_counts_system.in_set(SimSet::PostCollision));
        sim.add_systems(animation_controller.in_set(SimSet::PostCollision));
    }

    /// Registers `animation` (`SimSet::Drain`) exactly once, regardless of
    /// feature config -- a single call site so the Lua-only ordering edge
    /// below doesn't require duplicating the base registration per branch.
    /// `.after(lua_setup_entity_system)`, when Lua is active: both touch
    /// Signals/Animation/Sprite (`lua_setup_entity_system`'s ctx-building
    /// reads them for the entity's one-shot setup callback; `animation`
    /// writes them advancing frame_index/tex_key) with no prior edge --
    /// ambiguity_detection flags it. Pinned so setup is considered
    /// "settled" before the entity's first animation-advance tick.
    fn add_animation_system(sim: &mut Schedule, has_lua: bool) {
        #[cfg(feature = "lua")]
        if has_lua {
            sim.add_systems(animation.after(lua_setup_entity_system).in_set(SimSet::Drain));
            return;
        }
        #[cfg(not(feature = "lua"))]
        let _ = has_lua;
        sim.add_systems(animation.in_set(SimSet::Drain));
    }

    /// Build the two schedules the logic thread runs: `sim`
    /// (runs once per `Pacer`-paced sim tick at `[simulation] hz`, real dt --
    /// this is where essentially all gameplay logic lives: movement,
    /// collision, phases, animation, Lua scripting, GUI layout/hit-test,
    /// scene lifecycle, and per-tick housekeeping) and
    /// `present` (decimated to `[simulation] snapshot_skip` sim ticks, not run
    /// once per received input sample -- package the tick's
    /// fully-settled state into a `DrawableSnapshot` and publish it into the
    /// `SnapshotPublisher` triple buffer; nothing else). See
    /// `.claude/context/system-order.md` for the rationale behind the split
    /// and the full list of which system lives where.
    ///
    /// `sim`'s internal ordering is expressed via [`SimSet`]
    /// rather than per-system `.after()`/`.before()` edges: one
    /// `configure_sets((...).chain())` call declares the pipeline, and each
    /// system joins its group with `.in_set(SimSet::X)`. Only *intra*-set
    /// edges that are still load-bearing (e.g.
    /// `cleanup_orphaned_global_transforms.after(propagate_transforms)`
    /// within `Transforms`) are kept explicit -- cross-group ordering is
    /// implied by set order.
    ///
    /// `bevy_ecs` cannot express `.after()`/`.before()` across two separate
    /// `Schedule`s, so `present`'s `.after()` markers referencing `sim`
    /// systems (e.g. `build_drawable_snapshot`'s list below) remain vacuous
    /// doc-value markers, not real constraints -- `logic_thread_main`'s loop
    /// structure guarantees the tick's `sim` run completes before
    /// `present` runs, which is the ordering those edges express.
    pub(crate) fn build_logic_schedules(
        update_hook: Option<UpdateRegistrar>,
        extra_systems: Vec<UpdateRegistrar>,
        world: &mut World,
        has_lua: bool,
        use_scene_manager: bool,
    ) -> Result<(Schedule, Schedule), EngineError> {
        let mut sim = Schedule::default();
        // Pinned unconditionally (not gated behind a deterministic-mode
        // flag): removes cross-run interleaving nondeterminism in Commands
        // application / entity allocation order (determinism-02). Mirrors
        // the audio thread's own schedule (src/systems/audio/world.rs).
        // Only affects which systems can interleave -- auto_insert_apply_
        // deferred (Commands-flush ordering) is executor-independent, so
        // command-visibility timing is unchanged.
        sim.set_executor(SingleThreadedExecutor::new());

        Self::configure_sim_sets(&mut sim);
        Self::add_engine_sim_systems(&mut sim, has_lua);
        Self::apply_user_registrars(&mut sim, update_hook, extra_systems);
        Self::add_scene_manager_systems(&mut sim, use_scene_manager);

        let mut present = Self::build_present_schedule();
        present.set_executor(SingleThreadedExecutor::new());

        sim.initialize(world)
            .map_err(|source| EngineError::ScheduleInit {
                which: "sim",
                source,
            })?;
        present
            .initialize(world)
            .map_err(|source| EngineError::ScheduleInit {
                which: "present",
                source,
            })?;

        Ok((sim, present))
    }

    /// Single source of truth for cross-group ordering within `sim` --
    /// see `SimSet`'s doc comment for what each
    /// group holds. Systems join a group via `.in_set(SimSet::X)`; only
    /// load-bearing *intra*-group edges remain as explicit `.after()`
    /// calls in [`Self::add_engine_sim_systems`].
    fn configure_sim_sets(sim: &mut Schedule) {
        sim.configure_sets(
            (
                SimSet::ApplyIntents,
                SimSet::Spawn,
                SimSet::AudioPump,
                SimSet::ScriptUpdate,
                SimSet::Controllers,
                SimSet::Movement,
                SimSet::Transforms,
                SimSet::Collision,
                SimSet::Gui,
                SimSet::PostCollision,
                SimSet::Drain,
                SimSet::Bookkeeping,
            )
                .chain(),
        );
    }

    /// Registers the engine's own `sim`-schedule systems (everything except
    /// user-supplied hooks/systems, scene-manager systems, and the
    /// `present` schedule -- see [`Self::apply_user_registrars`],
    /// [`Self::add_scene_manager_systems`], [`Self::build_present_schedule`]).
    fn add_engine_sim_systems(sim: &mut Schedule, has_lua: bool) {
        // --- SIM: signal intents + state bookkeeping, one-shot spawns ---
        // apply_signal_intents runs first, before everything else this
        // tick (in particular before on_update_<scene>, dispatched from
        // lua_plugin::update in SimSet::Bookkeeping): intents queued by the
        // render thread's GuiCallback (inside render_system) each frame must
        // be visible to this tick's scene logic. The ordering is
        // implied by SimSet::ApplyIntents preceding every later group in
        // the chain.
        sim.add_systems(apply_signal_intents.in_set(SimSet::ApplyIntents));
        sim.add_systems(menu_spawn_system.in_set(SimSet::Spawn));
        sim.add_systems(gridlayout_spawn_system.in_set(SimSet::Spawn));
        // .after(menu_spawn_system): both queue into the logic world's
        // Messages<RenderAssetCmd> (RasterizeText / TilemapTexture) with no
        // prior edge between them -- ambiguity_detection flags the shared
        // writer access. Pinned to registration order; the two queued
        // commands are independent (process_render_asset_cmds treats the
        // queue as an unordered batch of loads), so this only silences a
        // latent-fragility warning, not a real behavior fix.
        sim.add_systems(
            tilemap_spawn_system
                .after(menu_spawn_system)
                .in_set(SimSet::Spawn),
        );
        sim.add_systems(check_pending_state.in_set(SimSet::Spawn));
        sim.add_systems(
            (
                update_bevy_audio_cmds,
                forward_audio_cmds,
                poll_audio_messages,
                update_bevy_audio_messages,
                land_audio_stats,
            )
                .chain()
                .in_set(SimSet::AudioPump),
        );
        // update_bevy_render_asset_cmds + forward_render_asset_cmds live on
        // the tail of `sim` (SimSet::Bookkeeping): with `present` decimated
        // to `[simulation] snapshot_skip` sim ticks (see `logic_thread_main`), asset
        // loads must still reach the render thread every sim tick, not just
        // on a publish tick, or a texture could sit queued for several ticks
        // before a snapshot referencing it is even built. See that block's
        // own comment for the real `.after()` edge between the pair.

        // --- SIM: input-driven forces/movement (InputState is resolved
        // sim-side once per tick by resolve_input_backlog, before this tick's
        // sim.run(), and held constant for every system that reads it here) ---
        sim.add_systems(input_simple_controller.in_set(SimSet::Controllers));
        // .ambiguous_with(input_simple_controller): both hold
        // Query<&mut RigidBody> (InputControlled vs AccelerationControlled
        // entities are disjoint in practice, but ambiguity_detection can't
        // see past the marker component to prove that) -- a genuine false
        // positive, not a real ordering requirement, so marked
        // ambiguous_with rather than given an arbitrary .after() that would
        // misread as "order matters here."
        sim.add_systems(
            input_acceleration_controller
                .ambiguous_with(input_simple_controller)
                .in_set(SimSet::Controllers),
        );
        sim.add_systems(mouse_controller.in_set(SimSet::Controllers));
        sim.add_systems(
            particle_emitter_system
                .before(movement)
                .in_set(SimSet::Movement),
        );
        sim.add_systems(movement.in_set(SimSet::Movement));
        sim.add_systems(ttl_system.after(movement).in_set(SimSet::Movement));
        // .before(particle_emitter_system): both write MapPosition with no
        // prior edge (ambiguity_detection flags it; also transitively
        // resolves tween_system::<MapPosition> vs movement, since
        // particle_emitter_system is already .before(movement)). A tweened
        // entity that's also a particle emitter (e.g. a projectile flying a
        // tween path with a trail) should spawn particles from this tick's
        // already-tweened position, not last tick's.
        sim.add_systems(
            tween_system::<MapPosition>
                .before(particle_emitter_system)
                .in_set(SimSet::Movement),
        );
        sim.add_systems(tween_system::<Rotation>.in_set(SimSet::Movement));
        sim.add_systems(tween_system::<Scale>.in_set(SimSet::Movement));
        // propagate_transforms/collision_detector's old .after(movement)/
        // .after(tween_system::<T>) edges are now implied by
        // SimSet::Movement preceding SimSet::Transforms/Collision.
        sim.add_systems(propagate_transforms.in_set(SimSet::Transforms));
        sim.add_systems(
            cleanup_orphaned_global_transforms
                .after(propagate_transforms)
                .in_set(SimSet::Transforms),
        );
        sim.add_systems(
            camera_follow_system
                .after(propagate_transforms)
                .in_set(SimSet::Transforms),
        );
        sim.add_systems(collision_detector.in_set(SimSet::Collision));
        sim.add_systems(
            stuck_to_entity_system
                .after(collision_detector)
                .in_set(SimSet::Collision),
        );
        // .after(stuck_to_entity_system): both write MapPosition with no
        // prior edge (ambiguity_detection flags it). Phase logic (state
        // machines, e.g. ground/patrol checks) should react to an entity's
        // final, stuck-to-resolved position this tick, not a pre-stuck-to
        // one.
        sim.add_systems(
            phase_system
                .after(collision_detector)
                .after(stuck_to_entity_system)
                .in_set(SimSet::Collision),
        );

        // --- SIM: GUI (tween_system::<ScreenPosition> feeds GUI layout, not
        // collision, so it's grouped with the GUI chain rather than its
        // MapPosition/Rotation/Scale siblings above; all four TweenValue
        // type parameters share sim cadence, so LuaOnTweenFinished<T> fires
        // at the same rate regardless of which T is tweened).
        sim.add_systems(tween_system::<ScreenPosition>.in_set(SimSet::Gui));
        // ambiguous_with: both mutate GuiThemeWarnCache (warn-once set
        // insert, commutative) -- a genuine false positive, not a real
        // ordering requirement.
        sim.add_systems(
            (
                gui_button_spawn_system,
                gui_label_spawn_system.ambiguous_with(gui_button_spawn_system),
                gui_image_spawn_system,
            )
                .before(gui_layout_system)
                .in_set(SimSet::Gui),
        );
        sim.add_systems(
            gui_layout_system
                .after(tween_system::<ScreenPosition>)
                .in_set(SimSet::Gui),
        );
        sim.add_systems(
            gui_hit_test_system
                .after(gui_layout_system)
                .in_set(SimSet::Gui),
        );
        sim.add_systems(
            gui_image_state_sync_system
                .after(gui_hit_test_system)
                .in_set(SimSet::Gui),
        );
        sim.add_systems(gui_progressbar_signal_update_system.in_set(SimSet::Gui));

        #[cfg(feature = "lua")]
        if has_lua {
            sim.add_systems(
                update_group_counts_system
                    .before(lua_phase_system)
                    .in_set(SimSet::PostCollision),
            );
            sim.add_systems(
                lua_phase_system
                    .run_if(state_is_playing)
                    .in_set(SimSet::PostCollision),
            );
            sim.add_systems(
                animation_controller
                    .after(lua_phase_system)
                    .in_set(SimSet::PostCollision),
            );
            // .after(lua_phase_system): both touch Timer<LuaTimerCallback>
            // (lua_phase_system's ctx-building reads it for ctx.timer;
            // update_lua_timers advances it) with no prior edge --
            // ambiguity_detection flags it. Pinned so a phase callback's
            // ctx.timer reflects this tick's pre-advance state, consistent
            // with ctx.time_in_phase's own semantics.
            sim.add_systems(
                update_lua_timers
                    .after(lua_phase_system)
                    .in_set(SimSet::PostCollision),
            );
            // `.after(lua_plugin::update)` on both systems: a queued
            // map/asset load is drained after on_update_<scene> has had a
            // chance to queue it this same tick, not before.
            // `.before(update_bevy_render_asset_cmds)` keeps a same-tick map
            // load's RenderAssetCmd forwarded to the render thread this same
            // tick rather than lagging one tick (both share
            // SimSet::Bookkeeping, so this ordering needs an explicit edge).
            sim.add_systems(
                process_lua_map_commands
                    .after(crate::lua_plugin::update)
                    .before(update_bevy_render_asset_cmds)
                    .in_set(SimSet::Bookkeeping),
            );
            sim.add_systems(
                crate::lua_plugin::process_lua_asset_commands
                    .run_if(state_is_playing)
                    .after(crate::lua_plugin::update)
                    .before(update_bevy_render_asset_cmds)
                    .in_set(SimSet::Bookkeeping),
            );
            // Fires the sim tick after spawn (up to 1/sim_hz latency).
            sim.add_systems(
                lua_setup_entity_system
                    .run_if(state_is_playing)
                    .in_set(SimSet::Drain),
            );
        } else {
            Self::add_non_lua_post_collision(sim);
        }

        #[cfg(not(feature = "lua"))]
        {
            // `has_lua` only exists to keep the build_schedules signature uniform
            // across feature combinations.
            let _ = has_lua;
            Self::add_non_lua_post_collision(sim);
        }

        Self::add_animation_system(sim, has_lua);
        sim.add_systems(update_timers.in_set(SimSet::Drain));
        sim.add_systems(update_world_signals_binding_system.in_set(SimSet::Bookkeeping));
        sim.add_systems(detect_window_resize.in_set(SimSet::Bookkeeping));
        sim.add_systems(
            dynamictext_size_system
                .after(update_world_signals_binding_system)
                .in_set(SimSet::Bookkeeping),
        );

        // Forwards RenderAssetCmd to the render thread (the GL drain itself,
        // process_render_asset_cmds, lives on the render schedule). This
        // pair lives on the tail of `sim` (SimSet::Bookkeeping) so it runs
        // every sim tick, independent of the decimated `present`/snapshot-
        // publish rate (see `logic_thread_main`'s doc comment) -- a tick's
        // asset loads must not sit queued for several ticks waiting for the
        // next publish. `.after(update_bevy_render_asset_cmds)` is a REAL
        // same-schedule edge (both land in `SimSet::Bookkeeping`); ordering
        // relative to menu/tilemap spawning (earlier `SimSet`s, via the
        // `configure_sets((...).chain())` pipeline) is implicit, but the Lua
        // asset/map command drains share this same `SimSet` too (running
        // `.after(lua_plugin::update)`) and need their own explicit
        // `.before(update_bevy_render_asset_cmds)` edge so a same-tick load
        // still forwards this tick instead of lagging one.
        sim.add_systems(update_bevy_render_asset_cmds.in_set(SimSet::Bookkeeping));
        sim.add_systems(
            forward_render_asset_cmds
                .after(update_bevy_render_asset_cmds)
                .in_set(SimSet::Bookkeeping),
        );
    }

    /// Applies user-supplied `sim`-schedule registrars: the single
    /// `update_hook` installed by `.on_update()`, plus every
    /// `extra_systems` closure installed by `.add_system()`/
    /// `.configure_schedule()`. Each closure supplies its own
    /// `.in_set(SimSet::X)` (see `on_update`/`add_system`/`with_lua`'s hook
    /// installation for the concrete sets used); `configure_schedule`
    /// closures are free to pick any `SimSet` (exported for exactly this).
    fn apply_user_registrars(
        sim: &mut Schedule,
        update_hook: Option<UpdateRegistrar>,
        extra_systems: Vec<UpdateRegistrar>,
    ) {
        if let Some(update_hook) = update_hook {
            update_hook(sim);
        }

        for extra in extra_systems {
            extra(sim);
        }
    }

    /// Registers the SceneManager's own `sim`-schedule systems, when
    /// `.add_scene()` was used.
    /// Accepted consequence: a scene switch resolved mid-tick runs the rest
    /// of that tick's pipeline against the newly-spawned scene, so the
    /// snapshot published at the end of that tick may show it partially
    /// constructed -- same accepted tradeoff as the Lua-direct switch path
    /// (see `lua_plugin::update`'s doc comment and `with_lua()`'s
    /// `update_hook` installation). `.add_scene()` (`use_scene_manager`)
    /// and Lua's `.with_lua()` are mutually exclusive (`validate_builder`
    /// rejects both `switch_scene_hook` and `use_scene_manager`), so this
    /// and the Lua-direct path never run together.
    fn add_scene_manager_systems(sim: &mut Schedule, use_scene_manager: bool) {
        if use_scene_manager {
            sim.add_systems(
                scene_update_system
                    .run_if(state_is_playing)
                    .in_set(SimSet::Drain),
            );
            sim.add_systems(
                scene_switch_poll
                    .run_if(state_is_playing)
                    .after(scene_update_system)
                    .in_set(SimSet::Drain),
            );
        }
    }

    /// Builds (but does not initialize) the logic thread's `present`
    /// schedule: package the tick's fully-settled state into a
    /// `DrawableSnapshot` and publish it. See
    /// [`Self::build_logic_schedules`]'s doc comment for the `sim`/`present`
    /// split rationale.
    fn build_present_schedule() -> Schedule {
        let mut present = Schedule::default();

        #[allow(unused_mut)] // only reassigned under #[cfg(feature = "lua")] below
        let mut drawable_snapshot_config = build_drawable_snapshot
            .after(gui_hit_test_system)
            .after(gui_image_state_sync_system)
            .after(gui_progressbar_signal_update_system)
            .after(dynamictext_size_system)
            .after(scene_switch_poll)
            .before(render_system);
        #[cfg(feature = "lua")]
        {
            drawable_snapshot_config = drawable_snapshot_config
                .after(crate::lua_plugin::update)
                .after(process_lua_map_commands);
        }
        present.add_systems(drawable_snapshot_config);

        // Tail of the logic `present` schedule: ship this frame's
        // fully-settled snapshot to the render thread. apply_gameconfig_changes
        // + render_system live on the render thread's own schedule
        // (build_render_schedule). InputBindings is logic-thread-only (no
        // render-side mirror to refresh), so there is no bindings-sync system
        // here.
        present.add_systems(send_drawable_snapshot.after(build_drawable_snapshot));

        present
    }
}

#[cfg(test)]
mod ambiguity_audit {
    //! Regression gate (determinism-02-deterministic-schedule.md §2):
    //! builds the real `sim` schedule (via the engine's own
    //! `configure_sim_sets`/`add_engine_sim_systems`, not a hand-listed
    //! system set) with `ambiguity_detection: LogLevel::Warn` and asserts
    //! zero ambiguities remain. Every ambiguity bevy 0.19 reported as of
    //! this audit (2026-07-19) was triaged and closed with an explicit
    //! `.after()`/`.before()` edge at its registration site in
    //! `add_engine_sim_systems` (see the comments there) -- most pin a real
    //! preferred order, one (`gui_label_spawn_system`/`gui_button_spawn_system`)
    //! is a documented false positive (commutative set-insert) pinned only
    //! to silence the warning. A future system addition that reintroduces
    //! an ambiguity should fail this test; run with
    //! `RUST_LOG=warn cargo test ambiguity_audit -- --nocapture` to have
    //! bevy's own `warn!` logging (emitted from inside
    //! `Schedule::initialize` while the graph still has system names
    //! resolved -- `ScheduleBuildWarning::to_string` panics if called after
    //! the fact, once `initialize` has moved systems into the executable)
    //! print the new finding's human-readable description, then either add
    //! an edge or an `.ambiguous_with()` with a comment explaining why it's
    //! safe.
    use bevy_ecs::schedule::{LogLevel, ScheduleBuildSettings};

    use super::*;

    fn assert_no_ambiguities(has_lua: bool) {
        let _ = env_logger::builder().is_test(true).try_init();

        let mut world = World::new();
        let mut sim = Schedule::default();
        sim.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Warn,
            ..Default::default()
        });
        EngineBuilder::configure_sim_sets(&mut sim);
        EngineBuilder::add_engine_sim_systems(&mut sim, has_lua);

        let metadata = sim
            .initialize(&mut world)
            .expect("sim schedule must still build successfully under ambiguity_detection=Warn");

        let warning_count = metadata.map(|m| m.warnings.len()).unwrap_or(0);
        assert_eq!(
            warning_count, 0,
            "new system ambiguity detected (has_lua={has_lua}) -- rerun with \
             RUST_LOG=warn cargo test ambiguity_audit -- --nocapture to see \
             which systems conflict, then add an .after()/.before() edge or \
             a documented .ambiguous_with()"
        );
    }

    #[test]
    fn sim_schedule_has_no_ambiguities_lua() {
        assert_no_ambiguities(true);
    }

    #[test]
    fn sim_schedule_has_no_ambiguities_no_lua() {
        assert_no_ambiguities(false);
    }
}
