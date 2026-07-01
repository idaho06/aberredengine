//! Lua-driven game lifecycle: asset loading, scene management, and per-frame update.
//!
//! # Key Functions
//!
//! - [`setup`] – calls `on_setup` in Lua to queue asset loads, then drains them into stores
//! - [`enter_play`] – calls `on_enter_play`, processes initial signals/groups, triggers first scene switch
//! - [`switch_scene`] – despawns non-persistent entities, calls `on_switch_scene`, drains all command queues
//! - [`update`] – calls `on_update_<scene>` each frame, drains command queues, handles quit/scene-switch flags
//!
//! # SystemParam Bundles
//!
//! - [`ScriptingContext`] – `LuaRuntime` + audio command writer
//! - [`GameSceneState`] – world signals, post-process, config, camera follow, stores
//! - [`EntityProcessing`] – entity command queries + LuaPhase query

use crate::components::luaphase::LuaPhase;
use crate::components::persistent::{CleanableEntity, Persistent};
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::AnimationStore;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::gamestate::{GameStates, NextGameState};
use crate::resources::group::TrackedGroups;
use crate::resources::guitheme::{GuiThemeStore, GuiThemeWarnCache};
use crate::resources::input::InputState;
use crate::resources::input_bindings::InputBindings;
use crate::resources::lua_runtime::{
    AnimationCmd, AssetCmd, CameraFollowCmd, GameConfigCmd, GroupCmd, InputCmd, InputSnapshot,
    LuaRuntime, PhaseCmd, RenderCmd,
};
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::screensize::ScreenSize;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturestore::TextureStore;

use crate::resources::signal_keys as sk;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    DrainScope, EffectCmdBufs, EntityCmdQueries, drain_and_process_effect_commands,
    drain_and_process_phase_commands, process_animation_command, process_asset_command,
    process_camera_follow_command, process_gameconfig_command, process_group_command,
    process_input_command, process_render_command, process_signal_command,
};
use crate::systems::mapspawn::load_font_with_mipmaps;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use log::{debug, error, info};
use mlua::prelude::LuaTable;
use raylib::prelude::*;
use rustc_hash::FxHashSet;

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
    pub post_process: ResMut<'w, PostProcessShader>,
    pub config: ResMut<'w, GameConfig>,
    pub camera_follow: ResMut<'w, CameraFollowConfig>,
    pub systems_store: Res<'w, SystemsStore>,
    pub anim_store: ResMut<'w, AnimationStore>,
}

/// Bundled entity processing queries.
#[derive(SystemParam)]
pub struct EntityProcessing<'w, 's> {
    pub cmd_queries: EntityCmdQueries<'w, 's>,
    pub luaphase: Query<'w, 's, (Entity, &'static mut LuaPhase)>,
}

/// Persistent per-frame buffers for the command queues drained by [`drain_common_commands`].
///
/// Hold one of these in a `Local<CommonCmdBufs>` on each Bevy system that calls
/// `drain_common_commands`. The Vecs retain heap capacity across frames.
#[derive(Default)]
pub(crate) struct CommonCmdBufs {
    phase: Vec<PhaseCmd>,
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
pub fn setup(
    mut commands: Commands,
    mut next_state: ResMut<NextGameState>,
    mut raylib: crate::systems::RaylibAccess,
    mut fonts: NonSendMut<FontStore>,
    mut shaders: NonSendMut<ShaderStore>,
    mut scripting: ScriptingContext,
) {
    // This function sets up the game world, loading resources
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);

    // Default camera. Needed to start the engine before entering play state
    // The camera will be overridden later in the scene setup
    let camera = Camera2D {
        target: Vector2 {
            x: 0.0,
            y: 0.0, //x: 0.0,
                    //y: 0.0,
        },
        offset: Vector2 {
            x: rl.get_screen_width() as f32 * 0.5,
            y: rl.get_screen_height() as f32 * 0.5,
        },
        rotation: 0.0,
        zoom: 1.0,
    };
    commands.insert_resource(Camera2DRes(camera));

    let lua_runtime = &scripting.lua_runtime;

    // Call Lua on_setup function to queue asset loading commands
    if lua_runtime.has_function("on_setup")
        && let Err(e) = lua_runtime.call_function::<_, ()>("on_setup", ())
    {
        error!("Error calling on_setup: {}", e);
    }

    // Initialize stores
    let mut tex_store = TextureStore::new();

    // Process asset commands queued by Lua (setup runs once; no persistent buffer needed)
    let mut asset_buf = Vec::new();
    lua_runtime.drain_asset_commands_into(&mut asset_buf);
    for cmd in asset_buf {
        process_asset_command(
            rl,
            th,
            cmd,
            &mut tex_store,
            &mut fonts,
            &mut shaders,
            &mut scripting.audio_cmd_writer,
            load_font_with_mipmaps,
        );
    }

    commands.insert_resource(tex_store);

    // Process animation registration commands from Lua
    let mut anim_store = AnimationStore::default();
    let mut anim_buf = Vec::new();
    lua_runtime.drain_animation_commands_into(&mut anim_buf);
    for cmd in anim_buf {
        process_animation_command(&mut anim_store, cmd);
    }
    commands.insert_resource(anim_store);

    // Change GameState to Playing
    next_state.set(GameStates::Playing);
    info!("Game setup() done, next state set to Playing");
}

pub use crate::systems::gamestate::quit_game;

// Create initial state of the game and observers
pub fn enter_play(
    mut commands: Commands,
    mut worldsignals: ResMut<WorldSignals>,
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
    for cmd in signal_buf {
        process_signal_command(&mut worldsignals, cmd);
    }

    let mut group_buf = Vec::new();
    lua_runtime.drain_group_commands_into(&mut group_buf);
    for cmd in group_buf {
        process_group_command(&mut tracked_groups, cmd);
    }

    // Update the tracked groups cache for Lua
    lua_runtime.update_tracked_groups_cache(&tracked_groups.groups);

    // NOTE: World signals (score, high_score, lives, level, scene) are now initialized by Lua in on_enter_play()

    // Finally, run the switch_scene system to spawn initial scene entities
    commands.run_system(*systems_store.get("switch_scene").expect(
        "'switch_scene' system not registered; validate_required_systems should have caught this",
    ));
}

/// Drains and processes the command queues that are common to both [`update`] and
/// [`switch_scene`]. Both contexts queue the same command types after their Lua callback
/// returns; this helper eliminates the duplicated drain loops.
#[allow(clippy::too_many_arguments)]
fn drain_common_commands(
    lua_runtime: &LuaRuntime,
    commands: &mut Commands,
    entities: &mut EntityProcessing,
    scene_state: &mut GameSceneState,
    audio_cmd_writer: &mut MessageWriter<AudioCmd>,
    bindings: &mut InputBindings,
    tracked_groups: &mut TrackedGroups,
    bufs: &mut CommonCmdBufs,
    gui_theme_store: &GuiThemeStore,
    gui_theme_warn_cache: &mut GuiThemeWarnCache,
) {
    // Drain animation registrations first so any same-batch SetAnimation/RestartAnimation
    // entity commands can resolve the newly-registered tex_key from AnimationStore.
    lua_runtime.drain_animation_commands_into(&mut bufs.animation);
    for cmd in bufs.animation.drain(..) {
        process_animation_command(&mut scene_state.anim_store, cmd);
    }

    drain_and_process_phase_commands(lua_runtime, &mut bufs.phase, &mut entities.luaphase);

    drain_and_process_effect_commands(
        lua_runtime,
        DrainScope::Regular,
        &mut bufs.effects,
        commands,
        &mut scene_state.world_signals,
        &mut entities.cmd_queries,
        audio_cmd_writer,
        &scene_state.systems_store,
        &scene_state.anim_store,
    );

    lua_runtime.drain_render_commands_into(&mut bufs.render);
    // gui_theme_commands is a separate, `preserve`-policy queue (see
    // queue_registry.rs) so a `set_gui_theme_*` call queued from on_setup()
    // survives the first switch_scene's clear_all_commands() — unlike
    // render_commands (post-process, `clear` policy), which is wiped every
    // scene switch since post-process commands may reference a scene's
    // about-to-be-despawned state.
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


/// Refreshes `cached` to `"{prefix}{scene}"`, but only rebuilds it when it
/// doesn't already match -- avoids reallocating/rewriting the cached callback
/// name on every call when the scene hasn't changed since the last one.
/// Shared by [`update`]'s required `on_update_<scene>` and [`fixed_update`]'s
/// optional `on_fixed_update_<scene>` callback-name resolution.
fn refresh_cached_callback_name(cached: &mut String, prefix: &str, scene: &str) {
    if cached.get(prefix.len()..) != Some(scene) {
        cached.clear();
        cached.push_str(prefix);
        cached.push_str(scene);
    }
}

/// Builds the Lua input table for this frame/substep from `input`, logging
/// and returning `None` on failure. Shared by [`update`] and [`fixed_update`]
/// -- the same `(frame_count, InputSnapshot)` diff-guard inside
/// `update_input_table` applies regardless of which one calls it first within
/// a given frame, so repeated calls across substeps are no-ops.
fn resolve_input_table(
    lua_runtime: &LuaRuntime,
    input: &InputState,
    frame_count: u64,
) -> Option<LuaTable> {
    let input_snapshot = InputSnapshot::from_input_state(input);
    match lua_runtime.update_input_table(&input_snapshot, frame_count) {
        Ok(table) => Some(table),
        Err(e) => {
            error!("Error creating input table: {}", e);
            None
        }
    }
}

/// Per-fixed-substep update system for scene-specific logic that must stay in
/// sync with collision/physics (240Hz), not the render frame rate.
///
/// Calls `on_fixed_update_<scene>` once per FIXED schedule substep (up to 8x
/// per render frame) -- **optional**: unlike [`update`]'s `on_update_<scene>`,
/// scenes that don't define this callback simply skip it silently (no warning
/// spam every substep). Use this for logic that must be re-evaluated every
/// substep to stay consistent with collision, e.g. clearing collision-derived
/// flags (`on_ground`, `touching_wall_*`) right before `collision_detector`
/// re-derives them this same substep -- clearing them once per render frame
/// in `on_update_<scene>` instead leaves them stale for however many FIXED
/// substeps run before the next render frame, which can cause a phase to
/// transition and immediately revert within the same frame (see
/// `docs/render-simulation-separation-brainstorm.md`).
///
/// Deliberately does not check the `"quit_game"`/`"switch_scene"` flags (unlike
/// [`update`]) -- scene teardown mid-accumulator-substep is not supported;
/// queue those from `on_update_<scene>` instead. Also does not refresh the
/// camera cache -- fixed-substep logic isn't expected to read camera state.
#[allow(clippy::too_many_arguments, private_interfaces)]
pub fn fixed_update(
    time: Res<WorldTime>,
    input: Res<InputState>,
    mut commands: Commands,
    mut scripting: ScriptingContext,
    mut scene_state: GameSceneState,
    mut entities: EntityProcessing,
    mut bindings: ResMut<InputBindings>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut common_bufs: Local<CommonCmdBufs>,
    mut cached_callback: Local<String>,
    gui_theme_store: Res<GuiThemeStore>,
    mut gui_theme_warn_cache: ResMut<GuiThemeWarnCache>,
) {
    crate::tracy::tracy_span!("lua_fixed_update");
    let lua_runtime = &scripting.lua_runtime;
    let delta_sec = time.delta;

    let scene_str = scene_state
        .world_signals
        .get_string(sk::SCENE)
        .map(|s| s.as_str())
        .unwrap_or(sk::DEFAULT_SCENE);

    refresh_cached_callback_name(&mut cached_callback, "on_fixed_update_", scene_str);

    // Update signal cache for Lua to read current values (world-level signals
    // can change between substeps via phase/collision callbacks).
    lua_runtime.update_signal_cache(scene_state.world_signals.snapshot());

    // Single cached lookup (unlike a has_function()-then-call_named() pair,
    // which would resolve the same global twice every substep) -- Ok(None)
    // means the optional callback isn't defined for this scene, silently
    // skipped (unlike `update`'s required on_update_<scene>, which warns).
    match lua_runtime.get_function_cached(cached_callback.as_str()) {
        Ok(Some(func)) => {
            if let Some(input_table) =
                resolve_input_table(lua_runtime, &input, time.frame_count)
                && let Err(e) = func.call::<()>((input_table, delta_sec))
            {
                error!(target: "lua", "Error in {}(): {}", cached_callback.as_str(), e);
            }
        }
        Ok(None) => {}
        Err(e) => {
            error!(target: "lua", "Error resolving {}(): {}", cached_callback.as_str(), e);
        }
    }

    drain_common_commands(
        lua_runtime,
        &mut commands,
        &mut entities,
        &mut scene_state,
        &mut scripting.audio_cmd_writer,
        &mut bindings,
        &mut tracked_groups,
        &mut common_bufs,
        &gui_theme_store,
        &mut gui_theme_warn_cache,
    );
}

/// Per-frame update system for scene-specific logic.
///
/// This system delegates scene behavior to Lua callbacks:
/// - Calls `on_update_<scene>` callback in Lua for the current scene
/// - Lua can queue signal commands (set_flag, set_string, etc.)
/// - Processes signal commands from Lua
/// - Reacts to flags set by Lua: "switch_scene", "quit_game"
#[allow(clippy::too_many_arguments, private_interfaces)]
pub fn update(
    time: Res<WorldTime>,
    input: Res<InputState>,
    camera: Res<Camera2DRes>,
    screen: Res<ScreenSize>,
    mut commands: Commands,
    mut next_game_state: ResMut<NextGameState>,
    mut scripting: ScriptingContext,
    mut scene_state: GameSceneState,
    mut entities: EntityProcessing,
    mut bindings: ResMut<InputBindings>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut common_bufs: Local<CommonCmdBufs>,
    mut cached_callback: Local<String>,
    gui_theme_store: Res<GuiThemeStore>,
    mut gui_theme_warn_cache: ResMut<GuiThemeWarnCache>,
) {
    crate::tracy::tracy_span!("lua_update");
    let lua_runtime = &scripting.lua_runtime;
    let delta_sec = time.delta;

    let scene_str = scene_state
        .world_signals
        .get_string(sk::SCENE)
        .map(|s| s.as_str())
        .unwrap_or(sk::DEFAULT_SCENE);

    refresh_cached_callback_name(&mut cached_callback, "on_update_", scene_str);

    // Update signal cache for Lua to read current values
    lua_runtime.update_signal_cache(scene_state.world_signals.snapshot());
    lua_runtime.update_gameconfig_cache(&scene_state.config);
    lua_runtime.update_camera_cache(&camera, &screen, scene_state.config.pixel_snap_camera);
    if bindings.take_dirty() {
        lua_runtime.update_bindings_cache(&bindings);
    }

    // Create input snapshot and Lua table for callbacks. Errors are logged
    // (inside resolve_input_table) and skip just the scene callback this frame
    // -- queues still get drained and quit/switch flags still get checked
    // below, so commands queued by timers/phase callbacks earlier this frame
    // must not be lost (P4-2).
    if let Some(input_table) = resolve_input_table(lua_runtime, &input, time.frame_count) {
        // Call scene-specific update callback with (input, dt)
        lua_runtime.call_named(cached_callback.as_str(), "Scene", |func| {
            func.call::<()>((input_table, delta_sec))
        });
    }

    drain_common_commands(
        lua_runtime,
        &mut commands,
        &mut entities,
        &mut scene_state,
        &mut scripting.audio_cmd_writer,
        &mut bindings,
        &mut tracked_groups,
        &mut common_bufs,
        &gui_theme_store,
        &mut gui_theme_warn_cache,
    );

    // Check for quit flag (set by Lua)
    if scene_state.world_signals.take_flag(sk::QUIT_GAME) {
        next_game_state.set(GameStates::Quitting);
        return;
    }

    // Check for scene switch flag (set by Lua)
    if scene_state.world_signals.take_flag(sk::SWITCH_SCENE) {
        debug!("Scene switch requested in world signals.");
        commands.run_system(*scene_state.systems_store.get("switch_scene").expect("'switch_scene' system not registered; validate_required_systems should have caught this"));
    }
}

pub use crate::systems::gamestate::clean_all_entities;
/// Processes scene switching: despawns old entities, calls Lua callbacks,
/// and processes all queued commands for the new scene.
#[allow(clippy::too_many_arguments, private_interfaces)]
pub fn switch_scene(
    mut commands: Commands,
    mut scripting: ScriptingContext,
    mut scene_state: GameSceneState,
    entities_to_clean: Query<Entity, CleanableEntity>,
    persistent_entities: Query<Entity, With<Persistent>>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut entities: EntityProcessing,
    mut bindings: ResMut<InputBindings>,
    mut common_bufs: Local<CommonCmdBufs>,
    gui_theme_store: Res<GuiThemeStore>,
    mut gui_theme_warn_cache: ResMut<GuiThemeWarnCache>,
) {
    let lua_runtime = &scripting.lua_runtime;
    debug!("switch_scene: System called!");

    // Clear all command queues FIRST to discard any stale commands from the previous scene
    // that might reference entities about to be despawned. This prevents panics when
    // entity commands are applied after their target entities have been despawned.
    lua_runtime.clear_all_commands();

    // Callbacks are re-injected per scene; drop cached function handles so
    // the new scene's definitions are resolved fresh.
    lua_runtime.clear_function_cache();

    for entity in entities_to_clean.iter() {
        commands.entity(entity).try_despawn();
    }

    // Clear entity registrations for despawned (non-persistent) entities
    let persistent_set: FxHashSet<Entity> = persistent_entities.iter().collect();
    scene_state
        .world_signals
        .clear_non_persistent_entities(&persistent_set);

    tracked_groups.clear();
    scene_state.world_signals.clear_group_counts();
    lua_runtime.update_tracked_groups_cache(&tracked_groups.groups);

    // Refresh the Lua signal cache so on_switch_scene sees the post-clear state
    // (cleared entity registry and group counts), not the previous scene's snapshot.
    lua_runtime.update_signal_cache(scene_state.world_signals.snapshot());

    let scene = scene_state
        .world_signals
        .get_string(sk::SCENE)
        .cloned()
        .unwrap_or_else(|| sk::DEFAULT_SCENE.to_string());

    // Call Lua on_switch_scene function if it exists
    if lua_runtime.has_function("on_switch_scene")
        && let Err(e) = lua_runtime.call_function::<_, ()>("on_switch_scene", scene.clone())
    {
        error!("Error calling on_switch_scene: {}", e);
    }

    drain_common_commands(
        lua_runtime,
        &mut commands,
        &mut entities,
        &mut scene_state,
        &mut scripting.audio_cmd_writer,
        &mut bindings,
        &mut tracked_groups,
        &mut common_bufs,
        &gui_theme_store,
        &mut gui_theme_warn_cache,
    );

    // Refresh the config cache after the drain may have applied GameConfigCmds.
    lua_runtime.update_gameconfig_cache(&scene_state.config);
}

/// Drains `asset_commands` queued from gameplay (`on_update_*`, `on_switch_scene`, phase/timer/
/// collision callbacks) and loads them into `TextureStore`/`FontStore`/`ShaderStore`/audio.
///
/// `setup()` drains this queue once for `on_setup`-time loads; this system is the reachable
/// drain site for any `engine.load_*` call made after setup. Mirrors
/// [`crate::systems::mapspawn::process_lua_map_commands`].
#[allow(clippy::too_many_arguments)]
pub fn process_lua_asset_commands(
    lua_runtime: NonSend<LuaRuntime>,
    mut raylib: crate::systems::RaylibAccess,
    mut tex_store: ResMut<TextureStore>,
    mut fonts: NonSendMut<FontStore>,
    mut shaders: NonSendMut<ShaderStore>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
    mut buf: Local<Vec<AssetCmd>>,
) {
    lua_runtime.drain_asset_commands_into(&mut buf);
    if buf.is_empty() {
        return;
    }
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    for cmd in buf.drain(..) {
        process_asset_command(
            rl,
            th,
            cmd,
            &mut tex_store,
            &mut fonts,
            &mut shaders,
            &mut audio_cmd_writer,
            load_font_with_mipmaps,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::animation::Animation;
    use crate::components::sprite::Sprite;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::{RunSystemOnce, SystemState};
    use std::sync::Arc;

    /// Builds a [`World`] with all resources [`drain_common_commands`] depends on.
    fn new_drain_test_world() -> World {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world.insert_resource(PostProcessShader::default());
        world.insert_resource(GameConfig::default());
        world.insert_resource(CameraFollowConfig::default());
        world.insert_resource(SystemsStore::default());
        world.insert_resource(AnimationStore::default());
        world.insert_resource(InputBindings::default());
        world.insert_resource(TrackedGroups::default());
        world.insert_resource(Messages::<AudioCmd>::default());
        world.insert_resource(GuiThemeStore::default());
        world.insert_resource(GuiThemeWarnCache::default());
        world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
        world
    }

    /// Runs [`drain_common_commands`] once against `world`, using a fresh
    /// [`CommonCmdBufs`] (mirrors a single frame's drain in `update`/`switch_scene`).
    /// Reads the existing `GuiThemeStore` resource, mirroring the real call sites.
    fn run_drain_common_commands(world: &mut World) {
        let mut system_state = SystemState::<(
            Commands,
            NonSend<LuaRuntime>,
            EntityProcessing,
            GameSceneState,
            MessageWriter<AudioCmd>,
            ResMut<InputBindings>,
            ResMut<TrackedGroups>,
            Res<GuiThemeStore>,
            ResMut<GuiThemeWarnCache>,
        )>::new(world);

        let mut bufs = CommonCmdBufs::default();
        {
            let (
                mut commands,
                lua_runtime,
                mut entities,
                mut scene_state,
                mut audio_cmd_writer,
                mut bindings,
                mut tracked_groups,
                gui_theme_store,
                mut gui_theme_warn_cache,
            ) = system_state
                .get_mut(world)
                .expect("drain_common_commands test params should fetch");

            drain_common_commands(
                &lua_runtime,
                &mut commands,
                &mut entities,
                &mut scene_state,
                &mut audio_cmd_writer,
                &mut bindings,
                &mut tracked_groups,
                &mut bufs,
                &gui_theme_store,
                &mut gui_theme_warn_cache,
            );
        }
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

    #[test]
    fn drain_common_commands_leaves_gui_theme_store_unchanged_when_no_render_commands_queued() {
        let mut world = new_drain_test_world();
        world.clear_trackers();

        run_drain_common_commands(&mut world);

        // No RenderCmd was queued this frame, so the gated seed/write-back in
        // drain_common_commands must not have cloned-and-reinserted GuiThemeStore —
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
            .spawn((
                Sprite {
                    tex_key: Arc::from("old_tex"),
                    width: 16.0,
                    height: 16.0,
                    offset: Vector2::default(),
                    origin: Vector2::default(),
                    flip_h: false,
                    flip_v: false,
                },
                Animation::new("idle"),
            ))
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

    #[test]
    fn switch_scene_preserves_map_and_asset_commands_but_clears_scene_scoped_commands() {
        let mut world = new_drain_test_world();

        {
            let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
            lua_runtime
                .lua()
                .load(
                    "engine.load_map('maps/dummy.json')\n\
                     engine.load_texture('boss', 'assets/boss.png')\n\
                     engine.set_flag('stale_flag')",
                )
                .exec()
                .expect("queue map/asset/signal commands");
        }

        world.run_system_once(switch_scene).unwrap();

        let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();

        let mut map_buf = Vec::new();
        lua_runtime.drain_map_commands_into(&mut map_buf);
        assert_eq!(
            map_buf.len(),
            1,
            "map_commands queued before switch_scene must survive its clear_all_commands"
        );

        let mut asset_buf = Vec::new();
        lua_runtime.drain_asset_commands_into(&mut asset_buf);
        assert_eq!(
            asset_buf.len(),
            1,
            "asset_commands queued before switch_scene must survive its clear_all_commands"
        );

        assert!(
            !world.resource::<WorldSignals>().has_flag("stale_flag"),
            "scene-scoped signal_commands should still be cleared by switch_scene"
        );
    }

    #[test]
    fn switch_scene_refreshes_signal_cache_before_on_switch_scene() {
        let mut world = new_drain_test_world();

        let player = world.spawn_empty().id();
        world
            .resource_mut::<WorldSignals>()
            .set_entity("player", player);
        let snapshot = world.resource_mut::<WorldSignals>().snapshot();

        {
            let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
            // Prime the cache with a snapshot that still contains "player",
            // mimicking the stale state on_switch_scene would otherwise see.
            lua_runtime.update_signal_cache(snapshot);

            lua_runtime
                .lua()
                .load(
                    "function on_switch_scene(scene)\n\
                         _G.player_seen = engine.get_entity('player')\n\
                     end",
                )
                .exec()
                .expect("define on_switch_scene");
        }

        world.run_system_once(switch_scene).unwrap();

        let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
        let player_seen: Option<u64> = lua_runtime
            .lua()
            .globals()
            .get("player_seen")
            .expect("player_seen global should be set");

        assert!(
            player_seen.is_none(),
            "on_switch_scene should see a refreshed snapshot where 'player' was already cleared"
        );
    }


    /// Builds a [`World`] with all resources [`fixed_update`] depends on
    /// (superset of [`new_drain_test_world`]'s: also needs `WorldTime`/`InputState`).
    fn new_fixed_update_test_world() -> World {
        let mut world = new_drain_test_world();
        world.insert_resource(WorldTime::default());
        world.insert_resource(InputState::default());
        world
    }

    #[test]
    fn fixed_update_calls_optional_scene_callback_and_drains_its_commands() {
        let mut world = new_fixed_update_test_world();
        world
            .resource_mut::<WorldSignals>()
            .set_string(sk::SCENE, "level1");

        let player = world
            .spawn(crate::components::signals::Signals::default())
            .id();

        {
            let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
            lua_runtime
                .lua()
                .load(format!(
                    "function on_fixed_update_level1(input, dt)\n\
                         _G.fixed_update_called = true\n\
                         engine.entity_signal_set_flag({}, \"on_ground\")\n\
                     end",
                    player.to_bits()
                ))
                .exec()
                .expect("define on_fixed_update_level1");
        }

        world.run_system_once(fixed_update).unwrap();

        let lua_runtime = world.get_non_send::<LuaRuntime>().unwrap();
        let called: Option<bool> = lua_runtime
            .lua()
            .globals()
            .get("fixed_update_called")
            .expect("fixed_update_called global should be readable");
        assert_eq!(
            called,
            Some(true),
            "on_fixed_update_<scene> should be called once per fixed_update run"
        );

        let signals = world
            .get::<crate::components::signals::Signals>(player)
            .expect("Signals component should exist after SignalSetFlag drains");
        assert!(
            signals.flags.contains("on_ground"),
            "entity_signal_set_flag queued in on_fixed_update_<scene> should be drained immediately, \
             same substep -- not deferred to the next on_update_<scene> call"
        );
    }

    #[test]
    fn fixed_update_is_silent_when_scene_callback_is_not_defined() {
        let mut world = new_fixed_update_test_world();
        world
            .resource_mut::<WorldSignals>()
            .set_string(sk::SCENE, "no_fixed_update_here");

        // Should not panic or spam a "callback not found" warning (unlike `update`'s
        // required on_update_<scene>) -- on_fixed_update_<scene> is optional.
        world.run_system_once(fixed_update).unwrap();
    }
}
