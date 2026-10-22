//! Lua-driven game lifecycle: asset loading, scene management, and per-frame update.
//!
//! # Key Functions
//!
//! - [`setup`] – calls `on_setup` in Lua to queue asset loads, then drains them into stores
//! - [`enter_play`] – calls `on_enter_play`, processes initial signals/groups, enters the first scene
//! - [`update`] – refreshes Lua caches, calls `on_update_<scene>`, drains command queues
//!
//! Scene switches run core's `scene_switch_system`; the Lua side of a switch
//! lives in the scene observers ([`crate::systems::lua_scene`]).
//!
//! # SystemParam Bundles
//!
//! - [`ScriptingContext`] – `LuaRuntime` + audio command writer
//! - [`GameSceneState`] – world signals, post-process, config, camera follow, stores

use crate::resources::lua_runtime::{
    AnimationCmd, AssetCmd, CameraFollowCmd, GameConfigCmd, GroupCmd, InputCmd, LuaRuntime,
    Ordered, RenderCmd,
};
use aberred_core::protocol::audio::AudioCmd;
use aberred_core::protocol::render_assets::RenderAssetCmd;
use aberred_core::resources::animation_store::AnimationStore;
use aberred_core::resources::camera_follow_config::CameraFollowConfig;
use aberred_core::resources::camera2d::MainCamera;
use aberred_core::resources::game_config::GameConfig;
use aberred_core::resources::game_state::NextGameState;
use aberred_core::resources::group::TrackedGroups;
use aberred_core::resources::gui_theme::{GuiThemeStore, GuiThemeWarnCache};
use aberred_core::resources::input::InputState;
use aberred_core::resources::input_bindings::InputBindings;
use aberred_core::resources::next_scene::NextScene;
use aberred_core::resources::post_process_shader::PostProcessShader;
use aberred_core::resources::render_size::RenderSize;
use aberred_core::resources::scene_manager::SceneManager;
use aberred_core::resources::systems_store::SystemsStore;

use crate::systems::lua_commands::{
    DrainScope, EffectCmdBufs, SignalTargets, asset_cmd_to_audio_cmd,
    asset_cmd_to_render_asset_cmd, drain_and_process_effect_commands, process_animation_command,
    process_camera_follow_command, process_gameconfig_command, process_group_command,
    process_input_command, process_render_command, process_signal_command, translate_asset_command,
};
use aberred_core::resources::camera2d::Camera2D;
use aberred_core::resources::signal_keys as sk;
use aberred_core::resources::world_signals::WorldSignals;
use aberred_core::resources::world_time::WorldTime;
use aberred_core::systems::scene_dispatch::switch_to;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use log::{debug, error, info};
use mlua::prelude::LuaTable;

/// Bundled Lua runtime + audio command writer for scripting systems.
#[derive(SystemParam)]
pub struct ScriptingContext<'w> {
    pub lua_runtime: NonSend<'w, LuaRuntime>,
    pub audio_cmd_writer: MessageWriter<'w, AudioCmd>,
}

/// Bundled game scene state resources.
#[derive(SystemParam)]
pub struct GameSceneState<'w> {
    pub world_signals: ResMut<'w, WorldSignals>,
    pub next_scene: ResMut<'w, NextScene>,
    pub next_game_state: ResMut<'w, NextGameState>,
    pub post_process: ResMut<'w, PostProcessShader>,
    pub config: ResMut<'w, GameConfig>,
    pub camera_follow: ResMut<'w, CameraFollowConfig>,
    pub systems_store: Res<'w, SystemsStore>,
    pub anim_store: ResMut<'w, AnimationStore>,
}

/// Persistent per-frame buffers for the command queues drained by [`DrainCtx`].
///
/// [`DrainCtx`] holds one in a `Local`, so the Vecs retain heap capacity
/// across frames.
#[derive(Default)]
pub struct CommonCmdBufs {
    effects: EffectCmdBufs,
    render: Vec<RenderCmd>,
    gui_theme: Vec<RenderCmd>,
    gameconfig: Vec<GameConfigCmd>,
    camera_follow: Vec<CameraFollowCmd>,
    input: Vec<InputCmd>,
    animation: Vec<AnimationCmd>,
    group: Vec<GroupCmd>,
}

// This function is meant to load all resources
//
// `setup()` is an ordinary `RenderAssetCmd` producer: it
// queues asset loads into `Messages<RenderAssetCmd>` (forwarded to the
// render thread, which performs the GL work and replies with
// `FontLoaded`/`TextureLoaded` notifications). The logic world has no GL
// resources at all, so bootstrap fonts' metrics arrive asynchronously
// (dynamic text/menu measurement retries each frame until they land).
pub fn setup(
    mut commands: Commands,
    render_size: Res<RenderSize>,
    mut render_asset_writer: MessageWriter<RenderAssetCmd>,
    mut scripting: ScriptingContext,
) {
    // Default camera. Needed to start the engine before entering play state
    // The camera will be overridden later in the scene setup.
    commands.insert_resource(MainCamera(Camera2D::screen_centered(&render_size)));

    let lua_runtime = &scripting.lua_runtime;

    // Call Lua on_setup function to queue asset loading commands
    if lua_runtime.has_function("on_setup")
        && let Err(e) = lua_runtime.call_function::<_, ()>("on_setup", ())
    {
        error!("Error calling on_setup: {}", e);
    }

    // Process asset commands queued by Lua (setup runs once; no persistent
    // buffer needed): Music/Sound to the audio thread, the rest queued as
    // RenderAssetCmds for the render side.
    let mut asset_buf = Vec::new();
    lua_runtime.drain_asset_commands_into(&mut asset_buf);
    for cmd in asset_buf {
        match asset_cmd_to_audio_cmd(cmd) {
            Ok(audio_cmd) => {
                scripting.audio_cmd_writer.write(audio_cmd);
            }
            Err(other) => {
                if let Some(render_cmd) = asset_cmd_to_render_asset_cmd(other) {
                    render_asset_writer.write(render_cmd);
                }
            }
        }
    }

    // Process animation registration commands from Lua
    let mut anim_store = AnimationStore::default();
    let mut anim_buf = Vec::new();
    lua_runtime.drain_animation_commands_into(&mut anim_buf);
    for cmd in anim_buf {
        process_animation_command(&mut anim_store, cmd);
    }
    commands.insert_resource(anim_store);
    info!("Game setup() done");
}

pub use aberred_core::systems::game_state::quit_game;

/// The `enter_play` hook of a Lua game: calls `on_enter_play`, applies the
/// signal and group commands it queued, then enters the first scene through
/// the `switch_scene` hook -- the scene `on_enter_play` requested with
/// `engine.request_scene`, else [`sk::DEFAULT_SCENE`]. The first entry tears
/// nothing down, so entities spawned in `on_setup`/`on_enter_play` survive it.
pub fn enter_play(
    commands: Commands,
    mut world_signals: ResMut<WorldSignals>,
    mut next_scene: ResMut<NextScene>,
    mut next_game_state: ResMut<NextGameState>,
    mut tracked_groups: ResMut<TrackedGroups>,
    systems_store: Res<SystemsStore>,
    lua_runtime: NonSend<LuaRuntime>,
) {
    // Call Lua on_enter_play function if it exists
    if lua_runtime.has_function("on_enter_play") {
        match lua_runtime.call_function::<_, String>("on_enter_play", ()) {
            Ok(result) => {
                debug!("Lua on_enter_play returned: {}", result);
            }
            Err(e) => {
                error!("Error calling on_enter_play: {}", e);
            }
        }
    }

    // enter_play runs once; stack-local buffers are sufficient
    let mut signal_buf = Vec::new();
    lua_runtime.drain_signal_commands_into(&mut signal_buf);
    let mut signals = SignalTargets {
        world_signals: &mut world_signals,
        next_scene: &mut next_scene,
        next_game_state: &mut next_game_state,
    };
    for Ordered { cmd, .. } in signal_buf {
        process_signal_command(&mut signals, cmd);
    }

    let mut group_buf = Vec::new();
    lua_runtime.drain_group_commands_into(&mut group_buf);
    for cmd in group_buf {
        process_group_command(&mut tracked_groups, cmd);
    }

    // Update the tracked groups cache for Lua
    lua_runtime.update_tracked_groups_cache(&tracked_groups.groups);

    let scene = next_scene
        .take()
        .unwrap_or_else(|| sk::DEFAULT_SCENE.to_owned());
    switch_to(commands, next_scene, &systems_store, scene);
}

/// The resources Lua's common command queues apply to, plus reusable drain
/// buffers: the drain [`update`] and the scene-entered observer
/// ([`lua_scene_entered_observer`](crate::systems::lua_scene::lua_scene_entered_observer))
/// run after their Lua callbacks return.
#[derive(SystemParam)]
pub struct DrainCtx<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub scripting: ScriptingContext<'w>,
    pub scene_state: GameSceneState<'w>,
    pub bindings: ResMut<'w, InputBindings>,
    pub tracked_groups: ResMut<'w, TrackedGroups>,
    pub gui_theme_store: Res<'w, GuiThemeStore>,
    pub gui_theme_warn_cache: ResMut<'w, GuiThemeWarnCache>,
    bufs: Local<'s, CommonCmdBufs>,
}

impl DrainCtx<'_, '_> {
    /// Drains and processes the common command queues.
    pub(crate) fn drain(&mut self) {
        let Self {
            commands,
            scripting,
            scene_state,
            bindings,
            tracked_groups,
            gui_theme_store,
            gui_theme_warn_cache,
            bufs,
        } = self;
        let lua_runtime: &LuaRuntime = &scripting.lua_runtime;
        let audio_cmd_writer = &mut scripting.audio_cmd_writer;
        let bufs: &mut CommonCmdBufs = bufs;
        let bindings: &mut InputBindings = bindings;
        let tracked_groups: &mut TrackedGroups = tracked_groups;
        let gui_theme_store: &GuiThemeStore = gui_theme_store;
        let gui_theme_warn_cache: &mut GuiThemeWarnCache = gui_theme_warn_cache;

        // Animation registrations write AnimationStore immediately, so a same-batch
        // SetAnimation (which reads the store when it applies, at the flush) sees them.
        lua_runtime.drain_animation_commands_into(&mut bufs.animation);
        for cmd in bufs.animation.drain(..) {
            process_animation_command(&mut scene_state.anim_store, cmd);
        }

        drain_and_process_effect_commands(
            lua_runtime,
            DrainScope::Regular,
            &mut bufs.effects,
            commands,
            &mut SignalTargets {
                world_signals: &mut scene_state.world_signals,
                next_scene: &mut scene_state.next_scene,
                next_game_state: &mut scene_state.next_game_state,
            },
            audio_cmd_writer,
            &scene_state.systems_store,
        );

        lua_runtime.drain_render_commands_into(&mut bufs.render);
        // gui_theme_commands is a separate, `preserve`-policy queue (see
        // queue_registry.rs) so a pending `set_gui_theme_*` call survives the
        // clear_all_commands() of a scene switch — unlike render_commands
        // (post-process, `clear` policy), which is wiped every scene switch since
        // post-process commands may reference a scene's about-to-be-despawned
        // state.
        lua_runtime.drain_gui_theme_commands_into(&mut bufs.gui_theme);
        if !bufs.render.is_empty() || !bufs.gui_theme.is_empty() {
            // Seed/write-back only when there's actually a render command to apply —
            // avoids cloning the whole GuiThemeStore and re-inserting it (marking it
            // "changed" for any Changed<GuiThemeStore> consumer) on every frame when
            // nothing was queued. Seeding from the current store (not a blank one)
            // means a SetGuiTheme* command for one key never disturbs any other
            // key already persisted in the resource.
            let mut gui_theme_staging = gui_theme_store.clone();
            for cmd in bufs.render.drain(..).chain(bufs.gui_theme.drain(..)) {
                process_render_command(cmd, &mut scene_state.post_process, &mut gui_theme_staging);
            }
            // Re-validate every staged theme's button skin (not just the ones a
            // command touched this batch) -- cheap (a handful of themes, one
            // bool check each) and avoids re-deriving "which keys changed" via a
            // second match over the just-drained commands.
            for (key, theme) in gui_theme_staging.themes.iter_mut() {
                if !theme.drop_invalid_button_skin()
                    && gui_theme_warn_cache.warn_once_invalid_button_skin(key)
                {
                    error!(
                        "GuiTheme '{key}' has button set but its 'normal' nine-patch was never set via \
                         engine.set_gui_theme_button(\"{key}\", \"normal\", ...) — button theme dropped, buttons render with no background"
                    );
                }
                if !theme.drop_invalid_progress_bar_skin()
                    && gui_theme_warn_cache.warn_once_invalid_progress_bar_skin(key)
                {
                    error!(
                        "GuiTheme '{key}' has progress_bar set but its 'fill' nine-patch was never set via \
                         engine.set_gui_theme_progress_bar(\"{key}\", \"fill\", ...) — progress bar theme dropped, bars render with no fill"
                    );
                }
            }
            commands.insert_resource(gui_theme_staging);
        }

        lua_runtime.drain_gameconfig_commands_into(&mut bufs.gameconfig);
        for cmd in bufs.gameconfig.drain(..) {
            process_gameconfig_command(cmd, &mut scene_state.config);
        }

        lua_runtime.drain_camera_follow_commands_into(&mut bufs.camera_follow);
        for cmd in bufs.camera_follow.drain(..) {
            process_camera_follow_command(cmd, &mut scene_state.camera_follow);
        }

        lua_runtime.drain_input_commands_into(&mut bufs.input);
        for cmd in bufs.input.drain(..) {
            process_input_command(cmd, bindings);
        }

        lua_runtime.drain_group_commands_into(&mut bufs.group);
        if !bufs.group.is_empty() {
            for cmd in bufs.group.drain(..) {
                process_group_command(tracked_groups, cmd);
            }
            lua_runtime.update_tracked_groups_cache(&tracked_groups.groups);
        }
    }
}

/// Refreshes `cached` to `"on_update_{scene}"`, but only rebuilds it when it
/// doesn't already match -- avoids reallocating/rewriting the cached callback
/// name on every call when the scene hasn't changed since the last one.
/// [`update`]'s only caller.
fn refresh_cached_callback_name(cached: &mut String, scene: &str) {
    const PREFIX: &str = "on_update_";
    if cached.get(PREFIX.len()..) != Some(scene) {
        cached.clear();
        cached.push_str(PREFIX);
        cached.push_str(scene);
    }
}

/// Builds the Lua input table for this tick from `input`, logging and
/// returning `None` on failure. Called once per sim tick by [`update`] (its
/// only caller) -- delegates to `LuaRuntime::resolve_input_table`, whose
/// `(frame_count, InputSnapshot)` diff-guard makes repeated calls within the
/// same tick no-ops without needing to build an `InputSnapshot` at all.
fn resolve_input_table(
    lua_runtime: &LuaRuntime,
    input: &InputState,
    frame_count: u64,
) -> Option<LuaTable> {
    match lua_runtime.resolve_input_table(input, frame_count) {
        Ok(table) => Some(table),
        Err(e) => {
            error!("Error creating input table: {}", e);
            None
        }
    }
}

/// Per-sim-tick update system for scene gameplay logic: refreshes Lua caches,
/// calls `on_update_<scene>` of the [`SceneManager`]'s active scene once per
/// sim tick, then drains common command queues. No callback runs before the
/// first scene, nor on the tick a scene is entered ([`SceneManager::entered_at`]):
/// a scene starts updating the tick after its entry, like a Rust scene
/// system. Scene requests switch at `scene_switch_poll` (`SimSet::Lifecycle`),
/// so a request made here switches at the next tick's `Lifecycle`.
///
/// Runs in `SimSet::LateUpdate`, between `SimSet::Lifecycle` and
/// `SimSet::Bookkeeping`, i.e. *after* `Movement`/`Transforms` (so
/// `camera_follow_system` has already updated `MainCamera` for this tick)
/// and after `Collision`/`PostCollision` (so `collision_detector`/
/// `lua_phase_callbacks_system` have already run). This restores the ordering guarantee the pre-thread-split engine
/// had via an explicit `.after(camera_follow_system)` constraint on its
/// single `lua_plugin::update` ("ensures Lua reads current-frame camera
/// state") -- scene logic that reads `engine.get_camera()`/
/// `get_camera_view_rect()` (e.g. parallax pinning) sees this tick's fresh
/// camera, not a stale one.
///
/// Collision-derived flags (`on_ground`, `touching_wall_*`) are cleared here
/// too, since `collision_detector` only ever *sets* them on overlap and never
/// clears them -- something has to reset them exactly once per tick, or a
/// flag would survive after the corresponding overlap ends. Clearing here
/// (after this tick's `collision_detector`/`lua_phase_callbacks_system` have already
/// consumed the fresh flags) is equivalent to clearing before next tick's
/// `collision_detector` runs: nothing between here and then reads these
/// flags, so "before this tick's detection" and "after this tick's
/// consumption, before next tick's detection" describe the same window.
///
/// `dt` is the fixed `1.0 / sim_hz` scaled by `time_scale`, not the real
/// elapsed time (a stall dilates game time), and "continuous while held"
/// logic fires once per sim tick. `bunnymark/{screen,map}_loop.lua`'s
/// held-spawn loops spawn as fast as `sim_hz` allows while the mouse button
/// is held -- a deliberate/disclosed tradeoff for a stress-test scene, not
/// an oversight. A missing callback warns rather than skipping silently,
/// since forgetting `on_update_<scene>` entirely is an authoring error
/// worth flagging -- deduped by name so a genuinely-missing callback logs
/// once, not every tick.
#[allow(clippy::too_many_arguments)]
pub fn update(
    time: Res<WorldTime>,
    input: Res<InputState>,
    camera: Res<MainCamera>,
    render_size: Res<RenderSize>,
    mut ctx: DrainCtx,
    scene_manager: Res<SceneManager>,
    mut cached_callback: Local<String>,
    mut missing_callback_warned: Local<String>,
) {
    aberred_core::tracy::tracy_span!("lua_update");
    let lua_runtime = &ctx.scripting.lua_runtime;
    let delta_sec = time.delta;
    let scene_state = &mut ctx.scene_state;

    // Update signal cache for Lua to read current values
    lua_runtime.sync_signals(&mut scene_state.world_signals);
    lua_runtime.update_gameconfig_cache(&scene_state.config);
    lua_runtime.update_camera_cache(&camera, &render_size, scene_state.config.pixel_snap_camera);
    if ctx.bindings.take_dirty() {
        lua_runtime.update_bindings_cache(&ctx.bindings);
    }

    // The scene the engine entered, not the `scene` string a script can write.
    if !scene_manager.entered_on(time.frame_count)
        && let Some(scene) = scene_manager.active_scene.as_deref()
        && let Some(input_table) = resolve_input_table(lua_runtime, &input, time.frame_count)
    {
        refresh_cached_callback_name(&mut cached_callback, scene);
        match lua_runtime.get_function_cached(cached_callback.as_str()) {
            Ok(Some(func)) => {
                missing_callback_warned.clear();
                lua_runtime.call_resolved(func, cached_callback.as_str(), |func| {
                    func.call::<()>((input_table, delta_sec))
                });
            }
            Ok(None) => {
                if missing_callback_warned.as_str() != cached_callback.as_str() {
                    log::warn!(target: "lua", "Scene callback '{}' not found", cached_callback.as_str());
                    missing_callback_warned.clear();
                    missing_callback_warned.push_str(cached_callback.as_str());
                }
            }
            Err(e) => {
                error!(target: "lua", "Error resolving {}(): {}", cached_callback.as_str(), e);
            }
        }
    }

    ctx.drain();
}

/// Drains `asset_commands` queued from gameplay (`on_update_*`, `on_scene_enter`, phase/timer/
/// collision callbacks) and translates them into `RenderAssetCmd`/`AudioCmd` messages — no GL
/// calls happen here; actual GL loading happens in
/// `process_render_asset_cmds` on the render thread (`aberred-render`, a
/// crate this one does not depend on).
///
/// `setup()` drains this same queue once for `on_setup`-time loads, using its own local buffer;
/// this system is the reachable drain site for any `engine.load_*` call made after setup.
/// Mirrors [`crate::systems::lua_map_spawn::process_lua_map_commands`]. The `RenderAssetCmd`s produced
/// here reach the render thread via `forward_render_asset_cmds` on the sim schedule.
pub fn process_lua_asset_commands(
    lua_runtime: NonSend<LuaRuntime>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    mut render_asset_cmd_writer: MessageWriter<
        aberred_core::protocol::render_assets::RenderAssetCmd,
    >,
    mut buf: Local<Vec<AssetCmd>>,
) {
    lua_runtime.drain_asset_commands_into(&mut buf);
    if buf.is_empty() {
        return;
    }
    for cmd in buf.drain(..) {
        translate_asset_command(cmd, &mut audio_cmd_writer, &mut render_asset_cmd_writer);
    }
}

/// Builds a [`World`] with all resources [`DrainCtx`] depends on.
#[cfg(test)]
pub(crate) fn new_drain_test_world() -> World {
    use aberred_core::systems::scene_dispatch::{SceneManagerInit, insert_scene_manager};
    let mut world = World::new();
    world.insert_resource(WorldSignals::default());
    world.insert_resource(NextScene::default());
    world.insert_resource(NextGameState::default());
    world.insert_resource(PostProcessShader::default());
    world.insert_resource(GameConfig::default());
    world.insert_resource(CameraFollowConfig::default());
    world.insert_resource(SystemsStore::default());
    world.insert_resource(AnimationStore::default());
    world.insert_resource(InputBindings::default());
    world.insert_resource(TrackedGroups::default());
    world.insert_resource(bevy_ecs::message::Messages::<AudioCmd>::default());
    world.insert_resource(GuiThemeStore::default());
    world.insert_resource(GuiThemeWarnCache::default());
    insert_scene_manager(
        &mut world,
        SceneManagerInit {
            on_demand: true,
            ..Default::default()
        },
    );
    world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
    world
}

/// Runs the Lua chunk `code` in `world`'s [`LuaRuntime`].
#[cfg(test)]
pub(crate) fn lua_exec(world: &World, code: &str) {
    world
        .non_send::<LuaRuntime>()
        .lua()
        .load(code)
        .exec()
        .expect("lua chunk");
}

/// Reads the Lua global `name` from `world`'s [`LuaRuntime`].
#[cfg(test)]
pub(crate) fn lua_global<T: mlua::FromLua>(world: &World, name: &str) -> T {
    world
        .non_send::<LuaRuntime>()
        .lua()
        .globals()
        .get(name)
        .expect(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::components::animation::Animation;
    use aberred_core::components::sprite::Sprite;
    use aberred_core::components::world_position::WorldPosition;
    use aberred_core::math::Vec2;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::{RunSystemOnce, SystemState};

    /// Runs [`DrainCtx::drain`] once against `world` (mirrors a single
    /// frame's drain in `update`).
    fn run_drain_common_commands(world: &mut World) {
        let mut system_state = SystemState::<DrainCtx>::new(world);
        system_state
            .get_mut(world)
            .expect("DrainCtx test params should fetch")
            .drain();
        system_state.apply(world);
    }

    #[test]
    fn drain_common_commands_processes_track_group_queued_mid_gameplay() {
        let mut world = new_drain_test_world();

        {
            let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
            lua_runtime
                .lua()
                .load("engine.track_group('enemies')")
                .exec()
                .expect("queue track_group");
        }

        run_drain_common_commands(&mut world);

        assert!(world.resource::<TrackedGroups>().groups.contains("enemies"));
    }

    /// Runs `script` through real Lua, then one [`DrainCtx::drain`] pass.
    fn exec_and_drain(world: &mut World, script: &str) {
        world
            .get_non_send::<LuaRuntime>()
            .unwrap()
            .lua()
            .load(script)
            .exec()
            .expect("script runs");
        run_drain_common_commands(world);
    }

    /// Every entity's `WorldPosition`, other than `except`'s.
    fn positions_except(world: &mut World, except: Entity) -> Vec<Vec2> {
        world
            .query::<(Entity, &WorldPosition)>()
            .iter(world)
            .filter(|(e, _)| *e != except)
            .map(|(_, p)| p.pos)
            .collect()
    }

    /// A template at (1, 2), registered as `"tpl"`.
    fn spawn_registered_template(world: &mut World) -> Entity {
        let tpl = world.spawn(WorldPosition::new(1.0, 2.0)).id();
        world
            .resource_mut::<WorldSignals>()
            .register_entity("tpl", tpl);
        tpl
    }

    #[test]
    fn spawn_register_as_then_clone_in_one_script_clones() {
        let mut world = new_drain_test_world();

        exec_and_drain(
            &mut world,
            "engine.spawn():with_position(1, 2):register_as('a'):build()\n\
             engine.clone('a'):build()",
        );

        let mut positions = world.query::<&WorldPosition>();
        assert_eq!(
            positions.iter(&world).map(|p| p.pos).collect::<Vec<_>>(),
            vec![Vec2::new(1.0, 2.0); 2]
        );
    }

    #[test]
    fn set_template_position_then_clone_gives_the_clone_the_new_position() {
        let mut world = new_drain_test_world();
        let tpl = spawn_registered_template(&mut world);

        exec_and_drain(
            &mut world,
            &format!(
                "engine.entity_set_position({}, 10, 20)\n\
                 engine.clone('tpl'):build()",
                tpl.to_bits()
            ),
        );

        assert_eq!(
            positions_except(&mut world, tpl),
            vec![Vec2::new(10.0, 20.0)]
        );
    }

    #[test]
    fn register_entity_then_clone_in_one_script_clones() {
        let mut world = new_drain_test_world();
        let tpl = world.spawn(WorldPosition::new(1.0, 2.0)).id();

        exec_and_drain(
            &mut world,
            &format!(
                "engine.register_entity('tpl', {})\n\
                 engine.clone('tpl'):build()",
                tpl.to_bits()
            ),
        );

        assert_eq!(positions_except(&mut world, tpl), vec![Vec2::new(1.0, 2.0)]);
    }

    #[test]
    fn clone_then_set_template_position_keeps_the_old_position() {
        let mut world = new_drain_test_world();
        let tpl = spawn_registered_template(&mut world);

        exec_and_drain(
            &mut world,
            &format!(
                "engine.clone('tpl'):build()\n\
                 engine.entity_set_position({}, 10, 20)",
                tpl.to_bits()
            ),
        );

        assert_eq!(positions_except(&mut world, tpl), vec![Vec2::new(1.0, 2.0)]);
    }

    #[test]
    fn clone_then_despawn_template_still_clones() {
        let mut world = new_drain_test_world();
        let tpl = spawn_registered_template(&mut world);

        exec_and_drain(
            &mut world,
            &format!(
                "engine.clone('tpl'):build()\n\
                 engine.entity_despawn({})",
                tpl.to_bits()
            ),
        );

        assert!(world.get_entity(tpl).is_err(), "template despawned");
        assert_eq!(positions_except(&mut world, tpl), vec![Vec2::new(1.0, 2.0)]);
    }

    #[test]
    fn clone_then_unregister_entity_key_still_clones() {
        let mut world = new_drain_test_world();
        let tpl = spawn_registered_template(&mut world);

        exec_and_drain(
            &mut world,
            "engine.clone('tpl'):build()\n\
             engine.unregister_entity('tpl')",
        );

        assert!(world.resource::<WorldSignals>().get_entity("tpl").is_none());
        assert_eq!(positions_except(&mut world, tpl), vec![Vec2::new(1.0, 2.0)]);
    }

    #[test]
    fn clone_register_as_then_spawn_reading_the_key_finds_it() {
        use aberred_core::components::particle_emitter::ParticleEmitter;
        let mut world = new_drain_test_world();
        spawn_registered_template(&mut world);

        exec_and_drain(
            &mut world,
            "engine.clone('tpl'):register_as('x'):build()\n\
             engine.spawn():with_particle_emitter({ templates = { 'x' } }):build()",
        );

        let x = world.resource::<WorldSignals>().get_entity("x");
        assert!(x.is_some(), "clone registered as x");
        let templates: Vec<Vec<Entity>> = world
            .query::<&ParticleEmitter>()
            .iter(&world)
            .map(|e| e.templates.clone())
            .collect();
        assert_eq!(templates, vec![vec![x.unwrap()]]);
    }

    #[test]
    fn drain_common_commands_leaves_gui_theme_store_unchanged_when_no_render_commands_queued() {
        let mut world = new_drain_test_world();
        world.clear_trackers();

        run_drain_common_commands(&mut world);

        // No RenderCmd was queued this frame, so the gated seed/write-back in
        // DrainCtx::drain must not have cloned-and-reinserted GuiThemeStore —
        // otherwise every consumer using Changed<GuiThemeStore> would see a spurious
        // change every frame regardless of whether a theme command ever fired.
        assert!(
            !world.resource_ref::<GuiThemeStore>().is_changed(),
            "GuiThemeStore must not be marked changed when no RenderCmd was queued"
        );
    }

    #[test]
    fn drain_common_commands_resolves_animation_registered_in_same_batch() {
        let mut world = new_drain_test_world();

        let entity = world
            .spawn((Sprite::new("old_tex", 16.0, 16.0), Animation::new("idle")))
            .id();

        {
            let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
            lua_runtime
                .lua()
                .load(format!(
                    "engine.register_animation('walk', 'player_walk', 0, 0, 16, 0, 1, 10, true)\n\
                     engine.entity_set_animation({}, 'walk')",
                    entity.to_bits()
                ))
                .exec()
                .expect("queue register_animation + entity_set_animation");
        }

        run_drain_common_commands(&mut world);

        let sprite = world.get::<Sprite>(entity).expect("sprite still present");
        assert_eq!(sprite.tex_key.as_ref(), "player_walk");

        let animation = world
            .get::<Animation>(entity)
            .expect("animation still present");
        assert_eq!(animation.animation_key, "walk");
    }

    /// Builds a [`World`] with all resources [`update`] depends on
    /// (superset of [`new_drain_test_world`]'s: also needs `WorldTime`/`InputState`/
    /// `MainCamera`/`RenderSize`).
    fn new_update_test_world() -> World {
        let mut world = new_drain_test_world();
        world.insert_resource(WorldTime::default());
        world.insert_resource(InputState::default());
        let render_size = RenderSize { w: 800, h: 600 };
        world.insert_resource(render_size);
        world.insert_resource(MainCamera(Camera2D::screen_centered(&render_size)));
        world
    }

    /// Defines `on_update_level1`/`on_update_level2` Lua globals that count
    /// their calls in `level1_called`/`level2_called`.
    fn define_level1_and_level2_lua_callbacks(world: &World) {
        let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
        lua_runtime
            .lua()
            .load(
                "function on_update_level1(input, dt)\n\
                     _G.level1_called = (_G.level1_called or 0) + 1\n\
                 end\n\
                 function on_update_level2(input, dt)\n\
                     _G.level2_called = (_G.level2_called or 0) + 1\n\
                 end",
            )
            .exec()
            .expect("define on_update_level1/on_update_level2");
    }

    /// `level1_called` / `level2_called` from [`define_level1_and_level2_lua_callbacks`].
    fn level_calls(world: &World) -> (Option<i64>, Option<i64>) {
        let globals = world.get_non_send::<LuaRuntime>().unwrap().lua().globals();
        (
            globals.get("level1_called").unwrap(),
            globals.get("level2_called").unwrap(),
        )
    }

    #[test]
    fn update_calls_scene_callback_and_drains_its_commands() {
        let mut world = new_update_test_world();
        world.resource_mut::<SceneManager>().active_scene = Some("level1".to_owned());

        let player = world
            .spawn(aberred_core::components::signals::Signals::default())
            .id();

        {
            let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
            lua_runtime
                .lua()
                .load(format!(
                    "function on_update_level1(input, dt)\n\
                         _G.update_called = true\n\
                         engine.entity_signal_set_flag({}, \"on_ground\")\n\
                     end",
                    player.to_bits()
                ))
                .exec()
                .expect("define on_update_level1");
        }

        world.run_system_once(update).unwrap();

        let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
        let called: Option<bool> = lua_runtime
            .lua()
            .globals()
            .get("update_called")
            .expect("update_called global should be readable");
        assert_eq!(
            called,
            Some(true),
            "on_update_<scene> should be called once per update() run (the only call site)"
        );

        let signals = world
            .get::<aberred_core::components::signals::Signals>(player)
            .expect("Signals component should exist after SignalSetFlag drains");
        assert!(
            signals.flags.contains("on_ground"),
            "entity_signal_set_flag queued in on_update_<scene> should be drained immediately, \
             same tick"
        );
    }

    /// `update` calls `on_update_<scene>` of the [`SceneManager`]'s active scene.
    #[test]
    fn update_calls_on_update_of_the_scene_manager_active_scene() {
        let mut world = new_update_test_world();
        world.resource_mut::<SceneManager>().active_scene = Some("level2".to_owned());
        define_level1_and_level2_lua_callbacks(&world);

        world.run_system_once(update).unwrap();

        assert_eq!(level_calls(&world), (None, Some(1)));
    }

    /// Before the first scene, `update` calls no scene callback.
    #[test]
    fn update_calls_no_scene_callback_without_an_active_scene() {
        let mut world = new_update_test_world();
        define_level1_and_level2_lua_callbacks(&world);

        world.run_system_once(update).unwrap();

        assert_eq!(level_calls(&world), (None, None));
    }

    /// On the tick a scene is entered, `update` calls no scene callback.
    #[test]
    fn update_calls_no_scene_callback_on_the_scene_entry_tick() {
        let mut world = new_update_test_world();
        world.resource_mut::<SceneManager>().active_scene = Some("level2".to_owned());
        world.resource_mut::<WorldTime>().frame_count = 7;
        world.resource_mut::<SceneManager>().entered_at = Some(7);
        define_level1_and_level2_lua_callbacks(&world);

        world.run_system_once(update).unwrap();
        assert_eq!(level_calls(&world), (None, None));

        world.resource_mut::<WorldTime>().frame_count = 8;
        world.run_system_once(update).unwrap();
        assert_eq!(level_calls(&world), (None, Some(1)));
    }

    #[test]
    fn update_warns_when_scene_callback_is_not_defined() {
        let mut world = new_update_test_world();
        world.resource_mut::<SceneManager>().active_scene = Some("no_update_here".to_owned());

        // on_update_<scene> is the only (required) scene callback -- a
        // missing one warns via LuaRuntime::call_named rather than skipping
        // silently, but must still not panic.
        world.run_system_once(update).unwrap();
    }

    #[test]
    fn process_lua_asset_commands_translates_load_texture_same_call() {
        // process_lua_asset_commands is schedule-agnostic (NonSend<LuaRuntime>
        // + MessageWriters + Local<Vec<_>>) -- this confirms an
        // engine.load_texture() call queued immediately before the system
        // runs is still translated into a RenderAssetCmd::Texture correctly.
        // It does NOT exercise the schedule-membership/ordering itself
        // (that's covered by `TestWorld`, which builds the real sim
        // schedule -- crates/aberredengine/tests/logic_world.rs).
        let mut world = new_drain_test_world();
        world.insert_resource(Messages::<
            aberred_core::protocol::render_assets::RenderAssetCmd,
        >::default());

        {
            let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
            lua_runtime
                .lua()
                .load("engine.load_texture('boss', 'assets/boss.png')")
                .exec()
                .expect("queue load_texture");
        }

        world.run_system_once(process_lua_asset_commands).unwrap();

        let cmds: Vec<_> = world
            .resource_mut::<Messages<aberred_core::protocol::render_assets::RenderAssetCmd>>()
            .drain()
            .collect();
        assert_eq!(cmds.len(), 1, "expected exactly one RenderAssetCmd");
        match &cmds[0] {
            aberred_core::protocol::render_assets::RenderAssetCmd::Texture {
                key, path, ..
            } => {
                assert_eq!(key, "boss");
                assert_eq!(path, "assets/boss.png");
            }
            other => panic!("expected RenderAssetCmd::Texture, got {other:?}"),
        }
    }
}
