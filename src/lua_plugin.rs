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
use crate::components::persistent::Persistent;
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::AnimationStore;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::gamestate::{GameStates, NextGameState};
use crate::resources::group::TrackedGroups;
use crate::resources::input::InputState;
use crate::resources::input_bindings::InputBindings;
use crate::resources::lua_runtime::{
    CameraFollowCmd, GameConfigCmd, GroupCmd, InputCmd, InputSnapshot, LuaRuntime, PhaseCmd,
    RenderCmd,
};
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturestore::TextureStore;

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
    pub anim_store: Res<'w, AnimationStore>,
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
    gameconfig: Vec<GameConfigCmd>,
    camera_follow: Vec<CameraFollowCmd>,
    input: Vec<InputCmd>,
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

/// Drains and processes the 11 command queues that are common to both [`update`] and
/// [`switch_scene`]. Both contexts queue the same command types after their Lua callback
/// returns; this helper eliminates the duplicated drain loops.
///
/// `switch_scene` additionally drains group and tilemap commands after this call.
fn drain_common_commands(
    lua_runtime: &LuaRuntime,
    commands: &mut Commands,
    entities: &mut EntityProcessing,
    scene_state: &mut GameSceneState,
    audio_cmd_writer: &mut MessageWriter<AudioCmd>,
    bindings: &mut InputBindings,
    bufs: &mut CommonCmdBufs,
) {
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
    for cmd in bufs.render.drain(..) {
        process_render_command(cmd, &mut scene_state.post_process);
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
    mut commands: Commands,
    mut next_game_state: ResMut<NextGameState>,
    mut scripting: ScriptingContext,
    mut scene_state: GameSceneState,
    mut entities: EntityProcessing,
    mut bindings: ResMut<InputBindings>,
    mut common_bufs: Local<CommonCmdBufs>,
    mut cached_callback: Local<String>,
) {
    crate::tracy::tracy_span!("lua_update");
    let lua_runtime = &scripting.lua_runtime;
    let delta_sec = time.delta;

    let scene_str = scene_state
        .world_signals
        .get_string("scene")
        .map(|s| s.as_str())
        .unwrap_or("menu");

    if cached_callback.get("on_update_".len()..) != Some(scene_str) {
        cached_callback.clear();
        cached_callback.push_str("on_update_");
        cached_callback.push_str(scene_str);
    }

    // Update signal cache for Lua to read current values
    lua_runtime.update_signal_cache(scene_state.world_signals.snapshot());
    lua_runtime.update_gameconfig_cache(&scene_state.config);
    if bindings.take_dirty() {
        lua_runtime.update_bindings_cache(&bindings);
    }

    // Create input snapshot and Lua table for callbacks
    let input_snapshot = InputSnapshot::from_input_state(&input);
    let input_table = match lua_runtime.update_input_table(&input_snapshot) {
        Ok(table) => table,
        Err(e) => {
            error!("Error creating input table: {}", e);
            return;
        }
    };

    // Call scene-specific update callback with (input, dt)
    match lua_runtime.get_function(cached_callback.as_str()) {
        Ok(Some(func)) => {
            if let Err(e) = func.call::<()>((input_table, delta_sec)) {
                error!("Error calling {}: {}", cached_callback.as_str(), e);
            }
        }
        Ok(None) => {}
        Err(e) => {
            error!("Error resolving {}: {}", cached_callback.as_str(), e);
        }
    }

    drain_common_commands(
        lua_runtime,
        &mut commands,
        &mut entities,
        &mut scene_state,
        &mut scripting.audio_cmd_writer,
        &mut bindings,
        &mut common_bufs,
    );

    // Check for quit flag (set by Lua)
    if scene_state.world_signals.take_flag("quit_game") {
        next_game_state.set(GameStates::Quitting);
        return;
    }

    // Check for scene switch flag (set by Lua)
    if scene_state.world_signals.take_flag("switch_scene") {
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
    entities_to_clean: Query<Entity, Without<Persistent>>,
    persistent_entities: Query<Entity, With<Persistent>>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut entities: EntityProcessing,
    mut bindings: ResMut<InputBindings>,
    mut common_bufs: Local<CommonCmdBufs>,
    mut group_buf: Local<Vec<GroupCmd>>,
) {
    let lua_runtime = &scripting.lua_runtime;
    debug!("switch_scene: System called!");

    // Clear all command queues FIRST to discard any stale commands from the previous scene
    // that might reference entities about to be despawned. This prevents panics when
    // entity commands are applied after their target entities have been despawned.
    lua_runtime.clear_all_commands();

    for entity in entities_to_clean.iter() {
        commands.entity(entity).despawn();
    }

    // Clear entity registrations for despawned (non-persistent) entities
    let persistent_set: FxHashSet<Entity> = persistent_entities.iter().collect();
    scene_state
        .world_signals
        .clear_non_persistent_entities(&persistent_set);

    tracked_groups.clear();
    scene_state.world_signals.clear_group_counts();

    let scene = scene_state
        .world_signals
        .get_string("scene")
        .cloned()
        .unwrap_or_else(|| "menu".to_string());

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
        &mut common_bufs,
    );

    group_buf.clear();
    lua_runtime.drain_group_commands_into(&mut group_buf);
    for cmd in group_buf.drain(..) {
        process_group_command(&mut tracked_groups, cmd);
    }
    lua_runtime.update_tracked_groups_cache(&tracked_groups.groups);

    // Refresh the config cache after the drain may have applied GameConfigCmds.
    lua_runtime.update_gameconfig_cache(&scene_state.config);
}
